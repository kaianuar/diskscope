use std::fs;
use std::path::Path;

use redb::{Database, TableDefinition};

use crate::domain::CachedEntry;

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
        let db = Database::create(path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(Self { db })
    }

    /// Open a cache in a temporary directory (for testing).
    pub fn open_temp() -> Result<Self, std::io::Error> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("cache.redb");
        Self::open(&path)
    }

    /// Look up a cached entry by canonical path.
    /// Returns `Ok(None)` if the key is absent or the table doesn't exist yet.
    pub fn get(&self, path: &Path) -> Result<Option<CachedEntry>, std::io::Error> {
        let key = Self::canonical_key(path);
        let txn = self.db
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
                if bytes.len() != 16 {
                    return Ok(None);
                }
                let size = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                let mtime = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                Ok(Some(CachedEntry { size, mtime }))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    }

    /// Store a cached entry keyed by canonical path.
    pub fn put(&self, path: &Path, entry: &CachedEntry) -> Result<(), std::io::Error> {
        let key = Self::canonical_key(path);
        let mut val = [0u8; 16];
        val[0..8].copy_from_slice(&entry.size.to_le_bytes());
        val[8..16].copy_from_slice(&entry.mtime.to_le_bytes());
        let txn = self.db
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
        txn.commit()
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    /// Remove a cached entry.
    pub fn invalidate(&self, path: &Path) -> Result<(), std::io::Error> {
        let key = Self::canonical_key(path);
        let txn = self.db
            .begin_write()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            let mut table = txn
                .open_table(TABLE)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            table
                .remove(key.as_str())
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        txn.commit()
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    fn canonical_key(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }
}
