use std::path::Path;

use ignore::WalkBuilder;

use crate::{FileEntry, NodeType, ScanError, ScanOptions};

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
