use crate::{DomainError, ScanResult};

// ── Scanner ──────────────────────────────────────────────────────────────

/// Port for filesystem scanning. Implemented by adapters (scan-engine).
pub trait Scanner {
    fn scan(&self, path: &str) -> Result<ScanResult, DomainError>;
}

// ── Trash ────────────────────────────────────────────────────────────────

/// Port for safe delete with undo. Implemented by adapters (scan-engine via `trash` crate).
pub trait Trash {
    fn move_to_trash(&self, path: &str) -> Result<(), DomainError>;
    fn undo_last(&self) -> Result<(), DomainError>;
}

// ── Cache ────────────────────────────────────────────────────────────────

/// Port for scan result caching. Implemented by adapters (scan-engine via redb).
pub trait Cache {
    fn get(&self, path: &str) -> Option<ScanResult>;
    fn put(&self, path: &str, result: &ScanResult) -> Result<(), DomainError>;
    fn invalidate(&self, path: &str) -> Result<(), DomainError>;
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileType, FileNode, ScanResult};
    use std::collections::HashMap;
    use std::cell::RefCell;

    // -- Mock Scanner --

    struct MockScanner {
        result: ScanResult,
    }

    impl Scanner for MockScanner {
        fn scan(&self, _path: &str) -> Result<ScanResult, DomainError> {
            Ok(self.result.clone())
        }
    }

    #[test]
    fn should_require_scan_method_when_scanner_trait_implemented() {
        let root = FileNode {
            path: "/test".into(),
            size: 100,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![],
        };
        let result = ScanResult::from_tree(root, 10);
        let scanner = MockScanner { result: result.clone() };

        let scanned = scanner.scan("/test").unwrap();
        assert_eq!(scanned, result);
    }

    // -- Mock Trash --

    struct MockTrash {
        deleted: RefCell<Vec<String>>,
        undo_stack: RefCell<Vec<String>>,
    }

    impl MockTrash {
        fn new() -> Self {
            Self {
                deleted: RefCell::new(Vec::new()),
                undo_stack: RefCell::new(Vec::new()),
            }
        }
    }

    impl Trash for MockTrash {
        fn move_to_trash(&self, path: &str) -> Result<(), DomainError> {
            self.deleted.borrow_mut().push(path.into());
            self.undo_stack.borrow_mut().push(path.into());
            Ok(())
        }

        fn undo_last(&self) -> Result<(), DomainError> {
            match self.undo_stack.borrow_mut().pop() {
                Some(_) => Ok(()),
                None => Err(DomainError::InvalidPath("nothing to undo".into())),
            }
        }
    }

    #[test]
    fn should_require_move_to_trash_and_undo_last_when_trash_trait_implemented() {
        let trash = MockTrash::new();

        trash.move_to_trash("/tmp/file.txt").unwrap();
        assert_eq!(trash.deleted.borrow().len(), 1);

        trash.undo_last().unwrap();
        assert!(trash.undo_stack.borrow().is_empty());
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
            self.entries.borrow_mut().insert(path.into(), result.clone());
            Ok(())
        }

        fn invalidate(&self, path: &str) -> Result<(), DomainError> {
            self.entries.borrow_mut().remove(path);
            Ok(())
        }
    }

    #[test]
    fn should_return_cached_result_when_cache_get_called_with_known_path() {
        let cache = MockCache::new();
        let root = FileNode {
            path: "/cached".into(),
            size: 512,
            modified: 42,
            file_type: FileType::Directory,
            children: vec![],
        };
        let result = ScanResult::from_tree(root, 5);

        cache.put("/cached", &result).unwrap();
        let cached = cache.get("/cached");
        assert_eq!(cached, Some(result));
    }

    #[test]
    fn should_invalidate_entry_when_cache_invalidate_called() {
        let cache = MockCache::new();
        let root = FileNode {
            path: "/stale".into(),
            size: 256,
            modified: 99,
            file_type: FileType::Directory,
            children: vec![],
        };
        let result = ScanResult::from_tree(root, 3);

        cache.put("/stale", &result).unwrap();
        assert!(cache.get("/stale").is_some());

        cache.invalidate("/stale").unwrap();
        assert!(cache.get("/stale").is_none());
    }

    #[test]
    fn should_return_none_when_cache_get_called_with_unknown_path() {
        let cache = MockCache::new();
        assert!(cache.get("/nonexistent").is_none());
    }
}
