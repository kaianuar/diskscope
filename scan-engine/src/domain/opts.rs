use super::filter::Filter;
use super::format::OutputFormat;
use super::sort::SortKey;

/// Options passed to a scan operation.
#[derive(Debug, Clone, Default)]
pub struct ScanOpts {
    pub filters: Vec<Filter>,
    pub sort: Option<SortKey>,
    pub depth: Option<u32>,
    pub format: OutputFormat,
}

impl ScanOpts {
    pub fn new() -> Self {
        Self::default()
    }
}
