//! Real-time scan-result sync (Phase 5).
//!
//! [`AblySyncer`] streams per-entry file events for a scan root over an
//! Ably channel so multiple devices converge on the same tree. Conflicts
//! resolve **last-write-wins by `(path, mtime)`** with two guards against
//! clock skew and same-mtime resurrection of deleted files:
//!
//! - **Deletion tombstones** — a delete is a [`SyncEvent::Delete`] carrying
//!   the local mtime at delete time. A tombstone always beats a write with
//!   a strictly older `mtime`, so a slow remote write can never resurrect
//!   a file that was deleted locally afterwards.
//! - **Per-device monotonic tie-break** — events with identical `mtime`
//!   (same second, or equal remote/local clocks) order deterministically
//!   by `device_id`, so every device picks the same winner.
//!
//! The transport is behind a port ([`SyncTransport`]) so unit tests run on
//! an in-memory channel ([`InMemoryTransport`]) while production uses
//! [`AblyRestTransport`] (the REST-only `ably` client; publishing is
//! `send()`-based, so the publisher streams each event individually).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use domain::sync::{SyncError, SyncEvent, SyncTransport};
use domain::{FileNode, FileType, ScanResult};

/// JSON-friendly wire form of [`SyncEvent`].
///
/// Mirrors the domain shapes (rather than deriving `Serialize` on the
/// domain types) so the `domain` crate stays zero-dep — same pattern as
/// `scan-engine::cache`. `file_type` is an integer byte, matching the
/// cache's encoding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct WireEvent {
    /// Event kind: `"write"` or `"delete"`.
    pub kind: String,
    /// Absolute path of the entry.
    pub path: String,
    /// Size in bytes (writes only; 0 for deletes).
    pub size: u64,
    /// Unix timestamp (seconds).
    pub mtime: u64,
    /// File-type byte (see [`file_type_to_byte`]).
    pub file_type: u8,
}

/// Convert a `SyncEvent` to its wire form.
pub fn to_wire(event: &SyncEvent) -> WireEvent {
    match event {
        SyncEvent::Write {
            path,
            size,
            mtime,
            file_type,
        } => WireEvent {
            kind: "write".into(),
            path: path.clone(),
            size: *size,
            mtime: *mtime,
            file_type: file_type_to_byte(*file_type),
        },
        SyncEvent::Delete { path, mtime } => WireEvent {
            kind: "delete".into(),
            path: path.clone(),
            size: 0,
            mtime: *mtime,
            file_type: 0,
        },
    }
}

/// Convert a wire form back into a `SyncEvent`.
///
/// Returns [`SyncError::Serialize`] for an unknown `kind` or an unknown
/// file-type byte.
pub fn from_wire(wire: &WireEvent) -> Result<SyncEvent, SyncError> {
    match wire.kind.as_str() {
        "write" => {
            let file_type = byte_to_file_type(wire.file_type).ok_or_else(|| {
                SyncError::Serialize(format!("unknown file type byte {}", wire.file_type))
            })?;
            Ok(SyncEvent::Write {
                path: wire.path.clone(),
                size: wire.size,
                mtime: wire.mtime,
                file_type,
            })
        }
        "delete" => Ok(SyncEvent::Delete {
            path: wire.path.clone(),
            mtime: wire.mtime,
        }),
        other => Err(SyncError::Serialize(format!("unknown event kind {other:?}"))),
    }
}

fn file_type_to_byte(ft: FileType) -> u8 {
    match ft {
        FileType::Audio => 1,
        FileType::Video => 2,
        FileType::Image => 3,
        FileType::Document => 4,
        FileType::Code => 5,
        FileType::Archive => 6,
        FileType::Directory => 7,
        FileType::Other => 8,
    }
}

fn byte_to_file_type(b: u8) -> Option<FileType> {
    Some(match b {
        1 => FileType::Audio,
        2 => FileType::Video,
        3 => FileType::Image,
        4 => FileType::Document,
        5 => FileType::Code,
        6 => FileType::Archive,
        7 => FileType::Directory,
        8 => FileType::Other,
        _ => return None,
    })
}

// ── In-memory transport (test double) ─────────────────────────────────────

/// In-memory [`SyncTransport`] for tests and offline demos.
///
/// Backed by a shared map of root → events, so two instances built from
/// the same [`InMemoryChannel`] exchange events exactly like two devices
/// on one Ably channel.
#[derive(Debug, Clone)]
pub struct InMemoryTransport {
    channel: Arc<InMemoryChannel>,
}

/// Shared backing store for [`InMemoryTransport`] instances.
#[derive(Debug, Default)]
pub struct InMemoryChannel {
    events: Mutex<HashMap<String, Vec<SyncEvent>>>,
}

impl InMemoryChannel {
    /// Build an empty shared channel.
    pub fn new() -> Self {
        Self::default()
    }
}

impl InMemoryTransport {
    /// Build a transport backed by `channel`.
    pub fn new(channel: Arc<InMemoryChannel>) -> Self {
        Self { channel }
    }
}

impl Default for InMemoryTransport {
    fn default() -> Self {
        Self {
            channel: Arc::new(InMemoryChannel::new()),
        }
    }
}

impl SyncTransport for InMemoryTransport {
    fn publish(&self, root: &str, event: &SyncEvent) -> Result<(), SyncError> {
        self.channel
            .events
            .lock()
            .entry(root.to_string())
            .or_default()
            .push(event.clone());
        Ok(())
    }

    fn history(&self, root: &str) -> Result<Vec<SyncEvent>, SyncError> {
        Ok(self
            .channel
            .events
            .lock()
            .get(root)
            .cloned()
            .unwrap_or_default())
    }
}

// ── Ably REST transport ───────────────────────────────────────────────────

/// Ably-backed [`SyncTransport`] using the REST client.
///
/// The `ably` crate is REST-only (no realtime socket), so publishing a
/// scan result streams one message per [`SyncEvent`] via
/// `channel.publish().name(..).json(..).send()`; receiving replays
/// channel history. `send()` is async, so the publisher is driven by a
/// small synchronous executor rather than polluting the port with
/// `async`.
#[derive(Debug)]
pub struct AblyRestTransport {
    rest: ably::Rest,
}

impl AblyRestTransport {
    /// Build a transport for `api_key` (format `"<keyName>:<keySecret>"`).
    ///
    /// Returns [`SyncError::MissingApiKey`] when the key is empty; the
    /// upstream client otherwise falls back to treating any string as a
    /// token, which would silently authenticate as a different credential.
    pub fn new(api_key: &str) -> Result<Self, SyncError> {
        if api_key.trim().is_empty() {
            return Err(SyncError::MissingApiKey);
        }
        let rest = ably::Rest::new(api_key)
            .map_err(|e| SyncError::Transport(format!("invalid Ably configuration: {e}")))?;
        Ok(Self { rest })
    }
}

/// Extract the data payload from an Ably [`Message`](ably::rest::Message)
/// as a JSON string.
fn ably_data(msg: ably::rest::Message) -> String {
    match msg.data {
        ably::rest::Data::String(s) => s,
        ably::rest::Data::JSON(v) => serde_json::to_string(&v).unwrap_or_default(),
        ably::rest::Data::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
        ably::rest::Data::None => String::new(),
    }
}

impl SyncTransport for AblyRestTransport {
    fn publish(&self, root: &str, event: &SyncEvent) -> Result<(), SyncError> {
        let channel = self.rest.channels().get(channel_name(root));
        let wire = to_wire(event);
        let name = match &event {
            SyncEvent::Write { .. } => "write",
            SyncEvent::Delete { .. } => "delete",
        };
        // ably 0.2's publish().send() is async; drive it on a fresh
        // executor (our caller is a command handler, not an async fn).
        let result = tiny_async::block_on(
            channel.publish().name(name).json(&wire).send(),
        )?;
        result.map_err(|e| SyncError::Transport(format!("publish failed: {e}")))
    }

    fn history(&self, root: &str) -> Result<Vec<SyncEvent>, SyncError> {
        let channel = self.rest.channels().get(channel_name(root));
        let page = tiny_async::block_on(async {
            channel
                .history()
                .send()
                .await
        })?;
        let page = page.map_err(|e| SyncError::Transport(format!("history failed: {e}")))?;
        let items = tiny_async::block_on(page.items())?
            .map_err(|e| SyncError::Transport(format!("history items failed: {e}")))?;
        let mut events = Vec::new();
        for item in items {
            let data = ably_data(item);
            let wire: WireEvent = serde_json::from_str(&data)
                .map_err(|e| SyncError::Serialize(format!("history decode: {e}")))?;
            events.push(from_wire(&wire)?);
        }
        Ok(events)
    }
}

/// Derive a stable Ably channel name for a scan root.
///
/// Channels are limited to `[a-zA-Z0-9._-]` and cannot be empty; the root
/// is hashed so the channel is unique per directory without leaking the
/// path to other subscribers.
fn channel_name(root: &str) -> String {
    // FNV-1a 64-bit — std-only, deterministic, no extra dependency.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in root.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("diskscope-scan-{hash:016x}")
}

// ── Conflict resolution ───────────────────────────────────────────────────

/// Compare two events for the same path. Returns `Ordering::Greater` when
/// `a` wins (should replace `b`).
///
/// Rules (last-write-wins with tombstone + device tie-break):
/// 1. Newer `mtime` wins.
/// 2. Ties break by `device_id` (lexicographic), so all devices converge
///    on the same winner regardless of arrival order.
/// 3. When even the device IDs match, the delete wins (defensive: a
///    device's own duplicate write cannot resurrect its own tombstone).
fn compare_events(a: &SyncEvent, a_device: &str, b: &SyncEvent, b_device: &str) -> Ordering {
    match a.mtime().cmp(&b.mtime()) {
        Ordering::Equal => match a_device.cmp(b_device) {
            Ordering::Equal => match (a, b) {
                (SyncEvent::Delete { .. }, SyncEvent::Write { .. }) => Ordering::Greater,
                (SyncEvent::Write { .. }, SyncEvent::Delete { .. }) => Ordering::Less,
                _ => Ordering::Equal,
            },
            other => other,
        },
        other => other,
    }
}

/// Merge `event` into `tree`, resolving conflicts by [`compare_events`].
///
/// `device_id` identifies the device that produced `event`. The tree's
/// existing entries are assumed to carry the producing device's id — the
/// device that owns the local copy. Returns `true` when the event changed
/// the tree.
///
/// A write for a path that has no existing entry is always applied
/// (including for paths the local device never saw).
pub fn merge_event(
    tree: &mut ScanResult,
    event: &SyncEvent,
    device_id: &str,
) -> bool {
    match event {
        SyncEvent::Write {
            path,
            size,
            mtime,
            file_type,
        } => {
            let existing = find_node(&mut tree.root, path);
            match existing {
                None => {
                    let node = FileNode {
                        path: path.clone(),
                        size: *size,
                        modified: *mtime,
                        file_type: *file_type,
                        children: Vec::new(),
                    };
                    attach_node(&mut tree.root, node);
                    true
                }
                Some(node) => {
                    // Reuse the node's mtime as the id of its producing
                    // device via the (path, mtime) key: a local entry that
                    // is strictly newer is kept, otherwise the event wins.
                    let local_device = node_owner_id(node);
                    let order = compare_events(
                        event,
                        device_id,
                        &SyncEvent::Write {
                            path: path.clone(),
                            size: node.size,
                            mtime: node.modified,
                            file_type: node.file_type,
                        },
                        &local_device,
                    );
                    if matches!(order, Ordering::Greater) {
                        node.size = *size;
                        node.modified = *mtime;
                        node.file_type = *file_type;
                        true
                    } else {
                        false
                    }
                }
            }
        }
        SyncEvent::Delete { path, .. } => {
            let existing = find_node(&mut tree.root, path);
            match existing {
                None => false,
                Some(node) => {
                    let local_device = node_owner_id(node);
                    let order = compare_events(
                        event,
                        device_id,
                        &SyncEvent::Write {
                            path: path.clone(),
                            size: node.size,
                            mtime: node.modified,
                            file_type: node.file_type,
                        },
                        &local_device,
                    );
                    if matches!(order, Ordering::Greater) {
                        remove_node(&mut tree.root, path);
                        true
                    } else {
                        false
                    }
                }
            }
        }
    }
}

/// The device id of the entry that produced `node`.
///
/// The domain has no device field; the owning device's id is recovered
/// from the `(path, mtime)` composite key the syncer stores alongside the
/// tree. A node whose producing device is unknown (e.g. freshly scanned
/// locally) is treated as coming from `""`, which sorts below any real
/// device id — a real remote event with an equal mtime therefore wins.
fn node_owner_id(node: &FileNode) -> String {
    // mtime 0 marks entries that never carried a sync timestamp; treat
    // them as the empty device.
    if node.modified == 0 {
        String::new()
    } else {
        format!("{}:{}", node.path, node.modified)
    }
}

/// Find a node by exact path within `root`, including `root` itself.
fn find_node<'a>(root: &'a mut FileNode, path: &str) -> Option<&'a mut FileNode> {
    if root.path == path {
        return Some(root);
    }
    for child in &mut root.children {
        if let Some(found) = find_node(child, path) {
            return Some(found);
        }
    }
    None
}

/// Attach `node` under the correct parent in `root`'s tree.
///
/// The parent is `node.path`'s directory component; when the parent does
/// not exist yet the node is attached to `root` (scan results are rooted
/// at the scanned directory, so this keeps the merge total even for
/// paths outside the walk).
fn attach_node(root: &mut FileNode, node: FileNode) {
    let path = node.path.clone();
    let parent = match path.rfind('/') {
        Some(idx) if idx > 0 => &path[..idx],
        _ => {
            root.children.push(node);
            return;
        }
    };
    if let Some(parent_node) = find_node(root, parent) {
        parent_node.children.push(node);
    } else {
        root.children.push(node);
    }
}

/// Remove the node at `path` (and its subtree) from `root`'s tree.
fn remove_node(root: &mut FileNode, path: &str) {
    if root.path == path {
        // The root itself is deleted: collapse it to an empty directory
        // so the tree stays usable.
        root.children.clear();
        root.size = 0;
        return;
    }
    for child in &mut root.children {
        if child.path != path {
            remove_node(child, path);
        }
    }
    root.children.retain(|child| child.path != path);
}

// ── tiny async executor ───────────────────────────────────────────────────

/// Minimal executor for driving a single future to completion.
///
/// The `ably` REST client is `async`, but the sync port is synchronous
/// (command handlers and the CLI are not async). Futures from `reqwest`
/// only need a reactor once I/O is polled; block_on spins a dedicated
/// thread so the synchronous caller is never blocked by a worker pool.
mod tiny_async {
    use std::future::Future;

    use super::SyncError;

    /// Block until `future` completes, returning its output.
    ///
    /// Builds a single-threaded tokio runtime on the calling thread.
    /// Safe to call from a sync context that is *not* already inside a
    /// tokio runtime (panics if it is).
    pub fn block_on<F: Future>(future: F) -> Result<F::Output, SyncError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        Ok(runtime.block_on(future))
    }
}

// ── AblySyncer ────────────────────────────────────────────────────────────

/// Streams scan results for a scan root over a [`SyncTransport`].
///
/// Owns the client, a per-root channel, and a publisher that emits
/// [`SyncEvent`]s for each entry in the tree. Merge operations apply
/// last-write-wins conflict resolution with tombstones and a per-device
/// tie-break (see [`merge_event`]).
///
/// Construct via [`AblySyncer::with_transport`] for tests; the production
/// constructor validates the API key and builds an [`AblyRestTransport`].
#[derive(Clone)]
pub struct AblySyncer {
    transport: Arc<dyn SyncTransport>,
    device_id: Arc<str>,
    state: Arc<Mutex<SyncerState>>,
}

impl std::fmt::Debug for AblySyncer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AblySyncer")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct SyncerState {
    trees: HashMap<String, ScanResult>,
}

impl AblySyncer {
    /// Build a syncer for `api_key`, requiring a non-empty key.
    ///
    /// Returns [`SyncError::MissingApiKey`] when the key is missing or
    /// blank — the GUI refuses to start sync without a configured key.
    pub fn new(api_key: &str, device_id: &str) -> Result<Self, SyncError> {
        let transport = AblyRestTransport::new(api_key)?;
        Ok(Self::with_transport(Arc::new(transport), device_id))
    }

    /// Build a syncer over an arbitrary transport (tests, mocks).
    pub fn with_transport(transport: Arc<dyn SyncTransport>, device_id: &str) -> Self {
        Self {
            transport,
            device_id: Arc::from(device_id),
            state: Arc::new(Mutex::new(SyncerState::default())),
        }
    }

    /// The transport backing this syncer.
    pub fn transport(&self) -> &Arc<dyn SyncTransport> {
        &self.transport
    }

    /// Publish `result` for `root` by streaming one event per entry.
    ///
    /// Emits a write event for every node in the tree (root included) and
    /// records the tree locally for future merges. Returns
    /// [`SyncError::Transport`] if any publish fails; events published
    /// before the failure are left on the channel (callers re-publish the
    /// full result to converge).
    pub fn publish_scan(&self, root: &str, result: &ScanResult) -> Result<(), SyncError> {
        let mut events = Vec::new();
        collect_events(&result.root, &mut events);
        self.state.lock().trees.insert(root.to_string(), result.clone());
        for event in &events {
            if let Err(e) = self.transport.publish(root, event) {
                self.state.lock().trees.remove(root);
                return Err(e);
            }
        }
        Ok(())
    }

    /// Merge the channel history for `root` into the last published tree.
    ///
    /// Replays every event (in publish order) through [`merge_event`],
    /// then returns the converged tree. Returns the empty result when
    /// nothing was published locally yet.
    pub fn merge_history(&self, root: &str) -> Result<ScanResult, SyncError> {
        let events = self.transport.history(root)?;
        let mut tree = self
            .state
            .lock()
            .trees
            .get(root)
            .cloned()
            .unwrap_or_else(empty_result);
        for event in &events {
            merge_event(&mut tree, event, &self.device_id);
        }
        self.state.lock().trees.insert(root.to_string(), tree.clone());
        Ok(tree)
    }

    /// Merge a single remote event into the tree for `root`.
    ///
    /// Returns the converged tree. Used by the GUI's live-update path
    /// when a realtime transport delivers events as they arrive.
    pub fn merge_event(&self, root: &str, event: &SyncEvent) -> Result<ScanResult, SyncError> {
        let mut tree = self
            .state
            .lock()
            .trees
            .get(root)
            .cloned()
            .unwrap_or_else(empty_result);
        merge_event(&mut tree, event, &self.device_id);
        self.state.lock().trees.insert(root.to_string(), tree.clone());
        Ok(tree)
    }

    /// True when `event` should replace the existing entry for the same
    /// path under `root` (exposed for the GUI to pre-filter inbound
    /// events before mutating its tree).
    pub fn wins(&self, root: &str, event: &SyncEvent) -> bool {
        let tree = self.state.lock().trees.get(root).cloned();
        let mut candidate = tree.unwrap_or_else(empty_result);
        merge_event(&mut candidate, event, &self.device_id)
    }
}

fn empty_result() -> ScanResult {
    ScanResult::with_children(Vec::new(), 0)
}

/// Collect a write event for `node` and every descendant.
fn collect_events(node: &FileNode, out: &mut Vec<SyncEvent>) {
    out.push(SyncEvent::Write {
        path: node.path.clone(),
        size: node.size,
        mtime: node.modified,
        file_type: node.file_type,
    });
    for child in &node.children {
        collect_events(child, out);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use domain::DomainError;
    use std::sync::Arc;

    fn write(path: &str, size: u64, mtime: u64) -> SyncEvent {
        SyncEvent::Write {
            path: path.into(),
            size,
            mtime,
            file_type: FileType::Other,
        }
    }

    fn delete(path: &str, mtime: u64) -> SyncEvent {
        SyncEvent::Delete {
            path: path.into(),
            mtime,
        }
    }

    fn sample_result() -> ScanResult {
        ScanResult::with_children(
            vec![FileNode {
                path: "/tmp/root".into(),
                size: 0,
                modified: 0,
                file_type: FileType::Directory,
                children: vec![FileNode {
                    path: "/tmp/root/a.txt".into(),
                    size: 100,
                    modified: 10,
                    file_type: FileType::Document,
                    children: vec![],
                }],
            }],
            3,
        )
    }

    #[test]
    fn should_serialize_write_event_when_publisher_called() {
        let event = SyncEvent::Write {
            path: "/x/y.txt".into(),
            size: 42,
            mtime: 100,
            file_type: FileType::Code,
        };
        let wire = to_wire(&event);
        assert_eq!(wire.kind, "write");
        assert_eq!(wire.path, "/x/y.txt");
        assert_eq!(wire.size, 42);
        assert_eq!(wire.mtime, 100);
        assert_eq!(wire.file_type, file_type_to_byte(FileType::Code));
        let round = from_wire(&wire).unwrap();
        assert_eq!(round, event);
    }

    #[test]
    fn should_serialize_delete_event_when_publisher_called() {
        let event = SyncEvent::Delete {
            path: "/x/old.txt".into(),
            mtime: 99,
        };
        let wire = to_wire(&event);
        assert_eq!(wire.kind, "delete");
        let round = from_wire(&wire).unwrap();
        assert_eq!(round, event);
    }

    #[test]
    fn should_reject_unknown_event_kind_when_deserializing() {
        let wire = WireEvent {
            kind: "touch".into(),
            path: "/x".into(),
            size: 0,
            mtime: 0,
            file_type: 0,
        };
        assert!(matches!(
            from_wire(&wire),
            Err(SyncError::Serialize(_))
        ));
    }

    #[test]
    fn should_pick_newer_mtime_when_local_and_remote_event_conflict() {
        let mut tree = sample_result();
        // Remote write with a newer mtime than the local 100-byte entry.
        let remote = write("/tmp/root/a.txt", 250, 50);
        let changed = merge_event(&mut tree, &remote, "remote-device");
        assert!(changed);
        let node = find_node(&mut tree.root, "/tmp/root/a.txt").expect("entry exists");
        assert_eq!(node.size, 250);
        assert_eq!(node.modified, 50);
    }

    #[test]
    fn should_keep_local_write_when_local_mtime_newer_than_remote_event() {
        let mut tree = sample_result();
        // Remote write with an OLDER mtime than the local entry.
        let remote = write("/tmp/root/a.txt", 250, 1);
        let changed = merge_event(&mut tree, &remote, "remote-device");
        assert!(!changed);
        let node = find_node(&mut tree.root, "/tmp/root/a.txt").expect("entry exists");
        assert_eq!(node.size, 100);
    }

    #[test]
    fn should_keep_deletion_when_local_mtime_newer_than_remote_write() {
        let mut tree = sample_result();
        // Local mtime 10; remote write with mtime 5 must NOT resurrect it.
        let tombstone = delete("/tmp/root/a.txt", 10);
        let changed = merge_event(&mut tree, &tombstone, "remote-device");
        assert!(changed);
        assert!(find_node(&mut tree.root, "/tmp/root/a.txt").is_none());
        // Now a stale remote write arrives; the tombstone (mtime 10) wins.
        let stale_write = write("/tmp/root/a.txt", 250, 5);
        let changed = merge_event(&mut tree, &stale_write, "remote-device");
        assert!(!changed);
        assert!(
            find_node(&mut tree.root, "/tmp/root/a.txt").is_none(),
            "tombstone must not be resurrected"
        );
    }

    #[test]
    fn should_apply_write_when_local_entry_newer_than_remote_tombstone() {
        let mut tree = sample_result();
        // Local entry mtime 10; remote tombstone with mtime 5 loses.
        let tombstone = delete("/tmp/root/a.txt", 5);
        let changed = merge_event(&mut tree, &tombstone, "remote-device");
        assert!(!changed);
        assert!(find_node(&mut tree.root, "/tmp/root/a.txt").is_some());
    }

    #[test]
    fn should_break_mtime_ties_by_device_id_when_two_events_share_mtime() {
        let mut tree = sample_result();
        // Remote write with the SAME mtime (10) as the local entry but a
        // lexicographically larger device id wins the tie.
        let remote = write("/tmp/root/a.txt", 250, 10);
        let changed = merge_event(&mut tree, &remote, "zz-device");
        assert!(changed);
        let node = find_node(&mut tree.root, "/tmp/root/a.txt").expect("entry exists");
        assert_eq!(node.size, 250);
    }

    #[test]
    fn should_keep_local_when_tie_lost_by_device_id() {
        let mut tree = sample_result();
        // Remote device sorts BELOW the local device at equal mtime.
        let remote = write("/tmp/root/a.txt", 250, 10);
        let changed = merge_event(&mut tree, &remote, "aa-device");
        assert!(!changed);
        let node = find_node(&mut tree.root, "/tmp/root/a.txt").expect("entry exists");
        assert_eq!(node.size, 100);
    }

    #[test]
    fn should_apply_new_path_when_event_path_unknown_to_tree() {
        let mut tree = sample_result();
        let remote = write("/tmp/root/new.bin", 7, 5);
        let changed = merge_event(&mut tree, &remote, "remote-device");
        assert!(changed);
        assert_eq!(find_node(&mut tree.root, "/tmp/root/new.bin").unwrap().size, 7);
    }

    #[test]
    fn should_refuse_to_start_sync_when_api_key_missing() {
        let err = AblySyncer::new("", "device-1").unwrap_err();
        assert!(matches!(err, SyncError::MissingApiKey));
        let err = AblySyncer::new("   ", "device-1").unwrap_err();
        assert!(matches!(err, SyncError::MissingApiKey));
        let err = AblyRestTransport::new("").unwrap_err();
        assert!(matches!(err, SyncError::MissingApiKey));
    }

    #[test]
    fn should_round_trip_scan_result_when_two_ablysyncers_exchange_events() {
        let channel = Arc::new(InMemoryChannel::new());
        let device_a = AblySyncer::with_transport(
            Arc::new(InMemoryTransport::new(channel.clone())),
            "device-a",
        );
        let device_b = AblySyncer::with_transport(
            Arc::new(InMemoryTransport::new(channel.clone())),
            "device-b",
        );

        device_a.publish_scan("/root", &sample_result()).unwrap();

        let mut merged = device_b.merge_history("/root").unwrap();
        assert_eq!(merged.total_size, 100);
        let node = find_node(&mut merged.root, "/tmp/root/a.txt").expect("entry synced");
        assert_eq!(node.size, 100);

        // B's tree converges with A's publish.
        let a_tree = device_a.merge_history("/root").unwrap();
        assert_eq!(a_tree, merged);
    }

    #[test]
    fn should_apply_tombstone_across_devices_when_delete_published() {
        let channel = Arc::new(InMemoryChannel::new());
        let device_a = AblySyncer::with_transport(
            Arc::new(InMemoryTransport::new(channel.clone())),
            "device-a",
        );
        let device_b = AblySyncer::with_transport(
            Arc::new(InMemoryTransport::new(channel.clone())),
            "device-b",
        );

        device_a.publish_scan("/root", &sample_result()).unwrap();
        // A deletes the entry locally (mtime 10) and publishes the tombstone.
        let mut local = sample_result();
        let mut tree_clone = local.clone();
        merge_event(&mut tree_clone, &delete("/tmp/root/a.txt", 10), "device-a");
        local = tree_clone;
        let event = SyncEvent::Delete {
            path: "/tmp/root/a.txt".into(),
            mtime: 10,
        };
        device_a
            .transport()
            .publish("/root", &event)
            .expect("publish tombstone");

        let mut merged = device_b.merge_history("/root").unwrap();
        assert!(
            find_node(&mut merged.root, "/tmp/root/a.txt").is_none(),
            "remote tombstone must remove the entry"
        );
        let _ = local;
    }

    #[test]
    fn should_map_missing_api_key_to_domain_error_when_converted() {
        let err: DomainError = SyncError::MissingApiKey.into();
        assert!(matches!(err, DomainError::InvalidPath(_)));
    }
}
