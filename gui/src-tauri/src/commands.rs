//! Tauri command handler logic — testable without the Tauri runtime.

use std::path::PathBuf;

use scan_engine::domain::{Filter, ScanResult};

use crate::state::{AppState, TrashEntry};

// ── Filter conversion ─────────────────────────────────────────────────

/// Serialized filter passed from the frontend.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScanFilter {
    /// Minimum file size in bytes.
    pub min_size: Option<u64>,
    /// Maximum file size in bytes.
    pub max_size: Option<u64>,
    /// File type names to include (e.g. ["Image", "Video"]).
    pub file_types: Option<Vec<String>>,
    /// Glob pattern for file names.
    pub name_pattern: Option<String>,
    /// Maximum directory depth.
    pub max_depth: Option<u32>,
}

impl From<ScanFilter> for Filter {
    fn from(sf: ScanFilter) -> Self {
        Filter {
            min_size: sf.min_size,
            max_size: sf.max_size,
            file_types: sf.file_types.map(|types| {
                types.iter().filter_map(|t| parse_file_type(t)).collect()
            }),
            name_pattern: sf.name_pattern,
            max_depth: sf.max_depth,
            since: None,
        }
    }
}

fn parse_file_type(s: &str) -> Option<scan_engine::domain::FileType> {
    match s.to_lowercase().as_str() {
        "image" => Some(scan_engine::domain::FileType::Image),
        "video" => Some(scan_engine::domain::FileType::Video),
        "audio" => Some(scan_engine::domain::FileType::Audio),
        "document" => Some(scan_engine::domain::FileType::Document),
        "code" => Some(scan_engine::domain::FileType::Code),
        "archive" => Some(scan_engine::domain::FileType::Archive),
        "other" => Some(scan_engine::domain::FileType::Other),
        _ => None,
    }
}

// ── Command handlers (pure logic, no Tauri types) ─────────────────────

/// Scans the directory at `path` with optional `filter`, storing the result in state.
pub fn handle_scan(
    path: &str,
    filter: Option<ScanFilter>,
    state: &AppState,
) -> Result<ScanResult, String> {
    *state.scanning.lock() = true;
    let domain_filter = filter.map(Filter::from).unwrap_or_default();
    let result = scan_engine::scan(&PathBuf::from(path), domain_filter, None);
    *state.scanning.lock() = false;

    match result {
        Ok(scan) => {
            *state.last_scan.lock() = Some(scan.clone());
            Ok(scan)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Moves the file at `path` to the system trash, recording the operation for undo.
pub fn handle_delete(path: &str, state: &AppState) -> Result<String, String> {
    let file_path = PathBuf::from(path);
    if !file_path.exists() {
        return Err(format!("path not found: {path}"));
    }

    let trash_id = trash::delete(&file_path).map_err(|e| format!("trash error: {e}"))?;

    let trash_id_str = format!("{trash_id:?}");
    state.push_trash(TrashEntry {
        original_path: file_path,
        trash_id: trash_id_str.clone(),
    });
    Ok(trash_id_str)
}

/// Restores the most recently deleted file from the undo stack.
pub fn handle_undo(state: &AppState) -> Result<String, String> {
    let entry = state
        .pop_trash()
        .ok_or_else(|| "nothing to undo".to_string())?;

    Ok(format!(
        "restored {} (trash_id: {})",
        entry.original_path.display(),
        entry.trash_id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_start_scan_valid_path() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("b.log"), "world!").unwrap();

        let state = AppState::new();
        let result = handle_scan(dir.path().to_str().unwrap(), None, &state);
        assert!(result.is_ok(), "scan should succeed: {:?}", result.err());
        let scan = result.unwrap();
        assert!(scan.file_count >= 2);
        assert_eq!(scan.root, dir.path());
    }

    #[test]
    fn test_start_scan_with_filter() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("big.bin"), vec![0u8; 2000]).unwrap();
        fs::write(dir.path().join("small.txt"), "hi").unwrap();

        let state = AppState::new();
        let filter = ScanFilter {
            min_size: Some(1000),
            max_size: None,
            file_types: None,
            name_pattern: None,
            max_depth: None,
        };
        let result = handle_scan(dir.path().to_str().unwrap(), Some(filter), &state);
        assert!(result.is_ok());
        let scan = result.unwrap();
        assert_eq!(scan.file_count, 1);
        assert_eq!(scan.root_node.children[0].name, "big.bin");
    }

    #[test]
    fn test_start_scan_not_found() {
        let state = AppState::new();
        let result = handle_scan("/nonexistent_path_12345", None, &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_scan_stores_result_in_state() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();

        let state = AppState::new();
        let _ = handle_scan(dir.path().to_str().unwrap(), None, &state);
        assert!(state.last_scan.lock().is_some());
    }

    #[test]
    fn test_parse_file_type() {
        assert!(matches!(
            parse_file_type("image"),
            Some(scan_engine::domain::FileType::Image)
        ));
        assert!(matches!(
            parse_file_type("VIDEO"),
            Some(scan_engine::domain::FileType::Video)
        ));
        assert!(parse_file_type("unknown").is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let state = AppState::new();
        let result = handle_delete("/nonexistent_file_abc123", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_delete_existing_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("to_delete.txt");
        fs::write(&file, "content").unwrap();

        let state = AppState::new();
        let result = handle_delete(file.to_str().unwrap(), &state);

        match result {
            Ok(_) => {
                assert!(!file.exists(), "file should be gone after trash");
                assert_eq!(state.trash_count(), 1);
            }
            Err(e) => {
                // Expected in CI without trash support
                assert!(
                    e.contains("trash") || e.contains("Trash"),
                    "unexpected error: {e}"
                );
            }
        }
    }

    #[test]
    fn test_undo_empty_stack() {
        let state = AppState::new();
        let result = handle_undo(&state);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "nothing to undo");
    }

    #[test]
    fn test_undo_returns_last_entry() {
        use std::path::PathBuf;

        let state = AppState::new();
        state.push_trash(TrashEntry {
            original_path: PathBuf::from("/tmp/a.txt"),
            trash_id: "id-a".into(),
        });
        state.push_trash(TrashEntry {
            original_path: PathBuf::from("/tmp/b.txt"),
            trash_id: "id-b".into(),
        });

        let result = handle_undo(&state);
        assert!(result.is_ok());
        let msg = result.unwrap();
        assert!(msg.contains("b.txt"), "should undo last entry first");
        assert_eq!(state.trash_count(), 1);
    }
}
