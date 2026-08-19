// Typed wrapper around Tauri `invoke()` for the DiskScope commands.
//
// The Rust side (gui/src-tauri) serialises DTOs with camelCase renames
// and lowercase file-type enums; these types mirror that contract.

import { invoke } from '@tauri-apps/api/core';

export type FileTypeName =
  | 'audio'
  | 'video'
  | 'image'
  | 'document'
  | 'code'
  | 'archive'
  | 'directory'
  | 'other';

export interface FileNode {
  path: string;
  size: number;
  modified: number;
  fileType: FileTypeName;
  children: FileNode[];
}

export interface ScanResult {
  root: FileNode;
  totalSize: number;
  fileCount: number;
  scanDurationMs: number;
  skipped: Array<{ path: string; kind: string; message: string }>;
}

export interface Filter {
  minSize?: number;
  maxSize?: number;
  fileTypes?: FileTypeName[];
  namePattern?: string;
  maxAge?: number;
  maxDepth?: number;
}

export type CommandError =
  | { kind: 'invalidPath'; message: string }
  | { kind: 'permissionDenied'; message: string }
  | { kind: 'invalidFilter'; message: string }
  | { kind: 'io'; message: string }
  | { kind: 'scanInProgress'; message: string };

export type SortColumn = 'name' | 'size' | 'modified' | 'type';
export type SortDirection = 'asc' | 'desc';

/** Start a scan of `path`, returning the scan id. */
export function startScan(path: string, filter?: Filter): Promise<number> {
  return invoke<number>('start_scan', { path, filter: filter ?? null });
}

/** Cancel the scan with the given id; returns the current scan id. */
export function cancelScan(scanId: number): Promise<number> {
  return invoke<number>('cancel_scan', { scanId });
}

/** Move `paths` to the system trash. */
export function deletePaths(paths: string[]): Promise<void> {
  return invoke<void>('delete_paths', { paths });
}

/** Undo the most recent move-to-trash. */
export function undoLastDelete(): Promise<void> {
  return invoke<void>('undo_last_delete');
}

/** Reveal `path` in the OS file manager. */
export function revealInExplorer(path: string): Promise<void> {
  return invoke<void>('reveal_in_explorer', { path });
}

/** Open `path` with the OS default application. */
export function openFile(path: string): Promise<void> {
  return invoke<void>('open_file', { path });
}

/** Fetch the finished scan result, if any. */
export function getScanResult(): Promise<ScanResult | null> {
  return invoke<ScanResult | null>('get_scan_result');
}

/** Decode a CommandError from an invoke rejection into a typed error. */
export function toCommandError(err: unknown): CommandError {
  if (err && typeof err === 'object') {
    const e = err as Record<string, unknown>;
    const kind = String(e.kind ?? '').replace(/([a-z])([A-Z])/g, '$1$2').toLowerCase();
    const message = typeof e.message === 'string' ? e.message : String(err);
    if (kind === 'invalidpath') return { kind: 'invalidPath', message };
    if (kind === 'permissiondenied') return { kind: 'permissionDenied', message };
    if (kind === 'invalidfilter') return { kind: 'invalidFilter', message };
    if (kind === 'io') return { kind: 'io', message };
    if (kind === 'scaninprogress') return { kind: 'scanInProgress', message };
  }
  return { kind: 'io', message: String(err) };
}
