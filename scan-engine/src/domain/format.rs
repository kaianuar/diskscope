/// Output format for rendered scan results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum OutputFormat {
    Json,
    #[default]
    Table,
    Jsonl,
    Tree,
}
