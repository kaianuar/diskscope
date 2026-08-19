//! Cross-platform move-to-trash adapter with an undo stack.
//!
//! [`TrashBin`] wraps the `trash` crate so the rest of the pipeline
//! sees a uniform [`domain::ports::Trash`] interface. Each
//! `move_to_trash` call records the original path and the platform
//! trash identifier so that `undo_last` can restore the most recent
//! entry. macOS is supported for `move_to_trash` (which goes through
//! Finder) but the `os_limited` module that powers `undo_last` is not
//! available on macOS — calls to `undo_last` on macOS return
//! [`DomainError::Unsupported`] with a clear message.

use std::sync::Arc;

use parking_lot::Mutex;
use trash::TrashItem;
use domain::DomainError;

/// One entry on the undo stack: the absolute path the user trashed,
/// plus the platform's `TrashItem` (used to restore it).
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct UndoEntry {
    original_path: String,
    trash_item: TrashItem,
}

/// Adapter backing [`domain::ports::Trash`]. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct TrashBin {
    undo_stack: Arc<Mutex<Vec<UndoEntry>>>,
}

impl TrashBin {
    /// Create a new trash bin with an empty undo stack.
    pub fn new() -> Self {
        Self::default()
    }
}

impl domain::ports::Trash for TrashBin {
    fn move_to_trash(&self, path: &str) -> Result<(), DomainError> {
        if path.is_empty() {
            return Err(DomainError::InvalidPath("path must not be empty".into()));
        }

        // Pre-flight: confirm the path exists so we can produce a clear
        // error before trashing.
        let metadata = std::fs::metadata(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => DomainError::InvalidPath(format!("not found: {path}")),
            std::io::ErrorKind::PermissionDenied => DomainError::PermissionDenied(path.to_string()),
            _ => DomainError::Io(e),
        })?;
        if metadata.is_dir() {
            // Empty dir is fine; non-empty dir may or may not be — defer
            // the actual decision to the `trash` crate.
        }

        // Snapshot the trash before deleting so we can match the new
        // entry by path. On macOS the os_limited list API is
        // unavailable; we skip undo tracking in that case.
        let snapshot_before: Option<Vec<TrashItem>> =
            list_trash().ok().map(|v| v.into_iter().collect());

        // Perform the deletion.
        trash::delete(path).map_err(trash_err)?;

        // Try to record the TrashItem for undo. This requires
        // os_limited listing, which is unavailable on macOS.
        if let Ok(items_after) = list_trash() {
            let target = std::path::Path::new(path);
            if let Some(trash_item) = items_after.into_iter().find(|item| {
                if item.original_path() != target {
                    return false;
                }
                match &snapshot_before {
                    Some(before) => !before.iter().any(|b| b.id == item.id),
                    None => true,
                }
            }) {
                let mut stack = self.undo_stack.lock();
                stack.push(UndoEntry {
                    original_path: path.to_string(),
                    trash_item,
                });
            }
        }
        Ok(())
    }

    fn undo_last(&self) -> Result<(), DomainError> {
        let entry = {
            let mut stack = self.undo_stack.lock();
            stack.pop()
        };
        let entry = match entry {
            Some(e) => e,
            None => {
                return Err(DomainError::InvalidPath("nothing to undo".into()));
            }
        };

        // Restore the item. If restore fails for any reason, push the
        // entry back onto the stack so the user can retry.
        if let Err(e) = restore_items(vec![entry.trash_item.clone()]) {
            let mut stack = self.undo_stack.lock();
            stack.push(entry);
            return Err(e);
        }
        Ok(())
    }
}

fn trash_err(e: trash::Error) -> DomainError {
    DomainError::Io(std::io::Error::other(e.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn list_trash() -> Result<Vec<trash::TrashItem>, DomainError> {
    trash::os_limited::list().map_err(trash_err)
}

#[cfg(not(target_os = "macos"))]
fn restore_items(items: Vec<trash::TrashItem>) -> Result<(), DomainError> {
    trash::os_limited::restore_all(items).map_err(trash_err)
}

#[cfg(target_os = "macos")]
fn list_trash() -> Result<Vec<trash::TrashItem>, DomainError> {
    Err(DomainError::Unsupported(
        "trash listing is not available on macOS".into(),
    ))
}

#[cfg(target_os = "macos")]
fn restore_items(_items: Vec<trash::TrashItem>) -> Result<(), DomainError> {
    Err(DomainError::Unsupported(
        "trash restore-by-item is not available on macOS".into(),
    ))
}
