//! DiskScope GUI crate (Tauri v2 backend).
//!
//! The crate is split into a library (this file: DTOs, commands, scan
//! runner) and a thin binary (`main.rs`) that only installs the Tauri
//! builder and registers the commands. Splitting keeps the Tauri
//! boundary testable without spawning a window and lets the scan thread
//! reference the DTOs through the `gui::` crate path.

#![deny(missing_docs)]
#![deny(clippy::all)]
#![forbid(unsafe_code)]

/// Tauri command handlers bridging the frontend to `scan-engine`.
pub mod commands;
/// IPC data-transfer objects (serde mirrors of `domain` types).
pub mod dto;
/// Scan lifecycle state machine (`Idle → Running → Done/Cancelled → Idle`).
pub mod scan_runner;

use std::sync::Arc;

use scan_engine::ScanService;
use tauri::{Listener, Manager};

pub use scan_runner::{ScanId, ScanRunner};

/// React to a `scan-done` event: join the background thread and promote
/// its result into the [`ScanRunner`]'s `Done` state.
///
/// Registered as a global `tauri` event listener in [`run`]. Must be
/// called on the main thread (Tauri delivers events there), so joining
/// the finished thread cannot deadlock the UI.
pub fn on_scan_done(app: &tauri::AppHandle) {
    if let Some(runner) = app.try_state::<ScanRunner>() {
        runner.collect();
    }
}

/// Build and run the Tauri application.
pub fn run() {
    let service = Arc::new(ScanService::new());
    let runner = ScanRunner::new(service);

    tauri::Builder::default()
        .manage(runner)
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::cancel_scan,
            commands::delete_paths,
            commands::undo_last_delete,
            commands::reveal_in_explorer,
            commands::get_scan_result,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let handle_for_event = handle.clone();
            let _ = handle.listen_any("scan-done", move |_event| on_scan_done(&handle_for_event));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
