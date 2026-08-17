use std::path::Path;

use super::CachedEntry;
use super::error::{CacheError, ScanError, TrashError};
use super::filenode::FileNode;
use super::opts::ScanOpts;
use super::TrashTicket;

/// Port: walks the filesystem and returns a [`FileNode`] tree.
///
/// This is the primary boundary between the domain and the filesystem.
/// Implementations handle I/O, parallelism, and `.gitignore` filtering;
/// the domain never touches the filesystem directly.
///
/// # Contracts
///
/// ## Preconditions
/// - `root` MUST exist and be a directory; implementations MUST return
///   [`ScanError::Io`] if it does not.
/// - `opts` is always valid (zero-value defaults are acceptable).
///
/// ## Postconditions
/// - The returned [`FileNode`] is the root of a tree whose children are
///   the entries discovered under `root`.
/// - Each node's `size` is the file's byte length (or the recursive sum
///   for directories).
/// - When `opts.depth` is `Some(n)`, the tree is truncated at depth `n`.
/// - When `opts.filters` is non-empty, only matching entries appear.
///
/// ## Errors
/// - [`ScanError::Io`] for any filesystem or permission failure.
///   Implementations SHOULD skip unreadable entries rather than aborting
///   the entire scan.
///
/// ## Thread Safety
/// Implementations MUST be `Send + Sync` so the scanner can be shared
/// across threads (e.g., Tauri command handlers, background tasks).
pub trait Scanner {
    /// Scan the directory at `root` with the given options.
    ///
    /// Returns the root [`FileNode`] whose children represent the
    /// directory contents. Filters and depth limits from `opts` are
    /// applied during the walk, not as a post-processing step.
    fn scan(&self, root: &Path, opts: &ScanOpts) -> Result<FileNode, ScanError>;
}

/// Port: persistent cache for incremental scans (mtime-based invalidation).
///
/// Stores [`CachedEntry`] values keyed by absolute path, enabling the
/// scanner to skip unchanged files on subsequent scans. The domain uses
/// this to decide whether a file's metadata has changed since the last
/// scan (compare cached `mtime` with current `mtime`).
///
/// # Contracts
///
/// ## Preconditions
/// - `path` MUST be an absolute path; relative paths produce
///   implementation-defined behavior (typically a cache miss).
/// - Entries are opaque to the domain — only `size` and `mtime` matter.
///
/// ## Postconditions
/// - `get` returns `Ok(None)` for cache misses (never an error).
/// - `put` stores the entry; a subsequent `get` with the same path
///   MUST return `Ok(Some(entry))` with the stored values.
/// - `invalidate` removes the entry; a subsequent `get` MUST return
///   `Ok(None)`.
///
/// ## Errors
/// - [`CacheError::Io`] for any storage-layer failure (corruption,
///   disk full, lock contention). Implementations MUST NOT return
///   errors for cache misses — only `Ok(None)`.
///
/// ## Thread Safety
/// Implementations MUST be `Send + Sync`. Concurrent `get`/`put` calls
/// MUST NOT corrupt the underlying store.
pub trait Cache {
    /// Look up a cached entry by path.
    ///
    /// Returns `Ok(None)` on cache miss — this is not an error.
    fn get(&self, path: &Path) -> Result<Option<CachedEntry>, CacheError>;

    /// Store a cached entry keyed by path.
    ///
    /// Overwrites any existing entry for the same path.
    fn put(&self, path: &Path, entry: &CachedEntry) -> Result<(), CacheError>;

    /// Remove a cached entry for the given path.
    ///
    /// No-op (returns `Ok(())`) if the path is not in the cache.
    fn invalidate(&self, path: &Path) -> Result<(), CacheError>;
}

/// Port: moves files to system trash with undo capability.
///
/// This is the **safety boundary** — no file is ever permanently deleted
/// through this port. Implementations delegate to the OS trash mechanism
/// (e.g., `trash` crate on all platforms, `gio` on Linux).
///
/// # Contracts
///
/// ## Preconditions
/// - `path` MUST exist at the time of `delete`; implementations MUST
///   return [`TrashError::Io`] if it does not.
/// - `ticket` passed to `undo` MUST have been returned by a prior
///   `delete` call on the same implementation instance.
///
/// ## Postconditions
/// - After `delete(path)` succeeds, the file is in the system trash
///   (NOT permanently deleted).
/// - The returned [`TrashTicket`] has `path` set to the original path
///   and `deleted_at` set to the current Unix timestamp (seconds since
///   epoch). Implementations MUST NOT leave `deleted_at` as zero.
/// - After `undo(ticket)` succeeds, the file is restored to its
///   original `ticket.path` location.
///
/// ## Errors
/// - [`TrashError::Io`] if the path does not exist, is locked, or the
///   trash mechanism fails.
/// - `undo` MUST return [`TrashError::Io`] if the ticket is invalid
///   (path not previously deleted, or already restored).
///
/// ## Safety Invariants
/// - **No permanent delete.** Implementations MUST move to trash, never
///   unlink/rm directly. This is a hard constraint — violating it is a
///   correctness bug, not a style issue.
/// - **Idempotent undo is not required.** Calling `undo` twice with the
///   same ticket MUST return an error on the second call.
///
/// ## Thread Safety
/// Implementations MUST be `Send + Sync`. Concurrent `delete` calls on
/// distinct paths MUST NOT interfere with each other.
pub trait Trash {
    /// Move `path` to the system trash.
    ///
    /// Returns a [`TrashTicket`] containing the original path and the
    /// deletion timestamp. The caller retains this ticket for a later
    /// [`undo`](Trash::undo) call.
    fn delete(&self, path: &Path) -> Result<TrashTicket, TrashError>;

    /// Restore the file identified by `ticket` from trash.
    ///
    /// The file is moved back to `ticket.path`. Returns
    /// [`TrashError::Io`] if the ticket is invalid or the restore fails.
    fn undo(&self, ticket: &TrashTicket) -> Result<(), TrashError>;
}
