use std::collections::HashMap;

use super::FileNode;

/// A complete file tree rooted at a single directory.
#[derive(Debug, Clone)]
pub struct FileTree {
    pub root: FileNode,
    pub total_size: u64,
    pub file_count: usize,
    pub dir_count: usize,
}

impl FileTree {
    /// Flatten the tree into a DFS-ordered list of references.
    pub fn flatten(&self) -> Vec<&FileNode> {
        let mut out = Vec::new();
        Self::flatten_dfs(&self.root, &mut out);
        out
    }

    fn flatten_dfs<'a>(node: &'a FileNode, out: &mut Vec<&'a FileNode>) {
        out.push(node);
        for child in &node.children {
            Self::flatten_dfs(child, out);
        }
    }

    /// Group all nodes by their file extension.
    pub fn by_extension(&self) -> HashMap<String, Vec<&FileNode>> {
        let mut map: HashMap<String, Vec<&FileNode>> = HashMap::new();
        for node in self.flatten() {
            let ext = node
                .extension()
                .unwrap_or("(no ext)")
                .to_ascii_lowercase();
            map.entry(ext).or_default().push(node);
        }
        map
    }

    /// Return the `n` largest nodes by `total_size`, descending.
    pub fn top_n(&self, n: usize) -> Vec<&FileNode> {
        let mut nodes = self.flatten();
        nodes.sort_by(|a, b| b.total_size().cmp(&a.total_size()));
        nodes.truncate(n);
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use crate::domain::NodeKind;


    fn node(name: &str, size: u64, children: Vec<FileNode>) -> FileNode {
        FileNode {
            name: name.to_string(),
            path: PathBuf::from(name),
            size,
            modified: SystemTime::UNIX_EPOCH,
            kind: if children.is_empty() {
                NodeKind::File
            } else {
                NodeKind::Directory
            },
            children,
        }
    }

    #[test]
    fn should_return_empty_vec_when_flatten_called_on_single_file_tree() {
        let tree = FileTree {
            root: node("only.txt", 100, vec![]),
            total_size: 100,
            file_count: 1,
            dir_count: 0,
        };
        let flat = tree.flatten();
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].name, "only.txt");
    }

    #[test]
    fn should_group_files_by_extension_when_by_extension_called_on_mixed_tree() {
        let tree = FileTree {
            root: node(
                "root",
                0,
                vec![
                    node("a.txt", 10, vec![]),
                    node("b.txt", 20, vec![]),
                    node("c.mp3", 30, vec![]),
                ],
            ),
            total_size: 60,
            file_count: 3,
            dir_count: 1,
        };
        let grouped = tree.by_extension();
        assert_eq!(grouped.get("txt").unwrap().len(), 2);
        assert_eq!(grouped.get("mp3").unwrap().len(), 1);
    }

    #[test]
    fn should_return_n_largest_files_when_top_n_called_with_n_less_than_file_count() {
        let tree = FileTree {
            root: node(
                "root",
                0,
                vec![
                    node("big.bin", 1000, vec![]),
                    node("mid.bin", 500, vec![]),
                    node("small.bin", 10, vec![]),
                ],
            ),
            total_size: 1510,
            file_count: 3,
            dir_count: 1,
        };
        let top = tree.top_n(2);
        assert_eq!(top.len(), 2);
        // root dir has total_size 1510 (sum of children), then big.bin at 1000
        assert_eq!(top[0].name, "root");
        assert_eq!(top[1].name, "big.bin");
    }
}
