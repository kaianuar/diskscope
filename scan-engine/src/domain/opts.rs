use super::filenode::FileNode;
use super::filter::Filter;
use super::format::OutputFormat;
use super::sort::SortKey;

/// Options passed to a scan operation.
#[derive(Debug, Clone, Default)]
pub struct ScanOpts {
    /// Filters to apply to scan results.
    pub filters: Vec<Filter>,
    /// Sort order for children.
    pub sort: Option<SortKey>,
    /// Maximum directory depth (None = unlimited).
    pub depth: Option<u32>,
    /// Output format for rendering.
    pub format: OutputFormat,
}

impl ScanOpts {
    /// Create a new default `ScanOpts`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply filters and sorting to a `FileNode` tree.
    ///
    /// 1. Filters children recursively via `FileNode::filter` (respects `depth`).
    /// 2. Sorts surviving children via `FileNode::sort` when `self.sort` is set.
    ///
    /// Returns `None` when every node is pruned by filters.
    pub fn apply(&self, root: &FileNode) -> Option<FileNode> {
        let filtered = if self.filters.is_empty() && self.depth.is_none() {
            Some(root.clone())
        } else {
            root.filter(&self.filters, self.depth)
        };
        filtered.map(|node| match self.sort {
            Some(key) => node.sort(key),
            None => node,
        })
    }
}
