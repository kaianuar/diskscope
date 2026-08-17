use std::path::{Path, PathBuf};

use crate::{CachedEntry, FileEntry, ScanError, ScanOptions, TrashedFile, TrashError};

/// Port for filesystem scanning.
pub trait Scanner {
    fn scan(&self, root: &Path, opts: &ScanOptions) -> Result<Vec<FileEntry>, ScanError>;
}

/// Port for caching scan results.
pub trait Cache {
    fn get(&self, path: &Path) -> Option<CachedEntry>;
    fn put(&self, entry: &CachedEntry);
    fn evict_stale(&self, root: &Path) -> usize;
}

/// Port for safe-delete (move to OS trash) and undo.
pub trait Trash {
    fn delete(&self, paths: &[PathBuf]) -> Result<Vec<TrashedFile>, TrashError>;
    fn undo(&self, files: &[TrashedFile]) -> Result<(), TrashError>;
}
