//! Recursive in-place sort of a `FileNode` tree.
//!
//! [`apply_sort`] sorts each level of a tree by the given [`SortSpec`].
//! It is a thin wrapper around `domain::SortSpec::apply` that does so
//! recursively down the tree.

use domain::{FileNode, SortSpec};

/// Recursively sort `entries` (and each of their subtrees) according
/// to `spec`. The sort is in place; the original `Vec` is reordered.
pub fn apply_sort(entries: &mut [FileNode], spec: SortSpec) {
    spec.apply(entries);
    for child in entries.iter_mut() {
        apply_sort(&mut child.children, spec);
    }
}

/// Recursively sort a [`ScanResult`]'s tree. Returns a new `ScanResult`
/// whose root's children are sorted in place.
pub fn apply_sort_result(result: &domain::ScanResult, spec: SortSpec) -> domain::ScanResult {
    let mut root = result.root.clone();
    apply_sort(&mut root.children, spec);
    domain::ScanResult::from_tree(root, result.scan_duration_ms)
}
