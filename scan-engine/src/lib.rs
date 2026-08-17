#![deny(clippy::all)]
#![deny(missing_docs)]

//! DiskScope scan engine — domain entities, port traits, and adapters.

/// Domain entities, port traits, and error types.
pub mod domain;
/// Scanner adapters for filesystem walking and caching.
pub mod scanner;
/// Output formatting dispatch.
pub mod output;

pub use domain::error::{CacheError, DomainError, ScanError, TrashError};
pub use domain::file_type::FileType;
pub use domain::filenode::FileNode;
pub use domain::filter::Filter;
pub use domain::format::OutputFormat;
pub use domain::opts::ScanOpts;
pub use domain::size::Size;
pub use domain::sort::SortKey;
pub use domain::tree::FileTree;
pub use domain::{CachedEntry, TrashTicket};

/// Re-export of port traits.
pub mod ports {
    pub use super::domain::ports::*;
}

/// Re-export of mock implementations.
pub mod mocks {
    pub use super::domain::mocks::*;
}
