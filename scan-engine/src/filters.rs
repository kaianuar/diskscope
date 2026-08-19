//! Per-node filtering of a [`ScanResult`].
//!
//! [`apply_filter`] walks the tree under `result.root` and prunes
//! every node that does not satisfy `filter`. The returned
//! `ScanResult` re-aggregates `total_size` / `file_count` from the
//! pruned tree.

use domain::{FileNode, FileType, Filter, ScanResult};

/// Apply `filter` to `result`, returning a new `ScanResult` whose tree
/// contains only nodes for which [`Filter::matches`] returns `true`.
///
/// `max_depth` is honoured here in addition to the per-node matches:
/// when set, nodes deeper than `max_depth` (counting the root as depth
/// 0) are pruned. Directories that are pruned also have all of their
/// descendants pruned.
///
/// `filter.max_age` is evaluated against `filter.now`; if `now` is
/// `0` (the default), the age filter degrades to "no entries are
/// older than `now - max_age`" which is everything, so all entries
/// pass. Callers that want a real age filter must set `now` to the
/// current Unix timestamp before calling.
pub fn apply_filter(result: &ScanResult, filter: &Filter) -> ScanResult {
    let pruned = prune_node(&result.root, filter, 0);
    ScanResult::from_tree(pruned, result.scan_duration_ms)
}

/// Walks `node` and returns a new node with children that pass
/// `filter.matches`. `depth` is the current depth (root == 0).
///
/// If `node` itself fails the filter, the function returns a
/// zero-children stub marked `FileType::Directory`. The caller uses
/// the `was_pruned` predicate to decide whether to drop the stub.
fn prune_node(node: &FileNode, filter: &Filter, depth: usize) -> (FileNode, bool) {
    let depth_ok = filter.max_depth.map_or(true, |max| depth <= max);
    let passes = depth_ok && filter.matches(node);

    if !passes {
        // Return a stub preserving the path, but with no children.
        let stub = FileNode {
            path: node.path.clone(),
            size: 0,
            modified: node.modified,
            file_type: FileType::Directory,
            children: Vec::new(),
        };
        return (stub, true);
    }

    let mut new_children = Vec::with_capacity(node.children.len());
    for child in &node.children {
        let (child_pruned, child_pruned_flag) = prune_node(child, filter, depth + 1);
        if !child_pruned_flag {
            new_children.push(child_pruned);
        }
    }

    (
        FileNode {
            path: node.path.clone(),
            size: node.size,
            modified: node.modified,
            file_type: node.file_type,
            children: new_children,
        },
        false,
    )
}
