use std::fmt;

/// Domain-level error, covering all adapter failure modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// The path is empty or otherwise invalid.
    InvalidPath(String),
    /// A scan operation failed.
    ScanFailed(String),
    /// A cache operation failed.
    CacheFailed(String),
    /// A trash (delete/undo) operation failed.
    TrashFailed(String),
    /// A filter operation failed.
    FilterFailed(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::InvalidPath(msg) => write!(f, "invalid path: {}", msg),
            DomainError::ScanFailed(msg)   => write!(f, "scan failed: {}", msg),
            DomainError::CacheFailed(msg)  => write!(f, "cache failed: {}", msg),
            DomainError::TrashFailed(msg)  => write!(f, "trash failed: {}", msg),
            DomainError::FilterFailed(msg) => write!(f, "filter failed: {}", msg),
        }
    }
}

impl std::error::Error for DomainError {}

/// Scan-specific error wrapping I/O failures.
#[derive(Debug)]
pub enum ScanError {
    /// An I/O error occurred during scanning.
    Io(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io(msg) => write!(f, "scan I/O error: {}", msg),
        }
    }
}

impl std::error::Error for ScanError {}

/// Cache-specific error.
#[derive(Debug)]
pub enum CacheError {
    /// An I/O error occurred during cache access.
    Io(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::Io(msg) => write!(f, "cache I/O error: {}", msg),
        }
    }
}

impl std::error::Error for CacheError {}

/// Trash-specific error.
#[derive(Debug)]
pub enum TrashError {
    /// An I/O error occurred during a trash operation.
    Io(String),
}

impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrashError::Io(msg) => write!(f, "trash I/O error: {}", msg),
        }
    }
}

impl std::error::Error for TrashError {}
