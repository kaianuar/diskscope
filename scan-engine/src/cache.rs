//! Embedded `redb` cache for incremental scan reuse.
//!
//! [`RedbCache`] persists the last-seen `ScanResult` per scan root
//! along with the root's mtime and total byte count; a follow-up scan
//! reuses the cached tree when both match.
//!
//! The on-disk value is a flat node list (path + parent index +
//! metadata) packed by a hand-rolled binary codec. We avoid pulling
//! `serde` into the `domain` crate to keep it zero-dep; the cache
//! flat node type is defined here and is plain `Vec<FlatNode>` plus
//! two `u64`s for mtime and size.
//! The `default` constructor builds an in-memory database (suitable for
//! tests and short-lived CLI invocations); [`RedbCache::open`] creates a
//! persistent database at the given path (used by [`crate::ScanService`]
//! when a cache file is desired).

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use domain::{DomainError, FileNode, FileType, ScanResult};

/// Single table mapping a path string to a serialised cache blob.
const SCANS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("scans");

/// Flat representation of a single node used for cache storage and
/// round-tripping. The tree is reconstructed from these entries on
/// load by following `parent_idx` pointers.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatNode {
    /// `0` for the root; otherwise the index of the parent in the flat
    /// list. Indices are dense — every node keeps the index of every
    /// node that contains it in the tree.
    pub parent_idx: u32,
    /// `FileType` discriminant (matches `FileType` byte order).
    pub file_type: u8,
    /// On-disk byte count for files; 0 for directories.
    pub size: u64,
    /// Last-modified time in Unix seconds.
    pub modified: u64,
    /// Absolute path string.
    pub path: String,
}

impl FlatNode {
    /// Convert a [`FileNode`] tree to a flat list via DFS. Returns the
    /// list and the root's index (always `0` in our use).
    pub fn flatten(root: &FileNode) -> Vec<FlatNode> {
        let mut out = Vec::new();
        flatten_into(root, u32::MAX, &mut out);
        out
    }

    /// Rebuild a [`FileNode`] tree from a flat list. Returns `None` if
    /// the list is empty or the data is malformed.
    pub fn rebuild(nodes: Vec<FlatNode>) -> Option<FileNode> {
        if nodes.is_empty() {
            return None;
        }
        // The root is the node with parent_idx == u32::MAX (we used
        // that sentinel when flattening).
        let root_idx = nodes
            .iter()
            .position(|n| n.parent_idx == u32::MAX)
            .unwrap_or(0);
        let root_flat = nodes[root_idx].clone();
        let mut children_per_idx: std::collections::HashMap<u32, Vec<u32>> =
            std::collections::HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            if n.parent_idx != u32::MAX {
                children_per_idx
                    .entry(n.parent_idx)
                    .or_default()
                    .push(i as u32);
            }
        }
        let mut root = FileNode {
            path: root_flat.path,
            size: root_flat.size,
            modified: root_flat.modified,
            file_type: FileType::Directory,
            children: Vec::new(),
        };
        rebuild_children(&nodes, &children_per_idx, root_idx as u32, &mut root.children);
        Some(root)
    }
}

fn flatten_into(node: &FileNode, parent_idx: u32, out: &mut Vec<FlatNode>) {
    let my_idx = out.len() as u32;
    out.push(FlatNode {
        parent_idx,
        file_type: file_type_to_byte(node.file_type),
        size: node.size,
        modified: node.modified,
        path: node.path.clone(),
    });
    let _ = my_idx; // index is positional; future hook for invariants.
    for child in &node.children {
        flatten_into(child, my_idx, out);
    }
}

fn rebuild_children(
    nodes: &[FlatNode],
    children_per_idx: &std::collections::HashMap<u32, Vec<u32>>,
    parent_idx: u32,
    out: &mut Vec<FileNode>,
) {
    if let Some(child_idxs) = children_per_idx.get(&parent_idx) {
        for &i in child_idxs {
            let flat = &nodes[i as usize];
            let mut child = FileNode {
                path: flat.path.clone(),
                size: flat.size,
                modified: flat.modified,
                file_type: byte_to_file_type(flat.file_type),
                children: Vec::new(),
            };
            rebuild_children(nodes, children_per_idx, i, &mut child.children);
            out.push(child);
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

    /// Look up `path` and return the cached entry with its stored
    /// mtime / size metadata. Used by the incremental-scan check.
    pub fn get_with_metadata(&self, path: &str) -> Option<(ScanResult, u64, u64)> {
        let read_txn = self.inner.db.begin_read().ok()?;
        let table = read_txn.open_table(SCANS_TABLE).ok()?;
        let guard = table.get(path).ok()?;
        let bytes = guard?.value();
        let (nodes, mtime, size) = decode(bytes).ok()?;
        let root = FlatNode::rebuild(nodes)?;
        let result = ScanResult {
            root,
            total_size: 0,
            file_count: 0,
            scan_duration_ms: 0,
        };
        let mut result = result;
        result.total_size = result.root.total_size();
        result.file_count = result.root.file_count();
        Some((result, mtime, size))
    }

    /// Persist `result` along with `mtime` and `size` for `path`.
    /// Companion to [`Self::get_with_metadata`].
    pub fn put_with_metadata(
        &self,
        path: &str,
        result: &ScanResult,
        mtime: u64,
        size: u64,
    ) -> Result<(), DomainError> {
        let nodes = FlatNode::flatten(&result.root);
        let bytes = encode(&nodes, mtime, size);
        let _guard = self.inner.write_lock.lock();
        let write_txn = self.inner.db.begin_write().map_err(map_txn_error)?;
        {
            let mut table = write_txn.open_table(SCANS_TABLE).map_err(map_table_error)?;
            table.insert(path, bytes.as_slice()).map_err(map_storage_error)?;
        }
        write_txn.commit().map_err(map_commit_error)?;
        Ok(())
    }

    /// Exact-paths-only prefix delete. Drops every cache entry whose
    /// key starts with `prefix`. Used by the incremental scan when a
    /// subtree has been invalidated.
    pub fn invalidate_prefix(&self, prefix: &str) -> Result<usize, DomainError> {
        let _guard = self.inner.write_lock.lock();
        let write_txn = self.inner.db.begin_write().map_err(map_txn_error)?;
        let mut removed = 0usize;
        {
            let mut table = write_txn.open_table(SCANS_TABLE).map_err(map_table_error)?;
            // extract_if walks the B-tree and removes matching entries
            // atomically. We clone matching keys out of the iterator so
            // we can count them after the loop drops the borrow.
            let keys: Vec<String> = {
                let iter = table
                    .extract_if(|key, _val| key.starts_with(prefix))
                    .map_err(map_storage_error)?;
                iter.filter_map(|r| r.ok())
                    .map(|(k, _v)| k.value().to_string())
                    .collect()
            };
            removed = keys.len();
        }
        write_txn.commit().map_err(map_commit_error)?;
        Ok(removed)
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
        // Default `put` (port-level) stores no metadata — incremental
        // checks via `get_with_metadata` will treat entries this way
        // as "no mtime to compare", forcing a rescan.
        let mtime = 0u64;
        let size = 0u64;
        self.put_with_metadata(path, result, mtime, size)
    }

    fn invalidate(&self, path: &str) -> Result<(), DomainError> {
        let _guard = self.inner.write_lock.lock();
        let write_txn = self.inner.db.begin_write().map_err(map_txn_error)?;
        {
            let mut table = write_txn.open_table(SCANS_TABLE).map_err(map_table_error)?;
            table.remove(path).map_err(map_table_error)?;
        }
        write_txn.commit().map_err(map_commit_error)?;
        Ok(())
    }
}

// ── Tiny binary codec (no serde dep) ──────────────────────────────────────

fn encode(nodes: &[FlatNode], mtime: u64, size: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + nodes.len() * 48);
    out.extend_from_slice(&mtime.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    encode_vec(nodes, &mut out);
    out
}

fn encode_vec(v: &[FlatNode], out: &mut Vec<u8>) {
    out.extend_from_slice(&(v.len() as u64).to_le_bytes());
    for n in v {
        out.extend_from_slice(&n.parent_idx.to_le_bytes());
        out.push(n.file_type);
        out.extend_from_slice(&n.size.to_le_bytes());
        out.extend_from_slice(&n.modified.to_le_bytes());
        encode_string(&n.path, out);
    }
}

fn encode_string(s: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(&(s.len() as u64).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn decode(bytes: &[u8]) -> Result<(Vec<FlatNode>, u64, u64), DomainError> {
    let mut cursor = Cursor::new(bytes);
    let mtime = cursor.read_u64()?;
    let size = cursor.read_u64()?;
    let nodes = cursor.read_vec()?;
    Ok((nodes, mtime, size))
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_u64(&mut self) -> Result<u64, DomainError> {
        if self.pos + 8 > self.buf.len() {
            return Err(io_err("u64 out of bounds"));
        }
        let v = u64::from_le_bytes(self.buf[self.pos..self.pos + 8].try_into().map_err(|_| {
            io_err("u64 slice conversion")
        })?);
        self.pos += 8;
        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32, DomainError> {
        if self.pos + 4 > self.buf.len() {
            return Err(io_err("u32 out of bounds"));
        }
        let v = u32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().map_err(|_| {
            io_err("u32 slice conversion")
        })?);
        self.pos += 4;
        Ok(v)
    }

    fn read_u8(&mut self) -> Result<u8, DomainError> {
        if self.pos + 1 > self.buf.len() {
            return Err(io_err("u8 out of bounds"));
        }
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_string(&mut self) -> Result<String, DomainError> {
        let len = self.read_u64()? as usize;
        if self.pos + len > self.buf.len() {
            return Err(io_err("string out of bounds"));
        }
        let s = std::str::from_utf8(&self.buf[self.pos..self.pos + len])
            .map_err(|e| io_err(&format!("invalid utf8: {e}")))?
            .to_string();
        self.pos += len;
        Ok(s)
    }

    fn read_vec(&mut self) -> Result<Vec<FlatNode>, DomainError> {
        let len = self.read_u64()? as usize;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(FlatNode {
                parent_idx: self.read_u32()?,
                file_type: self.read_u8()?,
                size: self.read_u64()?,
                modified: self.read_u64()?,
                path: self.read_string()?,
            });
        }
        Ok(out)
    }
}

fn io_err(msg: &str) -> DomainError {
    DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, msg.to_string()))
}
// ── Error mapping ──────────────────────────────────────────────────────────

impl From<redb::StorageError> for DomainError {
    fn from(e: redb::StorageError) -> Self {
        DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }
}

fn map_db_error(e: redb::DatabaseError) -> DomainError {
    DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn map_txn_error(e: redb::TransactionError) -> DomainError {
    DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn map_table_error(e: redb::TableError) -> DomainError {
    DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn map_storage_error(e: redb::StorageError) -> DomainError {
    DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn map_commit_error(e: redb::CommitError) -> DomainError {
    DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}
