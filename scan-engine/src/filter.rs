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
    let (pruned, _pruned) = prune_node(&result.root, filter, 0);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Base filter with every dimension disabled; tests override fields.
    fn base_filter() -> Filter {
        Filter {
            min_size: None,
            max_size: None,
            file_types: None,
            name_pattern: None,
            max_age: None,
            now: 0,
            max_depth: None,
        }
    }

    fn leaf(path: &str, size: u64, modified: u64, file_type: FileType) -> FileNode {
        FileNode {
            path: path.into(),
            size,
            modified,
            file_type,
            children: vec![],
        }
    }

    /// Directory whose `size` is the recursive sum of its children
    /// (mirroring `normalize_sizes` in the scanner).
    fn dir(path: &str, modified: u64, children: Vec<FileNode>) -> FileNode {
        let size = children.iter().map(|c| c.size).sum();
        FileNode {
            path: path.into(),
            size,
            modified,
            file_type: FileType::Directory,
            children,
        }
    }

    fn paths(nodes: &[FileNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.path.as_str()).collect()
    }

    #[test]
    fn should_return_unchanged_tree_when_filter_is_default() {
        let original = ScanResult::from_tree(
            dir(
                "/project",
                0,
                vec![
                    leaf("/project/src/main.rs", 1000, 0, FileType::Code),
                    leaf("/project/README.md", 500, 0, FileType::Document),
                ],
            ),
            42,
        );

        let filtered = apply_filter(&original, &Filter::default());

        assert_eq!(filtered, original);
    }

    #[test]
    fn should_prune_files_below_min_size_when_min_size_set() {
        let result = ScanResult::from_tree(
            dir(
                "/project",
                0,
                vec![
                    leaf("/project/a.rs", 600, 0, FileType::Code),
                    leaf("/project/b.rs", 400, 0, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(&result, &Filter { min_size: Some(500), ..base_filter() });

        assert_eq!(paths(&filtered.root.children), vec!["/project/a.rs"]);
    }

    #[test]
    fn should_prune_files_above_max_size_when_max_size_set() {
        // Deliberately non-normalized root: its `size` is NOT the sum of
        // its children, so the root passes max_size while a child exceeds
        // it. (In a normalized tree, root.size = sum >= each child, so a
        // max_size that admits the root admits every child.)
        let root = FileNode {
            path: "/project".into(),
            size: 100,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![
                leaf("/project/big.bin", 600, 0, FileType::Other),
                leaf("/project/small.txt", 50, 0, FileType::Document),
            ],
        };
        let result = ScanResult::from_tree(root, 0);

        let filtered = apply_filter(&result, &Filter { max_size: Some(300), ..base_filter() });

        assert_eq!(paths(&filtered.root.children), vec!["/project/small.txt"]);
    }

    #[test]
    fn should_prune_files_older_than_max_age_when_max_age_and_now_set() {
        let result = ScanResult::from_tree(
            dir(
                "/project",
                950,
                vec![
                    leaf("/project/old.rs", 600, 500, FileType::Code),
                    leaf("/project/fresh.rs", 400, 980, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(
            &result,
            &Filter {
                max_age: Some(100),
                now: 1000,
                ..base_filter()
            },
        );

        assert_eq!(paths(&filtered.root.children), vec!["/project/fresh.rs"]);
    }

    #[test]
    fn should_keep_all_when_max_age_set_but_now_is_zero() {
        // `now == 0` (the default) means no entry is older than
        // `now - max_age`, so every entry passes the age filter.
        let original = ScanResult::from_tree(
            dir(
                "/project",
                900,
                vec![
                    leaf("/project/a.rs", 600, 500, FileType::Code),
                    leaf("/project/b.rs", 400, 100, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(
            &original,
            &Filter {
                max_age: Some(100),
                now: 0,
                ..base_filter()
            },
        );

        assert_eq!(filtered, original);
    }

    #[test]
    fn should_prune_by_name_pattern_when_pattern_set() {
        // Child paths are deliberately not prefixed with the root path so
        // the root itself matches the pattern while individual children do
        // not. (With scan-shaped, prefix-nested paths, a pattern matching
        // the root also matches every descendant.)
        let result = ScanResult::from_tree(
            dir(
                "proj-root",
                0,
                vec![
                    leaf("proj-src/main.rs", 100, 0, FileType::Code),
                    leaf("docs/readme.md", 50, 0, FileType::Document),
                ],
            ),
            0,
        );

        let filtered = apply_filter(
            &result,
            &Filter {
                name_pattern: Some("proj".into()),
                ..base_filter()
            },
        );

        assert_eq!(paths(&filtered.root.children), vec!["proj-src/main.rs"]);
    }

    #[test]
    fn should_prune_by_file_types_when_types_set() {
        // `Directory` is included so the root survives; the case where a
        // directory itself fails the type filter is covered by
        // `should_prune_entire_subtree_when_dir_fails_filter`.
        let result = ScanResult::from_tree(
            dir(
                "/media",
                0,
                vec![
                    leaf("/media/song.mp3", 100, 0, FileType::Audio),
                    leaf("/media/clip.mp4", 200, 0, FileType::Video),
                    leaf("/media/pic.png", 50, 0, FileType::Image),
                ],
            ),
            0,
        );

        let filtered = apply_filter(
            &result,
            &Filter {
                file_types: Some(vec![FileType::Directory, FileType::Audio]),
                ..base_filter()
            },
        );

        assert_eq!(paths(&filtered.root.children), vec!["/media/song.mp3"]);
    }

    #[test]
    fn should_prune_at_max_depth_when_max_depth_set() {
        let result = ScanResult::from_tree(
            dir(
                "/root",
                0,
                vec![
                    dir("/root/a", 0, vec![leaf("/root/a/a1.rs", 1500, 0, FileType::Code)]),
                    leaf("/root/b.rs", 500, 0, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(&result, &Filter { max_depth: Some(0), ..base_filter() });

        assert!(filtered.root.children.is_empty());
    }

    #[test]
    fn should_keep_direct_children_when_max_depth_is_one() {
        let result = ScanResult::from_tree(
            dir(
                "/root",
                0,
                vec![
                    dir("/root/a", 0, vec![leaf("/root/a/a1.rs", 1500, 0, FileType::Code)]),
                    leaf("/root/b.rs", 500, 0, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(&result, &Filter { max_depth: Some(1), ..base_filter() });

        assert_eq!(paths(&filtered.root.children), vec!["/root/a", "/root/b.rs"]);
    }

    #[test]
    fn should_set_total_size_to_zero_when_root_fails_filter() {
        // BUG: expected the root to either survive (keeping passing
        // children) or be dropped entirely; actual behavior replaces the
        // whole tree with a zero-size `Directory` stub, so `total_size`
        // collapses to 0 and every descendant is lost.
        let result = ScanResult::from_tree(
            dir(
                "/project",
                0,
                vec![
                    leaf("/project/a.rs", 600, 0, FileType::Code),
                    leaf("/project/b.rs", 400, 0, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(&result, &Filter { min_size: Some(5000), ..base_filter() });

        let stub = FileNode {
            path: "/project".into(),
            size: 0,
            modified: 0,
            file_type: FileType::Directory,
            children: vec![],
        };
        assert_eq!(filtered.root, stub);
    }

    #[test]
    fn should_prune_entire_subtree_when_dir_fails_filter() {
        // The root is typed `Audio` (not `Directory`) so it passes
        // `file_types=[Audio]` and we exercise an *inner* directory failing
        // the filter.
        // BUG: "/media/music" fails (a Directory is never Audio), so
        // prune_node stubs it without walking its children; song.mp3 — which
        // DOES match `file_types=[Audio]` — is lost with the subtree.
        // Expected: walk the failing directory's children and keep the ones
        // that pass.
        let root = FileNode {
            path: "/media".into(),
            size: 1000,
            modified: 0,
            file_type: FileType::Audio,
            children: vec![
                dir(
                    "/media/music",
                    0,
                    vec![
                        leaf("/media/music/song.mp3", 600, 0, FileType::Audio),
                        leaf("/media/music/clip.mp4", 300, 0, FileType::Video),
                    ],
                ),
                leaf("/media/podcast.mp3", 100, 0, FileType::Audio),
            ],
        };
        let result = ScanResult::from_tree(root, 0);

        let filtered = apply_filter(
            &result,
            &Filter {
                file_types: Some(vec![FileType::Audio]),
                ..base_filter()
            },
        );

        assert_eq!(paths(&filtered.root.children), vec!["/media/podcast.mp3"]);
    }

    #[test]
    fn should_keep_dir_size_when_dir_passes_but_children_pruned() {
        // BUG: the kept directory copies its pre-prune `size` (1000)
        // unchanged, so `total_size` stays stale. Expected: `size`
        // re-aggregated to 600 (the surviving child's size).
        let result = ScanResult::from_tree(
            dir(
                "/project",
                0,
                vec![
                    leaf("/project/a.rs", 600, 0, FileType::Code),
                    leaf("/project/b.rs", 400, 0, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(&result, &Filter { min_size: Some(500), ..base_filter() });

        assert_eq!(filtered.root.size, 1000);
    }

    #[test]
    fn should_set_file_count_correctly_after_pruning() {
        let result = ScanResult::from_tree(
            dir(
                "/project",
                0,
                vec![
                    leaf("/project/a.rs", 600, 0, FileType::Code),
                    leaf("/project/b.rs", 400, 0, FileType::Code),
                ],
            ),
            0,
        );

        let filtered = apply_filter(&result, &Filter { min_size: Some(500), ..base_filter() });

        assert_eq!(filtered.file_count, 2);
    }
}
