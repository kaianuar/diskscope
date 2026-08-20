//! Sync-domain types and transport port.
//!
//! [`SyncEvent`] and [`SyncError`] are the domain-level change events
//! exchanged by the sync layer. [`SyncTransport`] is the port trait
//! that adapters (Ably, in-memory, etc.) implement to deliver those
//! events.
//!
//! Wire-format mirrors (`WireEvent`) and concrete transport
//! implementations live in `scan-engine::sync` — only the port and the
//! event types it depends on belong here in the domain.

use std::fmt;

use crate::{DomainError, FileType};

// ── SyncError ─────────────────────────────────────────────────────────────

/// Errors returned by the sync layer.
///
/// Kept as a concrete `DomainError` mapping so callers see a single error
/// type end-to-end: transport and serialization failures become
/// [`DomainError::Io`], missing keys become [`DomainError::InvalidPath`].
#[derive(Debug)]
pub enum SyncError {
    /// No API key is configured; sync cannot start.
    MissingApiKey,
    /// An event failed to serialize (should not happen for our types).
    Serialize(String),
    /// The transport rejected a publish or receive operation.
    Transport(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "sync requires an Ably API key"),
            Self::Serialize(msg) => write!(f, "sync serialize error: {msg}"),
            Self::Transport(msg) => write!(f, "sync transport error: {msg}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<SyncError> for DomainError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::MissingApiKey => DomainError::InvalidPath(err.to_string()),
            SyncError::Serialize(_) | SyncError::Transport(_) => {
                DomainError::Io(std::io::Error::other(err.to_string()))
            }
        }
    }
}

// ── Event types ───────────────────────────────────────────────────────────

/// A per-file change event streamed over the sync channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// A file/directory was created or changed. `mtime` is the local
    /// last-modified time at the moment of the change.
    Write {
        /// Absolute path of the changed entry.
        path: String,
        /// Size in bytes at the moment of the change.
        size: u64,
        /// Unix timestamp (seconds) at the moment of the change.
        mtime: u64,
        /// Coarse classification of the entry.
        file_type: FileType,
    },
    /// A deletion tombstone. `mtime` is the local mtime of the entry at
    /// delete time; a tombstone always beats an older write.
    Delete {
        /// Absolute path of the deleted entry.
        path: String,
        /// Unix timestamp (seconds) of the deleted entry's mtime.
        mtime: u64,
    },
}

impl SyncEvent {
    /// The path this event refers to.
    pub fn path(&self) -> &str {
        match self {
            Self::Write { path, .. } | Self::Delete { path, .. } => path,
        }
    }

    /// The event timestamp (mtime), used for LWW conflict resolution.
    pub fn mtime(&self) -> u64 {
        match self {
            Self::Write { mtime, .. } | Self::Delete { mtime, .. } => *mtime,
        }
    }
}

// ── Transport port ────────────────────────────────────────────────────────

/// Port for the underlying sync channel.
///
/// Implementations deliver events to a logical channel for a scan root and
/// expose the events previously published to it. This is what lets the
/// conflict-resolution logic be unit-tested without a network.
pub trait SyncTransport: Send + Sync {
    /// Publish `event` on the channel for `root`. Returns
    /// [`SyncError::Transport`] on backend failure.
    fn publish(&self, root: &str, event: &SyncEvent) -> Result<(), SyncError>;

    /// Events previously published to the channel for `root`, in publish
    /// order. Used to replay history into the local tree.
    fn history(&self, root: &str) -> Result<Vec<SyncEvent>, SyncError>;
}
