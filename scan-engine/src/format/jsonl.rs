use crate::{NodeType, ScanResult, TreeNode};

/// Render `ScanResult` as JSONL (one JSON object per line).
///
/// Each line is a flat JSON object representing one entry:
/// `{"name","path","size","total_size","modified","type","depth"}`
pub fn format(result: &ScanResult) -> String {
    let mut out = String::with_capacity(1024);
    // Summary line.
    out.push_str(&format!(
        "{{\"total_size\":{},\"entry_count\":{}}}\n",
        result.total_size, result.entry_count
    ));
    collect_lines(&result.root, 0, &mut out);
    out
}

fn collect_lines(node: &TreeNode, depth: u32, out: &mut String) {
    out.push('{');
    out.push_str(&format!("\"name\":\"{}\",", escape(&node.entry.name)));
    out.push_str(&format!(
        "\"path\":\"{}\",",
        escape(&node.entry.path.to_string_lossy())
    ));
    out.push_str(&format!("\"size\":{},", node.entry.size));
    out.push_str(&format!("\"total_size\":{},", node.total_size));
    out.push_str(&format!("\"modified\":{},", node.entry.modified));
    out.push_str(&format!(
        "\"type\":\"{}\",",
        match node.entry.node_type {
            NodeType::Dir => "dir",
            NodeType::File => "file",
        }
    ));
    out.push_str(&format!("\"depth\":{}", depth));
    out.push('}');
    out.push('\n');

    for child in &node.children {
        collect_lines(child, depth + 1, out);
    }
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
                path: PathBuf::from("/root/a.txt"),
                name: "a.txt".into(),
                size: 100,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 1,
            },
            FileEntry {
                path: PathBuf::from("/root/b.rs"),
                name: "b.rs".into(),
                size: 256,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 1,
            },
        ])
    }

    // Test 11: should format ScanResult as JSONL (one entry per line) when format=jsonl
    #[test]
    fn format_as_jsonl() {
        let result = make_result();
        let output = super::format(&result);
        let lines: Vec<&str> = output.trim().lines().collect();

        // First line = summary, then one line per node (root + 2 files = 3 entries).
        assert_eq!(lines.len(), 4, "expected 4 lines: summary + 3 entries");

        // Each line must be valid JSON.
        for (i, line) in lines.iter().enumerate() {
            let _: serde_json::Value =
                serde_json::from_str(line).expect(&format!("line {i} must be valid JSON"));
        }

        // Summary line.
        let summary: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(summary["total_size"], 356);
        assert_eq!(summary["entry_count"], 3);

        // Entry lines.
        let entry: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(entry["name"], "root");
        assert_eq!(entry["type"], "dir");
        assert_eq!(entry["depth"], 0);

        // Tree sorted by size desc: b.rs (256) before a.txt (100).
        let entry: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(entry["name"], "b.rs");
        assert_eq!(entry["depth"], 1);
    }
}
