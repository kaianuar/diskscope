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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{FileType, ScanResult, SortColumn, SortDirection};

    fn leaf(path: &str, size: u64) -> FileNode {
        FileNode {
            path: path.into(),
            size,
            modified: 0,
            file_type: FileType::Other,
            children: vec![],
        }
    }

    fn dir(path: &str, children: Vec<FileNode>) -> FileNode {
        FileNode {
            path: path.into(),
            size: children.iter().map(|c| c.size).sum(),
            modified: 0,
            file_type: FileType::Directory,
            children,
        }
    }

    fn size_spec(direction: SortDirection) -> SortSpec {
        SortSpec {
            column: SortColumn::Size,
            direction,
        }
    }

    fn name_spec(direction: SortDirection) -> SortSpec {
        SortSpec {
            column: SortColumn::Name,
            direction,
        }
    }

    #[test]
    fn should_sort_by_size_descending_when_spec_says_desc() {
        // Arrange
        let mut entries = vec![
            leaf("small", 10),
            leaf("large", 300),
            leaf("medium", 100),
        ];

        // Act
        apply_sort(&mut entries, size_spec(SortDirection::Descending));

        // Assert
        let sizes: Vec<u64> = entries.iter().map(|e| e.size).collect();
        assert_eq!(sizes, vec![300, 100, 10]);
    }

    #[test]
    fn should_sort_by_size_ascending_when_spec_says_asc() {
        // Arrange
        let mut entries = vec![
            leaf("large", 300),
            leaf("small", 10),
            leaf("medium", 100),
        ];

        // Act
        apply_sort(&mut entries, size_spec(SortDirection::Ascending));

        // Assert
        let sizes: Vec<u64> = entries.iter().map(|e| e.size).collect();
        assert_eq!(sizes, vec![10, 100, 300]);
    }

    #[test]
    fn should_sort_by_name_ascending_when_spec_says_name_asc() {
        // Arrange
        let mut entries = vec![
            leaf("zeta", 1),
            leaf("alpha", 1),
            leaf("mid", 1),
        ];

        // Act
        apply_sort(&mut entries, name_spec(SortDirection::Ascending));

        // Assert
        let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn should_sort_by_name_descending_when_spec_says_name_desc() {
        // Arrange
        let mut entries = vec![
            leaf("alpha", 1),
            leaf("mid", 1),
            leaf("zeta", 1),
        ];

        // Act
        apply_sort(&mut entries, name_spec(SortDirection::Descending));

        // Assert
        let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(names, vec!["zeta", "mid", "alpha"]);
    }

    #[test]
    fn should_sort_recursively_when_nested_tree() {
        // Arrange
        let mut root = dir("root", vec![
            dir("b_dir", vec![leaf("b2", 20), leaf("b1", 10)]),
            leaf("a_file", 5),
            dir("c_dir", vec![leaf("c2", 2), leaf("c1", 1)]),
        ]);

        // Act
        apply_sort(&mut root.children, size_spec(SortDirection::Descending));

        // Assert
        let top_names: Vec<&str> = root.children.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(top_names, vec!["b_dir", "a_file", "c_dir"]);
        let b_sizes: Vec<u64> = root.children[0]
            .children
            .iter()
            .map(|c| c.size)
            .collect();
        assert_eq!(b_sizes, vec![20, 10]);
        let c_sizes: Vec<u64> = root.children[2]
            .children
            .iter()
            .map(|c| c.size)
            .collect();
        assert_eq!(c_sizes, vec![2, 1]);
    }

    #[test]
    fn should_stabilize_equal_elements_when_same_size() {
        // Arrange
        let mut entries = vec![
            leaf("third", 100),
            leaf("first", 100),
            leaf("second", 100),
        ];

        // Act
        apply_sort(&mut entries, size_spec(SortDirection::Descending));

        // Assert
        let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(names, vec!["third", "first", "second"]);
    }

    #[test]
    fn should_handle_empty_children_when_no_children() {
        // Arrange
        let mut entries: Vec<FileNode> = vec![];

        // Act
        apply_sort(&mut entries, size_spec(SortDirection::Descending));

        // Assert
        assert!(entries.is_empty());
    }

    #[test]
    fn should_handle_single_child_when_one_entry() {
        // Arrange
        let mut entries = vec![leaf("only", 42)];

        // Act
        apply_sort(&mut entries, size_spec(SortDirection::Descending));

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "only");
    }

    #[test]
    fn should_preserve_root_when_apply_sort_result_called() {
        // Arrange
        let root = dir("root", vec![
            leaf("b", 2),
            leaf("a", 1),
            leaf("c", 3),
        ]);
        let result = ScanResult::from_tree(root, 7);

        // Act
        let sorted = apply_sort_result(&result, size_spec(SortDirection::Ascending));

        // Assert
        assert_eq!(sorted.root.path, "root");
        assert_eq!(sorted.root.file_type, FileType::Directory);
        let child_names: Vec<&str> = sorted.root.children.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(child_names, vec!["a", "b", "c"]);
    }

    #[test]
    fn should_preserve_scan_duration_ms_when_apply_sort_result_called() {
        // Arrange
        let root = dir("root", vec![leaf("b", 2), leaf("a", 1)]);
        let result = ScanResult::from_tree(root, 1234);

        // Act
        let sorted = apply_sort_result(&result, size_spec(SortDirection::Ascending));

        // Assert
        assert_eq!(sorted.scan_duration_ms, 1234);
    }
}
