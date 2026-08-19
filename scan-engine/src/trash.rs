//! Cross-platform move-to-trash adapter with an undo stack.
//!
//! [`TrashBin`] wraps the `trash` crate so the rest of the pipeline
//! sees a uniform [`domain::ports::Trash`] interface. Each
//! `move_to_trash` call records the original path and the platform
//! trash identifier so that `undo_last` can restore the most recent
//! entry. macOS is supported for `move_to_trash` (which goes through
//! Finder) but the `os_limited` module that powers `undo_last` is not
//! available on macOS — calls to `undo_last` on macOS return
//! [`DomainError::Io`] with a clear message.

use std::sync::Arc;

use parking_lot::Mutex;
use trash::os_limited;
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

        // Best-effort: try to snapshot the trash BEFORE deleting so we
        // can match the new entry by path. On macOS, `os_limited`
        // listing is unavailable; we silently skip the snapshot,
        // meaning future `undo_last` calls return an explicit error.
        let snapshot_before: Option<Vec<TrashItem>> =
            os_limited::list().ok().map(|v| v.into_iter().collect());

        // Perform the deletion.
        trash::delete(path).map_err(trash_err)?;
        let items_after = os_limited::list().map_err(trash_err)?;

        // Strategy: find items not in `snapshot_before` whose
        // `original_path` matches `path`. If snapshot is empty (first
        // call ever), match any item whose original_path matches.
        let target = std::path::Path::new(path);
        let trash_item = items_after
            .into_iter()
            .find(|item| {
                if item.original_path() != target {
                    return false;
                }
                match &snapshot_before {
                    Some(before) => !before.iter().any(|b| b.id == item.id),
                    None => true,
                }
            })
            .ok_or_else(|| {
                DomainError::Io(std::io::Error::other(format!(
                    "trash entry not found after delete for {path}"
                )))
            })?;

        let mut stack = self.undo_stack.lock();
        stack.push(UndoEntry {
            original_path: path.to_string(),
            trash_item,
        });
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
        if let Err(e) = os_limited::restore_all(vec![entry.trash_item.clone()]) {
            let mut stack = self.undo_stack.lock();
            stack.push(entry);
            return Err(trash_err(e));
        }
        Ok(())
    }
}

fn trash_err(e: trash::Error) -> DomainError {
    DomainError::Io(std::io::Error::other( e.to_string()))
}
