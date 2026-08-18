use std::time::SystemTime;

use super::{FileNode, FileType};

/// A single filter criterion applied to a `FileNode`.
#[derive(Debug, Clone)]
pub enum Filter {
    MinSize(u64),
    MaxSize(u64),
    FileType(FileType),
    Extension(String),
    ModifiedBefore(SystemTime),
    ModifiedAfter(SystemTime),
    NamePattern(String),
    MaxDepth(usize),
}

impl Filter {
    /// Returns `true` if the node matches this filter.
    pub fn apply(&self, node: &FileNode, depth: usize) -> bool {
        match self {
            Self::MinSize(min) => node.total_size() >= *min,
            Self::MaxSize(max) => node.total_size() <= *max,
            Self::FileType(ft) => node.file_type() == *ft,
            Self::Extension(ext) => {
                node.extension()
                    .map(|e| e.eq_ignore_ascii_case(ext))
                    .unwrap_or(false)
            }
            Self::ModifiedBefore(t) => node.modified < *t,
            Self::ModifiedAfter(t) => node.modified > *t,
            Self::NamePattern(pat) => node.name.contains(pat.as_str()),
            Self::MaxDepth(max) => depth <= *max,
        }
    }
}

/// A set of filters that all must match (AND combinator).
#[derive(Debug, Clone, Default)]
pub struct FilterSet(pub Vec<Filter>);

impl FilterSet {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, filter: Filter) {
        self.0.push(filter);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns `true` if the node passes every filter in the set.
    pub fn apply(&self, node: &FileNode, depth: usize) -> bool {
        self.0.iter().all(|f| f.apply(node, depth))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use crate::domain::NodeKind;

    fn file_node(name: &str, size: u64) -> FileNode {
        FileNode {
            name: name.to_string(),
            path: PathBuf::from(format!("/root/{name}")),
            size,
            modified: SystemTime::now(),
            kind: NodeKind::File,
            children: Vec::new(),
        }
    }

    #[test]
    fn should_filter_by_min_size_when_node_size_below_threshold() {
        let small = file_node("tiny.txt", 100);
        let large = file_node("big.bin", 1_000_000);
        let f = Filter::MinSize(500);

        assert!(!f.apply(&small, 0));
        assert!(f.apply(&large, 0));
    }

    #[test]
    fn should_filter_by_file_type_when_node_is_audio_file() {
        let audio = file_node("song.mp3", 5_000);
        let text = file_node("readme.txt", 500);
        let f = Filter::FileType(FileType::Audio);

        assert!(f.apply(&audio, 0));
        assert!(!f.apply(&text, 0));
    }

    #[test]
    fn should_filter_by_date_range_when_node_modified_outside_range() {
        let old = FileNode {
            name: "old.log".to_string(),
            path: PathBuf::from("/root/old.log"),
            size: 100,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
            kind: NodeKind::File,
            children: Vec::new(),
        };
        let cutoff = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000);

        let before = Filter::ModifiedBefore(cutoff);
        let after = Filter::ModifiedAfter(cutoff);

        assert!(before.apply(&old, 0));
        assert!(!after.apply(&old, 0));
    }

    #[test]
    fn should_filter_by_depth_when_node_exceeds_max_depth() {
        let node = file_node("deep.txt", 100);
        let f = Filter::MaxDepth(3);

        assert!(f.apply(&node, 2));
        assert!(f.apply(&node, 3));
        assert!(!f.apply(&node, 4));
    }

    #[test]
    fn should_combine_filters_when_filter_set_contains_multiple_criteria() {
        let node = file_node("archive.tar.gz", 5_000_000);
        let mut set = FilterSet::new();
        set.push(Filter::MinSize(1_000_000));
        set.push(Filter::FileType(FileType::Archive));

        assert!(set.apply(&node, 0));

        let small_text = file_node("notes.txt", 50);
        assert!(!set.apply(&small_text, 0));
    }
}
