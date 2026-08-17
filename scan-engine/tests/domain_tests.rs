use std::path::PathBuf;

use scan_engine::filter::Filter;
use scan_engine::format;
use scan_engine::tree::TreeBuilder;
use scan_engine::{
    FileEntry, FilterSpec, NodeType, ScanError, ScanResult, SortDir, SortKey, TrashError,
    TreeNode,
};

// ── Helpers ────────────────────────────────────────────────────────────────

fn file(path: &str, size: u64) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        name: PathBuf::from(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        size,
        modified: 1_700_000_000,
        node_type: NodeType::File,
        depth: path.matches('/').count() as u32,
    }
}

fn dir(path: &str) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        name: PathBuf::from(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        size: 0,
        modified: 1_700_000_000,
        node_type: NodeType::Dir,
        depth: path.matches('/').count() as u32,
    }
}

fn make_result() -> ScanResult {
    TreeBuilder::build(vec![
        dir("/root"),
        file("/root/a.txt", 50),
        file("/root/b.rs", 200),
        file("/root/c.txt", 10),
        dir("/root/sub"),
        file("/root/sub/d.rs", 500),
        file("/root/sub/e.txt", 30),
    ])
}

// ── Test 1: should create FileEntry with valid path and size ───────────────

#[test]
fn should_create_file_entry_with_valid_path_and_size() {
    let entry = FileEntry {
        path: PathBuf::from("/home/user/docs/readme.txt"),
        name: "readme.txt".into(),
        size: 4096,
        modified: 1_700_000_000,
        node_type: NodeType::File,
        depth: 4,
    };
    assert_eq!(entry.name, "readme.txt");
    assert_eq!(entry.size, 4096);
    assert_eq!(entry.path, PathBuf::from("/home/user/docs/readme.txt"));
    assert_eq!(entry.node_type, NodeType::File);
}

// ── Test 2: should build tree with correct parent-child nesting ────────────

#[test]
fn should_build_tree_with_correct_parent_child_nesting() {
    let entries = vec![
        dir("/root"),
        dir("/root/sub"),
        file("/root/a.txt", 100),
        file("/root/sub/b.txt", 200),
    ];
    let result = TreeBuilder::build(entries);

    assert_eq!(result.root.entry.path, PathBuf::from("/root"));
    assert_eq!(result.root.children.len(), 2); // sub/ and a.txt

    let sub = result
        .root
        .children
        .iter()
        .find(|c| c.entry.node_type == NodeType::Dir)
        .expect("sub dir");
    assert_eq!(sub.children.len(), 1);
    assert_eq!(sub.children[0].entry.name, "b.txt");
}

// ── Test 3: should compute total_size recursively ──────────────────────────

#[test]
fn should_compute_total_size_recursively() {
    let entries = vec![
        dir("/root"),
        dir("/root/sub"),
        file("/root/a.txt", 50),
        file("/root/sub/b.txt", 30),
        file("/root/sub/c.txt", 70),
    ];
    let result = TreeBuilder::build(entries);
    assert_eq!(result.total_size, 150);
    assert_eq!(result.entry_count, 5);
}

// ── Test 4: Filter::apply with min_size ────────────────────────────────────

#[test]
fn should_filter_by_min_size() {
    let result = make_result();
    let spec = FilterSpec {
        min_size: Some(100),
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);
    assert_eq!(filtered.total_size, 700); // 200 + 500
}

// ── Test 5: Filter::apply with file types ──────────────────────────────────

#[test]
fn should_filter_by_file_type() {
    let result = make_result();
    let spec = FilterSpec {
        types: vec!["rs".to_string()],
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);

    let names = collect_file_names(&filtered.root);
    assert!(names.contains(&"b.rs".to_string()));
    assert!(names.contains(&"d.rs".to_string()));
    assert!(!names.contains(&"a.txt".to_string()));
}

// ── Test 6: Filter::apply with name pattern ────────────────────────────────

#[test]
fn should_filter_by_name_pattern() {
    let result = make_result();
    let spec = FilterSpec {
        pattern: Some("b".to_string()),
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);

    let names = collect_file_names(&filtered.root);
    assert!(names.contains(&"b.rs".to_string()));
    assert!(!names.contains(&"a.txt".to_string()));
}

// ── Test 7: Filter::apply with max_age_days ────────────────────────────────

#[test]
fn should_filter_by_max_age() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let recent = now - 86_400; // 1 day ago
    let old = 946_684_800; // 2000-01-01

    let entries = vec![
        dir("/root"),
        FileEntry {
            path: PathBuf::from("/root/old.txt"),
            name: "old.txt".into(),
            size: 100,
            modified: old,
            node_type: NodeType::File,
            depth: 1,
        },
        FileEntry {
            path: PathBuf::from("/root/new.txt"),
            name: "new.txt".into(),
            size: 100,
            modified: recent,
            node_type: NodeType::File,
            depth: 1,
        },
    ];
    let result = TreeBuilder::build(entries);

    let spec = FilterSpec {
        max_age_days: Some(30),
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);

    let names = collect_file_names(&filtered.root);
    assert!(names.contains(&"new.txt".to_string()));
    assert!(!names.contains(&"old.txt".to_string()));
}

// ── Test 8: compose multiple filters ───────────────────────────────────────

#[test]
fn should_compose_multiple_filters() {
    let result = make_result();
    let spec = FilterSpec {
        min_size: Some(20),
        types: vec!["txt".to_string()],
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);

    let names = collect_file_names(&filtered.root);
    // a.txt (50, txt, >=20) → kept
    // c.txt (10, txt, <20) → filtered
    // e.txt (30, txt, >=20) → kept
    // b.rs (200, rs, not txt) → filtered
    // d.rs (500, rs, not txt) → filtered
    assert!(names.contains(&"a.txt".to_string()));
    assert!(names.contains(&"e.txt".to_string()));
    assert!(!names.contains(&"c.txt".to_string()));
    assert!(!names.contains(&"b.rs".to_string()));
}

// ── Test 9: should sort children by size descending ────────────────────────

#[test]
fn should_sort_children_by_size_descending() {
    let entries = vec![
        dir("/root"),
        file("/root/small.txt", 10),
        file("/root/big.txt", 500),
        file("/root/medium.txt", 100),
    ];
    let result = TreeBuilder::build(entries);

    assert_eq!(result.root.children[0].entry.name, "big.txt");
    assert_eq!(result.root.children[1].entry.name, "medium.txt");
    assert_eq!(result.root.children[2].entry.name, "small.txt");
}

// ── Test 10: OutputFormat::Json ────────────────────────────────────────────

#[test]
fn should_format_as_json() {
    let result = TreeBuilder::build(vec![
        dir("/root"),
        file("/root/hello.txt", 512),
    ]);
    let output = format::json::format(&result);

    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("output must be valid JSON");
    assert_eq!(parsed["total_size"], 512);
    assert_eq!(parsed["entry_count"], 2);
    assert_eq!(parsed["root"]["name"], "root");
    assert_eq!(parsed["root"]["type"], "dir");
    assert_eq!(parsed["root"]["children"][0]["name"], "hello.txt");
    assert_eq!(parsed["root"]["children"][0]["size"], 512);
}

// ── Test 11: OutputFormat::Table ───────────────────────────────────────────

#[test]
fn should_format_as_table() {
    let result = make_result();
    let output = format::table::format(&result, None, None);

    assert!(output.contains("Name"), "header missing Name");
    assert!(output.contains("Size"), "header missing Size");
    assert!(output.contains("Modified"), "header missing Modified");
    assert!(output.contains("Type"), "header missing Type");
}

// ── Test 12: OutputFormat::Jsonl ───────────────────────────────────────────

#[test]
fn should_format_as_jsonl() {
    let result = TreeBuilder::build(vec![
        dir("/root"),
        file("/root/a.txt", 100),
        file("/root/b.rs", 256),
    ]);
    let output = format::jsonl::format(&result);
    let lines: Vec<&str> = output.trim().lines().collect();

    // First line = summary, then one line per node (root + 2 files = 3 entries).
    assert_eq!(lines.len(), 4, "expected 4 lines: summary + 3 entries");

    for (i, line) in lines.iter().enumerate() {
        let _: serde_json::Value =
            serde_json::from_str(line).expect(&format!("line {i} must be valid JSON"));
    }

    let summary: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(summary["total_size"], 356);
    assert_eq!(summary["entry_count"], 3);
}

// ── Test 13: OutputFormat::Tree ────────────────────────────────────────────

#[test]
fn should_format_as_tree() {
    let result = TreeBuilder::build(vec![
        dir("/root"),
        dir("/root/sub"),
        file("/root/sub/data.rs", 300),
        file("/root/README.md", 150),
    ]);
    let output = format::tree::format(&result);

    assert!(output.starts_with("root/"), "should start with root/");
    assert!(output.contains("[450 B]"), "total_size should be 450");
    assert!(output.contains("├──"), "should contain branch connector");
    assert!(output.contains("└──"), "should contain last-child connector");
    assert!(output.contains("│"), "should contain vertical connector");
    assert!(output.contains("data.rs"), "should contain nested file");
    assert!(output.contains("README.md"), "should contain sibling file");
}

// ── Test 14: ScanError Display ─────────────────────────────────────────────

#[test]
fn should_display_scan_error_variants_correctly() {
    let cases: Vec<(ScanError, &str)> = vec![
        (
            ScanError::IoError("disk full".into()),
            "scan I/O error: disk full",
        ),
        (
            ScanError::PermissionDenied("/secret".into()),
            "permission denied: /secret",
        ),
        (
            ScanError::InvalidPath("empty".into()),
            "invalid path: empty",
        ),
    ];
    for (err, expected) in cases {
        assert_eq!(format!("{err}"), expected);
        let _: &dyn std::error::Error = &err;
    }
}

// ── Test 15: TrashError Display ────────────────────────────────────────────

#[test]
fn should_display_trash_error_variants_correctly() {
    let cases: Vec<(TrashError, &str)> = vec![
        (
            TrashError::IoError("disk full".into()),
            "trash I/O error: disk full",
        ),
        (
            TrashError::FileNotFound("/gone".into()),
            "file not found: /gone",
        ),
        (
            TrashError::UndoFailed("no ticket".into()),
            "undo failed: no ticket",
        ),
    ];
    for (err, expected) in cases {
        assert_eq!(format!("{err}"), expected);
        let _: &dyn std::error::Error = &err;
    }
}

// ── Test 16: should calculate total_size and entry_count with mixed types ──

#[test]
fn should_calculate_total_size_in_file_tree() {
    let entries = vec![
        dir("/root"),
        file("/root/a.txt", 100),
        file("/root/b.txt", 200),
    ];
    let result = TreeBuilder::build(entries);
    assert_eq!(result.total_size, 300);
    assert_eq!(result.entry_count, 3);
}

// ── Test 17: should count files recursively when tree has nested children ──

#[test]
fn should_count_files_recursively_in_tree() {
    let entries = vec![
        dir("/root"),
        file("/root/a.txt", 10),
        file("/root/b.txt", 20),
        dir("/root/sub"),
        file("/root/sub/c.txt", 30),
    ];
    let result = TreeBuilder::build(entries);
    assert_eq!(result.entry_count, 5);
}

// ── Test 18: should match file when size within range filter ───────────────

#[test]
fn should_match_file_when_size_within_range() {
    let result = TreeBuilder::build(vec![
        dir("/root"),
        file("/root/mid.txt", 500),
    ]);
    let spec = FilterSpec {
        min_size: Some(100),
        max_size: Some(1000),
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);
    let names = collect_file_names(&filtered.root);
    assert!(names.contains(&"mid.txt".to_string()));
}

// ── Test 19: should reject file when size outside range filter ─────────────

#[test]
fn should_reject_file_when_size_outside_range() {
    let result = TreeBuilder::build(vec![
        dir("/root"),
        file("/root/tiny.txt", 50),
        file("/root/huge.txt", 99999),
    ]);
    let spec = FilterSpec {
        min_size: Some(100),
        max_size: Some(1000),
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);
    let names = collect_file_names(&filtered.root);
    assert!(!names.contains(&"tiny.txt".to_string()));
    assert!(!names.contains(&"huge.txt".to_string()));
}

// ── Test 20: should combine multiple filters with AND logic ────────────────

#[test]
fn should_combine_multiple_filters_with_and_logic() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let entries = vec![
        dir("/root"),
        FileEntry {
            path: PathBuf::from("/root/large.rs"),
            name: "large.rs".into(),
            size: 5000,
            modified: now - 60,
            node_type: NodeType::File,
            depth: 1,
        },
        file("/root/small.rs", 100),
        FileEntry {
            path: PathBuf::from("/root/large.txt"),
            name: "large.txt".into(),
            size: 5000,
            modified: now - 60,
            node_type: NodeType::File,
            depth: 1,
        },
        FileEntry {
            path: PathBuf::from("/root/old.rs"),
            name: "old.rs".into(),
            size: 5000,
            modified: 946_684_800,
            node_type: NodeType::File,
            depth: 1,
        },
    ];
    let result = TreeBuilder::build(entries);

    let spec = FilterSpec {
        min_size: Some(1000),
        types: vec!["rs".to_string()],
        max_age_days: Some(300),
        ..Default::default()
    };
    let filtered = Filter::apply(result, &spec);

    let names = collect_file_names(&filtered.root);
    assert!(names.contains(&"large.rs".to_string()));
    assert!(!names.contains(&"small.rs".to_string()));
    assert!(!names.contains(&"large.txt".to_string()));
    assert!(!names.contains(&"old.rs".to_string()));
}

// ── Test 21: table sort by size desc ───────────────────────────────────────

#[test]
fn table_sort_by_size_desc() {
    let result = make_result();
    let output = format::table::format(&result, Some(SortKey::Size), Some(SortDir::Desc));
    let lines: Vec<&str> = output.lines().collect();

    assert!(lines.len() >= 3);
    let main_pos = lines.iter().position(|l| l.contains("d.rs")).unwrap();
    let readme_pos = lines.iter().position(|l| l.contains("b.rs")).unwrap();
    assert!(
        main_pos < readme_pos,
        "d.rs (500) should come before b.rs (200)"
    );
}

// ── Test 22: table sort by name ascending ──────────────────────────────────

#[test]
fn table_sort_by_name_asc() {
    let result = make_result();
    let output = format::table::format(&result, Some(SortKey::Name), Some(SortDir::Asc));
    let lines: Vec<&str> = output.lines().collect();

    let a_pos = lines.iter().position(|l| l.contains("a.txt")).unwrap();
    let b_pos = lines.iter().position(|l| l.contains("b.rs")).unwrap();
    assert!(
        a_pos < b_pos,
        "a.txt should come before b.rs alphabetically"
    );
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn collect_file_names(node: &TreeNode) -> Vec<String> {
    let mut names = Vec::new();
    for child in &node.children {
        if child.entry.node_type == NodeType::File {
            names.push(child.entry.name.clone());
        }
        names.extend(collect_file_names(child));
    }
    names
}
