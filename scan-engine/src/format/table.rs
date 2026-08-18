use crate::{NodeType, ScanResult, SortDir, SortKey, TreeNode};

/// Render `ScanResult` as an aligned ASCII table.
pub fn format(
    result: &ScanResult,
    sort_key: Option<SortKey>,
    sort_dir: Option<SortDir>,
) -> String {
    let mut rows: Vec<(String, String, u64, String, String)> = Vec::new();
    collect_rows(&result.root, 0, &mut rows);

    let dir = sort_dir.unwrap_or(SortDir::Desc);
    let key = sort_key.unwrap_or(SortKey::Size);
    match key {
        SortKey::Size => match dir {
            SortDir::Asc => rows.sort_by_key(|r| r.2),
            SortDir::Desc => rows.sort_by(|a, b| b.2.cmp(&a.2)),
        },
        SortKey::Name => match dir {
            SortDir::Asc => rows.sort_by(|a, b| a.1.cmp(&b.1)),
            SortDir::Desc => rows.sort_by(|a, b| b.1.cmp(&a.1)),
        },
        SortKey::Modified => match dir {
            SortDir::Asc => rows.sort_by(|a, b| a.3.cmp(&b.3)),
            SortDir::Desc => rows.sort_by(|a, b| b.3.cmp(&a.3)),
        },
    }

    let header = format!("{:<40} {:>12} {:>20} {:<6}", "Name", "Size", "Modified", "Type");
    let sep = "-".repeat(header.len());

    let mut out = String::with_capacity(header.len() + rows.len() * 80);
    out.push_str(&header);
    out.push('\n');
    out.push_str(&sep);
    out.push('\n');

    for (display_name, _raw_name, size, modified, typ) in &rows {
        out.push_str(&format!(
            "{:<40} {:>12} {:>20} {:<6}\n",
            display_name, size, modified, typ
        ));
    }

    out
}

fn collect_rows(
    node: &TreeNode,
    indent: usize,
    out: &mut Vec<(String, String, u64, String, String)>,
) {
    let prefix = "  ".repeat(indent);
    let display_name = format!("{}{}", prefix, node.entry.name);
    let raw_name = node.entry.name.clone();
    let typ = match node.entry.node_type {
        NodeType::Dir => "dir".to_string(),
        NodeType::Symlink => "symlink".to_string(),
        NodeType::File => node
            .entry
            .path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let modified = format_timestamp(node.entry.modified);

    out.push((display_name, raw_name, node.total_size, modified, typ));

    for child in &node.children {
        collect_rows(child, indent + 1, out);
    }
}

fn format_timestamp(ts: u64) -> String {
    let secs = ts % 60;
    let mins = (ts / 60) % 60;
    let hours = (ts / 3600) % 24;
    let days = ts / 86_400;
    format!("day {days} {hours:02}:{mins:02}:{secs:02}")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreeBuilder;
    use crate::{FileEntry, NodeType};
    use std::path::PathBuf;

    fn make_result() -> ScanResult {
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
                path: PathBuf::from("/root/README.md"),
                name: "README.md".into(),
                size: 500,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 1,
            },
            FileEntry {
                path: PathBuf::from("/root/src"),
                name: "src".into(),
                size: 0,
                modified: 1_700_000_000,
                node_type: NodeType::Dir,
                depth: 1,
            },
            FileEntry {
                path: PathBuf::from("/root/src/main.rs"),
                name: "main.rs".into(),
                size: 1024,
                modified: 1_700_000_000,
                node_type: NodeType::File,
                depth: 2,
            },
        ])
    }

    // Test 9: should format ScanResult as aligned table when format=table
    #[test]
    fn format_as_table() {
        let result = make_result();
        let output = super::format(&result, None, None);

        assert!(output.contains("Name"), "header missing Name");
        assert!(output.contains("Size"), "header missing Size");
        assert!(output.contains("Modified"), "header missing Modified");
        assert!(output.contains("Type"), "header missing Type");
        assert!(output.contains("README.md"), "missing README.md row");
        assert!(output.contains("main.rs"), "missing main.rs row");
        assert!(output.contains("dir"), "missing dir type");
    }

    // Test 13: sort by size desc
    #[test]
    fn table_sort_by_size_desc() {
        let result = make_result();
        let output = super::format(&result, Some(SortKey::Size), Some(SortDir::Desc));
        let lines: Vec<&str> = output.lines().collect();

        assert!(lines.len() >= 3);
        let main_pos = lines.iter().position(|l| l.contains("main.rs")).unwrap();
        let readme_pos = lines.iter().position(|l| l.contains("README.md")).unwrap();
        assert!(main_pos < readme_pos, "main.rs (1024) should come before README.md (500)");
    }

    // Test 14: sort by name ascending
    #[test]
    fn table_sort_by_name_asc() {
        let result = make_result();
        let output = super::format(&result, Some(SortKey::Name), Some(SortDir::Asc));
        let lines: Vec<&str> = output.lines().collect();

        let readme_pos = lines.iter().position(|l| l.contains("README.md")).unwrap();
        let main_pos = lines.iter().position(|l| l.contains("main.rs")).unwrap();
        assert!(
            readme_pos < main_pos,
            "README.md should come before main.rs alphabetically"
        );
    }
}
