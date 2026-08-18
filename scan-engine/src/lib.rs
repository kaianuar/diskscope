pub mod cache;
pub mod domain;
pub mod filter;
pub mod format;
pub mod output;
pub mod ports;
pub mod scanner;
pub mod tree;

use std::fmt;
use std::path::PathBuf;

// ── Public API ─────────────────────────────────────────────────────────────
//
// Hexagonal architecture: domain types at center, adapters at edges.
//
// Domain (pure, zero deps):
//   FileTree, Filter, FilterSet, FileNode, FileType, NodeKind
//
// Adapters:
//   Scanner  — parallel filesystem walker (ignore + rayon)
//   Cache    — persistent snapshot cache (redb)
//   ScanConfig — scanner knobs
//   OutputFormat — serialization format selector

/// Parallel directory scanner — walks a filesystem tree and builds a [`ScanResult`].
///
/// Uses `ignore::WalkBuilder` for `.gitignore`-aware, parallel traversal.
/// Configure with [`ScanConfig`].
pub use scanner::walker::Scanner;

/// Scanner configuration — minimal knobs for filesystem walking.
pub use scanner::walker::ScanConfig;

/// Persistent snapshot cache backed by `redb`.
///
/// Stores entire [`domain::FileTree`] snapshots for incremental re-scans.
pub use cache::Cache;

/// Complete file tree rooted at a single directory (domain model).
pub use domain::FileTree;

/// Single filter criterion applied to a file tree node.
pub use domain::Filter;

/// Set of filters combined with AND logic.
pub use domain::FilterSet;

// OutputFormat is defined inline below (used by ScanOptions).

// ── Enums ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Size,
    Name,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Jsonl,
    Tree,
}

// ── Core domain types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub modified: u64, // Unix timestamp (seconds)
    pub node_type: NodeType,
    pub depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub entry: FileEntry,
    pub children: Vec<TreeNode>,
    /// Aggregated size: own size + sum of all descendant file sizes.
    pub total_size: u64,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub root: TreeNode,
    pub total_size: u64,
    pub entry_count: usize,
}

// ── Options & filters ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub max_depth: Option<u32>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub types: Vec<String>,
    pub max_age_days: Option<u32>,
    pub pattern: Option<String>,
    pub sort_key: Option<SortKey>,
    pub sort_dir: Option<SortDir>,
    pub format: OutputFormat,
    pub follow_symlinks: bool,
    pub respect_gitignore: bool,
    pub filters: Vec<FilterSpec>,
}

#[derive(Debug, Clone, Default)]
pub struct FilterSpec {
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub types: Vec<String>,
    pub max_age_days: Option<u32>,
    pub pattern: Option<String>,
}

// ── Port helper types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedEntry {
    pub entry: FileEntry,
    pub scan_time: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedFile {
    pub original_path: PathBuf,
    pub trash_id: String,
}

// ── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    IoError(String),
    PermissionDenied(String),
    InvalidPath(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrashError {
    IoError(String),
    FileNotFound(String),
    UndoFailed(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::IoError(msg) => write!(f, "scan I/O error: {msg}"),
            ScanError::PermissionDenied(path) => {
                write!(f, "permission denied: {path}")
            }
            ScanError::InvalidPath(path) => write!(f, "invalid path: {path}"),
        }
    }
}

impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrashError::IoError(msg) => write!(f, "trash I/O error: {msg}"),
            TrashError::FileNotFound(path) => write!(f, "file not found: {path}"),
            TrashError::UndoFailed(msg) => write!(f, "undo failed: {msg}"),
        }
    }
}

impl std::error::Error for ScanError {}
impl std::error::Error for TrashError {}
