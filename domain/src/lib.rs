//! DiskScope domain layer.
//!
//! Pure-Rust entities, value objects, and ports. **Zero runtime dependencies**
//! outside of `std`. Every public item is documented; every behavior is
//! covered by a unit test (see the `tests` module at the bottom).
//!
//! The domain layer is the inner hexagon: it depends on nothing and is
//! depended on by every adapter (`scan-engine`, `cli`, `gui`). Adapters
//! implement the ports declared in [`ports`] to bridge the pure domain
//! with the outside world (filesystem, system trash, embedded cache, etc.).
//!
//! Lints enforced crate-wide:
//! - `unsafe_code` is forbidden (no `unsafe` allowed anywhere).
//! - `clippy::all` is denied.
//! - `missing_docs` is denied — every public item has a doc comment.

#![deny(missing_docs)]
#![deny(clippy::all)]
#![forbid(unsafe_code)]

use std::fmt;
use std::io;
use std::path::Path;

pub mod dupes;
pub mod junk;
pub mod ports;

#[cfg(feature = "sync")]
pub mod sync;

// ── PathError ─────────────────────────────────────────────────────────────

/// A non-fatal filesystem error encountered while scanning.
///
/// Scanners must surface permission-denied subtrees, broken symlinks, and
/// similar I/O failures as [`PathError`] entries inside
/// [`ScanResult::skipped`] rather than aborting the whole scan. Each entry
/// carries the path that triggered the error and the underlying
/// [`std::io::ErrorKind`] so callers can group / report them without
/// depending on the OS error string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// Path that triggered the error. May be a relative or absolute path;
    /// adapters normalize it however they see fit.
    pub path: String,
    /// The OS-level error kind, so callers can classify / filter without
    /// parsing the message.
    pub kind: io::ErrorKind,
    /// Free-form message from the underlying I/O failure.
    pub message: String,
}

impl PathError {
    /// Build a `PathError` from an [`io::Error`] and the path that caused it.
    pub fn from_io(path: impl Into<String>, err: &io::Error) -> Self {
        Self { path: path.into(), kind: err.kind(), message: err.to_string() }
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {} ({})", self.kind, self.message, self.path)
    }
}

impl std::error::Error for PathError {}

// ── FileType ──────────────────────────────────────────────────────────────

/// Coarse classification of a filesystem entry, derived from extension.
///
/// `Directory` is set by the scanner for directories regardless of name;
/// every other variant is keyed off the file extension via
/// [`FileType::from_extension`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    /// Audio files (mp3, wav, flac, ogg, aac, m4a, wma).
    Audio,
    /// Video files (mp4, mkv, avi, mov, wmv, flv, webm).
    Video,
    /// Image files (jpg, jpeg, png, gif, bmp, svg, webp, ico).
    Image,
    /// Document files (pdf, doc, docx, xls, xlsx, ppt, pptx, txt, md, csv).
    Document,
    /// Source code and config (rs, py, js, ts, go, c, cpp, h, java, rb, php,
    /// sh, toml, yaml, yml, json, xml, html, css, sql).
    Code,
    /// Archives (zip, tar, gz, tgz, bz2, xz, 7z, rar).
    Archive,
    /// A directory entry.
    Directory,
    /// Anything that didn't match a known extension family.
    Other,
}

impl FileType {
    /// Classify a file based on its extension (case-insensitive, no leading
    /// dot). Returns [`FileType::Other`] for empty or unknown inputs.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a" | "wma" => Self::Audio,
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => Self::Video,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" => Self::Image,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "csv" => {
                Self::Document
            }
            "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "java" | "rb" | "php" | "sh"
            | "toml" | "yaml" | "yml" | "json" | "xml" | "html" | "css" | "sql" => Self::Code,
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => Self::Archive,
            _ => Self::Other,
        }
    }
}

// ── DomainError ───────────────────────────────────────────────────────────

/// Errors that can occur inside the domain layer.
///
/// The domain never touches I/O directly, but adapters translate I/O
/// failures into [`DomainError::Io`] so callers see a single error type.
#[derive(Debug)]
pub enum DomainError {
    /// A path argument was empty, malformed, or otherwise unusable.
    InvalidPath(String),
    /// A `Filter` was constructed with an inconsistent configuration
    /// (e.g. `min_size > max_size`, empty pattern).
    InvalidFilter(String),
    /// Access to a path was denied by the OS or by a scanner check.
    PermissionDenied(String),
    /// An underlying I/O error from an adapter. Carries the source error
    /// so callers can downcast to `io::Error` for `ErrorKind` inspection.
    Io(io::Error),
    /// An operation that is not supported on the current platform.
    Unsupported(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Self::InvalidFilter(msg) => write!(f, "invalid filter: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "permission denied: {msg}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

impl std::error::Error for DomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for DomainError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl PartialEq for DomainError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidPath(a), Self::InvalidPath(b)) => a == b,
            (Self::InvalidFilter(a), Self::InvalidFilter(b)) => a == b,
            (Self::PermissionDenied(a), Self::PermissionDenied(b)) => a == b,
            // Compare io::Error by Display string — the inner fields of
            // io::Error are not part of its public API for PartialEq.
            (Self::Io(a), Self::Io(b)) => a.to_string() == b.to_string(),
            (Self::Unsupported(a), Self::Unsupported(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for DomainError {}

// ── FileNode ──────────────────────────────────────────────────────────────

/// A single filesystem entry and, optionally, its children.
///
/// `FileNode` is a tree node: a leaf has empty `children`; a directory has
/// its direct children in `children`. The tree is built bottom-up by the
/// scanner adapter and consumed by the GUI / CLI / formatters.
#[derive(Debug, Clone, PartialEq)]
pub struct FileNode {
    /// Absolute or scan-root-relative path string. Never empty for nodes
    /// created via [`FileNode::new`].
    pub path: String,
    /// Size of the entry in bytes. For directories, `normalize_sizes` in
    /// the scanner sets this to the recursive sum of all descendants.
    pub size: u64,
    /// Last-modified time as a Unix timestamp in seconds.
    pub modified: u64,
    /// Coarse classification of the entry.
    pub file_type: FileType,
    /// Direct children. Empty for leaf entries.
    pub children: Vec<FileNode>,
}

impl FileNode {
    /// Construct a `FileNode` from raw fields.
    ///
    /// Returns [`DomainError::InvalidPath`] when `path` is empty. The empty
    /// path is reserved for synthetic root nodes built by helpers like
    /// [`ScanResult::with_children`].
    pub fn new(
        path: impl Into<String>,
        size: u64,
        modified: u64,
        file_type: FileType,
    ) -> Result<Self, DomainError> {
        let path = path.into();
        if path.is_empty() {
            return Err(DomainError::InvalidPath("path must not be empty".into()));
        }
        Ok(Self { path, size, modified, file_type, children: Vec::new() })
    }

    /// Build a `FileNode` directly from a `Path`, using its file name as
    /// the path string and `FileType::Other` as the default classification.
    /// The caller is expected to refine `file_type` for real entries.
    pub fn from_path(path: &Path) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            size: 0,
            modified: 0,
            file_type: FileType::Other,
            children: Vec::new(),
        }
    }

    /// `true` when this node represents a directory.
    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    /// Recursive total size in bytes: the node's own size, which for
    /// directories is already the sum of all descendants (`normalize_sizes`
    /// in the scanner guarantees this).
    pub fn total_size(&self) -> u64 {
        self.size
    }

    /// Recursive count of file-system entries, including this node.
    pub fn file_count(&self) -> u64 {
        if self.children.is_empty() {
            return 1;
        }
        1 + self.children.iter().map(Self::file_count).sum::<u64>()
    }
}

// ── ScanResult ────────────────────────────────────────────────────────────

/// The output of a scan: a tree of [`FileNode`]s plus pre-aggregated metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanResult {
    /// Root of the scanned tree.
    pub root: FileNode,
    /// Total scan size in bytes: equals `root.size`, which for a
    /// directory root is the recursive sum of all descendants
    /// (guaranteed by `normalize_sizes` in the scanner).
    pub total_size: u64,
    /// Number of entries (files + directories) under `root`, including the
    /// root itself. Recomputed when built via the constructors above.
    pub file_count: u64,
    /// Wall-clock scan duration as reported by the caller, in milliseconds.
    pub scan_duration_ms: u64,
    /// Non-fatal filesystem errors encountered during the walk (permission
    /// denied subtrees, broken symlinks, etc.). The scan continues past
    /// these entries; callers can surface them in the UI / CLI.
    pub skipped: Vec<PathError>,
}

impl ScanResult {
    /// Build a `ScanResult` from a fully-populated tree. Aggregates
    /// `total_size` and `file_count` from `root`.
    pub fn from_tree(root: FileNode, scan_duration_ms: u64) -> Self {
        let total_size = root.total_size();
        let file_count = root.file_count();
        Self { root, total_size, file_count, scan_duration_ms, skipped: Vec::new() }
    }

    /// Build a `ScanResult` from a flat list of children. The resulting
    /// `root` is a synthetic `FileType::Directory` node with an empty
    /// path; the aggregated `total_size` and `file_count` are computed
    /// from the children.
    pub fn with_children(children: Vec<FileNode>, scan_duration_ms: u64) -> Self {
        let children_size = children.iter().map(|c| c.total_size()).sum::<u64>();
        let root = FileNode {
            path: String::new(),
            size: children_size,
            modified: 0,
            file_type: FileType::Directory,
            children,
        };
        let total_size = root.total_size();
        let file_count = root.file_count();
        Self { root, total_size, file_count, scan_duration_ms, skipped: Vec::new() }
    }

    /// Number of skipped paths (`self.skipped.len()`).
    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }
}

// ── Filter ────────────────────────────────────────────────────────────────

/// Predicate that accepts or rejects individual [`FileNode`]s.
///
/// `Filter` is the *per-node* part of the selection criteria. Tree-shaped
/// filters (e.g. `max_depth`) are left to the adapter that walks the tree;
/// the per-node fields here are what [`Filter::matches`] evaluates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    /// Minimum byte size, inclusive. `None` means no lower bound.
    pub min_size: Option<u64>,
    /// Maximum byte size, inclusive. `None` means no upper bound.
    pub max_size: Option<u64>,
    /// If `Some`, only entries whose `FileType` appears in this set are
    /// accepted. `None` accepts every type.
    pub file_types: Option<Vec<FileType>>,
    /// Case-insensitive substring match against the entry's path. `None`
    /// accepts every name. Must be non-empty when set — see [`Filter::validate`].
    pub name_pattern: Option<String>,
    /// Maximum age, in seconds, relative to [`Filter::now`]. Entries whose
    /// `modified` is older than `now - max_age` are rejected. `None` means
    /// no age limit.
    pub max_age: Option<u64>,
    /// Reference timestamp (Unix seconds) used when evaluating `max_age`.
    /// Defaults to `0` so tests are deterministic; the scanner adapter
    /// sets this to "now" before applying the filter.
    pub now: u64,
    /// Maximum tree depth, inclusive. `None` means no depth limit.
    /// Applied at tree-walk time by the adapter (not by `matches`).
    pub max_depth: Option<usize>,
}

impl Filter {
    /// Verify that the filter is internally consistent.
    /// Returns [`DomainError::InvalidFilter`] when `min_size > max_size`
    /// or when `name_pattern` is set to an empty string.
    pub fn validate(&self) -> Result<(), DomainError> {
        if let (Some(min), Some(max)) = (self.min_size, self.max_size) {
            if min > max {
                return Err(DomainError::InvalidFilter(format!(
                    "min_size ({min}) cannot exceed max_size ({max})"
                )));
            }
        }
        if let Some(ref pat) = self.name_pattern {
            if pat.is_empty() {
                return Err(DomainError::InvalidFilter("name_pattern must not be empty".into()));
            }
        }
        Ok(())
    }

    /// Per-node predicate. `true` means the entry passes every active
    /// filter dimension; `false` means it should be hidden.
    ///
    /// `max_depth` is intentionally NOT evaluated here — depth is a
    /// property of the walk, not of an individual node, and is enforced
    /// by the adapter that produces the tree.
    pub fn matches(&self, node: &FileNode) -> bool {
        // Size bounds (inclusive on both ends).
        if let Some(min) = self.min_size {
            if node.size < min {
                return false;
            }
        }
        if let Some(max) = self.max_size {
            if node.size > max {
                return false;
            }
        }
        // Type whitelist.
        if let Some(ref types) = self.file_types {
            if !types.contains(&node.file_type) {
                return false;
            }
        }
        // Name pattern (case-insensitive substring).
        if let Some(ref pat) = self.name_pattern {
            if !node.path.to_lowercase().contains(&pat.to_lowercase()) {
                return false;
            }
        }
        // Age: reject entries older than max_age seconds relative to `now`.
        if let Some(max_age) = self.max_age {
            if self.now >= node.modified {
                let age = self.now - node.modified;
                if age > max_age {
                    return false;
                }
            } else {
                // Entry is from the future relative to `now`; treat as fresh
                // and keep it. (Clock skew shouldn't hide data.)
            }
        }
        true
    }
}

// ── SortSpec ──────────────────────────────────────────────────────────────

/// Column that a [`SortSpec`] orders by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    /// Sort by `FileNode::path` (lexicographic).
    Name,
    /// Sort by `FileNode::size` (numeric).
    Size,
    /// Sort by `FileNode::modified` (numeric, Unix seconds).
    Modified,
    /// Sort by `FileType` (derived name).
    Type,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Smallest / earliest first.
    Ascending,
    /// Largest / latest first.
    Descending,
}

/// A complete sort order: which column, which direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSpec {
    /// Which column drives the comparison.
    pub column: SortColumn,
    /// Whether the order is ascending or descending.
    pub direction: SortDirection,
}

impl SortSpec {
    /// Sort `nodes` in place according to this spec.
    pub fn apply(&self, nodes: &mut [FileNode]) {
        let ascending = matches!(self.direction, SortDirection::Ascending);
        match self.column {
            SortColumn::Name => nodes.sort_by(|a, b| {
                let ord = a.path.cmp(&b.path);
                if ascending {
                    ord
                } else {
                    ord.reverse()
                }
            }),
            SortColumn::Size => nodes.sort_by(|a, b| {
                let ord = a.size.cmp(&b.size);
                if ascending {
                    ord
                } else {
                    ord.reverse()
                }
            }),
            SortColumn::Modified => nodes.sort_by(|a, b| {
                let ord = a.modified.cmp(&b.modified);
                if ascending {
                    ord
                } else {
                    ord.reverse()
                }
            }),
            SortColumn::Type => nodes.sort_by(|a, b| {
                let ord = format!("{:?}", a.file_type).cmp(&format!("{:?}", b.file_type));
                if ascending {
                    ord
                } else {
                    ord.reverse()
                }
            }),
        }
    }
}

// ── format_size ───────────────────────────────────────────────────────────

const SIZE_UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
const SIZE_THRESHOLD: u64 = 1024;

/// Render a byte count as a human-readable string using binary (IEC) units.
///
/// Examples:
/// - `0` → `"0 B"`
/// - `1024` → `"1.0 KiB"`
/// - `1_572_864` → `"1.5 MiB"`
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut value = bytes as f64;
    let mut unit_idx = 0usize;

    while value >= SIZE_THRESHOLD as f64 && unit_idx < SIZE_UNITS.len() - 1 {
        value /= SIZE_THRESHOLD as f64;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, SIZE_UNITS[0])
    } else {
        format!("{:.1} {}", value, SIZE_UNITS[unit_idx])
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── FileType::from_extension ────────────────────────────────────────

    #[test]
    fn should_classify_filetype_audio_when_extension_is_mp3() {
        assert_eq!(FileType::from_extension("mp3"), FileType::Audio);
    }

    #[test]
    fn should_classify_filetype_audio_when_extension_is_wav() {
        assert_eq!(FileType::from_extension("wav"), FileType::Audio);
    }

    #[test]
    fn should_classify_filetype_video_when_extension_is_mp4() {
        assert_eq!(FileType::from_extension("mp4"), FileType::Video);
    }

    #[test]
    fn should_classify_filetype_image_when_extension_is_png() {
        assert_eq!(FileType::from_extension("png"), FileType::Image);
    }

    #[test]
    fn should_classify_filetype_document_when_extension_is_pdf() {
        assert_eq!(FileType::from_extension("pdf"), FileType::Document);
    }

    #[test]
    fn should_classify_filetype_code_when_extension_is_rs() {
        assert_eq!(FileType::from_extension("rs"), FileType::Code);
    }

    #[test]
    fn should_classify_filetype_archive_when_extension_is_zip() {
        assert_eq!(FileType::from_extension("zip"), FileType::Archive);
    }

    #[test]
    fn should_classify_filetype_other_when_extension_is_unknown() {
        assert_eq!(FileType::from_extension("xyz"), FileType::Other);
    }

    #[test]
    fn should_classify_filetype_other_when_extension_is_empty() {
        assert_eq!(FileType::from_extension(""), FileType::Other);
    }

    #[test]
    fn should_classify_filetype_case_insensitively_when_extension_is_uppercase() {
        assert_eq!(FileType::from_extension("MP3"), FileType::Audio);
    }

    // ── FileNode::new ───────────────────────────────────────────────────

    #[test]
    fn should_reject_empty_path_when_filenode_new_called_with_empty_string() {
        let result = FileNode::new("", 0, 0, FileType::Other);
        assert!(matches!(result, Err(DomainError::InvalidPath(_))));
    }

    #[test]
    fn should_build_filenode_when_new_called_with_valid_path() {
        let node = FileNode::new("/x", 10, 0, FileType::Other).unwrap();
        assert_eq!(node.path, "/x");
        assert_eq!(node.size, 10);
        assert!(node.children.is_empty());
    }

    // ── ScanResult aggregation ─────────────────────────────────────────

    fn sample_tree() -> FileNode {
        // Sizes reflect `normalize_sizes`: directories store the recursive
        // sum of their descendants.
        FileNode {
            path: "/project".into(),
            size: 1500,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![
                FileNode {
                    path: "/project/src".into(),
                    size: 1000,
                    modified: 0,
                    file_type: FileType::Directory,
                    children: vec![FileNode {
                        path: "/project/src/main.rs".into(),
                        size: 1000,
                        modified: 0,
                        file_type: FileType::Code,
                        children: vec![],
                    }],
                },
                FileNode {
                    path: "/project/README.md".into(),
                    size: 500,
                    modified: 0,
                    file_type: FileType::Document,
                    children: vec![],
                },
            ],
        }
    }

    #[test]
    fn should_aggregate_parent_size_from_children_when_scanresult_with_children_built_from_child_nodes(
    ) {
        let children = vec![
            FileNode {
                path: "a".into(),
                size: 100,
                modified: 0,
                file_type: FileType::Other,
                children: vec![],
            },
            FileNode {
                path: "b".into(),
                size: 250,
                modified: 0,
                file_type: FileType::Other,
                children: vec![],
            },
        ];
        let result = ScanResult::with_children(children, 7);
        assert_eq!(result.total_size, 350);
        assert_eq!(result.scan_duration_ms, 7);
        assert!(result.root.is_dir());
    }

    #[test]
    fn should_report_zero_files_when_scanresult_empty() {
        let result = ScanResult::with_children(Vec::new(), 0);
        assert_eq!(result.file_count, 1);
        assert_eq!(result.total_size, 0);
    }

    #[test]
    fn should_aggregate_total_size_recursively_when_scanresult_from_tree_built() {
        let result = ScanResult::from_tree(sample_tree(), 42);
        assert_eq!(result.total_size, 1500);
        assert_eq!(result.file_count, 4);
    }

    // ── Filter::matches ────────────────────────────────────────────────

    fn leaf(size: u64, modified: u64, ft: FileType, path: &str) -> FileNode {
        FileNode { path: path.into(), size, modified, file_type: ft, children: vec![] }
    }

    #[test]
    fn should_keep_entry_when_filter_matches_accepts_the_filenode() {
        let filter = Filter::default();
        let node = leaf(100, 0, FileType::Other, "/x");
        assert!(filter.matches(&node));
    }

    #[test]
    fn should_drop_entry_when_filter_matches_rejects_by_min_size() {
        let filter = Filter { min_size: Some(1000), ..Filter::default() };
        let node = leaf(500, 0, FileType::Other, "/x");
        assert!(!filter.matches(&node));
    }

    #[test]
    fn should_drop_entry_when_filter_matches_rejects_by_max_age() {
        let filter = Filter { max_age: Some(100), now: 1000, ..Filter::default() };
        let node = leaf(0, 500, FileType::Other, "/x");
        assert!(!filter.matches(&node));
    }

    #[test]
    fn should_keep_entry_when_filter_matches_accepts_by_max_age() {
        let filter = Filter { max_age: Some(100), now: 1000, ..Filter::default() };
        let node = leaf(0, 950, FileType::Other, "/x");
        assert!(filter.matches(&node));
    }

    #[test]
    fn should_drop_entry_when_filter_matches_rejects_by_name_pattern() {
        let filter = Filter { name_pattern: Some("target".into()), ..Filter::default() };
        let node = leaf(0, 0, FileType::Other, "/project/src");
        assert!(!filter.matches(&node));
    }

    #[test]
    fn should_drop_entry_when_filter_matches_rejects_by_filetype_set() {
        let filter = Filter {
            file_types: Some(vec![FileType::Audio, FileType::Video]),
            ..Filter::default()
        };
        let node = leaf(0, 0, FileType::Code, "/x.rs");
        assert!(!filter.matches(&node));
    }

    // ── SortSpec::apply ───────────────────────────────────────────────

    fn three_unsorted() -> Vec<FileNode> {
        vec![
            leaf(100, 0, FileType::Other, "small.txt"),
            leaf(5000, 0, FileType::Other, "large.txt"),
            leaf(500, 0, FileType::Other, "mid.txt"),
        ]
    }

    #[test]
    fn should_sort_ascending_when_sortspec_apply_called_with_ascending() {
        let mut nodes = three_unsorted();
        let spec = SortSpec { column: SortColumn::Size, direction: SortDirection::Ascending };
        spec.apply(&mut nodes);
        assert_eq!(nodes[0].path, "small.txt");
        assert_eq!(nodes[1].path, "mid.txt");
        assert_eq!(nodes[2].path, "large.txt");
    }

    #[test]
    fn should_sort_descending_when_sortspec_apply_called_with_descending() {
        let mut nodes = three_unsorted();
        let spec = SortSpec { column: SortColumn::Size, direction: SortDirection::Descending };
        spec.apply(&mut nodes);
        assert_eq!(nodes[0].path, "large.txt");
        assert_eq!(nodes[1].path, "mid.txt");
        assert_eq!(nodes[2].path, "small.txt");
    }

    // ── format_size ────────────────────────────────────────────────────

    #[test]
    fn should_render_1_0_kib_when_format_size_called_with_1024() {
        assert_eq!(format_size(1024), "1.0 KiB");
    }

    #[test]
    fn should_render_1_5_mib_when_format_size_called_with_1_5_times_1024_times_1024() {
        let bytes = (1.5 * 1024.0 * 1024.0) as u64;
        assert_eq!(format_size(bytes), "1.5 MiB");
    }

    #[test]
    fn should_render_0_b_when_format_size_called_with_0() {
        assert_eq!(format_size(0), "0 B");
    }

    // ── DomainError ────────────────────────────────────────────────────

    #[test]
    fn should_carry_source_io_error_when_domainerror_io_returned_with_context() {
        let err = DomainError::from(io::Error::new(io::ErrorKind::NotFound, "missing"));
        // Inner io::Error must be reachable for kind/Display checks.
        match &err {
            DomainError::Io(inner) => {
                assert_eq!(inner.kind(), io::ErrorKind::NotFound);
                assert_eq!(inner.to_string(), "missing");
            }
            other => panic!("expected Io variant, got {other:?}"),
        }
        // Display impl must include both the "io error" prefix and the
        // source message — that's the "context" guarantee the port contract
        // promises callers.
        let rendered = err.to_string();
        assert!(rendered.contains("io error"), "rendered = {rendered}");
        assert!(rendered.contains("missing"), "rendered = {rendered}");
    }
}
