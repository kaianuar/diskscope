use crate::{FileEntry, FilterSpec, NodeType, ScanResult, TreeNode};

pub struct Filter;

impl Filter {
    /// Recursively prune a `ScanResult` according to `spec`.
    ///
    /// Entries that match ALL specified filter criteria are kept.
    /// Directories are always preserved; only file-level leaves are filtered.
    pub fn apply(result: ScanResult, spec: &FilterSpec) -> ScanResult {
        if spec.is_empty() {
            return result;
        }

        let filtered_root = Self::filter_node(&result.root, spec);
        let total_size = filtered_root.total_size;
        let entry_count = Self::count_nodes(&filtered_root);

        ScanResult {
            root: filtered_root,
            total_size,
            entry_count,
        }
    }

    fn filter_node(node: &TreeNode, spec: &FilterSpec) -> TreeNode {
        let filtered_children: Vec<TreeNode> = node
            .children
            .iter()
            .map(|child| Self::filter_node(child, spec))
            .filter(|child| Self::passes_filter(child, spec))
            .collect();

        let children_size: u64 = filtered_children.iter().map(|c| c.total_size).sum();
        let total_size = match node.entry.node_type {
            NodeType::File => node.entry.size,
            NodeType::Dir => children_size,
        };

        TreeNode {
            entry: node.entry.clone(),
            children: filtered_children,
            total_size,
        }
    }

    fn passes_filter(node: &TreeNode, spec: &FilterSpec) -> bool {
        let e = &node.entry;

        // Directories always pass — they're structural; only filter file leaves.
        if e.node_type == NodeType::Dir {
            // Only keep a directory if it has remaining children after filtering
            // (unless it's the root, which we always keep).
            if e.depth == 0 {
                return true;
            }
            return !node.children.is_empty() || Self::node_matches_spec(e, spec);
        }

        // File: must pass all specified criteria.
        Self::node_matches_spec(e, spec)
    }

    fn node_matches_spec(e: &FileEntry, spec: &FilterSpec) -> bool {
        // Size filter.
        if let Some(min) = spec.min_size {
            if e.size < min {
                return false;
            }
        }
        if let Some(max) = spec.max_size {
            if e.size > max {
                return false;
            }
        }

        // Type filter: match file extension (without dot).
        if !spec.types.is_empty() {
            let ext = e
                .path
                .extension()
                .map(|ext| ext.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !spec.types.iter().any(|t| *t == ext) {
                return false;
            }
        }

        // Age filter: entries older than max_age_days are removed.
        if let Some(max_age_days) = spec.max_age_days {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let age_secs = now.saturating_sub(e.modified);
            let max_age_secs = max_age_days as u64 * 86_400;
            if age_secs > max_age_secs {
                return false;
            }
        }

        // Pattern filter: substring match on name.
        if let Some(ref pattern) = spec.pattern {
            if !e.name.contains(pattern.as_str()) {
                return false;
            }
        }

        true
    }

    fn count_nodes(node: &TreeNode) -> usize {
        1 + node.children.iter().map(Self::count_nodes).sum::<usize>()
    }
}

impl FilterSpec {
    fn is_empty(&self) -> bool {
        self.min_size.is_none()
            && self.max_size.is_none()
            && self.types.is_empty()
            && self.max_age_days.is_none()
            && self.pattern.is_none()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreeBuilder;
    use crate::{FileEntry, NodeType};
    use std::path::PathBuf;

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

    fn make_result() -> ScanResult {
        TreeBuilder::build(vec![
            dir("/root"),
            file("/root/a.txt", 50),
            file("/root/b.rs", 200),
            file("/root/c.txt", 10),
            dir("/root/sub"),
            file("/root/sub/d.rs", 500),
            file("/root/sub/e.txt", 30),
        ])
    }

    // Test 4: should apply size filter when min_size specified
    #[test]
    fn filter_by_min_size() {
        let result = make_result();
        let spec = FilterSpec {
            min_size: Some(100),
            ..Default::default()
        };
        let filtered = Filter::apply(result, &spec);

        // Only files >= 100: b.rs (200), d.rs (500)
        assert_eq!(filtered.total_size, 700);
        let names: Vec<String> = collect_file_names(&filtered.root);
        assert!(names.contains(&"b.rs".to_string()));
        assert!(names.contains(&"d.rs".to_string()));
        assert!(!names.contains(&"a.txt".to_string()));
    }

    // Test 5: should apply file_type filter when types list provided
    #[test]
    fn filter_by_file_type() {
        let result = make_result();
        let spec = FilterSpec {
            types: vec!["rs".to_string()],
            ..Default::default()
        };
        let filtered = Filter::apply(result, &spec);

        let names: Vec<String> = collect_file_names(&filtered.root);
        assert!(names.contains(&"b.rs".to_string()));
        assert!(names.contains(&"d.rs".to_string()));
        assert!(!names.contains(&"a.txt".to_string()));
    }

    // Test 6: should apply name pattern filter when glob pattern given
    #[test]
    fn filter_by_name_pattern() {
        let result = make_result();
        let spec = FilterSpec {
            pattern: Some("b".to_string()),
            ..Default::default()
        };
        let filtered = Filter::apply(result, &spec);

        let names: Vec<String> = collect_file_names(&filtered.root);
        assert!(names.contains(&"b.rs".to_string()));
        assert!(!names.contains(&"a.txt".to_string()));
    }

    // Test 7: should apply age filter when max_age_days specified
    #[test]
    fn filter_by_age() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let recent = now - 86_400; // 1 day ago
        let old = 946_684_800; // 2000-01-01

        let entries = vec![
            dir("/root"),
            FileEntry {
                path: PathBuf::from("/root/old.txt"),
                name: "old.txt".into(),
                size: 100,
                modified: old,
                node_type: NodeType::File,
                depth: 2,
            },
            FileEntry {
                path: PathBuf::from("/root/new.txt"),
                name: "new.txt".into(),
                size: 100,
                modified: recent,
                node_type: NodeType::File,
                depth: 1,
            },
        ];
        let result = TreeBuilder::build(entries);

        let spec = FilterSpec {
            max_age_days: Some(30),
            ..Default::default()
        };
        let filtered = Filter::apply(result, &spec);

        let names: Vec<String> = collect_file_names(&filtered.root);
        assert!(names.contains(&"new.txt".to_string()));
        assert!(!names.contains(&"old.txt".to_string()));
    }

    // Test 8: should chain multiple filters when all specified
    #[test]
    fn chain_multiple_filters() {
        let result = make_result();
        let spec = FilterSpec {
            min_size: Some(20),
            types: vec!["txt".to_string()],
            ..Default::default()
        };
        let filtered = Filter::apply(result, &spec);

        let names: Vec<String> = collect_file_names(&filtered.root);
        // a.txt (50, txt, >=20) → kept
        // c.txt (10, txt, <20) → filtered
        // e.txt (30, txt, >=20) → kept
        // b.rs (200, rs, not txt) → filtered
        // d.rs (500, rs, not txt) → filtered
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"e.txt".to_string()));
        assert!(!names.contains(&"c.txt".to_string()));
        assert!(!names.contains(&"b.rs".to_string()));
    }

    fn collect_file_names(node: &TreeNode) -> Vec<String> {
        let mut names = Vec::new();
        for child in &node.children {
            if child.entry.node_type == NodeType::File {
                names.push(child.entry.name.clone());
            }
            names.extend(collect_file_names(child));
        }
        names
    }
}
