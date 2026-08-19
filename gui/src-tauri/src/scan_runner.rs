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
        /// Snapshot of the completed scan result.
        result: Arc<ScanResult>,
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
pub struct ScanRunner {
    state: Mutex<ScanState>,
    service: Arc<ScanService>,
    next_id: AtomicU64,
    /// Hand-off slot between the background scan thread and
    /// [`ScanRunner::collect`]: the thread stores the finished result
    /// (or `None` on error — the error already went out via `scan-done`),
    /// and `collect` reads it after joining.
    pending: Arc<Mutex<Option<Arc<ScanResult>>>>,
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

    /// Snapshot of the finished scan, if any.
    pub fn result(&self) -> Option<Arc<ScanResult>> {
        match &*self.state.lock() {
            ScanState::Done { result, .. } => Some(Arc::clone(result)),
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
            return Err(DomainError::InvalidPath(
                "scan path must not be empty".into(),
            ));
        }

        let mut guard = self.state.lock();
        if let ScanState::Running { id, .. } = &*guard {
            return Err(DomainError::InvalidPath(format!(
                "a scan is already running (id {id})"
            )));
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
                    Ok(()) => service
                        .scan(&path)
                        .map(|r| scan_engine::filter::apply_filter(&r, &f)),
                    Err(e) => Err(e),
                },
                None => service.scan(&path),
            };
            let payload = match outcome {
                Ok(result) => {
                    *pending.lock() = Some(Arc::new(result.clone()));
                    let dto = crate::dto::ScanResultDto::from(&result);
                    serde_json::to_value(dto).unwrap_or_else(|_| {
                        serde_json::json!({ "error": "failed to serialize scan result" })
                    })
                }
                Err(e) => serde_json::json!({ "error": e.to_string() }),
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
            let result = self.pending.lock().take().unwrap_or_else(|| {
                Arc::new(ScanResult::with_children(Vec::new(), 0))
            });
            *guard = ScanState::Done { id, result };
            Some(id)
        } else {
            None
        }
    }
}
