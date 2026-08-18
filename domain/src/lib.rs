use std::fmt;

// ── FileType ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Audio,
    Video,
    Image,
    Document,
    Code,
    Archive,
    Directory,
    Other,
}

impl FileType {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidPath(String),
    InvalidFilter(String),
    PermissionDenied(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Self::InvalidFilter(msg) => write!(f, "invalid filter: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "permission denied: {msg}"),
        }
    }
}

impl std::error::Error for DomainError {}

// ── FileNode ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct FileNode {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub file_type: FileType,
    pub children: Vec<FileNode>,
}

impl FileNode {
    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    pub fn total_size(&self) -> u64 {
        if self.children.is_empty() {
            return self.size;
        }
        self.size + self.children.iter().map(|c| c.total_size()).sum::<u64>()
    }

    pub fn file_count(&self) -> u64 {
        if self.children.is_empty() {
            return 1;
        }
        1 + self.children.iter().map(|c| c.file_count()).sum::<u64>()
    }

    pub fn apply_filter(&self, filter: &Filter) -> Option<FileNode> {
        let dominated = self.apply_filter_inner(filter, 0);
        dominated
    }

    fn apply_filter_inner(&self, filter: &Filter, depth: usize) -> Option<FileNode> {
        if let Some(max) = filter.max_depth {
            if depth > max {
                return None;
            }
        }

        let mut filtered_children = Vec::new();
        for child in &self.children {
            if let Some(node) = child.apply_filter_inner(filter, depth + 1) {
                filtered_children.push(node);
            }
        }

        let passes_size = filter.matches_size(self.size);
        let passes_type = filter.matches_type(&self.file_type);
        let passes_name = filter.matches_name(&self.path);

        if passes_size && passes_type && passes_name {
            Some(FileNode {
                path: self.path.clone(),
                size: self.size,
                modified: self.modified,
                file_type: self.file_type,
                children: filtered_children,
            })
        } else if !filtered_children.is_empty() {
            Some(FileNode {
                path: self.path.clone(),
                size: self.size,
                modified: self.modified,
                file_type: self.file_type,
                children: filtered_children,
            })
        } else {
            None
        }
    }
}

// ── ScanResult ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ScanResult {
    pub root: FileNode,
    pub total_size: u64,
    pub file_count: u64,
    pub scan_duration_ms: u64,
}

impl ScanResult {
    pub fn from_tree(root: FileNode, scan_duration_ms: u64) -> Self {
        let total_size = root.total_size();
        let file_count = root.file_count();
        Self {
            root,
            total_size,
            file_count,
            scan_duration_ms,
        }
    }
}

// ── Filter ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub file_types: Option<Vec<FileType>>,
    pub name_pattern: Option<String>,
    pub max_depth: Option<usize>,
}

impl Filter {
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
                return Err(DomainError::InvalidFilter(
                    "name_pattern must not be empty".into(),
                ));
            }
        }

        Ok(())
    }

    fn matches_size(&self, size: u64) -> bool {
        if let Some(min) = self.min_size {
            if size < min {
                return false;
            }
        }
        if let Some(max) = self.max_size {
            if size > max {
                return false;
            }
        }
        true
    }

    fn matches_type(&self, ft: &FileType) -> bool {
        match &self.file_types {
            Some(types) => types.contains(ft),
            None => true,
        }
    }

    fn matches_name(&self, path: &str) -> bool {
        match &self.name_pattern {
            Some(pat) => {
                let pat_lower = pat.to_lowercase();
                let path_lower = path.to_lowercase();
                path_lower.contains(&pat_lower)
            }
            None => true,
        }
    }
}

// ── SortSpec ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Size,
    Modified,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSpec {
    pub column: SortColumn,
    pub direction: SortDirection,
}

impl SortSpec {
    pub fn sort_nodes(&self, nodes: &mut [FileNode]) {
        match self.column {
            SortColumn::Name => nodes.sort_by(|a, b| {
                let ord = a.path.cmp(&b.path);
                if self.direction == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            }),
            SortColumn::Size => nodes.sort_by(|a, b| {
                let ord = a.size.cmp(&b.size);
                if self.direction == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            }),
            SortColumn::Modified => nodes.sort_by(|a, b| {
                let ord = a.modified.cmp(&b.modified);
                if self.direction == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            }),
            SortColumn::Type => nodes.sort_by(|a, b| {
                let ord = format!("{:?}", a.file_type).cmp(&format!("{:?}", b.file_type));
                if self.direction == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            }),
        }
    }
}

// ── format_size ───────────────────────────────────────────────────────────

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut value = bytes as f64;
    let mut unit_idx = 0;

    while value >= THRESHOLD as f64 && unit_idx < UNITS.len() - 1 {
        value /= THRESHOLD as f64;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit_idx])
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_classify_mp3_as_audio_when_from_extension_called() {
        assert_eq!(FileType::from_extension("mp3"), FileType::Audio);
    }

    #[test]
    fn should_classify_rs_as_code_when_from_extension_called() {
        assert_eq!(FileType::from_extension("rs"), FileType::Code);
    }

    #[test]
    fn should_classify_unknown_extension_as_other_when_from_extension_called() {
        assert_eq!(FileType::from_extension("xyz"), FileType::Other);
    }

    #[test]
    fn should_format_1024_as_1_0_kb_when_format_size_called() {
        assert_eq!(format_size(1024), "1.0 KB");
    }

    #[test]
    fn should_format_0_as_0_b_when_format_size_called() {
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn should_format_1073741824_as_1_0_gb_when_format_size_called() {
        assert_eq!(format_size(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn should_reject_negative_size_range_when_filter_validated() {
        let filter = Filter {
            min_size: Some(1000),
            max_size: Some(500),
            ..Default::default()
        };
        let result = filter.validate();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            DomainError::InvalidFilter("min_size (1000) cannot exceed max_size (500)".into())
        );
    }

    #[test]
    fn should_reject_empty_name_pattern_when_filter_validated() {
        let filter = Filter {
            name_pattern: Some(String::new()),
            ..Default::default()
        };
        let result = filter.validate();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            DomainError::InvalidFilter("name_pattern must not be empty".into())
        );
    }

    #[test]
    fn should_sort_descending_by_size_when_sortspec_direction_is_desc() {
        let mut nodes = vec![
            FileNode {
                path: "small.txt".into(),
                size: 100,
                modified: 0,
                file_type: FileType::Other,
                children: vec![],
            },
            FileNode {
                path: "large.txt".into(),
                size: 5000,
                modified: 0,
                file_type: FileType::Other,
                children: vec![],
            },
            FileNode {
                path: "mid.txt".into(),
                size: 500,
                modified: 0,
                file_type: FileType::Other,
                children: vec![],
            },
        ];

        let spec = SortSpec {
            column: SortColumn::Size,
            direction: SortDirection::Desc,
        };
        spec.sort_nodes(&mut nodes);

        assert_eq!(nodes[0].path, "large.txt");
        assert_eq!(nodes[1].path, "mid.txt");
        assert_eq!(nodes[2].path, "small.txt");
    }

    #[test]
    fn should_compute_total_size_recursively_when_scan_result_built_from_tree() {
        let root = FileNode {
            path: "/project".into(),
            size: 0,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![
                FileNode {
                    path: "/project/src".into(),
                    size: 0,
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
        };

        let result = ScanResult::from_tree(root, 42);
        assert_eq!(result.total_size, 1500);
    }

    #[test]
    fn should_respect_depth_limit_when_filter_applied_to_fileno_node_tree() {
        let root = FileNode {
            path: "/project".into(),
            size: 0,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![FileNode {
                path: "/project/src".into(),
                size: 0,
                modified: 0,
                file_type: FileType::Directory,
                children: vec![FileNode {
                    path: "/project/src/deep".into(),
                    size: 0,
                    modified: 0,
                    file_type: FileType::Directory,
                    children: vec![FileNode {
                        path: "/project/src/deep/file.rs".into(),
                        size: 100,
                        modified: 0,
                        file_type: FileType::Code,
                        children: vec![],
                    }],
                }],
            }],
        };

        let filter = Filter {
            max_depth: Some(1),
            ..Default::default()
        };

        let filtered = root.apply_filter(&filter).unwrap();
        // depth 0 = root, depth 1 = src, depth 2 = deep, depth 3 = file.rs
        // max_depth=1 keeps root (0) and src (1), prunes deep (2) and file.rs (3)
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.children[0].path, "/project/src");
        assert!(filtered.children[0].children.is_empty());
    }
}
