/** Union of file type classifications matching the Rust FileType enum. */
export type FileType =
  | "Image"
  | "Video"
  | "Audio"
  | "Document"
  | "Code"
  | "Archive"
  | "Other";

/** Serialized filter sent to the Rust backend. */
export interface ScanFilter {
  min_size?: number;
  max_size?: number;
  file_types?: string[];
  name_pattern?: string;
  max_depth?: number;
}

/** Mirrors scan_engine::domain::FileNode (serialized via serde). */
export interface FileNode {
  path: string;
  name: string;
  size: number;
  modified: string; // ISO 8601
  node_type: FileType;
  is_dir: boolean;
  children: FileNode[];
}

/** Mirrors scan_engine::domain::ScanResult (serialized via serde). */
export interface ScanResult {
  root: string;
  root_node: FileNode;
  total_size: number;
  file_count: number;
  dir_count: number;
  scan_duration: string; // e.g. "1.234s"
}
