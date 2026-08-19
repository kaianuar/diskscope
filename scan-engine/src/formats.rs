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
    // Header.
    writeln!(out, "{:>12}  {:>19}  {:<10}  PATH", "SIZE", "MTIME", "TYPE")?;
    let mut widths: TableWidths = TableWidths::default();
    for n in walk(result) {
        measure(n, &mut widths);
    }
    for n in walk(result) {
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
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    writeln!(out)?;
    Ok(())
}

// ── JSONL (one object per line) ────────────────────────────────────────────

fn render_jsonl(result: &ScanResult, out: &mut dyn Write) -> io::Result<()> {
    for node in walk(result) {
        let json = to_json(node);
        let bytes = serde_json::to_vec(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
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
    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
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

