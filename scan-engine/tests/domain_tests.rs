use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use scan_engine::{
    DomainError, FileNode, Filter, OutputFormat, SortKey,
};

// ---------- helpers ----------

fn make_node(name: &str, size: u64) -> FileNode {
    FileNode::new(
        PathBuf::from(format!("/tmp/{}", name)),
        name.to_string(),
        size,
        1_700_000_000, // arbitrary fixed mtime
        false,
    )
    .unwrap()
}

fn make_dir(name: &str, children: Vec<FileNode>) -> FileNode {
    let mut node = FileNode::new(
        PathBuf::from(format!("/tmp/{}", name)),
        name.to_string(),
        0,
        1_700_000_000,
        true,
    )
    .unwrap();
    node.children = children;
    node
}

fn recent_mtime() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 60 // 1 minute ago — always within any MaxAge ≥ 1 min
}

fn old_mtime() -> u64 {
    1_000_000_000 // ~2001
}

// ====================================================================
// Test 1: should create FileNode with valid path and size
// ====================================================================
#[test]
fn should_create_filenode_with_valid_path_and_size() {
    let node = FileNode::new(
        PathBuf::from("/home/user/docs/readme.txt"),
        "readme.txt".into(),
        4096,
        1_700_000_000,
        false,
    )
    .unwrap();

    assert_eq!(node.name, "readme.txt");
    assert_eq!(node.size, 4096);
    assert_eq!(node.path, PathBuf::from("/home/user/docs/readme.txt"));
    assert!(!node.is_dir);
    assert!(node.children.is_empty());
}

// ====================================================================
// Test 2: should reject FileNode with empty path
// ====================================================================
#[test]
fn should_reject_filenode_with_empty_path() {
    let result = FileNode::new(PathBuf::new(), "".into(), 0, 0, false);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), DomainError::InvalidPath("path is empty".into()));
}

// ====================================================================
// Test 3: should compute total_size recursively
// ====================================================================
#[test]
fn should_compute_total_size_recursively() {
    let child_a = make_node("a.txt", 100);
    let child_b = make_node("b.txt", 200);
    let child_c = make_node("c.txt", 300);
    let sub_dir = make_dir("sub", vec![child_c]);
    let root = make_dir("root", vec![child_a, child_b, sub_dir]);

    assert_eq!(root.total_size(), 600); // 100 + 200 + 300
}

// ====================================================================
// Test 4: Filter::MinSize
// ====================================================================
#[test]
fn should_filter_by_min_size() {
    let small = make_node("small.txt", 100);
    let big = make_node("big.txt", 5000);
    let root = make_dir("root", vec![small, big]);

    let filtered = root.filter(&[Filter::MinSize(500)], None).unwrap();
    assert_eq!(filtered.children.len(), 1);
    assert_eq!(filtered.children[0].name, "big.txt");
}

// ====================================================================
// Test 5: Filter::Extension
// ====================================================================
#[test]
fn should_filter_by_extension() {
    let rs = make_node("main.rs", 200);
    let txt = make_node("readme.txt", 300);
    let rs2 = make_node("lib.RS", 150); // case-insensitive
    let root = make_dir("root", vec![rs, txt, rs2]);

    let filtered = root.filter(&[Filter::Extension("rs".into())], None).unwrap();
    assert_eq!(filtered.children.len(), 2);
    let names: Vec<&str> = filtered.children.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"main.rs"));
    assert!(names.contains(&"lib.RS"));
}

// ====================================================================
// Test 6: Filter::MaxAge
// ====================================================================
#[test]
fn should_filter_by_max_age() {
    let recent = {
        let mut n = make_node("recent.log", 500);
        n.mtime = recent_mtime();
        n
    };
    let old = {
        let mut n = make_node("archive.tar", 800);
        n.mtime = old_mtime();
        n
    };
    let root = make_dir("root", vec![recent, old]);

    let max_age = Duration::from_secs(300); // 5 minutes
    let filtered = root.filter(&[Filter::MaxAge(max_age)], None).unwrap();
    assert_eq!(filtered.children.len(), 1);
    assert_eq!(filtered.children[0].name, "recent.log");
}

// ====================================================================
// Test 7: Filter::NamePattern
// ====================================================================
#[test]
fn should_filter_by_name_pattern() {
    let test1 = make_node("test_main.rs", 100);
    let test2 = make_node("test_lib.rs", 200);
    let src = make_node("main.rs", 300);
    let root = make_dir("root", vec![test1, test2, src]);

    let filtered = root.filter(&[Filter::NamePattern("test_*".into())], None).unwrap();
    assert_eq!(filtered.children.len(), 2);
    for child in &filtered.children {
        assert!(child.name.starts_with("test_"));
    }
}

// ====================================================================
// Test 8: SortKey::SizeDesc
// ====================================================================
#[test]
fn should_sort_children_by_size_descending() {
    let small = make_node("a.txt", 100);
    let big = make_node("b.txt", 900);
    let mid = make_node("c.txt", 500);
    let root = make_dir("root", vec![small, big, mid]);

    let sorted = root.sort(SortKey::SizeDesc);
    let sizes: Vec<u64> = sorted.children.iter().map(|c| c.total_size()).collect();
    assert_eq!(sizes, vec![900, 500, 100]);
}

// ====================================================================
// Test 9: SortKey::NameAsc
// ====================================================================
#[test]
fn should_sort_children_by_name_ascending() {
    let z = make_node("zebra.txt", 10);
    let a = make_node("alpha.txt", 20);
    let m = make_node("mu.txt", 30);
    let root = make_dir("root", vec![z, a, m]);

    let sorted = root.sort(SortKey::NameAsc);
    let names: Vec<&str> = sorted.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["alpha.txt", "mu.txt", "zebra.txt"]);
}

// ====================================================================
// Test 10: OutputFormat::Json
// ====================================================================
#[test]
fn should_format_as_json() {
    let child = make_node("a.txt", 100);
    let root = make_dir("src", vec![child]);
    let json = root.format(OutputFormat::Json).unwrap();

    assert!(json.contains("\"name\":\"src\""));
    assert!(json.contains("\"is_dir\":true"));
    assert!(json.contains("\"name\":\"a.txt\""));
    assert!(json.contains("\"size\":100"));
    assert!(json.contains("\"total_size\":100"));
    assert!(json.contains("\"children\":["));
}

// ====================================================================
// Test 11: OutputFormat::Table
// ====================================================================
#[test]
fn should_format_as_table() {
    let child_a = make_node("alpha.txt", 1024);
    let child_b = make_node("beta.txt", 2_097_152); // 2 MB
    let root = make_dir("root", vec![child_a, child_b]);
    let table = root.format(OutputFormat::Table).unwrap();

    let lines: Vec<&str> = table.trim().lines().collect();
    assert_eq!(lines[0], "Name\tSize\tModified\tType");
    assert!(lines[1].contains("alpha.txt"));
    assert!(lines[1].contains("1.0 KB"));
    assert!(lines[2].contains("beta.txt"));
    assert!(lines[2].contains("2.0 MB"));
}

// ====================================================================
// Test 12: DomainError Display
// ====================================================================
#[test]
fn should_display_domain_error_variants_correctly() {
    let cases: Vec<(DomainError, &str)> = vec![
        (DomainError::InvalidPath("empty".into()), "invalid path: empty"),
        (DomainError::ScanFailed("denied".into()), "scan failed: denied"),
        (DomainError::CacheFailed("corrupt".into()), "cache failed: corrupt"),
        (DomainError::TrashFailed("unavail".into()), "trash failed: unavail"),
        (DomainError::FilterFailed("bad pat".into()), "filter failed: bad pat"),
    ];

    for (err, expected) in cases {
        assert_eq!(format!("{}", err), expected);
        // Also verify std::error::Error is implemented
        let _: &dyn std::error::Error = &err;
    }
}

// ====================================================================
// Test 13: compose multiple filters
// ====================================================================
#[test]
fn should_compose_multiple_filters() {
    let a = {
        let mut n = make_node("big.rs", 5000);
        n.mtime = recent_mtime();
        n
    };
    let b = make_node("small.rs", 100); // too small
    let c = {
        let mut n = make_node("big.txt", 8000);
        n.mtime = recent_mtime();
        n
    };
    let d = {
        let mut n = make_node("old.rs", 6000);
        n.mtime = old_mtime();
        n
    };
    let root = make_dir("root", vec![a, b, c, d]);

    let filters = vec![
        Filter::MinSize(1000),
        Filter::Extension("rs".into()),
        Filter::MaxAge(Duration::from_secs(300)),
    ];
    let filtered = root.filter(&filters, None).unwrap();

    // Only "big.rs" satisfies all three: size >= 1000, ext == "rs", age <= 5 min
    assert_eq!(filtered.children.len(), 1);
    assert_eq!(filtered.children[0].name, "big.rs");
}

// ====================================================================
// Test 14: respect max_depth
// ====================================================================
#[test]
fn should_respect_max_depth() {
    let leaf = make_node("deep.txt", 42);
    let level2 = make_dir("level2", vec![leaf]);
    let level1 = make_dir("level1", vec![level2]);
    let root = make_dir("root", vec![level1]);

    // depth=1 should keep root and level1 children, but prune level2's children
    let filtered = root.filter(&[], Some(1)).unwrap();
    assert_eq!(filtered.name, "root");
    assert_eq!(filtered.children.len(), 1); // level1
    assert_eq!(filtered.children[0].name, "level1");
    assert!(filtered.children[0].children.is_empty()); // pruned at depth limit

    // depth=0 should prune root's children entirely
    let filtered0 = root.filter(&[], Some(0)).unwrap();
    assert!(filtered0.children.is_empty());
}
