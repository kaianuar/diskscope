//! Self-contained interactive HTML snapshot export.
//!
//! [`render_html_snapshot`] produces a single HTML document with an
//! inlined treemap, table, breadcrumb, hover tooltips, and click-to-
//! drill — zero network dependencies, opens in any browser.

use domain::{FileNode, FileType, ScanResult};

/// Maximum number of nodes to include in the snapshot by default.
pub const DEFAULT_MAX_NODES: usize = 50_000;

/// Serialize a [`ScanResult`] into a self-contained interactive HTML
/// document.
///
/// The document includes:
/// - An embedded JSON tree (`const TREE = {...}`)
/// - A squarify treemap renderer on `<canvas>`
/// - Hover tooltips (path + size)
/// - Click-to-drill into directories with a breadcrumb bar
/// - A table sidebar sorted by size descending
/// - Dark theme matching the DiskScope design tokens
///
/// `max_nodes` caps the total number of serialized nodes; the deepest
/// branches are pruned first to stay within the limit. A footer note
/// indicates when truncation occurred.
pub fn render_html_snapshot(result: &ScanResult, title: &str, max_nodes: usize) -> String {
    let tree_json = build_json(&result.root, max_nodes);
    let truncated = node_count(&result.root) > max_nodes;
    let truncated_note = if truncated {
        r#"<div class="trunc-note">Showing top entries by size (some deep branches pruned).</div>"#
    } else {
        ""
    };
    // Escape title for safe HTML embedding.
    let safe_title = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title} — DiskScope Snapshot</title>
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{ background:#0f172a; color:#e2e8f0; font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif; display:flex; height:100vh; overflow:hidden; }}
#sidebar {{ width:280px; min-width:200px; background:#1e293b; display:flex; flex-direction:column; border-right:1px solid #334155; }}
#breadcrumb {{ padding:8px 12px; display:flex; flex-wrap:wrap; gap:4px; align-items:center; border-bottom:1px solid #334155; min-height:36px; }}
.crumb {{ cursor:pointer; color:#2563eb; font-size:13px; }}
.crumb:hover {{ text-decoration:underline; }}
.crumb-sep {{ color:#64748b; font-size:13px; }}
.crumb-current {{ color:#e2e8f0; font-size:13px; }}
#table-wrap {{ flex:1; overflow-y:auto; }}
table {{ width:100%; border-collapse:collapse; font-size:12px; }}
th {{ position:sticky; top:0; background:#1e293b; text-align:left; padding:6px 8px; color:#94a3b8; border-bottom:1px solid #334155; }}
td {{ padding:5px 8px; border-bottom:1px solid rgba(51,65,85,.4); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; max-width:240px; cursor:pointer; }}
tr:hover td {{ background:rgba(37,99,235,.15); }}
.size-col {{ text-align:right; font-variant-numeric:tabular-nums; }}
.type-col {{ color:#94a3b8; }}
#main {{ flex:1; display:flex; flex-direction:column; position:relative; }}
#canvas-wrap {{ flex:1; position:relative; }}
canvas {{ display:block; width:100%; height:100%; }}
#tooltip {{ display:none; position:absolute; pointer-events:none; background:#1e293b; border:1px solid #334155; border-radius:4px; padding:6px 10px; font-size:12px; color:#e2e8f0; max-width:400px; word-break:break-all; z-index:10; box-shadow:0 4px 12px rgba(0,0,0,.4); }}
.trunc-note {{ padding:4px 12px; font-size:11px; color:#64748b; background:#0f172a; border-top:1px solid #334155; }}
.footer {{ padding:4px 12px; font-size:11px; color:#475569; background:#0f172a; border-top:1px solid #334155; }}
</style>
</head>
<body>
<div id="sidebar">
  <div id="breadcrumb"></div>
  <div id="table-wrap"><table><thead><tr><th>Name</th><th class="size-col">Size</th><th>Type</th></tr></thead><tbody id="tbody"></tbody></table></div>
  {truncated_note}
</div>
<div id="main">
  <div id="canvas-wrap"><canvas id="canvas"></canvas></div>
  <div id="tooltip"></div>
</div>
<script>
const TREE = {tree_json};

const TYPE_COLORS = {{
  audio:"#2563eb", video:"#f59e0b", image:"#22c55e", document:"#ef4444",
  code:"#e2e8f0", archive:"#94a3b8", dir:"#1e293b", other:"#475569"
}};

function formatSize(b) {{
  if (b === 0) return '0 B';
  const u = ['B','KiB','MiB','GiB','TiB','PiB'];
  const i = Math.min(Math.floor(Math.log2(b) / 10), u.length - 1);
  return (b / (1 << (10 * i))).toFixed(i === 0 ? 0 : 1) + ' ' + u[i];
}}

// Squarify treemap layout (Bruls, Huizing, van Wijk 2000).
function squarify(entries, rect, depth, out) {{
  if (!entries.length || rect.width <= 0 || rect.height <= 0) return;
  const sorted = [...entries].sort((a, b) => b.size - a.size);
  const total = sorted.reduce((s, n) => s + n.size, 0);
  if (total <= 0) return;
  const area = rect.width * rect.height;
  const scale = area / total;
  let cur = {{ ...rect }};
  let horiz = cur.width >= cur.height;
  let row = [], rowSum = 0;

  function worst(r, sum) {{
    if (!r.length || sum <= 0) return Infinity;
    const length = horiz ? cur.width : cur.height;
    const thickness = (sum * scale) / length;
    if (thickness <= 0) return Infinity;
    const cellLens = r.map(n => (n.size * scale) / thickness);
    const minLen = Math.min(...cellLens);
    const maxLen = Math.max(...cellLens);
    return Math.max(length / minLen, thickness / minLen, maxLen / minLen, thickness / maxLen);
  }}

  function emitRow(r, sum) {{
    if (!r.length || sum <= 0) return;
    const rowArea = sum * scale;
    const length = horiz ? cur.width : cur.height;
    const thickness = rowArea / length;
    if (thickness <= 0) return;
    let cursor = 0;
    for (const node of r) {{
      const cellLen = (node.size * scale) / thickness;
      const cellRect = horiz
        ? {{ x: cur.x + cursor, y: cur.y, width: cellLen, height: thickness }}
        : {{ x: cur.x, y: cur.y + cursor, width: thickness, height: cellLen }};
      out.push({{ node, rect: cellRect, depth }});
      cursor += cellLen;
    }}
    cur = horiz
      ? {{ x: cur.x, y: cur.y + thickness, width: cur.width, height: cur.height - thickness }}
      : {{ x: cur.x + thickness, y: cur.y, width: cur.width - thickness, height: cur.height }};
    horiz = cur.width >= cur.height;
  }}

  for (const node of sorted) {{
    const candidateSum = rowSum + node.size;
    if (!row.length || worst([...row, node], candidateSum) <= worst(row, rowSum)) {{
      row.push(node);
      rowSum = candidateSum;
    }} else {{
      emitRow(row, rowSum);
      row = [node];
      rowSum = node.size;
    }}
  }}
  if (row.length) emitRow(row, rowSum);
}}

// State
let currentNode = TREE;
let path = [TREE];
let layout = [];

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d');
const tooltip = document.getElementById('tooltip');
const breadcrumb = document.getElementById('breadcrumb');
const tbody = document.getElementById('tbody');

function resizeCanvas() {{
  const wrap = document.getElementById('canvas-wrap');
  const dpr = window.devicePixelRatio || 1;
  canvas.width = wrap.clientWidth * dpr;
  canvas.height = wrap.clientHeight * dpr;
  canvas.style.width = wrap.clientWidth + 'px';
  canvas.style.height = wrap.clientHeight + 'px';
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}}

function renderBreadcrumb() {{
  let html = '';
  path.forEach((n, i) => {{
    if (i > 0) html += '<span class="crumb-sep">/</span>';
    if (i < path.length - 1) {{
      const label = n.path || '(root)';
      html += '<span class="crumb" data-idx="' + i + '">' + label.replace(/</g, '&lt;') + '</span>';
    }} else {{
      const label = n.path || '(root)';
      html += '<span class="crumb-current">' + label.replace(/</g, '&lt;') + '</span>';
    }}
  }});
  breadcrumb.innerHTML = html;
  breadcrumb.querySelectorAll('.crumb').forEach(el => {{
    el.addEventListener('click', () => {{
      const idx = +el.dataset.idx;
      path = path.slice(0, idx + 1);
      currentNode = path[path.length - 1];
      renderAll();
    }});
  }});
}}

function renderTable() {{
  const children = currentNode.children || [];
  const sorted = [...children].sort((a, b) => b.size - a.size);
  let html = '';
  for (const n of sorted) {{
    const name = (n.path || '').split('/').pop() || '(root)';
    const nameEsc = name.replace(/</g, '&lt;');
    html += '<tr><td>' + nameEsc + '</td><td class="size-col">' + formatSize(n.size) +
            '</td><td class="type-col">' + n.fileType + '</td></tr>';
  }}
  tbody.innerHTML = html;
  tbody.querySelectorAll('tr').forEach((tr, i) => {{
    tr.addEventListener('click', () => {{
      const n = sorted[i];
      if (n.children && n.children.length) {{
        path.push(n);
        currentNode = n;
        renderAll();
      }}
    }});
  }});
}}

function renderTreemap() {{
  resizeCanvas();
  const w = canvas.width / (window.devicePixelRatio || 1);
  const h = canvas.height / (window.devicePixelRatio || 1);
  ctx.clearRect(0, 0, w, h);
  layout = [];
  squarify(currentNode.children || [], {{ x: 0, y: 0, width: w, height: h }}, 0, layout);

  for (const entry of layout) {{
    const {{ node, rect }} = entry;
    const color = TYPE_COLORS[node.fileType] || TYPE_COLORS.other;
    ctx.fillStyle = color;
    ctx.fillRect(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2);
    // Label (only if big enough).
    if (rect.width > 40 && rect.height > 16) {{
      ctx.fillStyle = node.fileType === 'dir' ? '#94a3b8' : '#0f172a';
      ctx.font = (rect.height > 24 ? 12 : 10) + 'px sans-serif';
      const label = (node.path || '').split('/').pop() || '(root)';
      ctx.save();
      ctx.beginPath();
      ctx.rect(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
      ctx.clip();
      ctx.fillText(label, rect.x + 4, rect.y + (rect.height > 24 ? 16 : 13), rect.width - 8);
      ctx.restore();
    }}
  }}
}}

function renderAll() {{
  renderBreadcrumb();
  renderTable();
  renderTreemap();
}}

// Tooltip on hover.
canvas.addEventListener('mousemove', e => {{
  const rect = canvas.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;
  let hit = null;
  for (const entry of layout) {{
    const r = entry.rect;
    if (mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height) {{
      hit = entry;
    }}
  }}
  if (hit) {{
    tooltip.style.display = 'block';
    tooltip.textContent = hit.node.path || '(root)' + ' — ' + formatSize(hit.node.size);
    tooltip.style.left = (e.clientX + 12) + 'px';
    tooltip.style.top = (e.clientY + 12) + 'px';
    canvas.style.cursor = hit.node.children && hit.node.children.length ? 'pointer' : 'default';
  }} else {{
    tooltip.style.display = 'none';
    canvas.style.cursor = 'default';
  }}
}});

canvas.addEventListener('mouseleave', () => {{ tooltip.style.display = 'none'; }});

// Click to drill into directory.
canvas.addEventListener('click', e => {{
  const rect = canvas.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;
  for (const entry of layout) {{
    const r = entry.rect;
    if (mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height) {{
      if (entry.node.children && entry.node.children.length) {{
        path.push(entry.node);
        currentNode = entry.node;
        renderAll();
        return;
      }}
    }}
  }}
}});

// Back via browser back button or keyboard.
window.addEventListener('keydown', e => {{
  if (e.key === 'Escape' && path.length > 1) {{
    path.pop();
    currentNode = path[path.length - 1];
    renderAll();
  }}
}});

window.addEventListener('resize', renderTreemap);
renderAll();
</script>
</body>
</html>"##,
        title = safe_title,
        tree_json = tree_json,
        truncated_note = truncated_note,
    )
}

/// Count total nodes in a tree (recursive).
fn node_count(node: &FileNode) -> usize {
    1 + node.children.iter().map(node_count).sum::<usize>()
}

/// Serialize a `FileNode` tree to a compact JSON string literal.
///
/// Nodes beyond `max_nodes` are pruned by dropping the smallest children
/// at each level until the limit is met.
fn build_json(root: &FileNode, max_nodes: usize) -> String {
    let budget = max_nodes.max(1);
    let mut buf = String::with_capacity(4096);
    write_node_json(root, budget, &mut buf);
    buf
}

/// Recursive JSON emitter with a node budget.
///
/// Returns the number of nodes emitted so far (including the current one).
fn write_node_json(node: &FileNode, budget: usize, buf: &mut String) -> usize {
    let name = escape_json(&node.path);
    let ft = file_type_label(node.file_type);
    buf.push_str(&format!(
        r#"{{"path":"{}","size":{},"fileType":"{}","children":["#,
        name, node.size, ft,
    ));

    let mut count = 1usize;
    if budget > 1 && !node.children.is_empty() {
        // Sort children by size desc; drop smallest when over budget.
        let mut sorted: Vec<&FileNode> = node.children.iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.size));

        let mut first = true;
        for child in sorted {
            if count >= budget {
                break;
            }
            if !first {
                buf.push(',');
            }
            first = false;
            count += write_node_json(child, budget - count, buf);
        }
    }

    buf.push_str("]}");
    count
}

/// Escape a string for embedding in a JSON string literal.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Map a `domain::FileType` to the short label used in the JSON / JS.
fn file_type_label(ft: FileType) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ────────────────────────────────────────────────────

    fn leaf(path: &str, size: u64, file_type: FileType) -> FileNode {
        FileNode { path: path.to_string(), size, modified: 0, file_type, children: Vec::new() }
    }

    fn dir(path: &str, children: Vec<FileNode>) -> FileNode {
        FileNode {
            path: path.to_string(),
            size: children.iter().map(|c| c.total_size()).sum(),
            modified: 0,
            file_type: FileType::Directory,
            children,
        }
    }

    fn simple_result() -> ScanResult {
        ScanResult::from_tree(
            dir(
                "/test",
                vec![
                    leaf("file_a.txt", 100, FileType::Document),
                    dir("sub", vec![leaf("file_b.bin", 300, FileType::Other)]),
                ],
            ),
            0,
        )
    }

    // ── render_html_snapshot ─────────────────────────────────────────────

    #[test]
    fn should_produce_doctype_html_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Test", DEFAULT_MAX_NODES);

        assert!(html.contains("<!DOCTYPE html>"), "missing doctype");
        assert!(html.contains("<html"), "missing html tag");
        assert!(html.contains("</html>"), "missing closing html tag");
    }

    #[test]
    fn should_embed_tree_paths_in_json_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Test", DEFAULT_MAX_NODES);

        assert!(html.contains(r#""path":"/test""#), "missing root path");
        assert!(html.contains(r#""path":"file_a.txt""#), "missing file_a.txt");
        assert!(html.contains(r#""path":"sub""#), "missing sub dir");
        assert!(html.contains(r#""path":"file_b.bin""#), "missing file_b.bin");
    }

    #[test]
    fn should_include_title_in_output_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "My Scan", DEFAULT_MAX_NODES);

        assert!(html.contains("My Scan"), "title missing from output");
    }

    #[test]
    fn should_escape_html_entities_in_title_when_title_has_special_chars() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "A <b>bold</b> & \"quoted\"", DEFAULT_MAX_NODES);

        assert!(!html.contains("<b>"), "raw HTML tag in title: {html}");
        assert!(html.contains("&lt;b&gt;"), "escaped tag missing");
        assert!(html.contains("&amp;"), "escaped ampersand missing");
    }

    #[test]
    fn should_truncate_large_trees_when_exceeding_max_nodes() {
        // Build a tree with 200 nodes.
        let children: Vec<FileNode> =
            (0..200).map(|i| leaf(&format!("f{i}.txt"), 10, FileType::Other)).collect();
        let result = ScanResult::from_tree(dir("/big", children), 0);

        let html = render_html_snapshot(&result, "Big", 10);

        // The TREE JSON should have fewer than 200 entries.
        let count_f = html.matches(r#""path":""#).count();
        assert!(count_f <= 12, "expected <= 12 node entries, got {count_f}");
        // Truncation note should be present.
        assert!(html.contains("top entries by size"), "missing truncation note");
    }

    #[test]
    fn should_not_show_truncation_note_when_tree_within_limit() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Small", DEFAULT_MAX_NODES);

        assert!(!html.contains("top entries by size"), "should not show truncation note");
    }

    #[test]
    fn should_embed_valid_json_in_tree_const_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Test", DEFAULT_MAX_NODES);

        // Extract TREE JSON: find `const TREE = ` and then parse the JSON.
        let marker = "const TREE = ";
        let start = html.find(marker).expect("TREE marker missing") + marker.len();
        // Find end of the JSON object (match braces).
        let bytes = html[start..].as_bytes();
        let mut depth = 0i32;
        let mut end = 0;
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
        }
        assert!(end > 0, "unbalanced braces in TREE JSON");
        let json_str = &html[start..start + end];
        let parsed: serde_json::Value =
            serde_json::from_str(json_str).expect("TREE should be valid JSON");
        assert!(parsed.is_object(), "TREE should be a JSON object");
        assert_eq!(parsed["path"], "/test");
        assert!(parsed["children"].is_array());
    }

    #[test]
    fn should_contain_canvas_and_sidebar_elements_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Test", DEFAULT_MAX_NODES);

        assert!(html.contains("<canvas"), "missing canvas element");
        assert!(html.contains("id=\"sidebar\""), "missing sidebar element");
        assert!(html.contains("id=\"breadcrumb\""), "missing breadcrumb element");
        assert!(html.contains("<table"), "missing table element");
    }

    #[test]
    fn should_include_dark_theme_background_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Test", DEFAULT_MAX_NODES);

        assert!(html.contains("#0f172a"), "missing dark theme background");
        assert!(html.contains("#1e293b"), "missing surface color");
        assert!(html.contains("#2563eb"), "missing accent color");
    }

    #[test]
    fn should_contain_squarify_function_when_rendering_snapshot() {
        let result = simple_result();
        let html = render_html_snapshot(&result, "Test", DEFAULT_MAX_NODES);

        assert!(html.contains("function squarify"), "missing squarify function");
        assert!(html.contains("function formatSize"), "missing formatSize function");
    }

    // ── build_json ──────────────────────────────────────────────────────

    #[test]
    fn should_emit_valid_json_when_building_tree_json() {
        let node = leaf("test.txt", 42, FileType::Document);
        let json = build_json(&node, 1000);

        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("should produce valid JSON");
        assert_eq!(parsed["path"], "test.txt");
        assert_eq!(parsed["size"], 42);
        assert_eq!(parsed["fileType"], "document");
    }

    // ── escape_json ─────────────────────────────────────────────────────

    #[test]
    fn should_escape_special_chars_when_path_has_quotes_and_backslashes() {
        let node = FileNode {
            path: r#"a"b\c"n"d"#.to_string(),
            size: 1,
            modified: 0,
            file_type: FileType::Other,
            children: Vec::new(),
        };
        let json = build_json(&node, 1000);

        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("should produce valid JSON despite special chars");
        assert_eq!(parsed["path"], r#"a"b\c"n"d"#);
    }

    // ── file_type_label ─────────────────────────────────────────────────

    #[test]
    fn should_map_all_file_types_to_short_labels_when_converting() {
        assert_eq!(file_type_label(FileType::Audio), "audio");
        assert_eq!(file_type_label(FileType::Video), "video");
        assert_eq!(file_type_label(FileType::Image), "image");
        assert_eq!(file_type_label(FileType::Document), "document");
        assert_eq!(file_type_label(FileType::Code), "code");
        assert_eq!(file_type_label(FileType::Archive), "archive");
        assert_eq!(file_type_label(FileType::Directory), "dir");
        assert_eq!(file_type_label(FileType::Other), "other");
    }
}
