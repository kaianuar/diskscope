//! Domain types for junk/cache detection.
//!
//! [`JunkRule`] describes a recognisable junk directory pattern (e.g.
//! `node_modules`, `target`). [`detect_junk`] walks an in-memory
//! [`FileNode`] tree and returns a [`JunkReport`] of matching
//! directories. Only the **outermost** match is reported per subtree to
//! avoid double-counting (e.g. `node_modules/.cache` inside a matched
//! `node_modules`).

use std::path::Path;

use crate::FileNode;

/// High-level category of junk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunkCategory {
    /// Generic cache directory.
    Cache,
    /// Compiler / build output.
    BuildArtifact,
    /// Dependencies that can be re-downinstalled.
    Regenerable,
    /// Language-specific virtual environment.
    VirtualEnv,
}

/// A rule describing a type of junk directory.
#[derive(Debug, Clone)]
pub struct JunkRule {
    /// Human-readable name, e.g. `"node_modules"`.
    pub name: &'static str,
    /// The exact directory basename to match.
    pub dir_name: &'static str,
    /// Category this rule belongs to.
    pub category: JunkCategory,
    /// Human-readable description for CLI output.
    pub description: &'static str,
}

/// A single instance of detected junk.
#[derive(Debug, Clone)]
pub struct JunkItem {
    /// Full path to the junk directory.
    pub path: String,
    /// Recursive size in bytes.
    pub size: u64,
    /// Name of the rule that matched.
    pub rule_name: &'static str,
    /// Category of the matched rule.
    pub category: JunkCategory,
}

/// The final report from junk detection.
#[derive(Debug, Clone)]
pub struct JunkReport {
    /// All detected junk items.
    pub items: Vec<JunkItem>,
    /// Sum of all item sizes.
    pub total_recoverable: u64,
}

/// Walk the in-memory tree and return junk matching the given rules.
///
/// Only the **outermost** matching directory per subtree is reported.
/// Children of a matched directory are skipped to avoid double-counting
/// (e.g. `node_modules/.cache` inside a matched `node_modules`).
pub fn detect_junk(root: &FileNode, rules: &[JunkRule]) -> JunkReport {
    let mut items = Vec::new();
    let mut total = 0u64;
    detect_junk_recursive(root, rules, &mut items, &mut total);
    JunkReport { items, total_recoverable: total }
}

fn detect_junk_recursive(
    node: &FileNode,
    rules: &[JunkRule],
    items: &mut Vec<JunkItem>,
    total: &mut u64,
) {
    // Extract the basename of this node's path.
    let basename = Path::new(&node.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Check if this directory matches any rule.
    if node.is_dir() {
        if let Some(rule) = rules.iter().find(|r| r.dir_name == basename) {
            items.push(JunkItem {
                path: node.path.clone(),
                size: node.size,
                rule_name: rule.name,
                category: rule.category,
            });
            *total += node.size;
            return; // Skip children — outermost match only.
        }
    }

    // No match — recurse into child directories.
    for child in &node.children {
        if child.is_dir() {
            detect_junk_recursive(child, rules, items, total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a `FileNode` with the given path, size, and children.
    fn make_dir(path: &str, size: u64, children: Vec<FileNode>) -> FileNode {
        FileNode {
            path: path.to_string(),
            size,
            modified: 0,
            file_type: crate::FileType::Directory,
            children,
        }
    }

    #[test]
    fn should_report_outermost_match_and_skip_children() {
        // /project/node_modules/.cache (50 bytes inside 100-byte node_modules)
        let inner = make_dir("/project/node_modules/.cache", 50, vec![]);
        let nm = make_dir("/project/node_modules", 100, vec![inner]);
        let root = make_dir("/project", 150, vec![nm]);

        let rules = vec![
            JunkRule { name: "node_modules", dir_name: "node_modules", category: JunkCategory::Regenerable, description: "" },
            JunkRule { name: ".cache", dir_name: ".cache", category: JunkCategory::Cache, description: "" },
        ];

        let report = detect_junk(&root, &rules);

        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].size, 100);
        assert_eq!(report.items[0].rule_name, "node_modules");
        assert_eq!(report.total_recoverable, 100);
    }

    #[test]
    fn should_report_separate_outermost_matches_in_different_subtrees() {
        let a_nm = make_dir("/project/a/node_modules", 10, vec![]);
        let a = make_dir("/project/a", 10, vec![a_nm]);
        let b_nm = make_dir("/project/b/node_modules", 20, vec![]);
        let b = make_dir("/project/b", 20, vec![b_nm]);
        let root = make_dir("/project", 30, vec![a, b]);

        let rules = vec![
            JunkRule { name: "node_modules", dir_name: "node_modules", category: JunkCategory::Regenerable, description: "" },
        ];
        let report = detect_junk(&root, &rules);

        assert_eq!(report.items.len(), 2);
        assert_eq!(report.total_recoverable, 30);
    }

    #[test]
    fn should_return_empty_report_when_no_junk_found() {
        let src = make_dir("/project/src", 500, vec![]);
        let root = make_dir("/project", 500, vec![src]);

        let rules = vec![
            JunkRule { name: "node_modules", dir_name: "node_modules", category: JunkCategory::Regenerable, description: "" },
        ];
        let report = detect_junk(&root, &rules);

        assert!(report.items.is_empty());
        assert_eq!(report.total_recoverable, 0);
    }

    #[test]
    fn should_match_multiple_rule_types_in_same_tree() {
        let nm = make_dir("/project/node_modules", 100, vec![]);
        let target = make_dir("/project/target", 200, vec![]);
        let root = make_dir("/project", 300, vec![nm, target]);

        let rules = vec![
            JunkRule { name: "node_modules", dir_name: "node_modules", category: JunkCategory::Regenerable, description: "" },
            JunkRule { name: "target", dir_name: "target", category: JunkCategory::BuildArtifact, description: "" },
        ];
        let report = detect_junk(&root, &rules);

        assert_eq!(report.items.len(), 2);
        assert_eq!(report.total_recoverable, 300);
    }
}
