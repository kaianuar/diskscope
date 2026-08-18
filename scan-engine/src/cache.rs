use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use redb::{Database, ReadableTable, TableDefinition};

use crate::domain::{FileNode, FileTree, NodeKind};
use crate::CachedEntry;

const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("entries");

/// Persistent snapshot cache backed by an embedded `redb` database.
///
/// Schema: table `entries` keyed by absolute path.
/// Value layout v2 (binary, little-endian):
///   [0..8]   size       u64
///   [8..16]  mtime      u64  (seconds since UNIX epoch)
///   [16..24] scan_time  u64  (seconds since UNIX epoch)
///   [24]     kind       u8   (0=File, 1=Directory, 2=Symlink)
///   [25..27] name_len   u16
///   [27..]   name       UTF-8
///
/// Legacy v1 entries (19+ bytes, no scan_time) are read transparently.
pub struct Cache {
    db: Database,
    root: PathBuf,
}

impl Cache {
    /// Open or create a cache database at `path`.
    ///
    /// The database file is created (along with parent directories) if it does
    /// not exist. `root` is the scan root used to reconstruct relative paths
    /// when loading a snapshot.
    pub fn open(path: &Path, root: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let db = Database::create(path)
            .map_err(|e| std::io::Error::other(format!("redb open: {e}")))?;
        Ok(Self {
            db,
            root: root.to_path_buf(),
        })
    }

    /// Persist the entire `FileTree` as a snapshot.
    ///
    /// Each node is keyed by its absolute path. The tree is reconstructed
    /// on [`load`] by sorting paths and attaching children to parents.
    pub fn store(&self, tree: &FileTree) -> std::io::Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|e| std::io::Error::other(format!("redb write txn: {e}")))?;

        {
            let mut table = txn
                .open_table(TABLE)
                .map_err(|e| std::io::Error::other(format!("redb open table: {e}")))?;

            for node in tree.flatten() {
                let key = node.path.to_string_lossy();
                let name_bytes = node.name.as_bytes();
                let kind_byte = match node.kind {
                    NodeKind::File => 0u8,
                    NodeKind::Directory => 1u8,
                    NodeKind::Symlink => 2u8,
                };
                let mtime = node
                    .modified
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let mut val = Vec::with_capacity(27 + name_bytes.len());
                val.extend_from_slice(&node.size.to_le_bytes());
                val.extend_from_slice(&mtime.to_le_bytes());
                val.extend_from_slice(&0u64.to_le_bytes()); // scan_time (unused for tree snapshots)
                val.push(kind_byte);
                val.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
                val.extend_from_slice(name_bytes);

                table
                    .insert(key.as_ref(), val.as_slice())
                    .map_err(|e| std::io::Error::other(format!("redb insert: {e}")))?;
            }


        }

        txn.commit()
            .map_err(|e| std::io::Error::other(format!("redb commit: {e}")))?;
        Ok(())
    }

    /// Restore the last snapshot from the database.
    ///
    /// Returns `Err` if no snapshot exists. The tree is rebuilt by sorting
    /// all stored paths and attaching children to parents (parents always
    /// sort before their children).
    pub fn load(&self) -> std::io::Result<FileTree> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| std::io::Error::other(format!("redb read txn: {e}")))?;

        let table = txn
            .open_table(TABLE)
            .map_err(|e| std::io::Error::other(format!("redb open table: {e}")))?;

        let root_str = self.root.to_string_lossy();
        let root_len = root_str.len();

        // Collect all real entries (skip the __root__ marker).
        let mut entries: Vec<(PathBuf, FileNode)> = Vec::new();
        let iter = table
            .iter()
            .map_err(|e| std::io::Error::other(format!("redb iter: {e}")))?;

        for row in iter {
            let row = row.map_err(|e| std::io::Error::other(format!("redb row: {e}")))?;
            let key = row.0.value();
            let val = row.1.value();

            let mut node = Self::deserialize_node(&val)?;
            node.path = PathBuf::from(key);
            entries.push((node.path.clone(), node));
        }

        if entries.is_empty() {
            return Err(std::io::Error::other("cache is empty"));
        }

        // Sort by path so parents come before children.
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut map: std::collections::HashMap<PathBuf, FileNode> =
            std::collections::HashMap::with_capacity(entries.len());

        for (path, node) in entries {
            if path == self.root {
                map.insert(path, node);
                continue;
            }

            let path_str = path.to_string_lossy();
            if path_str.len() > root_len && path_str.starts_with(root_str.as_ref()) {
                // Find parent by stripping last component.
                if let Some(parent_path) = path.parent() {
                    if let Some(parent) = map.get_mut(parent_path) {
                        parent.children.push(node);
                        continue;
                    }
                }
            }

            // Fallback: insert as standalone (orphaned entry).
            map.insert(path, node);
        }

        let root = map
            .remove(&self.root)
            .ok_or_else(|| std::io::Error::other("no root entry in cache"))?;

        let total_size = root.total_size();
        let flat = Self::count_all(&root);

        Ok(FileTree {
            root,
            total_size,
            file_count: flat.0,
            dir_count: flat.1,
        })
    }

    /// Check whether the cached metadata for `entry` differs from its current
    /// on-disk state (mtime or size changed, or file removed).
    pub fn is_stale(&self, entry: &FileNode) -> bool {
        match fs::metadata(&entry.path) {
            Ok(meta) => {
                let current_mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let cached_mtime = entry
                    .modified
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                meta.len() != entry.size || current_mtime != cached_mtime
            }
            Err(_) => true,
        }
    }

    // ── Port trait helpers ─────────────────────────────────────────────────

    /// Look up a single cached entry by path.
    pub fn get_entry(&self, path: &Path) -> Option<CachedEntry> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TABLE).ok()?;
        let key = path.to_string_lossy();
        let val = table.get(key.as_ref()).ok()??;
        let bytes = val.value();
        Self::deserialize_cached_entry(path, bytes)
    }

    /// Store a single entry in the cache (v2 format with scan_time).
    pub fn put_entry(&self, entry: &CachedEntry) -> std::io::Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|e| std::io::Error::other(format!("redb write txn: {e}")))?;

        {
            let mut table = txn
                .open_table(TABLE)
                .map_err(|e| std::io::Error::other(format!("redb open table: {e}")))?;

            let key = entry.entry.path.to_string_lossy();
            let name_bytes = entry.entry.name.as_bytes();
            let kind_byte: u8 = match entry.entry.node_type {
                crate::NodeType::File => 0,
                crate::NodeType::Dir => 1,
                crate::NodeType::Symlink => 2,
            };

            let mut val = Vec::with_capacity(27 + name_bytes.len());
            val.extend_from_slice(&entry.entry.size.to_le_bytes());
            val.extend_from_slice(&entry.entry.modified.to_le_bytes());
            val.extend_from_slice(&entry.scan_time.to_le_bytes());
            val.push(kind_byte);
            val.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            val.extend_from_slice(name_bytes);

            table
                .insert(key.as_ref(), val.as_slice())
                .map_err(|e| std::io::Error::other(format!("redb insert: {e}")))?;
        }

        txn.commit()
            .map_err(|e| std::io::Error::other(format!("redb commit: {e}")))
    }

    // ── Private ────────────────────────────────────────────────────────────

    fn deserialize_node(bytes: &[u8]) -> std::io::Result<FileNode> {
        if bytes.len() < 19 {
            return Err(std::io::Error::other("cache entry too short"));
        }

        // Try v2 format first: [size:8][mtime:8][scan_time:8][kind:1][name_len:2][name:N]
        if bytes.len() >= 27 {
            let kind_byte = bytes[24];
            if kind_byte <= 2 {
                let name_len = u16::from_le_bytes(bytes[25..27].try_into().unwrap()) as usize;
                if 27 + name_len == bytes.len() {
                    let size = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                    let mtime = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                    let kind = match kind_byte {
                        0 => NodeKind::File,
                        1 => NodeKind::Directory,
                        2 => NodeKind::Symlink,
                        _ => unreachable!(),
                    };
                    let name = std::str::from_utf8(&bytes[27..27 + name_len])
                        .map_err(|e| std::io::Error::other(format!("invalid utf8 name: {e}")))?;
                    return Ok(FileNode {
                        name: name.to_string(),
                        path: PathBuf::new(),
                        size,
                        modified: UNIX_EPOCH + Duration::from_secs(mtime),
                        kind,
                        children: Vec::new(),
                    });
                }
            }
        }

        // Fall back to v1 format: [size:8][mtime:8][kind:1][name_len:2][name:N]
        let kind = match bytes[16] {
            0 => NodeKind::File,
            1 => NodeKind::Directory,
            2 => NodeKind::Symlink,
            _ => return Err(std::io::Error::other("invalid node kind")),
        };
        let name_len = u16::from_le_bytes(bytes[17..19].try_into().unwrap()) as usize;
        if bytes.len() < 19 + name_len {
            return Err(std::io::Error::other("cache entry name truncated"));
        }
        let name = std::str::from_utf8(&bytes[19..19 + name_len])
            .map_err(|e| std::io::Error::other(format!("invalid utf8 name: {e}")))?;

        let size = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let mtime = u64::from_le_bytes(bytes[8..16].try_into().unwrap());

        Ok(FileNode {
            name: name.to_string(),
            path: PathBuf::new(),
            size,
            modified: UNIX_EPOCH + Duration::from_secs(mtime),
            kind,
            children: Vec::new(),
        })
    }

    /// Deserialize a `CachedEntry` from raw bytes, supporting both v1 and v2 formats.
    fn deserialize_cached_entry(path: &Path, bytes: &[u8]) -> Option<CachedEntry> {
        if bytes.len() < 19 {
            return None;
        }

        // Try v2: [size:8][mtime:8][scan_time:8][kind:1][name_len:2][name:N]
        if bytes.len() >= 27 {
            let kind_byte = bytes[24];
            if kind_byte <= 2 {
                let name_len = u16::from_le_bytes(bytes[25..27].try_into().unwrap()) as usize;
                if 27 + name_len == bytes.len() {
                    let size = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                    let modified = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                    let scan_time = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
                    let node_type = match kind_byte {
                        0 => crate::NodeType::File,
                        1 => crate::NodeType::Dir,
                        2 => crate::NodeType::Symlink,
                        _ => crate::NodeType::File,
                    };
                    let name = std::str::from_utf8(&bytes[27..27 + name_len])
                        .ok()?
                        .to_string();
                    return Some(CachedEntry {
                        entry: crate::FileEntry {
                            path: path.to_path_buf(),
                            name,
                            size,
                            modified,
                            node_type,
                            depth: 0,
                        },
                        scan_time,
                    });
                }
            }
        }

        // Fall back to v1: [size:8][mtime:8][kind:1][name_len:2][name:N]
        let kind_byte = bytes[16];
        if kind_byte > 2 {
            return None;
        }
        let name_len = u16::from_le_bytes(bytes[17..19].try_into().unwrap()) as usize;
        if bytes.len() < 19 + name_len {
            return None;
        }
        let size = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let modified = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let node_type = match kind_byte {
            0 => crate::NodeType::File,
            1 => crate::NodeType::Dir,
            2 => crate::NodeType::Symlink,
            _ => crate::NodeType::File,
        };
        let name = std::str::from_utf8(&bytes[19..19 + name_len])
            .ok()?
            .to_string();
        Some(CachedEntry {
            entry: crate::FileEntry {
                path: path.to_path_buf(),
                name,
                size,
                modified,
                node_type,
                depth: 0,
            },
            scan_time: modified, // v1 fallback: derive from mtime
        })
    }

    fn count_all(node: &FileNode) -> (usize, usize) {
        let mut files = 0usize;
        let mut dirs = 0usize;
        match node.kind {
            NodeKind::File => files += 1,
            NodeKind::Directory => dirs += 1,
            NodeKind::Symlink => files += 1,
        }
        for child in &node.children {
            let (f, d) = Self::count_all(child);
            files += f;
            dirs += d;
        }
        (files, dirs)
    }
}

impl crate::ports::Cache for Cache {
    fn get(&self, path: &Path) -> Option<CachedEntry> {
        self.get_entry(path)
    }

    fn put(&self, entry: &CachedEntry) {
        let _ = self.put_entry(entry);
    }

    fn evict_stale(&self, root: &Path) -> usize {
        let txn = match self.db.begin_write() {
            Ok(t) => t,
            Err(_) => return 0,
        };
        let mut evicted = 0usize;
        let root_str = root.to_string_lossy().into_owned();

        {
            let mut table = match txn.open_table(TABLE) {
                Ok(t) => t,
                Err(_) => return 0,
            };

            let keys: Vec<String> = match table.iter() {
                Ok(iter) => iter
                    .filter_map(|row| {
                        let row = row.ok()?;
                        let key = row.0.value();
                        // Scope to root: skip entries outside this root.
                        if !key.starts_with(root_str.as_str()) {
                            return None;
                        }
                        let val = row.1.value();
                        let node = Self::deserialize_node(&val).ok()?;
                        let path = PathBuf::from(&key);
                        let mtime = node
                            .modified
                            .duration_since(UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        match fs::metadata(&path) {
                            Ok(meta) => {
                                let current = meta
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                if meta.len() != node.size || current != mtime {
                                    Some(key.to_string())
                                } else {
                                    None
                                }
                            }
                            Err(_) => Some(key.to_string()),
                        }
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };

            for key in &keys {
                if table.remove(key.as_str()).ok().is_some() {
                    evicted += 1;
                }
            }
        }

        let _ = txn.commit();
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_node(name: &str, size: u64, kind: NodeKind, children: Vec<FileNode>) -> FileNode {
        FileNode {
            name: name.to_string(),
            path: PathBuf::new(),
            size,
            modified: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            kind,
            children,
        }
    }

    fn make_tree(root: FileNode) -> FileTree {
        let total_size = root.total_size();
        let (file_count, dir_count) = Cache::count_all(&root);
        FileTree {
            root,
            total_size,
            file_count,
            dir_count,
        }
    }

    #[test]
    fn should_persist_and_restore_single_file_tree() {
        let dir = TempDir::new().unwrap();
        let cache_path = dir.path().join("cache.db");
        let mut child = make_node("file.txt", 1024, NodeKind::File, vec![]);
        child.path = dir.path().join("file.txt");
        let mut root = make_node(dir.path().file_name().unwrap().to_str().unwrap(), 0, NodeKind::Directory, vec![child]);
        root.path = dir.path().to_path_buf();
        let tree = make_tree(root);

        let cache = Cache::open(&cache_path, dir.path()).unwrap();
        cache.store(&tree).unwrap();

        let loaded = cache.load().unwrap();
        assert_eq!(loaded.file_count, 1);
        assert_eq!(loaded.dir_count, 1);
        assert_eq!(loaded.root.kind, NodeKind::Directory);
        assert_eq!(loaded.root.path, dir.path());
        assert_eq!(loaded.root.children[0].name, "file.txt");
        assert_eq!(loaded.root.children[0].size, 1024);
        assert_eq!(loaded.root.children[0].path, dir.path().join("file.txt"));
    }

    #[test]
    fn should_persist_and_restore_nested_tree() {
        let dir = TempDir::new().unwrap();
        let cache_path = dir.path().join("cache.db");

        let child1 = {
            let mut n = make_node("a.txt", 100, NodeKind::File, vec![]);
            n.path = dir.path().join("a.txt");
            n
        };
        let child2 = {
            let mut n = make_node("b.txt", 200, NodeKind::File, vec![]);
            n.path = dir.path().join("b.txt");
            n
        };
        let mut root = make_node("root", 0, NodeKind::Directory, vec![child1, child2]);
        root.path = dir.path().to_path_buf();
        let tree = make_tree(root);

        let cache = Cache::open(&cache_path, dir.path()).unwrap();
        cache.store(&tree).unwrap();

        let loaded = cache.load().unwrap();
        assert_eq!(loaded.file_count, 2);
        assert_eq!(loaded.dir_count, 1);
        assert_eq!(loaded.total_size, 300);
        assert_eq!(loaded.root.children.len(), 2);
        assert_eq!(loaded.root.children[0].name, "a.txt");
        assert_eq!(loaded.root.children[1].name, "b.txt");
    }

    #[test]
    fn should_detect_stale_entry_when_mtime_changed() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("stale.txt");
        fs::write(&file_path, b"hello").unwrap();

        let meta = fs::metadata(&file_path).unwrap();
        let node = FileNode {
            name: "stale.txt".to_string(),
            path: file_path,
            size: meta.len(),
            modified: UNIX_EPOCH + Duration::from_secs(1000), // way in the past
            kind: NodeKind::File,
            children: vec![],
        };

        let cache = Cache::open(&dir.path().join("cache.db"), dir.path()).unwrap();
        assert!(cache.is_stale(&node));
    }

    #[test]
    fn should_detect_stale_entry_when_size_changed() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("grown.txt");
        fs::write(&file_path, b"small").unwrap();

        let node = FileNode {
            name: "grown.txt".to_string(),
            path: file_path,
            size: 999_999, // wrong size
            modified: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            kind: NodeKind::File,
            children: vec![],
        };

        let cache = Cache::open(&dir.path().join("cache.db"), dir.path()).unwrap();
        assert!(cache.is_stale(&node));
    }

    #[test]
    fn should_detect_fresh_entry_when_mtime_and_size_match() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("fresh.txt");
        fs::write(&file_path, b"hello").unwrap();

        let meta = fs::metadata(&file_path).unwrap();
        let node = FileNode {
            name: "fresh.txt".to_string(),
            path: file_path.clone(),
            size: meta.len(),
            modified: meta.modified().unwrap(),
            kind: NodeKind::File,
            children: vec![],
        };

        let cache = Cache::open(&dir.path().join("cache.db"), dir.path()).unwrap();
        assert!(!cache.is_stale(&node));
    }

    #[test]
    fn should_report_stale_when_file_deleted() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("deleted.txt");

        let node = FileNode {
            name: "deleted.txt".to_string(),
            path: file_path,
            size: 100,
            modified: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            kind: NodeKind::File,
            children: vec![],
        };

        let cache = Cache::open(&dir.path().join("cache.db"), dir.path()).unwrap();
        assert!(cache.is_stale(&node));
    }
}
