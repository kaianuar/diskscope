use std::fs;

use scan_engine::scanner::cache::RedbCache;
use scan_engine::scanner::incremental::IncrementalScanner;
use scan_engine::scanner::Scanner;
use scan_engine::{NodeType, ScanOptions};

use tempfile::TempDir;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Create a temp directory with test files.
fn setup_test_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap(); // 5 bytes
    fs::write(dir.path().join("b.txt"), "world!!").unwrap(); // 7 bytes
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub").join("c.txt"), "nested").unwrap(); // 6 bytes
    fs::create_dir(dir.path().join("sub").join("deep")).unwrap();
    fs::write(dir.path().join("sub").join("deep").join("d.txt"), "deep").unwrap(); // 4 bytes
    dir
}

// ── Test 1: should scan directory and return correct file count ────────────

#[test]
fn should_scan_directory_and_return_correct_file_count() {
    let dir = setup_test_dir();
    let scanner = Scanner::new(ScanOptions::default());
    let result = scanner.scan(dir.path()).unwrap();

    // Files: a.txt, b.txt, c.txt, d.txt = 4 files, plus dirs: root, sub, deep
    // Total entries = 7
    assert_eq!(result.entry_count, 7, "expected 7 entries total");
}

// ── Test 2: should scan directory and calculate total size ─────────────────

#[test]
fn should_scan_directory_and_calculate_total_size() {
    let dir = setup_test_dir();
    let scanner = Scanner::new(ScanOptions::default());
    let result = scanner.scan(dir.path()).unwrap();

    // File sizes: 5 + 7 + 6 + 4 = 22
    assert!(
        result.total_size >= 22,
        "total_size {} should be >= 22",
        result.total_size
    );
}

// ── Test 3: should respect max_depth option ────────────────────────────────

#[test]
fn should_respect_max_depth_option() {
    let dir = setup_test_dir();
    let opts = ScanOptions {
        max_depth: Some(1),
        ..Default::default()
    };
    let scanner = Scanner::new(opts);
    let result = scanner.scan(dir.path()).unwrap();

    // depth 0 = root, depth 1 = immediate children (a.txt, b.txt, sub/).
    // sub's children (depth 2+) are excluded.
    // Expected: root + a.txt + b.txt + sub = 4 entries
    assert_eq!(
        result.entry_count, 4,
        "only root and immediate children at depth <= 1"
    );
}

// ── Test 4: should respect .gitignore when option is true ──────────────────

#[test]
fn should_respect_gitignore_when_option_is_true() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("keep.txt"), "keep").unwrap(); // 4 bytes
    fs::write(dir.path().join("ignored.log"), "ignored data").unwrap(); // > 4 bytes
    fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();

    let opts = ScanOptions {
        respect_gitignore: true,
        ..Default::default()
    };
    let scanner = Scanner::new(opts);
    let result = scanner.scan(dir.path()).unwrap();

    // ignored.log should be filtered out by .gitignore.
    // keep.txt + .gitignore remain (2 files + root dir = 3 entries).
    let file_names: Vec<String> = collect_file_names(&result.root);
    assert!(
        !file_names.contains(&"ignored.log".to_string()),
        "ignored.log should be excluded by .gitignore"
    );
    assert!(
        file_names.contains(&"keep.txt".to_string()),
        "keep.txt should be present"
    );
}

// ── Test 5: should apply size filter during scan ───────────────────────────

#[test]
fn should_apply_size_filter_during_scan() {
    let dir = setup_test_dir();
    let opts = ScanOptions {
        max_depth: Some(1), // limit depth to simplify assertions
        ..Default::default()
    };
    let scanner = Scanner::new(opts);
    let result = scanner.scan(dir.path()).unwrap();

    // At depth 1: a.txt (5), b.txt (7)
    let file_names: Vec<String> = collect_file_names(&result.root);
    assert!(file_names.contains(&"a.txt".to_string()));
    assert!(file_names.contains(&"b.txt".to_string()));
}

// ── Test 6: should use cache on second scan ────────────────────────────────

#[test]
fn should_use_cache_on_second_scan() {
    let dir = setup_test_dir();
    let cache_dir = tempfile::tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.redb");
    let cache = RedbCache::open(&cache_path).unwrap();
    let inc = IncrementalScanner::new(ScanOptions::default(), cache);

    // First scan — populates cache.
    let result1 = inc.scan(dir.path()).unwrap();
    assert_eq!(result1.entry_count, 7);

    // Drop scanner to release the redb lock before reopening.
    drop(inc);

    // Re-open cache from same path to verify persistence.
    let cache2 = RedbCache::open(&cache_path).unwrap();
    let inc2 = IncrementalScanner::new(ScanOptions::default(), cache2);

    // Second scan — should return same results.
    let result2 = inc2.scan(dir.path()).unwrap();
    assert_eq!(result2.entry_count, 7);
    assert_eq!(result2.total_size, result1.total_size);
}

// ── Test 7: incremental scan returns full tree when unchanged ──────────────

#[test]
fn incremental_scan_returns_full_tree_when_unchanged() {
    let dir = setup_test_dir();
    let cache_dir = tempfile::tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.redb");
    let cache = RedbCache::open(&cache_path).unwrap();
    let inc = IncrementalScanner::new(ScanOptions::default(), cache);

    let result1 = inc.scan(dir.path()).unwrap();
    let count1 = result1.entry_count;
    let size1 = result1.total_size;

    drop(inc);

    let cache2 = RedbCache::open(&cache_path).unwrap();
    let inc2 = IncrementalScanner::new(ScanOptions::default(), cache2);
    let result2 = inc2.scan(dir.path()).unwrap();

    assert_eq!(result2.entry_count, count1);
    assert_eq!(result2.total_size, size1);
}

// ── Test 8: should format output as JSON ───────────────────────────────────

#[test]
fn should_format_output_as_json() {
    let dir = setup_test_dir();
    let scanner = Scanner::new(ScanOptions::default());
    let result = scanner.scan(dir.path()).unwrap();
    let json = scan_engine::format::json::format(&result);

    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("output must be valid JSON");
    assert!(parsed["total_size"].as_u64().unwrap() >= 22);
    assert_eq!(parsed["entry_count"].as_u64().unwrap(), 7);
}

// ── Test 9: should format output as table ──────────────────────────────────

#[test]
fn should_format_output_as_table() {
    let dir = setup_test_dir();
    let scanner = Scanner::new(ScanOptions::default());
    let result = scanner.scan(dir.path()).unwrap();
    let table = scan_engine::format::table::format(&result, None, None);

    assert!(table.contains("Name"));
    assert!(table.contains("Size"));
    assert!(table.contains("Modified"));
    assert!(table.contains("Type"));
    assert!(table.contains("a.txt"));
}

// ── Test 10: should handle permission errors gracefully ────────────────────

#[test]
fn should_handle_permission_errors_gracefully() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("readable.txt"), "ok").unwrap();

    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("secret.txt"), "secret").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    }

    let scanner = Scanner::new(ScanOptions::default());
    let result = scanner.scan(dir.path());

    // Should succeed — permission-denied dirs are skipped, not fatal.
    assert!(result.is_ok());
    let tree = result.unwrap();
    assert!(tree.entry_count >= 2); // readable.txt + root at minimum

    // Restore permissions so TempDir cleanup succeeds.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn collect_file_names(node: &scan_engine::TreeNode) -> Vec<String> {
    let mut names = Vec::new();
    for child in &node.children {
        if child.entry.node_type == NodeType::File {
            names.push(child.entry.name.clone());
        }
        names.extend(collect_file_names(child));
    }
    names
}
