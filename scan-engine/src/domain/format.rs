/// Output format for rendered scan results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum OutputFormat {
    /// JSON output (single object for the full tree).
    Json,
    /// Human-readable table (default).
    #[default]
    Table,
    /// JSON Lines (one JSON object per line).
    Jsonl,
    /// Indented tree view.
    Tree,
}
