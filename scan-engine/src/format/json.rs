use crate::{NodeType, ScanResult, TreeNode};

/// Render `ScanResult` as indented JSON.
///
/// Output shape:
/// ```json
/// {
///   "total_size": 12345,
///   "entry_count": 42,
///   "root": { ... }
/// }
/// ```
pub fn format(result: &ScanResult) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str("{\n");
    out.push_str(&format!("  \"total_size\": {},\n", result.total_size));
    out.push_str(&format!("  \"entry_count\": {},\n", result.entry_count));
    out.push_str("  \"root\": ");
    format_node(&result.root, &mut out, 2);
    out.push_str("\n}\n");
    out
}

fn format_node(node: &TreeNode, out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);

    out.push_str("{\n");
    out.push_str(&format!(
        "{}\"name\": \"{}\",\n",
        pad1,
        escape(&node.entry.name)
    ));
    out.push_str(&format!("{}\"path\": \"{}\",\n", pad1, escape(&node.entry.path.to_string_lossy())));
    out.push_str(&format!("{}\"size\": {},\n", pad1, node.entry.size));
    out.push_str(&format!("{}\"total_size\": {},\n", pad1, node.total_size));
    out.push_str(&format!("{}\"modified\": {},\n", pad1, node.entry.modified));
    out.push_str(&format!(
        "{}\"type\": \"{}\",\n",
        pad1,
        match node.entry.node_type {
            NodeType::Dir => "dir",
            NodeType::File => "file",
        }
    ));

    out.push_str(&format!("{}\"children\": [", pad1));
    if node.children.is_empty() {
        out.push(']');
    } else {
        out.push('\n');
        for (i, child) in node.children.iter().enumerate() {
            out.push_str(&format!("{}  ", pad1));
            format_node(child, out, indent + 2);
            if i < node.children.len() - 1 {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(&format!("{}]", pad1));
    }
    out.push('\n');
    out.push_str(&pad);
    out.push('}');
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::tree::TreeBuilder;
    use crate::{FileEntry, NodeType};
    use std::path::PathBuf;

    fn make_result() -> crate::ScanResult {
        TreeBuilder::build(vec![
            FileEntry {
                path: PathBuf::from("/root"),
                name: "root".into(),
                size: 0,
                modified: 1_700_000_000,
                node_type: NodeType::Dir,
                depth: 0,
            },
            FileEntry {
                path: PathBuf::from("/root/hello.txt"),
                name: "hello.txt".into(),
                size: 512,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 1,
            },
        ])
    }

    // Test 10: should format ScanResult as JSON when format=json
    #[test]
    fn format_as_json() {
        let result = make_result();
        let output = super::format(&result);

        // Must be valid JSON.
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("output must be valid JSON");

        assert_eq!(parsed["total_size"], 512);
        assert_eq!(parsed["entry_count"], 2);
        assert_eq!(parsed["root"]["name"], "root");
        assert_eq!(parsed["root"]["type"], "dir");
        assert_eq!(parsed["root"]["children"][0]["name"], "hello.txt");
        assert_eq!(parsed["root"]["children"][0]["size"], 512);
    }
}
