pub mod error;
pub mod filenode;
pub mod filter;
pub mod format;
pub mod opts;
pub mod ports;
pub mod sort;

// Re-export domain-common types at domain level
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedEntry {
    pub size: u64,
    pub mtime: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashTicket {
    pub path: PathBuf,
}

pub mod mocks;
