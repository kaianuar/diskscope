pub mod options;
pub mod walker;
pub mod cache;
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
    pub fn new(options: ScanOptions) -> Self {
        Self { options }
    }

    /// Scan a directory and return a FileTree.
    pub fn scan(&self, root: &Path, opts: &ScanOpts) -> Result<FileTree, ScanError> {
        let file_node = walk_directory(root, &self.options, opts)?;
        Ok(FileTree::new(file_node))
    }
}
