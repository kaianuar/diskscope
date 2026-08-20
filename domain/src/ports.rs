//! DiskScope domain ports.
//!
//! Ports are the *interfaces* the domain exposes to the outside world.
//! Adapters (`scan-engine`, future `cli` / `gui` consumers) implement
//! these traits to bridge the pure domain with concrete systems: the
//! filesystem, the OS trash, and an embedded cache.
//!
//! All ports are object-safe (`&self`-receivers, no associated types or
//! generic methods), so they can be used as trait objects behind
//! `Box<dyn Scanner>`, `Arc<dyn Trash>`, etc. — which is how the GUI /
//! CLI compose the adapter implementations at runtime.

use crate::{DomainError, ScanResult};

// ── Scanner ──────────────────────────────────────────────────────────────

/// Port for filesystem scanning.
///
/// Implementations walk a directory tree, classify entries, and report
/// results back as a [`ScanResult`]. They are responsible for respecting
/// `.gitignore`, parallelising the walk, and emitting progress if they
/// so choose — the domain cares only about the final tree.
pub trait Scanner {
    /// Scan `path` and return its full tree.
    ///
    /// `path` is an absolute or scan-root-relative path string. Returns
    /// [`DomainError::InvalidPath`] when the path is empty, malformed, or
    /// not a directory; [`DomainError::PermissionDenied`] when the OS
    /// refuses access; [`DomainError::Io`] for any other I/O failure.
    fn scan(&self, path: &str) -> Result<ScanResult, DomainError>;

    /// Return the last-modified time (Unix seconds) of the scan root
    /// directory. Used as a cheap staleness probe before deciding
    /// whether a full scan is needed.
    ///
    /// Returns the same error variants as [`Scanner::scan`].
    fn stat_root(&self, path: &str) -> Result<u64, DomainError>;
}

// ── Trash ────────────────────────────────────────────────────────────────

/// Port for safe delete with undo.
///
/// Implementations move entries to the system trash (so the user can
/// recover them) and record enough state to undo the most recent move.
/// "Undo" is therefore a stack operation, not a general rewind.
pub trait Trash {
    /// Move `path` to the trash. Returns [`DomainError::PermissionDenied`]
    /// or [`DomainError::Io`] on failure; on success, `path` becomes a
    /// candidate for [`Trash::undo_last`].
    fn move_to_trash(&self, path: &str) -> Result<(), DomainError>;

    /// Restore the most recently trashed entry. Returns
    /// [`DomainError::InvalidPath`] with a "nothing to undo" message when
    /// the undo stack is empty.
    fn undo_last(&self) -> Result<(), DomainError>;
}

// ── Cache ────────────────────────────────────────────────────────────────

/// Port for caching scan results.
///
/// Implementations are keyed by the scanned path string and store the
/// full [`ScanResult`] plus whatever metadata is needed to decide
/// whether a cache entry is still valid (mtime, size, etc.).
pub trait Cache {
    /// Look up `path` in the cache. Returns `Some(result)` on hit,
    /// `None` on miss.
    fn get(&self, path: &str) -> Option<ScanResult>;

    /// Persist `result` for `path`. Returns [`DomainError::Io`] on
    /// backend failure.
    fn put(&self, path: &str, result: &ScanResult) -> Result<(), DomainError>;

    /// Drop the cache entry for `path`. Returns [`DomainError::Io`] on
    /// backend failure; succeeds silently when the key is absent.
    fn invalidate(&self, path: &str) -> Result<(), DomainError>;

    /// Look up `path` along with scan-root metadata `(mtime, total_bytes)`
    /// used for incremental invalidation. Returns `None` on miss.
    ///
    /// The default implementation delegates to [`Cache::get`] and returns
    /// zero metadata — suitable for caches that don't track staleness.
    fn get_with_metadata(&self, path: &str) -> Option<(ScanResult, u64, u64)> {
        self.get(path).map(|r| (r, 0, 0))
    }

    /// Persist `result` along with scan-root `mtime` and `total_bytes`
    /// for `path`. Companion to [`Cache::get_with_metadata`].
    ///
    /// The default implementation delegates to [`Cache::put`], discarding
    /// the metadata — suitable for caches that don't track staleness.
    fn put_with_metadata(
        &self,
        path: &str,
        result: &ScanResult,
        mtime: u64,
        total_bytes: u64,
    ) -> Result<(), DomainError> {
        let _ = (mtime, total_bytes);
        self.put(path, result)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileNode, FileType};
    use std::cell::RefCell;
    use std::collections::HashMap;

    // -- Mock Scanner --

    struct MockScanner {
        result: ScanResult,
    }

    impl MockScanner {
        fn new(result: ScanResult) -> Self {
            Self { result }
        }
    }

    impl Scanner for MockScanner {
        fn scan(&self, _path: &str) -> Result<ScanResult, DomainError> {
            Ok(self.result.clone())
        }

        fn stat_root(&self, _path: &str) -> Result<u64, DomainError> {
            Ok(self.result.root.modified)
        }
    }

    #[test]
    fn should_return_canned_scanresult_when_scanner_scan_called_via_mock() {
        let root = FileNode {
            path: "/test".into(),
            size: 100,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![],
        };
        let result = ScanResult::from_tree(root, 10);
        let scanner = MockScanner::new(result.clone());

        let scanned = scanner.scan("/test").unwrap();
        assert_eq!(scanned, result);
    }

    // -- Mock Trash --

    struct MockTrash {
        recorded: RefCell<Vec<String>>,
    }

    impl MockTrash {
        fn new() -> Self {
            Self {
                recorded: RefCell::new(Vec::new()),
            }
        }
    }

    impl Trash for MockTrash {
        fn move_to_trash(&self, path: &str) -> Result<(), DomainError> {
            self.recorded.borrow_mut().push(path.into());
            Ok(())
        }

        fn undo_last(&self) -> Result<(), DomainError> {
            match self.recorded.borrow_mut().pop() {
                Some(_) => Ok(()),
                None => Err(DomainError::InvalidPath("nothing to undo".into())),
            }
        }
    }

    #[test]
    fn should_record_path_when_trash_move_to_trash_called() {
        let trash = MockTrash::new();
        trash.move_to_trash("/tmp/a.txt").unwrap();
        trash.move_to_trash("/tmp/b.txt").unwrap();
        assert_eq!(*trash.recorded.borrow(), vec!["/tmp/a.txt", "/tmp/b.txt"]);
    }

    #[test]
    fn should_pop_last_recorded_path_when_trash_undo_last_called() {
        let trash = MockTrash::new();
        trash.move_to_trash("/tmp/a.txt").unwrap();
        trash.move_to_trash("/tmp/b.txt").unwrap();
        trash.undo_last().unwrap();
        assert_eq!(*trash.recorded.borrow(), vec!["/tmp/a.txt"]);
    }

    #[test]
    fn should_return_invalid_path_error_when_trash_undo_last_called_on_empty_stack() {
        let trash = MockTrash::new();
        let result = trash.undo_last();
        assert!(matches!(result, Err(DomainError::InvalidPath(_))));
    }

    // -- Mock Cache --

    struct MockCache {
        entries: RefCell<HashMap<String, ScanResult>>,
    }

    impl MockCache {
        fn new() -> Self {
            Self {
                entries: RefCell::new(HashMap::new()),
            }
        }
    }

    impl Cache for MockCache {
        fn get(&self, path: &str) -> Option<ScanResult> {
            self.entries.borrow().get(path).cloned()
        }

        fn put(&self, path: &str, result: &ScanResult) -> Result<(), DomainError> {
            self.entries
                .borrow_mut()
                .insert(path.into(), result.clone());
            Ok(())
        }

        fn invalidate(&self, path: &str) -> Result<(), DomainError> {
            self.entries.borrow_mut().remove(path);
            Ok(())
        }
    }

    fn sample_result(path: &str, size: u64) -> ScanResult {
        let root = FileNode {
            path: path.into(),
            size,
            modified: 42,
            file_type: FileType::Directory,
            children: vec![],
        };
        ScanResult::from_tree(root, 5)
    }

    #[test]
    fn should_return_cached_scanresult_when_cache_get_called_with_known_key() {
        let cache = MockCache::new();
        let result = sample_result("/cached", 512);
        cache.put("/cached", &result).unwrap();
        let cached = cache.get("/cached");
        assert_eq!(cached, Some(result));
    }

    #[test]
    fn should_return_none_when_cache_get_called_with_unknown_key() {
        let cache = MockCache::new();
        assert!(cache.get("/nonexistent").is_none());
    }

    #[test]
    fn should_evict_entry_when_cache_invalidate_called() {
        let cache = MockCache::new();
        let result = sample_result("/stale", 256);
        cache.put("/stale", &result).unwrap();
        assert!(cache.get("/stale").is_some());
        cache.invalidate("/stale").unwrap();
        assert!(cache.get("/stale").is_none());
    }
}
