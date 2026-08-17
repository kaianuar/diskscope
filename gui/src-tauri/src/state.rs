//! Application state shared across Tauri commands.

use std::path::PathBuf;

use parking_lot::Mutex;
use scan_engine::domain::ScanResult;

/// Tracks the most recent scan and all pending trash operations.
pub struct AppState {
    /// Most recent completed scan result (if any).
    pub last_scan: Mutex<Option<ScanResult>>,
    /// Stack of trash operations for undo (path → trash id).
    pub trash_log: Mutex<Vec<TrashEntry>>,
    /// Whether a scan is currently running.
    pub scanning: Mutex<bool>,
}

/// Record of a single delete-to-trash operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrashEntry {
    /// Original path before deletion.
    pub original_path: PathBuf,
    /// Opaque trash identifier for undo.
    pub trash_id: String,
}

impl AppState {
    /// Creates a new empty application state.
    pub fn new() -> Self {
        Self {
            last_scan: Mutex::new(None),
            trash_log: Mutex::new(Vec::new()),
            scanning: Mutex::new(false),
        }
    }

    /// Pushes a trash entry onto the undo stack.
    pub fn push_trash(&self, entry: TrashEntry) {
        self.trash_log.lock().push(entry);
    }

    /// Pops the most recent trash entry for undo.
    pub fn pop_trash(&self) -> Option<TrashEntry> {
        self.trash_log.lock().pop()
    }

    /// Returns the number of entries on the undo stack.
    pub fn trash_count(&self) -> usize {
        self.trash_log.lock().len()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop_trash() {
        let state = AppState::new();
        state.push_trash(TrashEntry {
            original_path: PathBuf::from("/tmp/test.txt"),
            trash_id: "id-1".into(),
        });
        state.push_trash(TrashEntry {
            original_path: PathBuf::from("/tmp/test2.txt"),
            trash_id: "id-2".into(),
        });
        assert_eq!(state.trash_count(), 2);
        let entry = state.pop_trash().unwrap();
        assert_eq!(entry.trash_id, "id-2");
        let entry = state.pop_trash().unwrap();
        assert_eq!(entry.trash_id, "id-1");
        assert!(state.pop_trash().is_none());
    }

    #[test]
    fn test_scanning_flag() {
        let state = AppState::new();
        assert!(!*state.scanning.lock());
        *state.scanning.lock() = true;
        assert!(*state.scanning.lock());
    }
}
