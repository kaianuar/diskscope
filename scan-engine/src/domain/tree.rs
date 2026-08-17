use super::filenode::FileNode;

/// Wrapper around a root FileNode providing aggregate stats.
#[derive(Debug, Clone, PartialEq)]
pub struct FileTree {
    pub root: FileNode,
}

impl FileTree {
    /// Create a new FileTree from a root FileNode.
    pub fn new(root: FileNode) -> Self {
        Self { root }
    }

    /// Total size of all nodes in the tree (recursive).
    pub fn total_size(&self) -> u64 {
        self.root.total_size()
    }

    /// Total count of file (non-directory) nodes in the tree (recursive).
    pub fn file_count(&self) -> u64 {
        self.root.file_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_file(name: &str, size: u64) -> FileNode {
        FileNode::new(
            PathBuf::from(format!("/test/{}", name)),
            name.to_string(),
            size,
            1_700_000_000,
            false,
        )
        .unwrap()
    }

    fn make_dir(name: &str, children: Vec<FileNode>) -> FileNode {
        let mut node = FileNode::new(
            PathBuf::from(format!("/test/{}", name)),
            name.to_string(),
            0,
            1_700_000_000,
            true,
        )
        .unwrap();
        node.children = children;
        node
    }

    #[test]
    fn should_calculate_total_size_from_children() {
        let a = make_file("a.txt", 100);
        let b = make_file("b.txt", 200);
        let root = make_dir("root", vec![a, b]);
        let tree = FileTree::new(root);
        assert_eq!(tree.total_size(), 300);
    }

    #[test]
    fn should_count_files_recursively() {
        let a = make_file("a.txt", 10);
        let b = make_file("b.txt", 20);
        let sub = make_dir("sub", vec![make_file("c.txt", 30)]);
        let root = make_dir("root", vec![a, b, sub]);
        let tree = FileTree::new(root);
        assert_eq!(tree.file_count(), 3);
    }
}
