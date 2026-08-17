use std::path::Path;

use super::CachedEntry;
use super::error::{CacheError, ScanError, TrashError};
use super::filenode::FileNode;
use super::opts::ScanOpts;
use super::TrashTicket;

/// Port: walks the filesystem and returns a FileNode tree.
pub trait Scanner {
    /// Scan the directory at `root` with the given options.
    fn scan(&self, root: &Path, opts: &ScanOpts) -> Result<FileNode, ScanError>;
}

/// Port: persistent cache for incremental scans (mtime-based invalidation).
pub trait Cache {
    /// Look up a cached entry by path.
    fn get(&self, path: &Path) -> Result<Option<CachedEntry>, CacheError>;
    /// Store a cached entry keyed by path.
    fn put(&self, path: &Path, entry: &CachedEntry) -> Result<(), CacheError>;
    /// Remove a cached entry for the given path.
    fn invalidate(&self, path: &Path) -> Result<(), CacheError>;
}

/// Port: moves files to system trash with undo capability.
///
/// Implementations MUST set `TrashTicket::deleted_at` to the current Unix
/// timestamp (seconds since epoch) when the delete succeeds.
pub trait Trash {
    /// Move `path` to the system trash. Returns a ticket with `path` and
    /// `deleted_at` for later undo.
    fn delete(&self, path: &Path) -> Result<TrashTicket, TrashError>;
    /// Restore the file identified by `ticket` from trash.
    fn undo(&self, ticket: &TrashTicket) -> Result<(), TrashError>;
}
