//! Embedded `redb` cache for incremental scan reuse.
//!
//! [`RedbCache`] persists the last-seen `ScanResult` per scan root
//! along with the root's mtime and total byte count; a follow-up scan
//! reuses the cached tree when both match.
//!
//! The on-disk value is a JSON-serialised envelope holding the
//! `(mtime, size, ScanResult)` triple. `ScanResult` is mirrored by
//! `CachedScanResult` (a serde-friendly view that converts to and
//! from `domain::ScanResult`) so the `domain` crate can stay
//! zero-dep.
//!
//! The `default` constructor builds an in-memory database (suitable for
//! tests and short-lived CLI invocations); [`RedbCache::open`] creates a
//! persistent database at the given path (used by [`crate::ScanService`]
//! when a cache file is desired).

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use redb::{Database, ReadableDatabase, TableDefinition};
use serde::{Deserialize, Serialize};

use domain::{DomainError, FileNode, FileType, ScanResult};

/// Single table mapping a path string to a JSON-serialised
/// [`CachedScanResult`] blob.
const SCANS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("scans");

/// JSON-friendly cache envelope: stores the result tree plus the
/// `(mtime, size)` pair used for incremental invalidation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedScanResult {
    /// Last-modified time of the scan root, in Unix seconds.
    mtime: u64,
    /// Recursive total byte count for the scan root.
    size: u64,
    /// Serialised root tree.
    root: CachedNode,
}

/// JSON-friendly mirror of [`domain::FileNode`]. We mirror instead of
/// deriving `Serialize` on the domain type so the domain crate stays
/// zero-dep.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedNode {
    path: String,
    size: u64,
    modified: u64,
    file_type: u8,
    children: Vec<CachedNode>,
}

impl CachedNode {
    fn from_domain(node: &FileNode) -> Self {
        Self {
            path: node.path.clone(),
            size: node.size,
            modified: node.modified,
            file_type: file_type_to_byte(node.file_type),
            children: node.children.iter().map(Self::from_domain).collect(),
        }
    }

    fn into_domain(self) -> FileNode {
        FileNode {
            path: self.path,
            size: self.size,
            modified: self.modified,
            file_type: byte_to_file_type(self.file_type),
            children: self.children.into_iter().map(Self::into_domain).collect(),
        }
    }
}

fn file_type_to_byte(ft: FileType) -> u8 {
    match ft {
        FileType::Audio => 0,
        FileType::Video => 1,
        FileType::Image => 2,
        FileType::Document => 3,
        FileType::Code => 4,
        FileType::Archive => 5,
        FileType::Directory => 6,
        FileType::Other => 7,
    }
}

fn byte_to_file_type(b: u8) -> FileType {
    match b {
        0 => FileType::Audio,
        1 => FileType::Video,
        2 => FileType::Image,
        3 => FileType::Document,
        4 => FileType::Code,
        5 => FileType::Archive,
        6 => FileType::Directory,
        _ => FileType::Other,
    }
}

/// Embedded cache backend. Cheap to clone (internally `Arc`'d).
#[derive(Debug, Clone)]
pub struct RedbCache {
    inner: Arc<CacheInner>,
}

#[derive(Debug)]
struct CacheInner {
    /// Backing database. When `path` is `None`, this is an in-memory DB.
    db: Database,
    /// When `Some`, the filesystem path backing the DB; exposed for
    /// tests and diagnostics.
    #[allow(dead_code)]
    path: Option<std::path::PathBuf>,
    /// Serialises exclusive writers — redb itself enforces a single
    /// write txn, but we wrap for clarity.
    write_lock: Mutex<()>,
}

impl RedbCache {
    /// Open an in-memory cache (no filesystem effects, no persistence).
    pub fn new() -> Self {
        let db = Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .expect("in-memory redb creation never fails");
        Self {
            inner: Arc::new(CacheInner {
                db,
                path: None,
                write_lock: Mutex::new(()),
            }),
        }
    }

    /// Open or create a persistent cache at `cache_path`.
    pub fn open(cache_path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let db = Database::create(cache_path.as_ref()).map_err(map_db_error)?;
        Ok(Self {
            inner: Arc::new(CacheInner {
                db,
                path: Some(cache_path.as_ref().to_path_buf()),
                write_lock: Mutex::new(()),
            }),
        })
    }

}

impl Default for RedbCache {
    fn default() -> Self {
        Self::new()
    }
}

impl domain::ports::Cache for RedbCache {
    fn get(&self, path: &str) -> Option<ScanResult> {
        self.get_with_metadata(path).map(|(r, _, _)| r)
    }

    fn put(&self, path: &str, result: &ScanResult) -> Result<(), DomainError> {
        self.put_with_metadata(path, result, 0, 0)
    }

    fn invalidate(&self, path: &str) -> Result<(), DomainError> {
        let _guard = self.inner.write_lock.lock();
        let write_txn = self.inner.db.begin_write().map_err(map_txn_error)?;
        {
            let mut table = write_txn.open_table(SCANS_TABLE).map_err(map_table_error)?;
            table.remove(path).map_err(map_storage_error)?;
        }
        write_txn.commit().map_err(map_commit_error)?;
        Ok(())
    }

    fn get_with_metadata(&self, path: &str) -> Option<(ScanResult, u64, u64)> {
        let read_txn = self.inner.db.begin_read().ok()?;
        let table = read_txn.open_table(SCANS_TABLE).ok()?;
        let guard = table.get(path).ok()?;
        let value = guard?;
        let bytes = value.value();
        let cached: CachedScanResult = serde_json::from_slice(bytes).ok()?;
        let root = cached.root.into_domain();
        let mut result = ScanResult {
            root,
            total_size: 0,
            file_count: 0,
            scan_duration_ms: 0,
            skipped: Vec::new(),
        };
        result.total_size = result.root.total_size();
        result.file_count = result.root.file_count();
        Some((result, cached.mtime, cached.size))
    }

    fn put_with_metadata(
        &self,
        path: &str,
        result: &ScanResult,
        mtime: u64,
        size: u64,
    ) -> Result<(), DomainError> {
        let cached = CachedScanResult {
            mtime,
            size,
            root: CachedNode::from_domain(&result.root),
        };
        let bytes = serde_json::to_vec(&cached).map_err(|e| {
            DomainError::Io(std::io::Error::other( e.to_string()))
        })?;
        let _guard = self.inner.write_lock.lock();
        let write_txn = self.inner.db.begin_write().map_err(map_txn_error)?;
        {
            let mut table = write_txn.open_table(SCANS_TABLE).map_err(map_table_error)?;
            table.insert(path, bytes.as_slice()).map_err(map_storage_error)?;
        }
        write_txn.commit().map_err(map_commit_error)?;
        Ok(())
    }
}

// ── Error mapping ──────────────────────────────────────────────────────────

fn map_db_error(e: redb::DatabaseError) -> DomainError {
    DomainError::Io(std::io::Error::other( e.to_string()))
}

fn map_txn_error(e: redb::TransactionError) -> DomainError {
    DomainError::Io(std::io::Error::other( e.to_string()))
}

fn map_table_error(e: redb::TableError) -> DomainError {
    DomainError::Io(std::io::Error::other( e.to_string()))
}

fn map_storage_error(e: redb::StorageError) -> DomainError {
    DomainError::Io(std::io::Error::other( e.to_string()))
}

fn map_commit_error(e: redb::CommitError) -> DomainError {
    DomainError::Io(std::io::Error::other( e.to_string()))
}
