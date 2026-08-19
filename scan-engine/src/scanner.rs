//! Parallel filesystem walker with `.gitignore` awareness.
//!
//! [`JwalkScanner`] implements the [`domain::ports::Scanner`] port by
//! walking a directory tree in parallel (via `jwalk`) and consulting a
//! `gitignore` matcher for the root-level `.gitignore` (via the `ignore`
//! crate). Outputs are `domain::FileNode` trees that the rest of the
//! pipeline can filter, sort, and render.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use jwalk::{DirEntry, WalkDir};

use domain::{DomainError, FileNode, FileType, ScanResult};

/// Concrete `DirEntry` type used by the default `jwalk::WalkDir`.
type JwalkDirEntry = DirEntry<((), ())>;

/// Result of stat-ing a scan root — used by [`ScanService`] to decide
/// whether a cached entry is still valid (same mtime + same size).
#[derive(Debug, Clone, Copy)]
pub struct RootMeta {
    /// Last-modified time of the root directory, in Unix seconds.
    pub modified: u64,
    /// Total recursive bytes under the root. Used as a secondary
    /// proxy for "did anything change?" — if the on-disk byte count
    /// differs, the cached tree is stale.
    pub total_bytes: u64,
}

/// Parallel filesystem walker. Cheap to construct.
#[derive(Debug, Clone, Default)]
pub struct JwalkScanner {
    _priv: (),
}

impl JwalkScanner {
    /// Create a new scanner. The thread count is read from the
    /// `RAYON_NUM_THREADS` environment variable (same as rayon); set
    /// to `1` for deterministic single-threaded walks in tests.
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Stat the scan root without walking the tree. Returns the root's
    /// mtime and the recursive total byte count.
    pub fn stat_root(&self, path: &str) -> Result<RootMeta, DomainError> {
        let root = validate_root(path)?;

        let mtime = root
            .metadata()
            .map_err(DomainError::Io)?
            .modified()
            .map_err(DomainError::Io)?;
        let modified = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| io::Error::other(format!("mtime before unix epoch: {e}")))?;

        let total_bytes: u64 = WalkDir::new(&root)
            .skip_hidden(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();

        Ok(RootMeta {
            modified,
            total_bytes,
        })
    }

    /// Walk `path` and build the full `FileNode` tree.
    ///
    /// The walk is parallel (`jwalk`); `.gitignore` rules at the root
    /// are honored via a `Gitignore` matcher. After the parallel walk,
    /// the flat list of entries is folded into a tree in `build_tree`.
    pub fn scan_raw(&self, path: &str) -> Result<FileNode, DomainError> {
        let root = validate_root(path)?;
        let gitignore = build_root_gitignore(&root);

        let entries: Vec<WalkEntry> = WalkDir::new(&root)
            .skip_hidden(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| !is_gitignored(&gitignore, e.path().as_path(), &root))
            .map(|e| WalkEntry::from_jwalk(e, &root))
            .collect();

        Ok(build_tree(&root, &entries))
    }
}

impl domain::ports::Scanner for JwalkScanner {
    fn scan(&self, path: &str) -> Result<ScanResult, DomainError> {
        let start = std::time::Instant::now();
        let root = self.scan_raw(path)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        Ok(ScanResult::from_tree(root, elapsed_ms))
    }
}

/// Validate that the scan root exists, is a directory, and is non-empty.
fn validate_root(path: &str) -> Result<PathBuf, DomainError> {
    if path.is_empty() {
        return Err(DomainError::InvalidPath("path must not be empty".into()));
    }
    let p = PathBuf::from(path);
    let meta = fs::metadata(&p).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => DomainError::InvalidPath(format!("not found: {path}")),
        io::ErrorKind::PermissionDenied => DomainError::PermissionDenied(path.to_string()),
        _ => DomainError::Io(e),
    })?;
    if !meta.is_dir() {
        return Err(DomainError::InvalidPath(format!("not a directory: {path}")));
    }
    Ok(p)
}

/// Build a `Gitignore` matcher rooted at `root`, using the root's
/// `.gitignore` if present. Returns a no-op matcher when no file exists.
fn build_root_gitignore(root: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    let gi = root.join(".gitignore");
    if gi.is_file() {
        let _ = builder.add(gi);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// `true` when `path` is ignored by the root `.gitignore` matcher.
fn is_gitignored(gi: &Gitignore, path: &Path, root: &Path) -> bool {
    // `.git` directories should never be recursed into.
    if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
        return true;
    }
    let rel = path.strip_prefix(root).unwrap_or(path);
    let is_dir = path.is_dir();
    gi.matched(rel, is_dir).is_ignore()
}

/// Lightweight record for one walked entry, used to build the tree.
#[derive(Debug, Clone)]
struct WalkEntry {
    path: PathBuf,
    rel: PathBuf,
    size: u64,
    modified: u64,
    is_dir: bool,
    file_type: FileType,
}

impl WalkEntry {
    fn from_jwalk(e: JwalkDirEntry, root: &Path) -> Self {
        let path = e.path();
        let rel = path
            .strip_prefix(root)
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let (size, modified) = match e.metadata() {
            Ok(m) => (m.len(), mtime_to_secs(m.modified())),
            Err(_) => (0, 0),
        };
        let is_dir = e.file_type().is_dir();
        let file_type = if is_dir {
            FileType::Directory
        } else {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            FileType::from_extension(ext)
        };
        Self {
            path,
            rel,
            size,
            modified,
            is_dir,
            file_type,
        }
    }
}

/// Convert a `SystemTime` to Unix seconds. Returns `0` if the value is
/// before the epoch or not available.
fn mtime_to_secs(time: std::io::Result<SystemTime>) -> u64 {
    time.ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Recursively replace each directory's `size` with the sum of its
/// children's sizes. Leaves (files) keep their real size.
fn normalize_sizes(node: &mut FileNode) -> u64 {
    if node.children.is_empty() {
        node.size
    } else {
        let sum: u64 = node.children.iter_mut().map(normalize_sizes).sum();
        node.size = sum;
        sum
    }
}

/// Fold a flat list of entries into a `FileNode` tree.
fn build_tree(root: &Path, entries: &[WalkEntry]) -> FileNode {
    if entries.is_empty() {
        return FileNode {
            path: root.to_string_lossy().into_owned(),
            size: 0,
            modified: 0,
            file_type: FileType::Directory,
            children: Vec::new(),
        };
    }

    let mut by_parent: BTreeMap<PathBuf, Vec<WalkEntry>> = BTreeMap::new();
    for entry in entries {
        let parent = entry
            .rel
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        by_parent.entry(parent).or_default().push(entry.clone());
    }

    let root_entry = entries.iter().find(|e| e.rel.as_os_str().is_empty()).cloned();
    let (root_size, root_modified) = root_entry
        .as_ref()
        .map(|e| (e.size, e.modified))
        .unwrap_or((0, 0));

    let mut node = FileNode {
        path: root.to_string_lossy().into_owned(),
        size: root_size,
        modified: root_modified,
        file_type: FileType::Directory,
        children: build_children(&mut by_parent, PathBuf::new()),
    };
    normalize_sizes(&mut node);
    node
}

/// Recursively build children for entries whose `rel` parent matches
/// `parent_rel`. Drains matching entries from `by_parent`.
fn build_children(
    by_parent: &mut BTreeMap<PathBuf, Vec<WalkEntry>>,
    parent_rel: PathBuf,
) -> Vec<FileNode> {
    let mut children = Vec::new();
    if let Some(mut entries) = by_parent.remove(&parent_rel) {
        entries.sort_by(|a, b| a.rel.cmp(&b.rel));
        for e in entries {
            if e.rel.as_os_str().is_empty() {
                continue; // root is not its own child
            }
            let child_rel = e.rel.clone();
            let children_out = if e.is_dir {
                build_children(by_parent, child_rel)
            } else {
                Vec::new()
            };
            children.push(FileNode {
                path: e.path.to_string_lossy().into_owned(),
                size: e.size,
                modified: e.modified,
                file_type: e.file_type,
                children: children_out,
            });
        }
    }
    children
}
