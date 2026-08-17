//! DiskScope GUI — Tauri application backend.

pub mod commands;
pub mod state;

/// Initializes and runs the Tauri application.
#[cfg(feature = "tauri")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use state::AppState;

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            tauri_commands::start_scan,
            tauri_commands::delete_file,
            tauri_commands::undo_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DiskScope");
}

/// Thin Tauri command wrappers that delegate to the pure handler functions.
#[cfg(feature = "tauri")]
mod tauri_commands {
    use tauri::State;

    use crate::commands::{handle_delete, handle_scan, handle_undo, ScanFilter};
    use crate::state::AppState;
    use scan_engine::domain::ScanResult;

    #[tauri::command]
    pub fn start_scan(
        path: String,
        filter: Option<ScanFilter>,
        state: State<'_, AppState>,
    ) -> Result<ScanResult, String> {
        handle_scan(&path, filter, &state)
    }

    #[tauri::command]
    pub fn delete_file(path: String, state: State<'_, AppState>) -> Result<String, String> {
        handle_delete(&path, &state)
    }

    #[tauri::command]
    pub fn undo_delete(state: State<'_, AppState>) -> Result<String, String> {
        handle_undo(&state)
    }
}
