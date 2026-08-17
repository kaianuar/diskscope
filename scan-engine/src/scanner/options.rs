use crate::domain::filter::Filter;

/// Adapter-level options for the scanner.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Maximum directory depth to scan. None = unlimited.
    pub max_depth: Option<usize>,
    /// Follow symbolic links.
    pub follow_symlinks: bool,
    /// Respect .gitignore files during walk.
    pub respect_gitignore: bool,
    /// File size/type filters applied during walk.
    pub filters: Vec<Filter>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: None,
            follow_symlinks: false,
            respect_gitignore: true,
            filters: Vec::new(),
        }
    }
}
