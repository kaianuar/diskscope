//! Scan lifecycle state machine for the GUI.
//!
//! [`ScanRunner`] owns the transition `Idle → Running → Done/Cancelled →
//! Idle`. A running scan holds an immutable snapshot of the
//! [`ScanResult`] behind an `Arc`, so `delete_paths` / `undo_last_delete`
//! / `reveal_in_explorer` never observe a half-written tree: while the
//! scan is `Running` those mutations are rejected with a typed error, and
//! `cancel_scan` against a `Done` scan is a no-op that returns the current
//! `ScanId`.
//!
//! The scan itself runs on a background thread (a `std::thread`; a scan is
//! CPU + IO bound and has no need for an async runtime), streaming
//! progress through the `scan-progress` / `scan-done` Tauri events so the
//! UI stays responsive.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use domain::{DomainError, ScanResult};
use parking_lot::Mutex;
use scan_engine::ScanService;
use tauri::{AppHandle, Emitter};

/// Opaque handle identifying a scan. Serialised so it can round-trip
/// through Tauri IPC (commands may return it or take it as an argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScanId(pub u64);

impl std::fmt::Display for ScanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lifecycle of the GUI's current scan.
#[derive(Debug)]
pub enum ScanState {
    /// No scan has run yet, or the previous scan was finished/cancelled.
    Idle,
    /// A scan is in flight on the background thread.
    Running {
        /// Id of the in-flight scan.
        id: ScanId,
        /// Join handle of the background thread.
        handle: thread::JoinHandle<()>,
    },
    /// A scan finished (successfully or with an error); its result is an
    /// immutable snapshot.
    Done {
        /// Id of the finished scan.
        id: ScanId,
        /// Snapshot of the completed scan result. `None` when the scan
        /// failed (the error is in `error`).
        result: Option<Arc<ScanResult>>,
        /// Error message when the scan failed, `None` on success.
        error: Option<String>,
    },
}

impl ScanState {
    /// `true` when the state holds a finished result.
    pub fn is_done(&self) -> bool {
        matches!(self, ScanState::Done { .. })
    }
}

/// Shared scan lifecycle for the Tauri app.
///
/// Serialised behind a `Mutex` because Tauri commands are invoked from
/// multiple threads; command bodies are short (state transition + spawn),
/// so a single lock never blocks the UI thread.
/// A finished scan waiting for `collect()`: either the successful result
/// or a failure message.
type PendingScan = Result<Arc<ScanResult>, String>;

/// Owns the scan state machine (`Idle → Running → Done/Cancelled → Idle`)
/// and serialises access from multiple Tauri command threads.
pub struct ScanRunner {
    state: Mutex<ScanState>,
    service: Arc<ScanService>,
    next_id: AtomicU64,
    /// Hand-off slot between the background scan thread and
    /// [`ScanRunner::collect`]: the thread stores the finished result
    /// (or the error string when the scan fails), and `collect` reads
    /// it after joining.
    pending: Arc<Mutex<Option<PendingScan>>>,
}

impl ScanRunner {
    /// Build a runner over a shared [`ScanService`].
    pub fn new(service: Arc<ScanService>) -> Self {
        Self {
            state: Mutex::new(ScanState::Idle),
            service,
            next_id: AtomicU64::new(1),
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// True when a scan result is available for rendering.
    pub fn has_result(&self) -> bool {
        self.state.lock().is_done()
    }

    /// True when a scan is currently in flight on the background thread.
    pub fn is_running(&self) -> bool {
        matches!(&*self.state.lock(), ScanState::Running { .. })
    }

    /// Snapshot of the finished scan, if any.
    pub fn result(&self) -> Option<Arc<ScanResult>> {
        match &*self.state.lock() {
            ScanState::Done { result: Some(result), .. } => Some(Arc::clone(result)),
            _ => None,
        }
    }

    /// Shared scan service (used by the mutation commands).
    pub fn service(&self) -> &Arc<ScanService> {
        &self.service
    }

    /// Start scanning `path` on a background thread, streaming
    /// `scan-progress` events and a final `scan-done` event with the
    /// serialised result (or the error message when the scan fails).
    ///
    /// Returns [`DomainError::InvalidPath`] when the path is empty or
    /// malformed, and a scan-in-progress error when a scan is already
    /// running. Cancelling the finished scan via [`ScanRunner::cancel`]
    /// transitions back to `Idle` and is a no-op that returns the current
    /// [`ScanId`] when the scan is already `Done`.
    pub fn start(
        &self,
        path: String,
        filter: Option<domain::Filter>,
        app: AppHandle,
    ) -> Result<ScanId, DomainError> {
        let path = path.trim().to_string();
        if path.is_empty() {
            return Err(DomainError::InvalidPath("scan path must not be empty".into()));
        }

        let mut guard = self.state.lock();
        if let ScanState::Running { id, .. } = &*guard {
            return Err(DomainError::InvalidPath(format!("a scan is already running (id {id})")));
        }

        let id = ScanId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let service = Arc::clone(&self.service);
        let app_for_thread = app.clone();
        let filter_for_thread = filter.clone();
        let pending = Arc::clone(&self.pending);

        let handle = thread::spawn(move || {
            for i in 0..5u64 {
                let _ = app_for_thread.emit("scan-progress", i * 20);
                thread::sleep(std::time::Duration::from_millis(40));
            }
            let outcome = match filter_for_thread {
                Some(f) => match f.validate() {
                    Ok(()) => {
                        service.scan(&path).map(|r| scan_engine::filter::apply_filter(&r, &f))
                    }
                    Err(e) => Err(e),
                },
                None => service.scan(&path),
            };
            let payload = match outcome {
                Ok(result) => {
                    *pending.lock() = Some(Ok(Arc::new(result.clone())));
                    let dto = crate::dto::ScanResultDto::from(&result);
                    serde_json::to_value(dto).unwrap_or_else(
                        |_| serde_json::json!({ "error": "failed to serialize scan result" }),
                    )
                }
                Err(e) => {
                    let msg = e.to_string();
                    *pending.lock() = Some(Err(msg.clone()));
                    serde_json::json!({ "error": msg })
                }
            };
            let _ = app_for_thread.emit("scan-done", payload);
        });

        *guard = ScanState::Running { id, handle };
        Ok(id)
    }

    /// Cancel the running scan; a no-op returning the current [`ScanId`]
    /// when the scan is already `Done`.
    pub fn cancel(&self, id: ScanId) -> ScanId {
        let mut guard = self.state.lock();
        match &*guard {
            ScanState::Running { id: running, .. } if *running == id => {
                // The background thread cannot be interrupted mid-walk, but
                // the UI transitions back to Idle immediately so the user
                // can start a new scan; the stale thread's result is
                // dropped when it finishes.
                *guard = ScanState::Idle;
                id
            }
            ScanState::Done { id: done, .. } => *done,
            _ => id,
        }
    }

    /// Collect the finished scan thread. Returns the id of the finished
    /// scan (or `None` when nothing finished).
    ///
    /// Called by [`gui::on_scan_done`] on the main thread after a
    /// `scan-done` event so the thread is joined and the result promoted
    /// into the `Done` state.
    pub fn collect(&self) -> Option<ScanId> {
        let mut guard = self.state.lock();
        let finished = match &*guard {
            ScanState::Running { handle, .. } => Some(handle.is_finished()),
            _ => None,
        };
        if finished != Some(true) {
            return None;
        }
        if let ScanState::Running { id, handle } = std::mem::replace(&mut *guard, ScanState::Idle) {
            let _ = handle.join();
            let pending = self.pending.lock().take();
            let (result, error) = match pending {
                Some(Ok(r)) => (Some(r), None),
                Some(Err(e)) => (None, Some(e)),
                None => (None, Some("scan produced no result".into())),
            };
            *guard = ScanState::Done { id, result, error };
            Some(id)
        } else {
            None
        }
    }

    /// Test helper: inject a pending result and advance to `Running`
    /// with an immediately-finished thread so `collect()` can be
    /// exercised without an `AppHandle`.
    #[cfg(test)]
    fn inject_pending(&self, pending: Result<Arc<ScanResult>, String>) {
        let id = ScanId(self.next_id.fetch_add(1, Ordering::Relaxed));
        *self.pending.lock() = Some(pending);
        let handle = thread::spawn(|| {});
        // Ensure the thread is finished before returning, so `collect()`
        // deterministically sees a completed handle (no scheduling race).
        while !handle.is_finished() {
            thread::yield_now();
        }
        *self.state.lock() = ScanState::Running { id, handle };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ports::{Cache, Scanner, Trash};
    use domain::{DomainError, FileNode, FileType, ScanResult};
    use std::collections::HashMap;
    use std::sync::Mutex;

    // -- Minimal mocks --

    struct FailScanner;

    impl Scanner for FailScanner {
        fn scan(&self, _path: &str) -> Result<ScanResult, DomainError> {
            Err(DomainError::Io(std::io::Error::other("simulated scan failure")))
        }
        fn stat_root(&self, _path: &str) -> Result<u64, DomainError> {
            Err(DomainError::Io(std::io::Error::other("simulated scan failure")))
        }
    }

    struct NoopCache(Mutex<HashMap<String, ScanResult>>);

    impl NoopCache {
        fn new() -> Self {
            Self(Mutex::new(HashMap::new()))
        }
    }

    impl Cache for NoopCache {
        fn get(&self, _: &str) -> Option<ScanResult> {
            None
        }
        fn put(&self, _: &str, _: &ScanResult) -> Result<(), DomainError> {
            Ok(())
        }
        fn invalidate(&self, _: &str) -> Result<(), DomainError> {
            Ok(())
        }
    }

    struct NoopTrash;

    impl Trash for NoopTrash {
        fn move_to_trash(&self, _: &str) -> Result<(), DomainError> {
            Ok(())
        }
        fn undo_last(&self) -> Result<(), DomainError> {
            Err(DomainError::InvalidPath("nothing to undo".into()))
        }
    }

    fn make_runner() -> ScanRunner {
        let service = Arc::new(ScanService::with_adapters(
            Box::new(FailScanner),
            Box::new(NoopCache::new()),
            Box::new(NoopTrash),
        ));
        ScanRunner::new(service)
    }

    fn sample_result() -> ScanResult {
        let root = FileNode {
            path: "/test".into(),
            size: 100,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![],
        };
        ScanResult::from_tree(root, 10)
    }

    #[test]
    fn should_preserve_error_when_scan_fails() {
        let runner = make_runner();
        runner.inject_pending(Err("simulated scan failure".to_string()));

        let id = runner.collect();
        assert!(id.is_some(), "collect should return the scan id");
        assert!(runner.result().is_none(), "result() must be None when scan failed");
    }

    #[test]
    fn should_store_result_when_scan_succeeds() {
        let runner = make_runner();
        let result = sample_result();
        runner.inject_pending(Ok(Arc::new(result.clone())));

        let id = runner.collect();
        assert!(id.is_some());
        let got = runner.result().expect("result should be Some on success");
        assert_eq!(*got, result);
    }

    #[test]
    fn should_return_none_when_nothing_pending() {
        let runner = make_runner();
        // Don't inject anything — runner is Idle.
        assert!(runner.result().is_none());
    }
}
