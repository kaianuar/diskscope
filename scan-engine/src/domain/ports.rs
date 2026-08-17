use std::path::Path;

use super::CachedEntry;
use super::error::{CacheError, ScanError, TrashError};
use super::filenode::FileNode;
use super::opts::ScanOpts;
use super::TrashTicket;

/// Port: walks the filesystem and returns a FileNode tree.
pub trait Scanner {
    fn scan(&self, root: &Path, opts: &ScanOpts) -> Result<FileNode, ScanError>;
}

/// Port: persistent cache for incremental scans (mtime-based invalidation).
pub trait Cache {
    fn get(&self, path: &Path) -> Result<Option<CachedEntry>, CacheError>;
    fn put(&self, path: &Path, entry: &CachedEntry) -> Result<(), CacheError>;
    fn invalidate(&self, path: &Path) -> Result<(), CacheError>;
}

/// Port: moves files to system trash with undo capability.
pub trait Trash {
    fn delete(&self, path: &Path) -> Result<TrashTicket, TrashError>;
    fn undo(&self, ticket: &TrashTicket) -> Result<(), TrashError>;
}
