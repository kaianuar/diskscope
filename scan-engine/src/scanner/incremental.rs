use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tree::TreeBuilder;
use crate::{CachedEntry, ScanError, ScanOptions, ScanResult};

use super::cache::RedbCache;
use super::walker::walk_directory;

/// Scanner that stores/retrieves metadata in a persistent `redb` cache,
/// enabling incremental scans that skip I/O for unchanged files.
pub struct IncrementalScanner {
    options: ScanOptions,
    cache: RedbCache,
}

impl IncrementalScanner {
    pub fn new(options: ScanOptions, cache: RedbCache) -> Self {
        Self { options, cache }
    }

    /// Scan `root`, populating the cache for every entry.
    pub fn scan(&self, root: &Path) -> Result<ScanResult, ScanError> {
        let mut entries = walk_directory(root, &self.options)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for entry in &mut entries {
            let cached = self.cache.get(&entry.path).ok().flatten();
            if let Some(c) = cached {
                if c.entry.size == entry.size && c.entry.modified == entry.modified {
                    // Unchanged — use cached metadata (already correct).
                    continue;
                }
            }
            // Store/update cache.
            let _ = self.cache.put(
                &entry.path,
                &CachedEntry {
                    entry: entry.clone(),
                    scan_time: now,
                },
            );
        }

        Ok(TreeBuilder::build(entries))
    }
}
