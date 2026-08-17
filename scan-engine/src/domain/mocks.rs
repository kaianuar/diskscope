use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::CachedEntry;
use super::error::{CacheError, ScanError, TrashError};
use super::filenode::FileNode;
use super::opts::ScanOpts;
use super::ports::{Cache, Scanner, Trash};
use super::TrashTicket;

// --- MockScanner ---

/// Returns a pre-built FileNode tree. For testing scan callers.
pub struct MockScanner {
    pub tree: FileNode,
}

impl MockScanner {
    pub fn new(tree: FileNode) -> Self {
        Self { tree }
    }
}

impl Scanner for MockScanner {
    fn scan(&self, _root: &Path, _opts: &ScanOpts) -> Result<FileNode, ScanError> {
        Ok(self.tree.clone())
    }
}

// --- MockCache ---

/// HashMap-backed in-memory cache.
#[derive(Default)]
pub struct MockCache {
    entries: HashMap<PathBuf, CachedEntry>,
}

impl MockCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }
}

impl Cache for MockCache {
    fn get(&self, path: &Path) -> Result<Option<CachedEntry>, CacheError> {
        Ok(self.entries.get(path).cloned())
    }

    fn put(&self, _path: &Path, _entry: &CachedEntry) -> Result<(), CacheError> {
        // Interior mutability would be needed for real put; mock is read-only by design.
        Ok(())
    }

    fn invalidate(&self, _path: &Path) -> Result<(), CacheError> {
        Ok(())
    }
}

// --- MockTrash ---

/// Tracks deleted paths; `undo` checks the ticket path was previously deleted.
pub struct MockTrash {
    deleted: std::cell::RefCell<Vec<PathBuf>>,
}

impl MockTrash {
    pub fn new() -> Self {
        Self { deleted: std::cell::RefCell::new(Vec::new()) }
    }

    pub fn deleted(&self) -> Vec<PathBuf> {
        self.deleted.borrow().clone()
    }
}

impl Trash for MockTrash {
    fn delete(&self, path: &Path) -> Result<TrashTicket, TrashError> {
        self.deleted.borrow_mut().push(path.to_path_buf());
        Ok(TrashTicket { path: path.to_path_buf() })
    }

    fn undo(&self, ticket: &TrashTicket) -> Result<(), TrashError> {
        let mut deleted = self.deleted.borrow_mut();
        if let Some(pos) = deleted.iter().position(|p| p == &ticket.path) {
            deleted.remove(pos);
            Ok(())
        } else {
            Err(TrashError::Io("ticket not found".into()))
        }
    }
}
