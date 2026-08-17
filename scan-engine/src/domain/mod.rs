/// Domain error types.
pub mod error;
/// File type classification by extension.
pub mod file_type;
/// File tree node representation.
pub mod filenode;
/// Filter criteria for scan results.
pub mod filter;
/// Output format definitions.
pub mod format;
/// Scan operation options.
pub mod opts;
/// Port traits for scanners, caches, and trash.
pub mod ports;
/// Byte size with human-readable display.
pub mod size;
/// Sort key definitions.
pub mod sort;
/// File tree wrapper with aggregate stats.
pub mod tree;

use std::path::PathBuf;

/// Cached file metadata for incremental scans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedEntry {
    /// File size in bytes.
    pub size: u64,
    /// Modification time as Unix timestamp (seconds).
    pub mtime: u64,
}

/// Receipt from a trash operation, used to undo deletions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashTicket {
    /// Original path of the deleted file or directory.
    pub path: PathBuf,
    /// Unix timestamp (seconds since epoch) when the file was deleted.
    pub deleted_at: u64,
}

/// Mock implementations of port traits for testing.
pub mod mocks;
