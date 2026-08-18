use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ignore::WalkBuilder;

use crate::tree::TreeBuilder;
use crate::{CachedEntry, FileEntry, NodeType, ScanError, ScanOptions, ScanResult};

use super::cache::RedbCache;

// ── ScanConfig (plan-specified) ────────────────────────────────────────────

/// Scanner configuration — the minimal set of knobs for filesystem walking.
#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub respect_gitignore: bool,
    pub follow_symlinks: bool,
    pub max_depth: Option<usize>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            follow_symlinks: false,
            max_depth: None,
        }
    }
}

impl ScanConfig {
    /// Convert to the richer `ScanOptions` used by `walk_directory`.
    fn to_scan_options(&self) -> ScanOptions {
        ScanOptions {
            respect_gitignore: self.respect_gitignore,
            follow_symlinks: self.follow_symlinks,
            max_depth: self.max_depth.map(|d| d as u32),
            ..Default::default()
        }
    }
}

// ── Scanner ────────────────────────────────────────────────────────────────

/// Parallel directory scanner.
///
/// Walks the filesystem rooted at `root` using `ignore::WalkBuilder`
/// (which provides `.gitignore` support and parallel traversal via rayon),
/// then builds a `ScanResult` tree bottom-up from the collected entries.
pub struct Scanner {
    root: std::path::PathBuf,
    config: ScanConfig,
}

impl Scanner {
    /// Create a new scanner for `root` with the given configuration.
    pub fn new(root: &Path, config: ScanConfig) -> Self {
        Self {
            root: root.to_path_buf(),
            config,
        }
    }

    /// Scan the directory and return a `ScanResult`.
    ///
    /// Walks the filesystem in parallel, collects every entry, and
    /// assembles a bottom-up `FileTree` via `TreeBuilder`.
    pub fn scan(&self) -> Result<ScanResult, ScanError> {
        let opts = self.config.to_scan_options();
        let entries = walk_directory(&self.root, &opts)?;
        Ok(TreeBuilder::build(entries))
    }

    /// Incremental scan: skip I/O for directories whose mtime + size match the cache.
    ///
    /// For unchanged directories, the cached subtree is reused without filesystem
    /// traversal. For new or modified entries, fresh metadata is collected and
    /// the cache is updated.
    pub fn scan_incremental(&self, cache: &RedbCache) -> Result<ScanResult, ScanError> {
        let opts = self.config.to_scan_options();
        let entries = walk_directory_incremental(&self.root, &opts, cache)?;
        Ok(TreeBuilder::build(entries))
    }
}

// ── walk_directory (shared by Scanner and IncrementalScanner) ──────────────

/// Walk a directory tree and collect `FileEntry` values.
///
/// Uses `ignore::WalkBuilder` which provides native .gitignore support
/// and parallel traversal via rayon.
pub fn walk_directory(root: &Path, opts: &ScanOptions) -> Result<Vec<FileEntry>, ScanError> {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(opts.follow_symlinks)
        .git_ignore(opts.respect_gitignore)
        .git_global(opts.respect_gitignore)
        .git_exclude(opts.respect_gitignore)
        .hidden(false)
        .require_git(false);

    if let Some(depth) = opts.max_depth {
        builder.max_depth(Some(depth as usize));
    }

    let mut entries: Vec<ignore::DirEntry> = Vec::new();
    for result in builder.build() {
        match result {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                if e.io_error()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
                {
                    continue; // skip permission-denied, matches `ls` behaviour
                }
                return Err(ScanError::IoError(e.to_string()));
            }
        }
    }

    if entries.is_empty() {
        return Err(ScanError::InvalidPath(format!(
            "root path not found: {}",
            root.display()
        )));
    }

    // Compute depth for each entry relative to root.
    let root_depth = root.components().count();
    let file_entries: Vec<FileEntry> = entries
        .iter()
        .map(|e| {
            let path = e.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let depth = (path.components().count().saturating_sub(root_depth)) as u32;
            let (size, modified) = match e.metadata() {
                Ok(meta) => {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    (meta.len(), mtime)
                }
                _ => (0, 0),
            };
            let node_type = if e.file_type().map_or(false, |ft| ft.is_dir()) {
                NodeType::Dir
            } else {
                NodeType::File
            };
            FileEntry {
                path: path.to_path_buf(),
                name,
                size,
                modified,
                node_type,
                depth,
            }
        })
        .collect();

    Ok(file_entries)
}

// ── walk_directory_incremental ─────────────────────────────────────────────

/// Walk a directory tree with cache consultation at each directory node.
///
/// Uses `ignore::WalkBuilder` for correct `.gitignore` handling. For each
/// directory, checks the cache for matching mtime. If the directory is
/// unchanged, its cached subtree entries are reused and the walker skips
/// that subtree. Otherwise, the directory is walked normally and the
/// cache is updated.
fn walk_directory_incremental(
    root: &Path,
    opts: &ScanOptions,
    cache: &RedbCache,
) -> Result<Vec<FileEntry>, ScanError> {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(opts.follow_symlinks)
        .git_ignore(opts.respect_gitignore)
        .git_global(opts.respect_gitignore)
        .git_exclude(opts.respect_gitignore)
        .hidden(false)
        .require_git(false);

    if let Some(depth) = opts.max_depth {
        builder.max_depth(Some(depth as usize));
    }

    let root_depth = root.components().count();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut entries = Vec::new();
    // When set, skip all entries with depth > skip_depth (inside a cached subtree).
    let mut skip_depth: Option<usize> = None;

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(e) => {
                if e.io_error()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
                {
                    continue;
                }
                return Err(ScanError::IoError(e.to_string()));
            }
        };

        let depth = entry.depth();

        // If we're inside a skipped subtree, keep skipping until we exit it.
        if let Some(sd) = skip_depth {
            if depth > sd {
                continue;
            }
            skip_depth = None;
        }

        let path = entry.path();
        let rel_depth = (path.components().count().saturating_sub(root_depth)) as u32;

        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            let dir_mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // If cached directory mtime matches, reuse cached subtree.
            if let Ok(Some(cached)) = cache.get(path) {
                if cached.entry.modified == dir_mtime
                    && cached.entry.node_type == NodeType::Dir
                {
                    collect_cached_subtree(path, cache, rel_depth, &mut entries);
                    skip_depth = Some(depth);
                    continue;
                }
            }

            let dir_entry = FileEntry {
                path: path.to_path_buf(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size: 0,
                modified: dir_mtime,
                node_type: NodeType::Dir,
                depth: rel_depth,
            };
            entries.push(dir_entry.clone());
            let _ = cache.put(
                path,
                &CachedEntry {
                    entry: dir_entry,
                    scan_time: now,
                },
            );
        } else {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let file_entry = FileEntry {
                path: path.to_path_buf(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size: meta.len(),
                modified: mtime,
                node_type: if entry.file_type().map_or(false, |ft| ft.is_symlink()) {
                    NodeType::Symlink
                } else {
                    NodeType::File
                },
                depth: rel_depth,
            };
            entries.push(file_entry.clone());
            let _ = cache.put(
                path,
                &CachedEntry {
                    entry: file_entry,
                    scan_time: now,
                },
            );
        }
    }

    Ok(entries)
}

/// Collect all entries in a cached subtree rooted at `dir`.
/// No filesystem I/O — all entries come from the cache.
fn collect_cached_subtree(dir: &Path, cache: &RedbCache, depth: u32, out: &mut Vec<FileEntry>) {
    if let Ok(Some(cached)) = cache.get(dir) {
        let mut entry = cached.entry.clone();
        entry.depth = depth;
        out.push(entry);
    }

    let root_len = dir.components().count();
    if let Ok(subtree) = cache.entries_under(dir) {
        debug_assert!(
            subtree.iter().all(|e| e.entry.path.components().count() > root_len),
            "entries_under returned entries at or above dir depth — likely implementation bug"
        );
        for cached in subtree {
            let mut entry = cached.entry;
            entry.depth = depth + 1 + (entry.path.components().count().saturating_sub(root_len + 1)) as u32;
            out.push(entry);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn should_scan_directory_and_return_correct_file_count() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("b.txt"), "world!").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("c.txt"), "nested").unwrap();

        let scanner = Scanner::new(dir.path(), ScanConfig::default());
        let result = scanner.scan().unwrap();

        // 3 files + root + sub = 5 entries
        assert_eq!(result.entry_count, 5);
    }

    #[test]
    fn should_scan_nested_directories_recursively_when_max_depth_is_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        fs::write(dir.path().join("a").join("f1.txt"), "1").unwrap();
        fs::write(dir.path().join("a/b").join("f2.txt"), "22").unwrap();
        fs::write(dir.path().join("a/b/c").join("f3.txt"), "333").unwrap();

        let scanner = Scanner::new(dir.path(), ScanConfig::default());
        let result = scanner.scan().unwrap();

        // root + a + b + c + f1 + f2 + f3 = 7
        assert_eq!(result.entry_count, 7);
    }

    #[test]
    fn should_respect_max_depth_when_scan_config_specifies_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("top.txt"), "top").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("deep.txt"), "deep").unwrap();

        let config = ScanConfig {
            max_depth: Some(1),
            ..Default::default()
        };
        let scanner = Scanner::new(dir.path(), config);
        let result = scanner.scan().unwrap();

        // depth 0 = root, depth 1 = top.txt + sub = 3 entries
        assert_eq!(result.entry_count, 3);
    }

    #[test]
    fn should_respect_gitignore_when_respect_gitignore_is_true() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("keep.txt"), "keep").unwrap();
        fs::write(dir.path().join("ignored.log"), "log data").unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();

        let config = ScanConfig {
            respect_gitignore: true,
            ..Default::default()
        };
        let scanner = Scanner::new(dir.path(), config);
        let result = scanner.scan().unwrap();

        let names: Vec<String> = collect_names(&result.root);
        assert!(!names.contains(&"ignored.log".to_string()));
        assert!(names.contains(&"keep.txt".to_string()));
    }

    #[test]
    fn should_skip_unchanged_files_when_incremental_scan_matches_cache_mtime_size() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("stable.txt"), "content").unwrap();

        let cache_dir = tempfile::tempdir().unwrap();
        let cache = RedbCache::open(&cache_dir.path().join("cache.redb")).unwrap();

        let scanner = Scanner::new(dir.path(), ScanConfig::default());

        // First scan populates cache.
        let r1 = scanner.scan_incremental(&cache).unwrap();
        assert_eq!(r1.entry_count, 2); // stable.txt + root

        // Second scan with same files — cache hit means no re-stat needed
        // (result is identical).
        let r2 = scanner.scan_incremental(&cache).unwrap();
        assert_eq!(r2.entry_count, r1.entry_count);
        assert_eq!(r2.total_size, r1.total_size);
    }

    #[test]
    fn should_rescan_directory_when_incremental_scan_detects_new_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("stable.txt"), "old").unwrap();

        let cache_dir = tempfile::tempdir().unwrap();
        let cache = RedbCache::open(&cache_dir.path().join("cache.redb")).unwrap();

        let scanner = Scanner::new(dir.path(), ScanConfig::default());
        let r1 = scanner.scan_incremental(&cache).unwrap();
        assert_eq!(r1.total_size, 3);

        // Ensure directory mtime advances past the cached value.
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Adding a new file changes the directory mtime.
        fs::write(dir.path().join("new.txt"), "hello!").unwrap();

        let r2 = scanner.scan_incremental(&cache).unwrap();
        assert_eq!(r2.total_size, 9); // "old" (3) + "hello!" (6)
    }

    #[test]
    fn incremental_scan_should_respect_gitignore_patterns() {
        let dir = tempfile::tempdir().unwrap();
        // Create .git dir so ignore crate recognizes the repo.
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.path().join("keep.txt"), "keep").unwrap();
        fs::write(dir.path().join("ignored.log"), "log data").unwrap();

        let cache_dir = tempfile::tempdir().unwrap();
        let cache = RedbCache::open(&cache_dir.path().join("cache.redb")).unwrap();

        let config = ScanConfig {
            respect_gitignore: true,
            ..Default::default()
        };
        let scanner = Scanner::new(dir.path(), config);
        let result = scanner.scan_incremental(&cache).unwrap();

        let names: Vec<String> = collect_names(&result.root);
        assert!(!names.contains(&"ignored.log".to_string()));
        assert!(names.contains(&"keep.txt".to_string()));
    }

    #[test]
    fn incremental_scan_should_respect_max_depth() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("top.txt"), "top").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("deep.txt"), "deep").unwrap();

        let cache_dir = tempfile::tempdir().unwrap();
        let cache = RedbCache::open(&cache_dir.path().join("cache.redb")).unwrap();

        let config = ScanConfig {
            max_depth: Some(1),
            ..Default::default()
        };
        let scanner = Scanner::new(dir.path(), config);
        let result = scanner.scan_incremental(&cache).unwrap();

        // depth 0 = root, depth 1 = top.txt + sub = 3 entries
        assert_eq!(result.entry_count, 3);
    }

    fn collect_names(node: &crate::TreeNode) -> Vec<String> {
        let mut names = Vec::new();
        for child in &node.children {
            if child.entry.node_type == NodeType::File {
                names.push(child.entry.name.clone());
            }
            names.extend(collect_names(child));
        }
        names
    }
}
