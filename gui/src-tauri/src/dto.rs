//! IPC data-transfer objects (DTOs).
//!
//! The `domain` crate is deliberately zero-dependency (no `serde`), so
//! the Tauri boundary serialises through mirror DTOs defined here. Every
//! DTO maps 1:1 onto a `domain` type; conversion lives in
//! [`From`]/[`TryFrom`] impls below.
//!
//! Serde renames are lowercase because that is what the TypeScript side
//! (`gui/web/src/ipc.ts`) declares.

use serde::{Deserialize, Serialize};

use domain::ports::Scanner;
use domain::{DomainError, FileNode, FileType, Filter, PathError, ScanResult};

/// JSON value used to serialise [`std::io::ErrorKind`] (an enum with
/// platform-specific variants). The frontend only needs the debug-ish
/// name; everything it cannot parse falls back to `"Other"`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKindDto {
    /// The path does not exist.
    NotFound,
    /// Access to the path was denied.
    PermissionDenied,
    /// The operation timed out.
    TimedOut,
    /// Any other platform-specific error kind.
    Other,
}

impl From<&std::io::ErrorKind> for ErrorKindDto {
    fn from(kind: &std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::TimedOut => Self::TimedOut,
            _ => Self::Other,
        }
    }
}

/// Mirror of [`domain::PathError`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathErrorDto {
    /// Path that triggered the error.
    pub path: String,
    /// Classified OS error kind.
    pub kind: ErrorKindDto,
    /// Free-form message from the underlying I/O failure.
    pub message: String,
}

impl From<&PathError> for PathErrorDto {
    fn from(e: &PathError) -> Self {
        Self {
            path: e.path.clone(),
            kind: ErrorKindDto::from(&e.kind),
            message: e.message.clone(),
        }
    }
}

/// Mirror of [`domain::FileType`]. Ordering/values must match
/// `gui/web/src/lib/fileTypeColors.ts`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileTypeDto {
    /// Audio files (mp3, wav, flac, …).
    Audio,
    /// Video files (mp4, mkv, avi, …).
    Video,
    /// Image files (jpg, png, gif, …).
    Image,
    /// Document files (pdf, docx, txt, …).
    Document,
    /// Source code and config (rs, py, json, …).
    Code,
    /// Archives (zip, tar, gz, …).
    Archive,
    /// A directory entry.
    Directory,
    /// Anything that didn't match a known extension family.
    Other,
}

impl From<FileType> for FileTypeDto {
    fn from(t: FileType) -> Self {
        match t {
            FileType::Audio => Self::Audio,
            FileType::Video => Self::Video,
            FileType::Image => Self::Image,
            FileType::Document => Self::Document,
            FileType::Code => Self::Code,
            FileType::Archive => Self::Archive,
            FileType::Directory => Self::Directory,
            FileType::Other => Self::Other,
        }
    }
}

impl From<FileTypeDto> for FileType {
    fn from(t: FileTypeDto) -> Self {
        match t {
            FileTypeDto::Audio => FileType::Audio,
            FileTypeDto::Video => FileType::Video,
            FileTypeDto::Image => FileType::Image,
            FileTypeDto::Document => FileType::Document,
            FileTypeDto::Code => FileType::Code,
            FileTypeDto::Archive => FileType::Archive,
            FileTypeDto::Directory => FileType::Directory,
            FileTypeDto::Other => FileType::Other,
        }
    }
}

/// Mirror of [`domain::FileNode`]. The tree is serialised recursively.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNodeDto {
    /// Absolute or scan-root-relative path string.
    pub path: String,
    /// Size of the entry in bytes (own size; directories aggregate via
    /// `totalSize` on the result).
    pub size: u64,
    /// Last-modified time as a Unix timestamp in seconds.
    pub modified: u64,
    /// Coarse classification of the entry.
    #[serde(rename = "fileType")]
    pub file_type: FileTypeDto,
    /// Direct children. Empty for leaf entries.
    pub children: Vec<FileNodeDto>,
}

impl From<&FileNode> for FileNodeDto {
    fn from(n: &FileNode) -> Self {
        Self {
            path: n.path.clone(),
            size: n.size,
            modified: n.modified,
            file_type: FileTypeDto::from(n.file_type),
            children: n.children.iter().map(FileNodeDto::from).collect(),
        }
    }
}

impl FileNodeDto {
    /// Recurse back into a domain node (used by the GUI's in-process
    /// filter/sort, which reuses `scan-engine` helpers).
    pub fn to_domain(&self) -> FileNode {
        FileNode {
            path: self.path.clone(),
            size: self.size,
            modified: self.modified,
            file_type: FileType::from(self.file_type),
            children: self.children.iter().map(FileNodeDto::to_domain).collect(),
        }
    }
}

/// Mirror of [`domain::ScanResult`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResultDto {
    /// Root of the scanned tree.
    pub root: FileNodeDto,
    /// Recursive total size in bytes.
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    /// Number of entries (files + directories) under `root`.
    #[serde(rename = "fileCount")]
    pub file_count: u64,
    /// Wall-clock scan duration in milliseconds.
    #[serde(rename = "scanDurationMs")]
    pub scan_duration_ms: u64,
    /// Non-fatal filesystem errors encountered during the walk.
    pub skipped: Vec<PathErrorDto>,
}

impl From<&ScanResult> for ScanResultDto {
    fn from(r: &ScanResult) -> Self {
        Self {
            root: FileNodeDto::from(&r.root),
            total_size: r.total_size,
            file_count: r.file_count,
            scan_duration_ms: r.scan_duration_ms,
            skipped: r.skipped.iter().map(PathErrorDto::from).collect(),
        }
    }
}

impl ScanResultDto {
    /// Rebuild a domain result (for in-process filtering/sorting in the
    /// GUI backend, which reuses `scan-engine` helpers).
    pub fn to_domain(&self) -> ScanResult {
        ScanResult::from_tree(self.root.to_domain(), self.scan_duration_ms)
    }
}

/// Mirror of [`domain::Filter`]. `max_age`/`now` are set by the caller:
/// the GUI sends `max_age` and the Rust side stamps `now` (see
/// [`FilterDto::into_domain`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FilterDto {
    /// Minimum byte size, inclusive.
    #[serde(rename = "minSize")]
    pub min_size: Option<u64>,
    /// Maximum byte size, inclusive.
    #[serde(rename = "maxSize")]
    pub max_size: Option<u64>,
    /// If `Some`, only entries whose `FileType` appears in this set are accepted.
    #[serde(rename = "fileTypes")]
    pub file_types: Option<Vec<FileTypeDto>>,
    /// Case-insensitive substring match against the entry's path.
    #[serde(rename = "namePattern")]
    pub name_pattern: Option<String>,
    /// Maximum age in seconds relative to "now" (stamped by
    /// [`FilterDto::into_domain`]).
    #[serde(rename = "maxAge")]
    pub max_age: Option<u64>,
    /// Maximum tree depth, inclusive.
    #[serde(rename = "maxDepth")]
    pub max_depth: Option<usize>,
}

impl FilterDto {
    /// Convert into a domain [`Filter`], stamping `now` with the current
    /// Unix timestamp so `max_age` behaves as "entries modified more
    /// than `max_age` seconds ago are excluded".
    pub fn into_domain(self) -> Filter {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Filter {
            min_size: self.min_size,
            max_size: self.max_size,
            file_types: self
                .file_types
                .map(|v| v.into_iter().map(FileType::from).collect()),
            name_pattern: self.name_pattern,
            max_age: self.max_age,
            now,
            max_depth: self.max_depth,
        }
    }
}

/// A typed error the frontend can render. Maps every
/// [`domain::DomainError`] variant; `ScanInProgress` is GUI-specific.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum CommandErrorDto {
    /// The path argument was empty, malformed, or unusable.
    InvalidPath(String),
    /// Access to the path was denied.
    PermissionDenied(String),
    /// A `Filter` was constructed with an inconsistent configuration.
    InvalidFilter(String),
    /// An underlying I/O error.
    Io(String),
    /// A mutation was rejected while a scan is running.
    ScanInProgress(String),
}

impl From<DomainError> for CommandErrorDto {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::InvalidPath(p) => Self::InvalidPath(p),
            DomainError::PermissionDenied(p) => Self::PermissionDenied(p),
            DomainError::InvalidFilter(m) => Self::InvalidFilter(m),
            DomainError::Io(io) => Self::Io(io.to_string()),
            DomainError::Unsupported(msg) => Self::Io(msg),
        }
    }
}

impl From<CommandErrorDto> for DomainError {
    fn from(e: CommandErrorDto) -> Self {
        match e {
            CommandErrorDto::InvalidPath(p) => DomainError::InvalidPath(p),
            CommandErrorDto::PermissionDenied(p) => DomainError::PermissionDenied(p),
            CommandErrorDto::InvalidFilter(m) => DomainError::InvalidFilter(m),
            CommandErrorDto::Io(m) => DomainError::Io(std::io::Error::other(m)),
            CommandErrorDto::ScanInProgress(m) => {
                DomainError::InvalidPath(format!("scan in progress: {m}"))
            }
        }
    }
}

/// The `Scanner` port is object-safe, so `ScanService` can hand it to
/// the GUI as a `Box<dyn Scanner>` state handle without leaking the
/// concrete adapter type.
pub type DynScanner = Box<dyn Scanner + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_type_roundtrip() {
        for t in [
            FileType::Audio,
            FileType::Video,
            FileType::Image,
            FileType::Document,
            FileType::Code,
            FileType::Archive,
            FileType::Directory,
            FileType::Other,
        ] {
            assert_eq!(FileType::from(FileTypeDto::from(t)), t);
        }
    }

    #[test]
    fn file_type_dto_json_is_lowercase() {
        let dto = FileTypeDto::from(FileType::Video);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json, serde_json::json!("video"));
    }

    #[test]
    fn filter_dto_stamps_now() {
        let dto = FilterDto {
            max_age: Some(3600),
            min_size: Some(10),
            ..Default::default()
        };
        let f = dto.into_domain();
        assert_eq!(f.max_age, Some(3600));
        assert_eq!(f.min_size, Some(10));
        assert!(f.now > 0, "now must be stamped by into_domain");
    }

    #[test]
    fn command_error_roundtrip() {
        let e = CommandErrorDto::PermissionDenied("/root".into());
        let json = serde_json::to_value(&e).unwrap();
        let back: CommandErrorDto = serde_json::from_value(json).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn scan_result_dto_roundtrip() {
        let node = FileNode {
            path: "/".into(),
            size: 100,
            modified: 1,
            file_type: FileType::Directory,
            children: vec![FileNode {
                path: "/a.txt".into(),
                size: 40,
                modified: 2,
                file_type: FileType::Document,
                children: vec![],
            }],
        };
        let r = ScanResult {
            root: node,
            total_size: 100,
            file_count: 1,
            scan_duration_ms: 5,
            skipped: vec![],
        };
        let dto = ScanResultDto::from(&r);
        let json = serde_json::to_value(&dto).unwrap();
        // Lowercase file type + camelCase field rename must hold.
        assert_eq!(json["root"]["children"][0]["fileType"], serde_json::json!("document"));
        assert_eq!(json["totalSize"], serde_json::json!(100));
        // Domain round-trip preserves structure.
        let back = dto.to_domain();
        assert_eq!(back.total_size, 100);
        assert_eq!(back.root.children[0].file_type, FileType::Document);
    }
}
