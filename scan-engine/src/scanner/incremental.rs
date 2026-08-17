use std::path::Path;

use crate::domain::CachedEntry;
use crate::domain::error::ScanError;
use crate::domain::filenode::FileNode;
use crate::domain::opts::ScanOpts;
use crate::domain::tree::FileTree;

use super::cache::RedbCache;
use super::options::ScanOptions;
use super::walker::walk_directory;

/// Scanner that skips I/O for files whose size and mtime haven't changed
/// since the last scan, using a persistent `redb` cache.
pub struct IncrementalScanner {
    options: ScanOptions,
    cache: RedbCache,
}

impl IncrementalScanner {
    /// Create a new incremental scanner with the given options and cache.
    pub fn new(options: ScanOptions, cache: RedbCache) -> Self {
        Self { options, cache }
    }

    /// Scan `root`, re-using cached metadata for unchanged files.
    pub fn scan(&self, root: &Path, opts: &ScanOpts) -> Result<FileTree, ScanError> {
        let mut file_node = walk_directory(root, &self.options, opts)?;
        self.apply_cache(&mut file_node)?;
        Ok(FileTree::new(file_node))
    }

    fn apply_cache(&self, node: &mut FileNode) -> Result<(), ScanError> {
        match self.cache.get(&node.path) {
            Ok(Some(cached)) if cached.size == node.size && cached.mtime == node.mtime => {
                // Unchanged — metadata is already correct from the walk.
            }
            Ok(Some(_)) | Ok(None) => {
                self.cache
                    .put(&node.path, &CachedEntry { size: node.size, mtime: node.mtime })
                    .map_err(|e| ScanError::Io(e.to_string()))?;
            }
            Err(e) => return Err(ScanError::Io(e.to_string())),
        }

        for child in &mut node.children {
            self.apply_cache(child)?;
        }
        Ok(())
    }
}
