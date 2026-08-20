//! Output formats for [`ScanResult`].
//!
//! Four renderers are exposed via [`OutputFormat`]:
//!
//! - [`OutputFormat::Table`] — aligned columns: `SIZE`, `MTIME`,
//!   `TYPE`, `PATH`.
//! - [`OutputFormat::Json`] — a single JSON array of
//!   `{path, size, modified, file_type, children}` objects (one array
//!   element per node, recursively embedded).
//! - [`OutputFormat::Jsonl`] — one JSON object per line, depth-first.
//! - [`OutputFormat::Tree`] — `tree`-style indented output, with
//!   branch glyphs and human-readable sizes.
//!
//! All writers go through a `&mut dyn Write` so callers can route to
//! stdout, a file, or a buffer.

use std::fmt::Write as _;
use std::io::{self, Write};

use domain::{format_size, FileNode, FileType, ScanResult};

/// Output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Aligned columns: `SIZE`, `MTIME`, `TYPE`, `PATH`.
    Table,
    /// Single JSON object (root) with embedded children.
    Json,
    /// One JSON object per line, depth-first.
    Jsonl,
    /// Indented tree-style output.
    Tree,
}

impl OutputFormat {
    /// Parse a CLI flag value (`"table"`, `"json"`, `"jsonl"`, `"tree"`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "table" => Some(Self::Table),
            "json" => Some(Self::Json),
            "jsonl" => Some(Self::Jsonl),
            "tree" => Some(Self::Tree),
            _ => None,
        }
    }
}

/// Render `result` in the chosen format, writing to `out`.
pub fn render(result: &ScanResult, fmt: OutputFormat, out: &mut dyn Write) -> io::Result<()> {
    match fmt {
        OutputFormat::Table => render_table(result, out),
        OutputFormat::Json => render_json(result, out),
        OutputFormat::Jsonl => render_jsonl(result, out),
        OutputFormat::Tree => render_tree(result, out),
    }
}

// ── Table ─────────────────────────────────────────────────────────────────

fn render_table(result: &ScanResult, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "{:>12}  {:>19}  {:<10}  PATH", "SIZE", "MTIME", "TYPE")?;
    let mut widths: TableWidths = TableWidths::default();
    let nodes = walk(result);
    for n in &nodes {
        measure(n, &mut widths);
    }
    for n in &nodes {
        write_row(n, &widths, out)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct TableWidths {
    size: usize,
    mtime: usize,
    file_type: usize,
    path: usize,
}

fn measure(node: &FileNode, w: &mut TableWidths) {
    let size_str = format_size(node.size);
    let mtime_str = node.modified.to_string();
    let type_str = format!("{:?}", node.file_type);
    w.size = w.size.max(size_str.len());
    w.mtime = w.mtime.max(mtime_str.len());
    w.file_type = w.file_type.max(type_str.len());
    w.path = w.path.max(node.path.len());
}

fn write_row(node: &FileNode, w: &TableWidths, out: &mut dyn Write) -> io::Result<()> {
    let size_str = format_size(node.size);
    let mtime_str = node.modified.to_string();
    let type_str = format!("{:?}", node.file_type);
    writeln!(
        out,
        "{:>wsize$}  {:>wmtime$}  {:<wtype$}  {}",
        size_str,
        mtime_str,
        type_str,
        node.path,
        wsize = w.size,
        wmtime = w.mtime,
        wtype = w.file_type,
    )
    .map(|_| ())
}

#[derive(Debug, serde::Serialize)]
struct JsonNode<'a> {
    path: &'a str,
    size: u64,
    modified: u64,
    file_type: &'a str,
    children: Vec<JsonNode<'a>>,
}

fn to_json(node: &FileNode) -> JsonNode<'_> {
    JsonNode {
        path: &node.path,
        size: node.size,
        modified: node.modified,
        file_type: file_type_name(node.file_type),
        children: node.children.iter().map(to_json).collect(),
    }
}

fn render_json(result: &ScanResult, out: &mut dyn Write) -> io::Result<()> {
    let json = to_json(&result.root);
    serde_json::to_writer_pretty(&mut *out, &json)
        .map_err(|e| io::Error::other( e.to_string()))?;
    writeln!(out)?;
    Ok(())
}

// ── JSONL (one object per line) ────────────────────────────────────────────

fn render_jsonl(result: &ScanResult, out: &mut dyn Write) -> io::Result<()> {
    for node in walk(result) {
        let json = to_json(node);
        let bytes = serde_json::to_vec(&json)
            .map_err(|e| io::Error::other( e.to_string()))?;
        out.write_all(&bytes)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

// ── Tree ──────────────────────────────────────────────────────────────────

fn render_tree(result: &ScanResult, out: &mut dyn Write) -> io::Result<()> {
    let mut s = String::new();
    write_tree_node(&result.root, "", true, &mut s);
    out.write_all(s.as_bytes())?;
    Ok(())
}

fn write_tree_node(node: &FileNode, prefix: &str, is_last: bool, out: &mut String) {
    let branch = if prefix.is_empty() {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };
    let size_str = format_size(node.size);
    let type_str = file_type_name(node.file_type);
    let _ = writeln!(
        out,
        "{prefix}{branch}{name} [{size_str}, {type_str}]",
        name = node.path,
    );
    let child_prefix = if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };
    let last_idx = node.children.len().saturating_sub(1);
    for (i, child) in node.children.iter().enumerate() {
        write_tree_node(child, &child_prefix, i == last_idx, out);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn file_type_name(ft: FileType) -> &'static str {
    match ft {
        FileType::Audio => "audio",
        FileType::Video => "video",
        FileType::Image => "image",
        FileType::Document => "document",
        FileType::Code => "code",
        FileType::Archive => "archive",
        FileType::Directory => "dir",
        FileType::Other => "other",
    }
}

/// DFS iterator over all nodes in a `ScanResult`, including the root.
fn walk(result: &ScanResult) -> Vec<&FileNode> {
    let mut out = Vec::new();
    walk_node(&result.root, &mut out);
    out
}

fn walk_node<'a>(node: &'a FileNode, out: &mut Vec<&'a FileNode>) {
    out.push(node);
    for child in &node.children {
        walk_node(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ────────────────────────────────────────────────────

    /// Leaf node with the given path, size, and file type.
    fn leaf(path: &str, size: u64, file_type: FileType) -> FileNode {
        FileNode {
            path: path.to_string(),
            size,
            modified: 0,
            file_type,
            children: Vec::new(),
        }
    }

    /// Directory node with the given path and children.
    fn dir(path: &str, children: Vec<FileNode>) -> FileNode {
        FileNode {
            path: path.to_string(),
            size: children.iter().map(|c| c.total_size()).sum(),
            modified: 0,
            file_type: FileType::Directory,
            children,
        }
    }

    /// Synthetic root with no children (empty path reserved for roots).
    fn empty_root() -> FileNode {
        FileNode {
            path: String::new(),
            size: 0,
            modified: 0,
            file_type: FileType::Directory,
            children: Vec::new(),
        }
    }

    fn render_to_string(result: &ScanResult, fmt: OutputFormat) -> String {
        let mut out = Vec::new();
        render(result, fmt, &mut out).expect("render should succeed");
        String::from_utf8(out).expect("render output should be UTF-8")
    }

    // ── Table ───────────────────────────────────────────────────────────

    #[test]
    fn should_write_header_when_result_is_empty() {
        let result = ScanResult::from_tree(empty_root(), 0);

        let output = render_to_string(&result, OutputFormat::Table);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2, "header plus root row");
        assert_eq!(lines[0], "        SIZE                MTIME  TYPE        PATH");
        // Root has an empty path; the row still shows its type.
        assert!(lines[1].contains("Directory"), "root row: {}", lines[1]);
    }

    #[test]
    fn should_align_columns_when_rows_have_varying_widths() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![
                    leaf("tiny.bin", 42, FileType::Other),
                    leaf("medium.bin", 1_572_864, FileType::Audio),
                    leaf("very-long-name.bin", 10_485_760, FileType::Document),
                ],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Table);

        // Column separator: two spaces before SIZE, MTIME, TYPE, PATH.
        // PATH is the last column and the measured widths are identical
        // across rows, so every row's PATH must start at the same byte
        // offset. The header uses fixed 12/19/10 widths, so it is skipped.
        // `rfind("  ")` locates the separator before PATH: paths in this
        // test contain no double spaces, and the root's empty path simply
        // ends the row right after that separator.
        let rows: Vec<&str> = output.lines().skip(1).collect();
        assert!(rows.len() >= 4, "expected root + 3 data rows: {output}");
        let path_starts: Vec<usize> = rows
            .iter()
            .map(|line| line.rfind("  ").map(|i| i + 2).unwrap_or(0))
            .collect();
        assert!(
            path_starts.windows(2).all(|w| w[0] == w[1]),
            "PATH column starts at inconsistent offsets: {path_starts:?}"
        );
    }

    #[test]
    fn should_show_human_readable_sizes_when_bytes_are_large() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![
                    leaf("empty.bin", 0, FileType::Other),
                    leaf("kib.bin", 1024, FileType::Other),
                    leaf("mib.bin", 1_572_864, FileType::Other),
                ],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Table);

        assert!(output.contains("0 B"), "got: {output}");
        assert!(output.contains("1.0 KiB"), "got: {output}");
        assert!(output.contains("1.5 MiB"), "got: {output}");
    }

    // ── JSON ────────────────────────────────────────────────────────────

    #[test]
    fn should_produce_valid_json_when_result_has_children() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![leaf("file.txt", 10, FileType::Document)],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Json);

        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("output should be valid JSON");
        assert!(parsed.is_object(), "expected a JSON object, got: {parsed}");
    }

    #[test]
    fn should_include_all_fields_when_serializing_a_node() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![leaf("song.mp3", 2048, FileType::Audio)],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Json);

        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("output should be valid JSON");
        let root = parsed
            .as_object()
            .expect("root should be an object");
        assert_eq!(root["path"], "");
        assert_eq!(root["size"], 2048);
        assert_eq!(root["modified"], 0);
        assert_eq!(root["file_type"], "dir");
        let children = root["children"]
            .as_array()
            .expect("children should be an array");
        assert_eq!(children.len(), 1);
        let child = &children[0];
        assert_eq!(child["path"], "song.mp3");
        assert_eq!(child["size"], 2048);
        assert_eq!(child["modified"], 0);
        assert_eq!(child["file_type"], "audio");
        assert!(child["children"].as_array().unwrap().is_empty());
    }

    // ── JSONL ───────────────────────────────────────────────────────────

    #[test]
    fn should_write_one_line_per_node_when_tree_is_nested() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![
                    dir(
                        "sub",
                        vec![leaf("deep.txt", 5, FileType::Document)],
                    ),
                    leaf("top.bin", 7, FileType::Other),
                ],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Jsonl);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4, "root + sub + deep + top: {output}");
    }

    #[test]
    fn should_emit_valid_json_on_each_line_when_serializing() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![dir(
                    "sub",
                    vec![leaf("deep.txt", 5, FileType::Document)],
                )],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Jsonl);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3, "root + sub + deep: {output}");
        let parsed: Vec<serde_json::Value> = lines
            .iter()
            .map(|l| serde_json::from_str(l).expect("each line should be valid JSON"))
            .collect();
        assert_eq!(parsed[0]["path"], "");
        assert_eq!(parsed[1]["path"], "sub");
        assert_eq!(parsed[2]["path"], "deep.txt");
    }

    // ── Tree ────────────────────────────────────────────────────────────

    #[test]
    fn should_use_branch_glyphs_when_rendering_nested_nodes() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![
                    dir("docs", vec![leaf("readme.md", 3, FileType::Document)]),
                    leaf("top.txt", 9, FileType::Other),
                ],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Tree);

        // docs is not the last child → ├─; top.txt is last → └─.
        let docs_line = output.lines().find(|l| l.contains("docs [")).unwrap();
        assert!(docs_line.contains("├── docs"), "got: {docs_line}");
        let top_line = output.lines().find(|l| l.contains("top.txt")).unwrap();
        assert!(top_line.contains("└── top.txt"), "got: {top_line}");
        // readme.md is nested under docs (only child → └──).
        let readme_line = output.lines().find(|l| l.contains("readme.md")).unwrap();
        assert!(readme_line.contains("└── readme.md"), "got: {readme_line}");
        assert!(readme_line.starts_with("    "), "expected indent, got: {readme_line}");
    }

    #[test]
    fn should_indent_children_when_rendering_nested_tree() {
        let result = ScanResult::from_tree(
            dir(
                "",
                vec![dir("docs", vec![leaf("readme.md", 3, FileType::Document)])],
            ),
            0,
        );

        let output = render_to_string(&result, OutputFormat::Tree);

        //     [3 B, dir]
        //     └── docs [3 B, dir]
        //         └── readme.md [3 B, document]
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3, "root + docs + readme.md: {output}");
        assert!(lines[1].contains("└── docs"), "got: {}", lines[1]);
        // readme.md indented under docs.
        assert!(
            lines[2].starts_with("    ") && lines[2].contains("└── readme.md"),
            "expected indentation + glyph, got: {}",
            lines[2]
        );
    }

    // ── Cross-format ────────────────────────────────────────────────────

    #[test]
    fn should_write_nothing_beyond_root_when_result_is_empty() {
        let result = ScanResult::from_tree(empty_root(), 0);

        let table = render_to_string(&result, OutputFormat::Table);
        let jsonl = render_to_string(&result, OutputFormat::Jsonl);
        let tree = render_to_string(&result, OutputFormat::Tree);

        // Table: header plus the root row only — no extra data rows.
        assert_eq!(table.lines().count(), 2, "table: {table}");
        // JSONL: exactly one line — the root itself.
        assert_eq!(jsonl.lines().count(), 1, "jsonl: {jsonl}");
        // Tree: exactly one line — the root itself.
        assert_eq!(tree.lines().count(), 1, "tree: {tree}");
    }

    #[test]
    fn should_always_include_root_when_rendering_any_format() {
        let result = ScanResult::from_tree(empty_root(), 0);

        let table = render_to_string(&result, OutputFormat::Table);
        let json = render_to_string(&result, OutputFormat::Json);
        let jsonl = render_to_string(&result, OutputFormat::Jsonl);
        let tree = render_to_string(&result, OutputFormat::Tree);

        // Table: root row present (empty path row after the header).
        assert!(table.lines().nth(1).is_some(), "table: {table}");
        // JSON: root object present even with no children.
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should be valid");
        assert!(parsed["children"].is_array(), "json: {json}");
        assert!(parsed["children"].as_array().unwrap().is_empty());
        // JSONL: first (only) line is the root node as a JSON object.
        let root_line = jsonl.lines().next().expect("jsonl should have a root line");
        let root_json: serde_json::Value =
            serde_json::from_str(root_line).expect("root line should be valid JSON");
        assert_eq!(root_json["path"], "");
        // Tree: root line present even with no children.
        assert!(!tree.is_empty(), "tree: {tree}");
    }
}
