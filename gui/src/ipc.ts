import { invoke } from "@tauri-apps/api/core";
import type { ScanResult, ScanFilter } from "./domain";

/**
 * Type-safe wrappers around Tauri IPC commands.
 * Falls back to mock implementations when running outside Tauri (browser dev).
 */

const IS_TAURI = "__TAURI_INTERNALS__" in window;

export async function startScan(
  path: string,
  filter?: ScanFilter
): Promise<ScanResult> {
  if (IS_TAURI) {
    return invoke<ScanResult>("start_scan", { path, filter });
  }
  // Dev fallback: return empty result
  return {
    root: path,
    root_node: { path, name: path.split("/").pop() || path, size: 0, modified: new Date().toISOString(), node_type: "Other", is_dir: true, children: [] },
    total_size: 0,
    file_count: 0,
    dir_count: 0,
    scan_duration: "0s",
  };
}

export async function deleteFile(path: string): Promise<string> {
  if (IS_TAURI) {
    return invoke<string>("delete_file", { path });
  }
  return "mock-trash-id";
}

export async function undoDelete(): Promise<string> {
  if (IS_TAURI) {
    return invoke<string>("undo_delete");
  }
  return "mock-undo";
}
