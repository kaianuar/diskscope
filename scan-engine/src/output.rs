use crate::domain::error::DomainError;
use crate::domain::format::OutputFormat;
use crate::domain::tree::FileTree;

/// Format a `FileTree` into the given output format.
pub fn format_tree(tree: &FileTree, format: OutputFormat) -> Result<String, DomainError> {
    tree.root.format(format)
}
