use std::path::Path;

use ignore::WalkBuilder;

use crate::domain::error::ScanError;
use crate::domain::filenode::FileNode;
use crate::domain::opts::ScanOpts;
use crate::domain::filter::Filter;

use super::options::ScanOptions;

/// Walk a directory tree in parallel and build a `FileNode` tree.
///
/// Uses `ignore::WalkBuilder` which provides native .gitignore support
/// and parallel traversal via rayon.
pub fn walk_directory(
    root: &Path,
    scan_opts: &ScanOptions,
    domain_opts: &ScanOpts,
) -> Result<FileNode, ScanError> {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(scan_opts.follow_symlinks)
        .git_ignore(scan_opts.respect_gitignore)
        .git_global(scan_opts.respect_gitignore)
        .git_exclude(scan_opts.respect_gitignore)
        .hidden(false);

    if let Some(depth) = scan_opts.max_depth {
        builder.max_depth(Some(depth));
    }

    // Collect entries, skipping permission errors (matches `ls` behaviour).
    let mut entries: Vec<ignore::DirEntry> = Vec::new();
    for result in builder.build() {
        match result {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                if e.io_error().is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied) {
                    continue;
                }
                return Err(ScanError::Io(e.to_string()));
            }
        }
    }

    if entries.is_empty() {
        return Err(ScanError::Io(format!("root path not found: {}", root.display())));
    }

    // Index entries by parent path for O(n) tree assembly.
    use std::collections::HashMap;
    let mut by_parent: HashMap<std::path::PathBuf, Vec<&ignore::DirEntry>> = HashMap::new();
    for entry in &entries {
        if let Some(parent) = entry.path().parent() {
            by_parent
                .entry(parent.to_path_buf())
                .or_default()
                .push(entry);
        }
    }

    let all_filters = scan_opts.filters.iter()
        .chain(domain_opts.filters.iter())
        .cloned()
        .collect::<Vec<_>>();

    // Recursive tree builder.
    fn build_tree(
        entry: &ignore::DirEntry,
        by_parent: &HashMap<std::path::PathBuf, Vec<&ignore::DirEntry>>,
        filters: &[Filter],
    ) -> Option<FileNode> {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let (file_size, mtime) = match entry.metadata() {
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

        let children = if by_parent.contains_key(path) {
            by_parent[path]
                .iter()
                .filter_map(|child| build_tree(child, by_parent, filters))
                .collect()
        } else {
            Vec::new()
        };

        let node = FileNode {
            path: path.to_path_buf(),
            name,
            size: file_size,
            mtime,
            is_dir: entry.file_type().map_or(false, |ft| ft.is_dir()),
            children,
        };

        // Apply filters: keep node if it passes or has surviving descendants.
        if filters.is_empty() {
            return Some(node);
        }
        let passes = node.matches_filters(filters);
        if passes || !node.children.is_empty() {
            Some(node)
        } else {
            None
        }
    }

    let root_entry = &entries[0];
    build_tree(root_entry, &by_parent, &all_filters)
        .ok_or_else(|| ScanError::Io("root filtered out".into()))
}
