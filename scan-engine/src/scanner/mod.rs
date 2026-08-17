pub mod cache;
pub mod incremental;
pub mod walker;

use std::path::Path;

use crate::tree::TreeBuilder;
use crate::{ScanError, ScanOptions, ScanResult};

pub use cache::RedbCache;
pub use incremental::IncrementalScanner;

/// Parallel directory scanner.
pub struct Scanner {
    options: ScanOptions,
}

impl Scanner {
    pub fn new(options: ScanOptions) -> Self {
        Self { options }
    }

    /// Scan a directory and return a `ScanResult`.
    pub fn scan(&self, root: &Path) -> Result<ScanResult, ScanError> {
        let entries = walker::walk_directory(root, &self.options)?;
        Ok(TreeBuilder::build(entries))
    }
}
