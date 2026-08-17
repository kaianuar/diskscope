use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{FileEntry, NodeType, ScanResult, TreeNode};

pub struct TreeBuilder;

impl TreeBuilder {
    /// Build a `ScanResult` from a flat list of `FileEntry` values.
    ///
    /// The entry with the shortest path becomes the root node.
    /// All other entries are nested under their parent directories.
    /// Children at every level are sorted by `total_size` descending.
    pub fn build(entries: Vec<FileEntry>) -> ScanResult {
        if entries.is_empty() {
            return ScanResult {
                root: TreeNode {
                    entry: FileEntry {
                        path: PathBuf::new(),
                        name: String::new(),
                        size: 0,
                        modified: 0,
                        node_type: NodeType::Dir,
                        depth: 0,
                    },
                    children: Vec::new(),
                    total_size: 0,
                },
                total_size: 0,
                entry_count: 0,
            };
        }

        let by_path: HashMap<PathBuf, FileEntry> =
            entries.iter().map(|e| (e.path.clone(), e.clone())).collect();

        // Root = entry with the shortest path (the scan root).
        let root_path = entries
            .iter()
            .min_by_key(|e| e.path.as_os_str().len())
            .unwrap()
            .path
            .clone();

        let root = Self::build_node(&root_path, &by_path);

        ScanResult {
            total_size: root.total_size,
            entry_count: Self::count_nodes(&root),
            root,
        }
    }

    fn build_node(dir: &Path, by_path: &HashMap<PathBuf, FileEntry>) -> TreeNode {
        let entry = by_path.get(dir).cloned().unwrap_or_else(|| FileEntry {
            path: dir.to_path_buf(),
            name: dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.to_string_lossy().into_owned()),
            size: 0,
            modified: 0,
            node_type: NodeType::Dir,
            depth: 0,
        });

        let mut children: Vec<TreeNode> = Vec::new();

        for (path, e) in by_path {
            if path.as_path().parent() == Some(dir) && path.as_path() != dir {
                if e.node_type == NodeType::Dir {
                    children.push(Self::build_node(path, by_path));
                } else {
                    children.push(TreeNode {
                        total_size: e.size,
                        entry: e.clone(),
                        children: Vec::new(),
                    });
                }
            }
        }

        // Sort: largest total_size first.
        children.sort_by(|a, b| b.total_size.cmp(&a.total_size));

        let total_size = children.iter().map(|c| c.total_size).sum::<u64>();

        TreeNode {
            entry,
            children,
            total_size,
        }
    }

    fn count_nodes(node: &TreeNode) -> usize {
        1 + node.children.iter().map(Self::count_nodes).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeType;

    fn file(path: &str, size: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            name: PathBuf::from(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            size,
            modified: 1_700_000_000,
            node_type: NodeType::File,
            depth: path.matches('/').count() as u32,
        }
    }

    fn dir(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            name: PathBuf::from(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            size: 0,
            modified: 1_700_000_000,
            node_type: NodeType::Dir,
            depth: path.matches('/').count() as u32,
        }
    }

    // Test 2: should build tree with correct parent-child nesting
    #[test]
    fn build_tree_parent_child_nesting() {
        let entries = vec![
            dir("/root"),
            dir("/root/sub"),
            file("/root/a.txt", 100),
            file("/root/sub/b.txt", 200),
        ];
        let result = TreeBuilder::build(entries);

        assert_eq!(result.root.entry.path, PathBuf::from("/root"));
        assert_eq!(result.root.children.len(), 2); // sub/ and a.txt

        let sub = result
            .root
            .children
            .iter()
            .find(|c| c.entry.node_type == NodeType::Dir)
            .expect("sub dir");
        assert_eq!(sub.children.len(), 1);
        assert_eq!(sub.children[0].entry.name, "b.txt");
    }

    // Test 3: should calculate total_size and entry_count with mixed types
    #[test]
    fn build_tree_total_size_and_count() {
        let entries = vec![
            dir("/root"),
            dir("/root/sub"),
            file("/root/a.txt", 50),
            file("/root/sub/b.txt", 30),
            file("/root/sub/c.txt", 70),
        ];
        let result = TreeBuilder::build(entries);

        assert_eq!(result.total_size, 150);
        assert_eq!(result.entry_count, 5);
    }

    // Test 13: should sort by size descending
    #[test]
    fn build_tree_sorts_by_size_desc() {
        let entries = vec![
            dir("/root"),
            file("/root/small.txt", 10),
            file("/root/big.txt", 500),
            file("/root/medium.txt", 100),
        ];
        let result = TreeBuilder::build(entries);

        assert_eq!(result.root.children[0].entry.name, "big.txt");
        assert_eq!(result.root.children[1].entry.name, "medium.txt");
        assert_eq!(result.root.children[2].entry.name, "small.txt");
    }

    // Test 14: structure correct for name-sort verification
    #[test]
    fn build_tree_structure_correct_for_name_sort_test() {
        let entries = vec![
            dir("/root"),
            file("/root/zebra.txt", 10),
            file("/root/alpha.txt", 10),
            file("/root/middle.txt", 10),
        ];
        let result = TreeBuilder::build(entries);

        assert_eq!(result.root.children.len(), 3);
        assert_eq!(result.total_size, 30);
    }
}
