use std::path::PathBuf;
use std::time::SystemTime;

use super::FileType;

/// Type of filesystem entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
    Symlink,
}

/// A node in the file tree — represents a single filesystem entry.
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub kind: NodeKind,
    pub children: Vec<FileNode>,
}

impl FileNode {
    /// File extension without the leading dot, if present.
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|e| e.to_str())
    }

    /// Classify this node by its file extension.
    pub fn file_type(&self) -> FileType {
        self.extension()
            .map(FileType::from_extension)
            .unwrap_or(FileType::Other)
    }

    /// Recursive total size — for directories, sums all descendants.
    pub fn total_size(&self) -> u64 {
        if self.children.is_empty() {
            self.size
        } else {
            self.size + self.children.iter().map(|c| c.total_size()).sum::<u64>()
        }
    }
}
