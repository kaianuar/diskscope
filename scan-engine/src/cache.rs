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
        Self { inner: Arc::new(CacheInner { db, path: None, write_lock: Mutex::new(()) }) }
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
        let cached = CachedScanResult { mtime, size, root: CachedNode::from_domain(&result.root) };
        let bytes = serde_json::to_vec(&cached)
            .map_err(|e| DomainError::Io(std::io::Error::other(e.to_string())))?;
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
    DomainError::Io(std::io::Error::other(e.to_string()))
}

fn map_txn_error(e: redb::TransactionError) -> DomainError {
    DomainError::Io(std::io::Error::other(e.to_string()))
}

fn map_table_error(e: redb::TableError) -> DomainError {
    DomainError::Io(std::io::Error::other(e.to_string()))
}

fn map_storage_error(e: redb::StorageError) -> DomainError {
    DomainError::Io(std::io::Error::other(e.to_string()))
}

fn map_commit_error(e: redb::CommitError) -> DomainError {
    DomainError::Io(std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ports::Cache;

    /// A three-level tree mirroring the domain tests' `sample_tree`:
    /// `/project` (Directory) → `/project/src` (Directory) →
    /// `/project/src/main.rs` (Code), plus `/project/README.md` (Document).
    /// Directory sizes are the recursive sum of descendants, matching the
    /// scanner's `normalize_sizes` invariant.
    fn sample_result() -> ScanResult {
        ScanResult::from_tree(
            FileNode {
                path: "/project".into(),
                size: 1500,
                modified: 100,
                file_type: FileType::Directory,
                children: vec![
                    FileNode {
                        path: "/project/src".into(),
                        size: 1000,
                        modified: 90,
                        file_type: FileType::Directory,
                        children: vec![FileNode {
                            path: "/project/src/main.rs".into(),
                            size: 1000,
                            modified: 80,
                            file_type: FileType::Code,
                            children: vec![],
                        }],
                    },
                    FileNode {
                        path: "/project/README.md".into(),
                        size: 500,
                        modified: 70,
                        file_type: FileType::Document,
                        children: vec![],
                    },
                ],
            },
            0,
        )
    }

    #[test]
    fn should_return_none_when_get_called_on_empty_cache() {
        // Arrange
        let cache = RedbCache::new();

        // Act
        let got = cache.get("/project");

        // Assert
        assert_eq!(got, None);
    }

    #[test]
    fn should_return_stored_result_when_put_then_get() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();
        cache.put("/project", &result).unwrap();

        // Act
        let got = cache.get("/project").unwrap();

        // Assert
        assert_eq!(got.root, result.root);
    }

    #[test]
    fn should_return_none_after_invalidate_when_entry_removed() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();
        cache.put("/project", &result).unwrap();
        assert!(cache.get("/project").is_some());

        // Act
        cache.invalidate("/project").unwrap();

        // Assert
        assert_eq!(cache.get("/project"), None);
    }

    #[test]
    fn should_silently_succeed_when_invalidate_missing_key() {
        // Arrange
        let cache = RedbCache::new();

        // Act
        let result = cache.invalidate("/never-stored");

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn should_overwrite_when_put_called_twice_for_same_key() {
        // Arrange
        let cache = RedbCache::new();
        let first = sample_result();
        let second = ScanResult::from_tree(
            FileNode {
                path: "/project".into(),
                size: 10,
                modified: 5,
                file_type: FileType::Directory,
                children: vec![FileNode {
                    path: "/project/notes.txt".into(),
                    size: 10,
                    modified: 5,
                    file_type: FileType::Other,
                    children: vec![],
                }],
            },
            0,
        );
        cache.put("/project", &first).unwrap();

        // Act
        cache.put("/project", &second).unwrap();

        // Assert
        assert_eq!(cache.get("/project").unwrap().root, second.root);
    }

    #[test]
    fn should_store_and_retrieve_metadata_when_put_with_metadata() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();

        // Act
        cache.put_with_metadata("/project", &result, 100, 200).unwrap();
        let got = cache.get_with_metadata("/project").unwrap();

        // Assert
        assert_eq!(got, (result, 100, 200));
    }

    #[test]
    fn should_return_zero_metadata_when_put_without_metadata() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();
        cache.put("/project", &result).unwrap();

        // Act
        let (_, mtime, size) = cache.get_with_metadata("/project").unwrap();

        // Assert
        assert_eq!((mtime, size), (0, 0));
    }

    #[test]
    fn should_round_trip_file_types_when_cached() {
        // Arrange
        let cache = RedbCache::new();
        let result = ScanResult::from_tree(
            FileNode {
                path: "/media".into(),
                size: 3000,
                modified: 0,
                file_type: FileType::Directory,
                children: vec![
                    FileNode {
                        path: "/media/song.mp3".into(),
                        size: 1000,
                        modified: 0,
                        file_type: FileType::Audio,
                        children: vec![],
                    },
                    FileNode {
                        path: "/media/clip.mp4".into(),
                        size: 2000,
                        modified: 0,
                        file_type: FileType::Video,
                        children: vec![],
                    },
                ],
            },
            0,
        );
        cache.put("/media", &result).unwrap();

        // Act
        let got = cache.get("/media").unwrap();

        // Assert
        assert_eq!(got.root, result.root);
    }

    #[test]
    fn should_round_trip_nested_tree_when_cached() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();
        cache.put("/project", &result).unwrap();

        // Act
        let got = cache.get("/project").unwrap();

        // Assert
        assert_eq!(got.root, result.root);
    }

    #[test]
    fn should_preserve_total_size_when_round_tripped() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();
        cache.put("/project", &result).unwrap();

        // Act
        let got = cache.get("/project").unwrap();

        // Assert
        assert_eq!(got.total_size, result.total_size);
    }

    #[test]
    fn should_preserve_file_count_when_round_tripped() {
        // Arrange
        let cache = RedbCache::new();
        let result = sample_result();
        cache.put("/project", &result).unwrap();

        // Act
        let got = cache.get("/project").unwrap();

        // Assert
        assert_eq!(got.file_count, result.file_count);
    }
}
