use crate::{NodeType, ScanResult, TreeNode};

/// Render `ScanResult` as an indented tree (like `tree` command output).
pub fn format(result: &ScanResult) -> String {
    let mut out = String::with_capacity(1024);
    // Root node — no connector.
    let suffix = match result.root.entry.node_type {
        NodeType::Dir => "/",
        NodeType::File => "",
    };
    out.push_str(&format!(
        "{}{}  [{}]\n",
        result.root.entry.name,
        suffix,
        human_size(result.root.total_size)
    ));

    // Children of root with connectors.
    for (i, child) in result.root.children.iter().enumerate() {
        let is_last = i == result.root.children.len() - 1;
        render_child(child, "", is_last, &mut out);
    }

    out
}

fn render_child(node: &TreeNode, prefix: &str, is_last: bool, out: &mut String) {
    let connector = if is_last { "└── " } else { "├── " };
    let suffix = match node.entry.node_type {
        NodeType::Dir => "/",
        NodeType::File => "",
    };

    out.push_str(prefix);
    out.push_str(connector);
    out.push_str(&node.entry.name);
    out.push_str(suffix);
    out.push_str(&format!("  [{}]", human_size(node.total_size)));
    out.push('\n');

    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

    for (i, child) in node.children.iter().enumerate() {
        let child_is_last = i == node.children.len() - 1;
        render_child(child, &child_prefix, child_is_last, out);
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    for unit in &UNITS {
        if size < 1024.0 {
            return format!("{:.0} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PiB", size)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::tree::TreeBuilder;
    use crate::{FileEntry, NodeType};
    use std::path::PathBuf;

    // Test 12: should format ScanResult as indented tree when format=tree
    #[test]
    fn format_as_tree() {
        let result = TreeBuilder::build(vec![
            FileEntry {
                path: PathBuf::from("/root"),
                name: "root".into(),
                size: 0,
                modified: 1_700_000_000,
                node_type: NodeType::Dir,
                depth: 0,
            },
            FileEntry {
                path: PathBuf::from("/root/sub"),
                name: "sub".into(),
                size: 0,
                modified: 1_700_000_000,
                node_type: NodeType::Dir,
                depth: 1,
            },
            FileEntry {
                path: PathBuf::from("/root/sub/data.rs"),
                name: "data.rs".into(),
                size: 300,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 2,
            },
            FileEntry {
                path: PathBuf::from("/root/README.md"),
                name: "README.md".into(),
                size: 150,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 1,
            },
        ]);

        let output = super::format(&result);

        assert!(output.starts_with("root/"), "should start with root/");
        assert!(output.contains("[450 B]"), "total_size should be 450");
        assert!(output.contains("├──"), "should contain branch connector");
        assert!(output.contains("└──"), "should contain last-child connector");
        assert!(output.contains("│"), "should contain vertical connector");
        assert!(output.contains("data.rs"), "should contain nested file");
        assert!(output.contains("README.md"), "should contain sibling file");
        assert!(output.contains("[300 B]"), "data.rs size");
        assert!(output.contains("[150 B]"), "README.md size");
    }
}
