//! DiskScope scan engine adapter.
//!
//! Concrete implementations of the [`domain::ports::Scanner`],
//! [`domain::ports::Trash`], and [`domain::ports::Cache`] ports, plus
//! filter/sort/output-format helpers used by the CLI and GUI.
//!
//! # Layout
//! - [`scanner::JwalkScanner`] — parallel filesystem walk via `jwalk`,
//!   respecting `.gitignore` via the `ignore` crate.
//! - [`cache::RedbCache`] — embedded `redb` cache of recent scan results,
//!   keyed by scan root, storing `(mtime, size, ScanResult)` for
//!   incremental invalidation.
//! - [`trash::TrashBin`] — wrapper around the `trash` crate that records
//!   an undo stack of `(original_path, TrashItem)`.
//! - [`filter::apply_filter`] — prune a `ScanResult` according to a
//!   `domain::Filter`.
//! - [`sort::apply_sort`] — recursively sort a `ScanResult`'s children by
//!   a `domain::SortSpec`.
//! - [`format::OutputFormat`] + [`format::render`] — table / JSON / JSONL
//!   / tree renderers.
//!
//! [`ScanService`] is a convenience composition of the above; the domain
//! logic itself lives in `domain::*` traits and types.

#![deny(missing_docs)]
#![deny(clippy::all)]
#![forbid(unsafe_code)]

pub mod cache;
pub mod filter;
pub mod format;
pub mod scanner;
pub mod sort;
pub mod trash;

use std::path::Path;
use std::time::Instant;

use domain::ports::{Scanner, Trash};
use domain::{DomainError, ScanResult};

pub use cache::RedbCache;
pub use format::OutputFormat;
pub use scanner::JwalkScanner;
pub use trash::TrashBin;

/// Convenience composition of the scan-engine adapters.
///
/// `ScanService` is the single struct a CLI / GUI binary needs to hold:
/// it composes a [`JwalkScanner`] (parallel walk), a [`RedbCache`]
/// (incremental results), and a [`TrashBin`] (safe delete with undo),
/// and exposes the high-level operations those callers need.
///
/// All domain logic stays in the `domain` crate; this is purely adapter
/// wiring.
pub struct ScanService {
    scanner: JwalkScanner,
    cache: RedbCache,
    trash: TrashBin,
}

impl ScanService {
    /// Build a new service backed by a default in-memory cache and an
    /// in-process trash undo stack.
    ///
    /// For tests that need to inspect the cache file, use
    /// [`ScanService::with_cache_path`] instead.
    pub fn new() -> Self {
        Self {
            scanner: JwalkScanner::new(),
            cache: RedbCache::new(),
            trash: TrashBin::new(),
        }
    }

    /// Build a service whose underlying cache lives at `cache_path`.
    /// Cached scan results survive across processes.
    pub fn with_cache_path(cache_path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let cache = RedbCache::open(cache_path)?;
        Ok(Self {
            scanner: JwalkScanner::new(),
            cache,
            trash: TrashBin::new(),
        })
    }

    /// Borrow the scanner.
    pub fn scanner(&self) -> &JwalkScanner {
        &self.scanner
    }

    /// Borrow the cache.
    pub fn cache(&self) -> &RedbCache {
        &self.cache
    }

    /// Borrow the trash.
    pub fn trash(&self) -> &TrashBin {
        &self.trash
    }

    /// Scan `path`, returning a cached result when the root's mtime is
    /// unchanged since the previous scan (incremental reuse).
    ///
    /// On a cache miss or stale entry, the tree is rewalked and the new
    /// result is persisted before being returned.
    pub fn scan(&self, path: &str) -> Result<ScanResult, DomainError> {
        let start = Instant::now();

        // 1. Root mtime probe (cheap) — used to detect "did anything change?"
        let root_meta = self.scanner.stat_root(path)?;
        let root_mtime = root_meta.modified;
        let root_size = root_meta.total_bytes;

        // 2. Cache probe — if we have an entry whose stored root mtime
        //    matches *exactly*, reuse it.
        if let Some((cached, cached_mtime, cached_size)) = self.cache.get_with_metadata(path) {
            if cached_mtime == root_mtime && cached_size == root_size {
                return Ok(cached);
            }
        }

        // 3. Miss / stale — walk the tree.
        let mut result = self.scanner.scan(path)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        result.scan_duration_ms = elapsed_ms;

        // 4. Persist for next time.
        self.cache
            .put_with_metadata(path, &result, root_mtime, root_size)?;
        Ok(result)
    }

    /// Move `path` to trash and remember how to undo it.
    pub fn move_to_trash(&self, path: &str) -> Result<(), DomainError> {
        self.trash.move_to_trash(path)
    }

    /// Undo the most recent move-to-trash.
    pub fn undo_last(&self) -> Result<(), DomainError> {
        self.trash.undo_last()
    }
}

impl Default for ScanService {
    fn default() -> Self {
        Self::new()
    }
}
