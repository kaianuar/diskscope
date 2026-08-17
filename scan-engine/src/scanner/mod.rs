/// Scanner configuration options.
pub mod options;
/// Parallel directory walker.
pub mod walker;
/// Persistent file metadata cache.
pub mod cache;
/// Incremental scanner with cache.
pub mod incremental;

use std::path::Path;

use crate::domain::error::ScanError;
use crate::domain::tree::FileTree;
use crate::domain::opts::ScanOpts;
use options::ScanOptions;
use walker::walk_directory;

/// Parallel directory scanner.
pub struct Scanner {
    options: ScanOptions,
}

impl Scanner {
    /// Create a new scanner with the given options.
    pub fn new(options: ScanOptions) -> Self {
        Self { options }
    }

    /// Scan a directory and return a FileTree.
    pub fn scan(&self, root: &Path, opts: &ScanOpts) -> Result<FileTree, ScanError> {
        let file_node = walk_directory(root, &self.options, opts)?;
        Ok(FileTree::new(file_node))
    }
}
