//! Duplicate-file detection domain types.
//!
//! [`DuplicateGroup`] clusters files that share identical content (by hash),
//! and [`DuplicateReport`] aggregates all groups found during a scan.
//! These are pure value types — no I/O, no serde, no adapter dependencies.

/// A set of files whose content is byte-for-byte identical.
///
/// Invariant: `files` always contains **at least two** entries (a single
/// file is never a "duplicate"). The `hash` is the hex-encoded SHA-256
/// digest that proved equality; `size` is the byte length of every file
/// in the group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// Hex-encoded SHA-256 content hash shared by every file in this group.
    pub hash: String,
    /// Size in bytes of each file (all files in the group are the same size).
    pub size: u64,
    /// Absolute paths of the duplicate files. Always `len() >= 2`.
    pub files: Vec<String>,
}

impl DuplicateGroup {
    /// Bytes that could be recovered by keeping exactly one copy and
    /// deleting the rest.
    ///
    /// Equals `size × (files.len() − 1)`.
    pub fn recoverable_bytes(&self) -> u64 {
        // Safe: `files` is guaranteed to have >= 2 entries by construction.
        self.size * (self.files.len() as u64 - 1)
    }
}

/// The complete result of a duplicate-file scan.
///
/// Built by the scan-engine adapter; consumed by the CLI / GUI for
/// display and by the delete path for bulk trash operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateReport {
    /// Groups of files with identical content. Empty when no duplicates
    /// were found.
    pub groups: Vec<DuplicateGroup>,
    /// Sum of [`DuplicateGroup::recoverable_bytes`] across all groups.
    pub total_recoverable: u64,
    /// Total number of *extra* files (i.e. `Σ (group.files.len() − 1)`).
    pub total_duplicate_files: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_compute_recoverable_bytes_for_two_file_group() {
        let group = DuplicateGroup {
            hash: "abc123".into(),
            size: 1024,
            files: vec!["/a".into(), "/b".into()],
        };
        assert_eq!(group.recoverable_bytes(), 1024);
    }

    #[test]
    fn should_compute_recoverable_bytes_for_three_file_group() {
        let group = DuplicateGroup {
            hash: "abc123".into(),
            size: 500,
            files: vec!["/a".into(), "/b".into(), "/c".into()],
        };
        assert_eq!(group.recoverable_bytes(), 1000);
    }

    #[test]
    fn should_aggregate_totals_in_report() {
        let report = DuplicateReport {
            groups: vec![
                DuplicateGroup {
                    hash: "aaa".into(),
                    size: 100,
                    files: vec!["/a".into(), "/b".into()],
                },
                DuplicateGroup {
                    hash: "bbb".into(),
                    size: 200,
                    files: vec!["/c".into(), "/d".into(), "/e".into()],
                },
            ],
            total_recoverable: 500,
            total_duplicate_files: 3,
        };
        assert_eq!(report.total_recoverable, 500);
        assert_eq!(report.total_duplicate_files, 3);
    }
}
