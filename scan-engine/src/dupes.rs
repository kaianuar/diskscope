//! Duplicate-file detection pipeline.
//!
//! Operates on an **already-scanned** [`FileNode`] tree — no filesystem
//! re-walk. The algorithm is:
//!
//! 1. Flatten the tree to leaf (file) nodes with `size >= min_size`.
//! 2. Group by size; only sizes with ≥ 2 files are candidates.
//! 3. For each candidate group: hash the first 64 KB (partial hash);
//!    group by `(size, partial_hash)`. For groups still ≥ 2: compute
//!    full SHA-256; group by full hash.
//! 4. Groups with ≥ 2 files sharing a full hash become
//!    [`DuplicateGroup`] entries.
//!
//! Files that cannot be read (permissions, vanished) are silently
//! skipped; the scan continues.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use sha2::{Digest, Sha256};

use domain::dupes::{DuplicateGroup, DuplicateReport};
use domain::FileNode;

/// Size of the partial-hash read buffer (64 KiB).
const PARTIAL_HASH_BYTES: usize = 64 * 1024;

/// Default minimum file size to consider (1 MiB).
pub const DEFAULT_MIN_SIZE: u64 = 1_048_576;

/// Find duplicate files in an already-scanned [`FileNode`] tree.
///
/// `min_size` skips files smaller than the threshold (default 1 MiB).
/// `max_candidates` bounds the total number of file groups examined to
/// cap I/O (default 5000).
pub fn find_duplicates(root: &FileNode, min_size: u64, max_candidates: usize) -> DuplicateReport {
    // 1. Flatten to leaf files meeting the size threshold.
    let mut files: Vec<&FileNode> = Vec::new();
    collect_leaves(root, min_size, &mut files);

    // 2. Group by size.
    let by_size = group_by_size(&files);

    // 3. Partial hash pass.
    let mut candidates_examined = 0usize;
    let mut full_hash_targets: Vec<&FileNode> = Vec::new();

    for group in by_size.values() {
        if group.len() < 2 {
            continue;
        }
        let by_partial = group_by_partial_hash(group);
        for (_key, bucket) in by_partial {
            if bucket.len() < 2 {
                continue;
            }
            if candidates_examined >= max_candidates {
                break;
            }
            candidates_examined += 1;
            full_hash_targets.extend(bucket.iter());
        }
        if candidates_examined >= max_candidates {
            break;
        }
    }

    // 4. Full hash pass.
    let by_full = group_by_full_hash(&full_hash_targets);

    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut total_recoverable = 0u64;
    let mut total_duplicate_files = 0usize;

    for (hash, bucket) in by_full {
        if bucket.len() < 2 {
            continue;
        }
        let size = bucket[0].size;
        let files: Vec<String> = bucket.iter().map(|n| n.path.clone()).collect();
        let dup_count = files.len() - 1;
        let group = DuplicateGroup { hash, size, files };
        total_recoverable += group.recoverable_bytes();
        total_duplicate_files += dup_count;
        groups.push(group);
    }

    // Sort largest recoverable first.
    groups.sort_by_key(|b| std::cmp::Reverse(b.recoverable_bytes()));

    DuplicateReport { groups, total_recoverable, total_duplicate_files }
}

// ── Internal helpers ──────────────────────────────────────────────────────

/// Recursively collect leaf (file) nodes with `size >= min_size`.
fn collect_leaves<'a>(node: &'a FileNode, min_size: u64, out: &mut Vec<&'a FileNode>) {
    if node.children.is_empty() {
        if node.size >= min_size {
            out.push(node);
        }
    } else {
        for child in &node.children {
            collect_leaves(child, min_size, out);
        }
    }
}

/// Group files by size. Only groups with ≥ 2 entries are kept.
fn group_by_size<'a>(files: &[&'a FileNode]) -> HashMap<u64, Vec<&'a FileNode>> {
    let mut map: HashMap<u64, Vec<&'a FileNode>> = HashMap::new();
    for f in files {
        map.entry(f.size).or_default().push(f);
    }
    map
}

/// Compute a partial hash (first 64 KB) for a file. Returns `None` on
/// read error (file vanished, permissions, etc.).
fn partial_hash(path: &str) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; PARTIAL_HASH_BYTES];
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);
    let mut hasher = Sha256::new();
    hasher.update(&buf);
    Some(hex_encode(hasher.finalize()))
}

/// Compute a full SHA-256 hash of a file. Returns `None` on read error.
fn full_hash(path: &str) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return None,
        }
    }
    Some(hex_encode(hasher.finalize()))
}

/// Group files by `(size, partial_hash)`. Files whose partial hash
/// fails are silently dropped.
fn group_by_partial_hash<'a>(files: &[&'a FileNode]) -> HashMap<String, Vec<&'a FileNode>> {
    let mut map: HashMap<String, Vec<&'a FileNode>> = HashMap::new();
    for f in files {
        if let Some(ph) = partial_hash(&f.path) {
            let key = format!("{}:{}", f.size, ph);
            map.entry(key).or_default().push(f);
        }
    }
    map
}

/// Group files by full SHA-256 hash. Files whose full hash fails are
/// silently dropped.
fn group_by_full_hash<'a>(files: &[&'a FileNode]) -> HashMap<String, Vec<&'a FileNode>> {
    let mut map: HashMap<String, Vec<&FileNode>> = HashMap::new();
    for f in files {
        if let Some(fh) = full_hash(&f.path) {
            map.entry(fh).or_default().push(f);
        }
    }
    map
}

/// Encode bytes as lowercase hex without pulling in a hex crate.
fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::FileType;
    use tempfile::TempDir;

    /// Helper: create a `FileNode` pointing at a real file.
    fn file_node(path: &str, size: u64) -> FileNode {
        FileNode {
            path: path.to_string(),
            size,
            modified: 0,
            file_type: FileType::Other,
            children: Vec::new(),
        }
    }

    /// Helper: build a directory `FileNode` wrapping children.
    fn dir_node(path: &str, children: Vec<FileNode>) -> FileNode {
        let size = children.iter().map(|c| c.size).sum();
        FileNode {
            path: path.to_string(),
            size,
            modified: 0,
            file_type: FileType::Directory,
            children,
        }
    }

    #[test]
    fn should_group_identical_files_by_hash() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        std::fs::write(&a, vec![0xAB_u8; 2048]).unwrap();
        std::fs::write(&b, vec![0xAB_u8; 2048]).unwrap();

        let root = dir_node(
            tmp.path().to_str().unwrap(),
            vec![file_node(a.to_str().unwrap(), 2048), file_node(b.to_str().unwrap(), 2048)],
        );

        let report = find_duplicates(&root, 0, 5000);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 2);
        assert_eq!(report.groups[0].size, 2048);
        assert_eq!(report.groups[0].recoverable_bytes(), 2048);
        assert_eq!(report.total_recoverable, 2048);
        assert_eq!(report.total_duplicate_files, 1);
    }

    #[test]
    fn should_ignore_files_below_min_size() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("small_a.bin");
        let b = tmp.path().join("small_b.bin");
        std::fs::write(&a, vec![0_u8; 100]).unwrap();
        std::fs::write(&b, vec![0_u8; 100]).unwrap();

        let root = dir_node(
            tmp.path().to_str().unwrap(),
            vec![file_node(a.to_str().unwrap(), 100), file_node(b.to_str().unwrap(), 100)],
        );

        // min_size = 1024 → both files are below threshold
        let report = find_duplicates(&root, 1024, 5000);
        assert!(report.groups.is_empty());
        assert_eq!(report.total_recoverable, 0);
    }

    #[test]
    fn should_not_group_files_with_same_size_but_different_content() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        std::fs::write(&a, vec![0xAA_u8; 2048]).unwrap();
        std::fs::write(&b, vec![0xBB_u8; 2048]).unwrap();

        let root = dir_node(
            tmp.path().to_str().unwrap(),
            vec![file_node(a.to_str().unwrap(), 2048), file_node(b.to_str().unwrap(), 2048)],
        );

        let report = find_duplicates(&root, 0, 5000);
        assert!(report.groups.is_empty());
    }

    #[test]
    fn should_group_three_identical_files() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        let c = tmp.path().join("c.bin");
        let content = vec![0x42_u8; 4096];
        std::fs::write(&a, &content).unwrap();
        std::fs::write(&b, &content).unwrap();
        std::fs::write(&c, &content).unwrap();

        let root = dir_node(
            tmp.path().to_str().unwrap(),
            vec![
                file_node(a.to_str().unwrap(), 4096),
                file_node(b.to_str().unwrap(), 4096),
                file_node(c.to_str().unwrap(), 4096),
            ],
        );

        let report = find_duplicates(&root, 0, 5000);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 3);
        assert_eq!(report.groups[0].recoverable_bytes(), 4096 * 2);
        assert_eq!(report.total_duplicate_files, 2);
    }

    #[test]
    fn should_skip_unreadable_files() {
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real.bin");
        let phantom = tmp.path().join("phantom.bin");
        std::fs::write(&real, vec![0x01_u8; 2048]).unwrap();
        // phantom does NOT exist on disk — partial_hash/full_hash will
        // return None and the file is silently skipped.

        let root = dir_node(
            tmp.path().to_str().unwrap(),
            vec![
                file_node(real.to_str().unwrap(), 2048),
                file_node(phantom.to_str().unwrap(), 2048),
            ],
        );

        let report = find_duplicates(&root, 0, 5000);
        // No panic, no group (only one readable file with that size).
        assert!(report.groups.is_empty());
    }

    #[test]
    fn should_return_empty_report_when_no_duplicates() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        std::fs::write(&a, vec![0xAA_u8; 1024]).unwrap();
        std::fs::write(&b, vec![0xBB_u8; 2048]).unwrap();

        let root = dir_node(
            tmp.path().to_str().unwrap(),
            vec![file_node(a.to_str().unwrap(), 1024), file_node(b.to_str().unwrap(), 2048)],
        );

        let report = find_duplicates(&root, 0, 5000);
        assert!(report.groups.is_empty());
        assert_eq!(report.total_recoverable, 0);
        assert_eq!(report.total_duplicate_files, 0);
    }
}
