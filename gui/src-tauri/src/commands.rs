//! Tauri commands bridging the frontend to `scan-engine`.
//!
//! Every command maps 1:1 onto a `scan_runner` / `scan-engine`
//! operation and returns a typed [`CommandErrorDto`] on failure so the
//! frontend can render it. All commands are thin: validation and state
//! transitions live in `scan_runner` / the domain.

use tauri::{AppHandle, State};

use crate::dto::{CommandErrorDto, FilterDto};
use crate::scan_runner::{ScanId, ScanRunner};

/// Start a scan of `path`, optionally applying `filter` (the GUI sends
/// `max_age`; the Rust side stamps `now` via [`FilterDto::into_domain`]).
/// Returns the new [`ScanId`].
#[tauri::command]
pub fn start_scan(
    path: String,
    filter: Option<FilterDto>,
    runner: State<'_, ScanRunner>,
    app: AppHandle,
) -> Result<ScanId, CommandErrorDto> {
    let domain_filter = filter.map(FilterDto::into_domain);
    runner.start(path, domain_filter, app).map_err(CommandErrorDto::from)
}

/// Cancel the scan identified by `scan_id`. Cancelling an already-Done
/// scan is a no-op returning the current [`ScanId`].
#[tauri::command]
pub fn cancel_scan(
    scan_id: ScanId,
    runner: State<'_, ScanRunner>,
) -> Result<ScanId, CommandErrorDto> {
    Ok(runner.cancel(scan_id))
}

/// Move `paths` to the system trash. Returns
/// [`CommandErrorDto::ScanInProgress`] while a scan is running so a
/// mutation never races a half-written tree.
#[tauri::command]
pub fn delete_paths(
    paths: Vec<String>,
    runner: State<'_, ScanRunner>,
) -> Result<(), CommandErrorDto> {
    if runner.is_running() {
        return Err(CommandErrorDto::ScanInProgress("scan in progress".into()));
    }
    for path in paths {
        runner.service().move_to_trash(&path)?;
    }
    Ok(())
}

/// Undo the most recent move-to-trash. Returns a "nothing to undo" error
/// when the undo stack is empty.
#[tauri::command]
pub fn undo_last_delete(runner: State<'_, ScanRunner>) -> Result<(), CommandErrorDto> {
    if runner.is_running() {
        return Err(CommandErrorDto::ScanInProgress("scan in progress".into()));
    }
    runner.service().undo_last().map_err(CommandErrorDto::from)
}

/// Reveal `path` in the OS file manager. Uses the `opener` crate, which
/// resolves the platform's native reveal action.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), CommandErrorDto> {
    if path.trim().is_empty() {
        return Err(CommandErrorDto::InvalidPath("path must not be empty".into()));
    }
    opener::reveal(&path).map_err(|e| CommandErrorDto::Io(e.to_string()))
}

/// Open `path` with the OS default application. Uses the `opener` crate.
#[tauri::command]
pub fn open_file(path: String) -> Result<(), CommandErrorDto> {
    if path.trim().is_empty() {
        return Err(CommandErrorDto::InvalidPath("path must not be empty".into()));
    }
    opener::open(&path).map_err(|e| CommandErrorDto::Io(e.to_string()))
}

/// Return the finished scan result (if any) as a DTO, so the frontend
/// can refresh after reconnection or on demand.
#[tauri::command]
pub fn get_scan_result(
    runner: State<'_, ScanRunner>,
) -> Result<Option<crate::dto::ScanResultDto>, CommandErrorDto> {
    Ok(runner.result().map(|r| crate::dto::ScanResultDto::from(&*r)))
}
