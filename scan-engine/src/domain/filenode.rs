use std::path::PathBuf;
use std::time::SystemTime;

use super::error::DomainError;
use super::filter::Filter;
use super::format::OutputFormat;
use super::sort::SortKey;

/// A node in the file tree: either a file or a directory with children.
#[derive(Debug, Clone, PartialEq)]
pub struct FileNode {
    /// Absolute path on the filesystem.
    pub path: PathBuf,
    /// File or directory name (not the full path).
    pub name: String,
    /// Size of this node in bytes (0 for directories whose size is the sum of children).
    pub size: u64,
    /// Modification time as a Unix timestamp (seconds since epoch).
    pub mtime: u64,
    /// Child nodes (empty for files).
    pub children: Vec<FileNode>,
    /// Whether this node is a directory.
    pub is_dir: bool,
}

impl FileNode {
    /// Create a new FileNode. `path` must be non-empty.
    pub fn new(path: PathBuf, name: String, size: u64, mtime: u64, is_dir: bool) -> Result<Self, DomainError> {
        if path.as_os_str().is_empty() {
            return Err(DomainError::InvalidPath("path is empty".into()));
        }
        Ok(Self { path, name, size, mtime, children: Vec::new(), is_dir })
    }

    /// Total size: this node's own size plus the recursive total of all children.
    pub fn total_size(&self) -> u64 {
        self.size + self.children.iter().map(|c| c.total_size()).sum::<u64>()
    }

    /// Count of non-directory leaf nodes (recursive).
    pub fn file_count(&self) -> u64 {
        if self.children.is_empty() && !self.is_dir {
            1
        } else {
            self.children.iter().map(|c| c.file_count()).sum()
        }
    }

    /// Return a new tree containing only nodes matching all `filters`, bounded
    /// by `max_depth` (None = unlimited). Directories are pruned when no
    /// descendant survives.
    pub fn filter(&self, filters: &[Filter], max_depth: Option<u32>) -> Option<Self> {
        self.filter_inner(filters, max_depth, 0)
    }

    fn filter_inner(&self, filters: &[Filter], max_depth: Option<u32>, depth: u32) -> Option<Self> {
        let at_depth_limit = max_depth.is_some_and(|d| depth >= d);

        let filtered_children: Vec<FileNode> = if at_depth_limit || !self.is_dir {
            Vec::new()
        } else {
            self.children
                .iter()
                .filter_map(|c| c.filter_inner(filters, max_depth, depth + 1))
                .collect()
        };

        let passes_filters = self.matches_filters(filters);

        if passes_filters || !filtered_children.is_empty() {
            Some(FileNode {
                path: self.path.clone(),
                name: self.name.clone(),
                size: self.size,
                mtime: self.mtime,
                children: filtered_children,
                is_dir: self.is_dir,
            })
        } else {
            None
        }
    }

    /// Returns `true` when this node satisfies every filter in `filters` (AND logic).
    pub fn matches_filters(&self, filters: &[Filter]) -> bool {
        filters.iter().all(|f| self.matches_filter(f))
    }

    fn matches_filter(&self, filter: &Filter) -> bool {
        match filter {
            Filter::MinSize(min) => self.size >= *min,
            Filter::MaxSize(max) => self.size <= *max,
            Filter::Extension(ext) => self.name.contains('.').then(|| {
                self.name.rsplit_once('.').map(|(_, e)| e.eq_ignore_ascii_case(ext))
            }).flatten().unwrap_or(false),
            Filter::MaxAge(duration) => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let age_secs = now.saturating_sub(self.mtime);
                age_secs <= duration.as_secs()
            }
            Filter::NamePattern(pattern) => glob_matches(pattern, &self.name),
        }
    }

    /// Return a new tree with children sorted by `key` recursively.
    pub fn sort(&self, key: SortKey) -> Self {
        let mut node = self.clone();
        match key {
            SortKey::SizeDesc => node.children.sort_by_key(|a| std::cmp::Reverse(a.total_size())),
            SortKey::SizeAsc  => node.children.sort_by_key(|a| a.total_size()),
            SortKey::NameAsc  => node.children.sort_by(|a, b| a.name.cmp(&b.name)),
            SortKey::NameDesc => node.children.sort_by(|a, b| b.name.cmp(&a.name)),
        }
        for child in &mut node.children {
            *child = child.sort(key);
        }
        node
    }

    /// Format this node (and its subtree) in the given output format.
    pub fn format(&self, fmt: OutputFormat) -> Result<String, DomainError> {
        match fmt {
            OutputFormat::Json   => format_json(self),
            OutputFormat::Table  => format_table(self),
            OutputFormat::Jsonl  => format_jsonl(self),
            OutputFormat::Tree   => format_tree(self, 0),
        }
    }
}

// --- JSON helpers (no serde dependency) ---

fn format_json(node: &FileNode) -> Result<String, DomainError> {
    let mut s = String::new();
    node_to_json(node, &mut s);
    Ok(s)
}

fn node_to_json(node: &FileNode, s: &mut String) {
    s.push('{');
    json_str(s, "name", &node.name);
    s.push(',');
    json_str(s, "path", &node.path.display().to_string());
    s.push(',');
    json_u64(s, "size", node.size);
    s.push(',');
    json_u64(s, "total_size", node.total_size());
    s.push(',');
    json_u64(s, "mtime", node.mtime);
    s.push(',');
    json_bool(s, "is_dir", node.is_dir);
    if !node.children.is_empty() {
        s.push_str(",\"children\":[");
        for (i, child) in node.children.iter().enumerate() {
            if i > 0 { s.push(','); }
            node_to_json(child, s);
        }
        s.push(']');
    }
    s.push('}');
}

fn json_str(s: &mut String, key: &str, val: &str) {
    s.push('"');
    s.push_str(key);
    s.push_str("\":\"");
    // Escape JSON-special chars in value
    for c in val.chars() {
        match c {
            '"'  => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            _    => s.push(c),
        }
    }
    s.push('"');
}

fn json_u64(s: &mut String, key: &str, val: u64) {
    s.push('"');
    s.push_str(key);
    s.push_str("\":");
    s.push_str(&val.to_string());
}

fn json_bool(s: &mut String, key: &str, val: bool) {
    s.push('"');
    s.push_str(key);
    s.push_str("\":");
    s.push_str(if val { "true" } else { "false" });
}

// --- Table formatter ---

fn format_table(root: &FileNode) -> Result<String, DomainError> {
    let mut rows: Vec<&FileNode> = Vec::new();
    collect_leaves(root, &mut rows);
    if rows.is_empty() {
        return Ok("Name\tSize\tModified\tType\n".into());
    }

    let mut out = String::from("Name\tSize\tModified\tType\n");
    for node in &rows {
        let size_str = human_size(node.total_size());
        let type_str = if node.is_dir { "dir" } else { file_ext(node) };
        out.push_str(&format!("{}\t{}\t{}\t{}\n", node.name, size_str, node.mtime, type_str));
    }
    Ok(out)
}

fn collect_leaves<'a>(node: &'a FileNode, out: &mut Vec<&'a FileNode>) {
    if node.children.is_empty() {
        out.push(node);
    } else {
        for child in &node.children {
            collect_leaves(child, out);
        }
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}

fn file_ext(node: &FileNode) -> &str {
    node.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("")
}

// --- JSONL formatter ---

fn format_jsonl(root: &FileNode) -> Result<String, DomainError> {
    let mut out = String::new();
    collect_jsonl(root, &mut out);
    Ok(out)
}

fn collect_jsonl(node: &FileNode, out: &mut String) {
    let mut line = String::new();
    node_to_json(node, &mut line);
    out.push_str(&line);
    out.push('\n');
    for child in &node.children {
        collect_jsonl(child, out);
    }
}

// --- Tree formatter ---

fn format_tree(node: &FileNode, depth: usize) -> Result<String, DomainError> {
    let mut out = String::new();
    let indent = "  ".repeat(depth);
    let size_str = human_size(node.total_size());
    out.push_str(&format!("{}{} ({})\n", indent, node.name, size_str));
    for child in &node.children {
        out.push_str(&format_tree(child, depth + 1)?);
    }
    Ok(out)
}

// --- Glob matcher (minimal, no dependency) ---

/// Simple glob matching: `*` matches any sequence, `?` matches one char.
/// Case-sensitive.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let pb: Vec<char> = pattern.chars().collect();
    let tb: Vec<char> = text.chars().collect();
    matches_inner(&pb, &tb)
}

fn matches_inner(pattern: &[char], text: &[char]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX; // pattern position of last '*'
    let mut star_ti = 0;          // text position when '*' was matched

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1; // consume the '*'
        } else if star_pi != usize::MAX {
            // backtrack: let the '*' consume one more char
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    // consume trailing '*' in pattern
    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }
    pi == pattern.len()
}

#[cfg(test)]
mod glob_tests {
    use super::*;

    #[test]
    fn test_glob_basic() {
        assert!(glob_matches("*.rs", "main.rs"));
        assert!(!glob_matches("*.rs", "main.txt"));
    }

    #[test]
    fn test_glob_question_mark() {
        assert!(glob_matches("?.txt", "a.txt"));
        assert!(!glob_matches("?.txt", "ab.txt"));
    }

    #[test]
    fn test_glob_star_matches_empty() {
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("*", ""));
    }
}
