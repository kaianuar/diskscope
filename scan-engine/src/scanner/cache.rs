use std::fs;
use std::path::Path;

use redb::{Database, TableDefinition};

use crate::CachedEntry;

const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("cache");

/// Persistent file metadata cache backed by `redb`.
pub struct RedbCache {
    db: Database,
}

impl RedbCache {
    /// Open or create a cache database at `path`.
    pub fn open(path: &Path) -> Result<Self, std::io::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let db =
            Database::create(path).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(Self { db })
    }

    /// Look up a cached entry by canonical path.
    pub fn get(&self, path: &Path) -> Result<Option<CachedEntry>, std::io::Error> {
        let key = Self::canonical_key(path);
        let txn = self
            .db
            .begin_read()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let table = match txn.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };
        match table.get(key.as_str()) {
            Ok(Some(guard)) => {
                let bytes = guard.value();
                // Store: size(8) + mtime(8) + scan_time(8) = 24 bytes
                if bytes.len() < 16 {
                    return Ok(None);
                }
                let size = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                let modified = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                let scan_time = if bytes.len() >= 24 {
                    u64::from_le_bytes(bytes[16..24].try_into().unwrap())
                } else {
                    0
                };
                Ok(Some(CachedEntry {
                    entry: crate::FileEntry {
                        path: path.to_path_buf(),
                        name: String::new(), // caller fills in
                        size,
                        modified,
                        node_type: crate::NodeType::File,
                        depth: 0,
                    },
                    scan_time,
                }))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    }

    /// Store a cached entry keyed by canonical path.
    pub fn put(&self, path: &Path, entry: &CachedEntry) -> Result<(), std::io::Error> {
        let key = Self::canonical_key(path);
        let mut val = [0u8; 24];
        val[0..8].copy_from_slice(&entry.entry.size.to_le_bytes());
        val[8..16].copy_from_slice(&entry.entry.modified.to_le_bytes());
        val[16..24].copy_from_slice(&entry.scan_time.to_le_bytes());
        let txn = self
            .db
            .begin_write()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            let mut table = txn
                .open_table(TABLE)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            table
                .insert(key.as_str(), val.as_slice())
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        txn
            .commit()
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    fn canonical_key(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }
}
