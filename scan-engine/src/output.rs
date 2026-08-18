use std::fmt;
use std::time::UNIX_EPOCH;

use crate::domain::{FileNode, FileTree, FilterSet, NodeKind};

/// Supported output formats for scan results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Jsonl,
    Table,
    Tree,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Jsonl => write!(f, "jsonl"),
            Self::Table => write!(f, "table"),
            Self::Tree => write!(f, "tree"),
        }
    }
}

impl OutputFormat {
    /// Format a `FileTree`, applying filters to select which nodes appear.
    ///
    /// - `Json`: indented JSON with nested children, filtered tree structure.
    /// - `Jsonl`: one JSON object per line (flat, filtered, DFS order).
    /// - `Table`: tabwriter-aligned columns: Name, Size, Modified, Type.
    /// - `Tree`: indented ASCII art (`├──` / `└──` connectors).
    pub fn format(
        &self,
        tree: &FileTree,
        filters: &FilterSet,
    ) -> Result<String, std::io::Error> {
        match self {
            Self::Json => Ok(format_json(tree, filters)),
            Self::Jsonl => Ok(format_jsonl(tree, filters)),
            Self::Table => Ok(format_table(tree, filters)),
            Self::Tree => Ok(format_tree(tree, filters)),
        }
    }
}

// ── JSON ───────────────────────────────────────────────────────────────────

fn format_json(tree: &FileTree, filters: &FilterSet) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str("{\n");
    out.push_str(&format!("  \"total_size\": {},\n", tree.total_size));
    out.push_str(&format!("  \"file_count\": {},\n", tree.file_count));
    out.push_str(&format!("  \"dir_count\": {},\n", tree.dir_count));
    out.push_str("  \"root\": ");
    json_node(&tree.root, &mut out, 2, filters, 0);
    out.push_str("\n}\n");
    out
}

fn json_node(
    node: &FileNode,
    out: &mut String,
    indent: usize,
    filters: &FilterSet,
    depth: usize,
) {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);

    out.push_str("{\n");
    out.push_str(&format!("{}\"name\": \"{}\",\n", pad1, json_escape(&node.name)));
    out.push_str(&format!(
        "{}\"path\": \"{}\",\n",
        pad1,
        json_escape(&node.path.to_string_lossy())
    ));
    out.push_str(&format!("{}\"size\": {},\n", pad1, node.size));
    out.push_str(&format!("{}\"total_size\": {},\n", pad1, node.total_size()));
    out.push_str(&format!(
        "{}\"kind\": \"{}\",\n",
        pad1,
        kind_label(&node.kind)
    ));
    out.push_str(&format!(
        "{}\"modified\": {},\n",
        pad1,
        epoch_secs(node.modified)
    ));

    out.push_str(&format!("{}\"children\": [", pad1));
    let filtered_children: Vec<&FileNode> = node
        .children
        .iter()
        .filter(|c| filters.apply(c, depth + 1))
        .collect();

    if filtered_children.is_empty() {
        out.push(']');
    } else {
        out.push('\n');
        for (i, child) in filtered_children.iter().enumerate() {
            out.push_str(&"  ".repeat(indent + 2));
            json_node(child, out, indent + 2, filters, depth + 1);
            if i + 1 < filtered_children.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(&pad1);
        out.push(']');
    }
    out.push('\n');
    out.push_str(&pad);
    out.push('}');
}

// ── JSONL ──────────────────────────────────────────────────────────────────

fn format_jsonl(tree: &FileTree, filters: &FilterSet) -> String {
    let mut out = String::with_capacity(1024);
    jsonl_node(&tree.root, &mut out, filters, 0);
    out
}

fn jsonl_node(
    node: &FileNode,
    out: &mut String,
    filters: &FilterSet,
    depth: usize,
) {
    if !filters.apply(node, depth) {
        return;
    }

    out.push('{');
    out.push_str(&format!("\"name\":\"{}\",", json_escape(&node.name)));
    out.push_str(&format!(
        "\"path\":\"{}\",",
        json_escape(&node.path.to_string_lossy())
    ));
    out.push_str(&format!("\"size\":{},", node.size));
    out.push_str(&format!("\"total_size\":{},", node.total_size()));
    out.push_str(&format!("\"kind\":\"{}\",", kind_label(&node.kind)));
    out.push_str(&format!("\"modified\":{},", epoch_secs(node.modified)));
    out.push_str(&format!("\"depth\":{}", depth));
    out.push('}');
    out.push('\n');

    for child in &node.children {
        jsonl_node(child, out, filters, depth + 1);
    }
}

// ── Table ──────────────────────────────────────────────────────────────────

fn format_table(tree: &FileTree, filters: &FilterSet) -> String {
    let mut rows: Vec<(String, u64, String, String)> = Vec::new();
    collect_table_rows(&tree.root, 0, &mut rows, filters, 0);

    // Sort by size descending.
    rows.sort_by(|a, b| b.1.cmp(&a.1));

    let header = format!(
        "{:<50} {:>12} {:>20} {:<8}",
        "Name", "Size", "Modified", "Type"
    );
    let sep = "-".repeat(header.len());

    let mut out = String::with_capacity(header.len() + rows.len() * 80);
    out.push_str(&header);
    out.push('\n');
    out.push_str(&sep);
    out.push('\n');

    for (name, size, modified, typ) in &rows {
        out.push_str(&format!(
            "{:<50} {:>12} {:>20} {:<8}",
            name,
            human_size(*size),
            modified,
            typ,
        ));
        out.push('\n');
    }
    out
}

fn collect_table_rows(
    node: &FileNode,
    indent: usize,
    out: &mut Vec<(String, u64, String, String)>,
    filters: &FilterSet,
    depth: usize,
) {
    if !filters.apply(node, depth) {
        return;
    }

    let prefix = "  ".repeat(indent);
    let name = format!("{}{}", prefix, node.name);
    let typ = kind_label(&node.kind).to_owned();
    let modified = format_timestamp(node.modified);

    out.push((name, node.total_size(), modified, typ));

    for child in &node.children {
        collect_table_rows(child, indent + 1, out, filters, depth + 1);
    }
}

// ── Tree ───────────────────────────────────────────────────────────────────

fn format_tree(tree: &FileTree, filters: &FilterSet) -> String {
    let mut out = String::with_capacity(1024);

    // Root node — no connector.
    out.push_str(&tree.root.name);
    out.push_str(&format!("  [{}]", human_size(tree.root.total_size())));
    out.push('\n');

    let filtered_children: Vec<&FileNode> = tree
        .root
        .children
        .iter()
        .filter(|c| filters.apply(c, 1))
        .collect();

    for (i, child) in filtered_children.iter().enumerate() {
        render_tree_child(child, "", i + 1 == filtered_children.len(), &mut out, filters, 1);
    }

    out
}

fn render_tree_child(
    node: &FileNode,
    prefix: &str,
    is_last: bool,
    out: &mut String,
    filters: &FilterSet,
    depth: usize,
) {
    let connector = if is_last { "└── " } else { "├── " };

    out.push_str(prefix);
    out.push_str(connector);
    out.push_str(&node.name);
    out.push_str(&format!("  [{}]", human_size(node.total_size())));
    out.push('\n');

    let child_prefix = format!(
        "{}{}",
        prefix,
        if is_last { "    " } else { "│   " }
    );

    let filtered_children: Vec<&FileNode> = node
        .children
        .iter()
        .filter(|c| filters.apply(c, depth + 1))
        .collect();

    for (i, child) in filtered_children.iter().enumerate() {
        render_tree_child(
            child,
            &child_prefix,
            i + 1 == filtered_children.len(),
            out,
            filters,
            depth + 1,
        );
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::File => "file",
        NodeKind::Directory => "dir",
        NodeKind::Symlink => "link",
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    for unit in &UNITS {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PiB", size)
}

fn epoch_secs(ts: std::time::SystemTime) -> u64 {
    ts.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn format_timestamp(ts: std::time::SystemTime) -> String {
    let secs = epoch_secs(ts);
    if secs == 0 {
        return "—".to_owned();
    }
    // Simple UTC datetime: YYYY-MM-DD HH:MM:SS
    let total_days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Days since epoch → Y/M/D via simple civil calendar.
    let (year, month, day) = civil_from_days(total_days as i64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, seconds,
    )
}

/// Convert days since Unix epoch to (year, month, day).
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(mut days: i64) -> (i64, u32, u32) {
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileNode, FileTree, Filter, FilterSet, NodeKind};
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn make_file(name: &str, size: u64) -> FileNode {
        FileNode {
            name: name.to_owned(),
            path: PathBuf::from(format!("/root/{}", name)),
            size,
            modified: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            kind: NodeKind::File,
            children: Vec::new(),
        }
    }

    fn make_dir(name: &str, children: Vec<FileNode>) -> FileNode {
        let size = children.iter().map(|c| c.total_size()).sum();
        FileNode {
            name: name.to_owned(),
            path: PathBuf::from(format!("/root/{}", name)),
            size,
            modified: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            kind: NodeKind::Directory,
            children,
        }
    }

    fn sample_tree() -> FileTree {
        let root = make_dir(
            "root",
            vec![
                make_dir(
                    "src",
                    vec![
                        make_file("main.rs", 1024),
                        make_file("lib.rs", 2048),
                    ],
                ),
                make_file("readme.md", 512),
            ],
        );
        let total_size = root.total_size();
        FileTree {
            root,
            total_size,
            file_count: 3,
            dir_count: 2,
        }
    }

    // Test 8: should output valid JSON when format is Json
    #[test]
    fn format_json_output() {
        let tree = sample_tree();
        let filters = FilterSet::new();
        let out = OutputFormat::Json.format(&tree, &filters).unwrap();
        assert!(out.contains("\"total_size\""));
        assert!(out.contains("\"root\""));
        assert!(out.contains("\"main.rs\""));
        assert!(out.contains("\"lib.rs\""));
    }

    // Test 9: should output indented tree when format is Tree
    #[test]
    fn format_tree_output() {
        let tree = sample_tree();
        let filters = FilterSet::new();
        let out = OutputFormat::Tree.format(&tree, &filters).unwrap();
        assert!(out.contains("root"));
        assert!(out.contains("├──"));
        assert!(out.contains("└──"));
        assert!(out.contains("main.rs"));
    }

    // Test 10: should apply filters to output when filter set is non-empty
    #[test]
    fn format_applies_filters() {
        let tree = sample_tree();
        let mut filters = FilterSet::new();
        filters.push(Filter::MinSize(1000));
        let out = OutputFormat::Jsonl.format(&tree, &filters).unwrap();
        assert!(out.contains("main.rs"));
        assert!(out.contains("lib.rs"));
        // readme.md is 512 bytes, below 1000 threshold.
        assert!(!out.contains("readme.md"));
    }

    #[test]
    fn format_jsonl_output() {
        let tree = sample_tree();
        let filters = FilterSet::new();
        let out = OutputFormat::Jsonl.format(&tree, &filters).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // Root + src dir + main.rs + lib.rs + readme.md = 5 lines
        assert_eq!(lines.len(), 5);
        for line in &lines {
            assert!(line.starts_with('{'));
            assert!(line.ends_with('}'));
        }
    }

    #[test]
    fn format_table_output() {
        let tree = sample_tree();
        let filters = FilterSet::new();
        let out = OutputFormat::Table.format(&tree, &filters).unwrap();
        assert!(out.contains("Name"));
        assert!(out.contains("Size"));
        assert!(out.contains("Modified"));
        assert!(out.contains("Type"));
    }

    #[test]
    fn display_format_names() {
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Jsonl.to_string(), "jsonl");
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Tree.to_string(), "tree");
    }

    #[test]
    fn tree_format_respects_filters_for_children() {
        let tree = sample_tree();
        let mut filters = FilterSet::new();
        filters.push(Filter::MinSize(1000));
        let out = OutputFormat::Tree.format(&tree, &filters).unwrap();
        // src/ children should appear (both > 1000), readme.md should not
        assert!(out.contains("main.rs"));
        assert!(!out.contains("readme.md"));
    }

    #[test]
    fn human_size_formats_correctly() {
        assert_eq!(human_size(0), "0.0 B");
        assert_eq!(human_size(512), "512.0 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(1_048_576), "1.0 MiB");
        assert_eq!(human_size(1_073_741_824), "1.0 GiB");
    }
}
