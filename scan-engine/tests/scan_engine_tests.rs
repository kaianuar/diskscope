use std::fs;
use std::path::PathBuf;

use scan_engine::domain::error::DomainError;
use scan_engine::domain::filter::Filter;
use scan_engine::domain::format::OutputFormat;
use scan_engine::domain::opts::ScanOpts;
use scan_engine::domain::tree::FileTree;
use scan_engine::domain::filenode::FileNode;
use scan_engine::output::format_tree;
use scan_engine::scanner::cache::RedbCache;
use scan_engine::scanner::incremental::IncrementalScanner;
use scan_engine::scanner::options::ScanOptions;
use scan_engine::scanner::Scanner;

use tempfile::TempDir;

// ---------- helpers ----------

/// Create a temp directory with test files and return the TempDir guard.
fn setup_test_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();           // 5 bytes
    fs::write(dir.path().join("b.txt"), "world!!").unwrap();         // 7 bytes
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub").join("c.txt"), "nested").unwrap(); // 6 bytes
    fs::create_dir(dir.path().join("sub").join("deep")).unwrap();
    fs::write(dir.path().join("sub").join("deep").join("d.txt"), "deep").unwrap(); // 4 bytes
    dir
}

// ====================================================================
// Debug: max_depth
// ====================================================================
#[test]
fn debug_max_depth() {
    let dir = setup_test_dir();
    let scan_opts = ScanOptions {
        max_depth: Some(1),
        ..Default::default()
    };
    let scanner = Scanner::new(scan_opts);
    let opts = ScanOpts::default();
    let tree = scanner.scan(dir.path(), &opts).unwrap();

    fn count_all(node: &FileNode) -> usize {
        1 + node.children.iter().map(|c| count_all(c)).sum::<usize>()
    }
    eprintln!("max_depth=1: file_count={}, total_size={}, all_nodes={}",
        tree.file_count(), tree.total_size(), count_all(&tree.root));
    print_node(&tree.root, 0);
}

// ====================================================================
// Test 1 (plan): should scan directory and return correct file count
// ====================================================================
#[test]
fn should_scan_directory_and_return_correct_file_count() {
    let dir = setup_test_dir();
    let scanner = Scanner::new(ScanOptions::default());
    let opts = ScanOpts::default();
    let tree = scanner.scan(dir.path(), &opts).unwrap();

    assert_eq!(tree.file_count(), 4);
}

// ====================================================================
// Test 2 (plan): should scan directory and calculate total size
// ====================================================================
#[test]
fn should_scan_directory_and_calculate_total_size() {
    let dir = setup_test_dir();
    let scanner = Scanner::new(ScanOptions::default());
    let opts = ScanOpts::default();
    let tree = scanner.scan(dir.path(), &opts).unwrap();

    // File sizes: 5 + 7 + 6 + 4 = 22. total_size also includes
    // directory metadata sizes (varies by filesystem), so check >= 22.
    assert!(tree.total_size() >= 22, "total_size {} should be >= 22", tree.total_size());
}

// ====================================================================
// Test 3 (plan): should respect max_depth option when depth limit is set
// ====================================================================
#[test]
fn should_respect_max_depth_option() {
    let dir = setup_test_dir();
    let scan_opts = ScanOptions {
        max_depth: Some(1),
        ..Default::default()
    };
    let scanner = Scanner::new(scan_opts);
    let opts = ScanOpts::default();
    let tree = scanner.scan(dir.path(), &opts).unwrap();

    // depth 0 = root, depth 1 = immediate children (a.txt, b.txt, sub/).
    // sub's children (depth 2+) are excluded.
    assert_eq!(tree.file_count(), 2, "only a.txt and b.txt at depth <= 1");
}

// ====================================================================
// Test 4 (plan): should respect .gitignore when ignore option is true
// ====================================================================
#[test]
fn should_respect_gitignore_when_option_is_true() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("keep.txt"), "keep").unwrap();       // 4 bytes
    fs::write(dir.path().join("ignored.log"), "ignored data").unwrap(); // > 4 bytes
    fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();

    let scan_opts = ScanOptions {
        respect_gitignore: true,
        ..Default::default()
    };
    let scanner = Scanner::new(scan_opts);
    let opts = ScanOpts::default();
    let tree = scanner.scan(dir.path(), &opts).unwrap();

    // ignored.log should be filtered out by .gitignore.
    // keep.txt + .gitignore remain (2 files).
    assert_eq!(tree.file_count(), 2, "ignored.log should be excluded by .gitignore");
}

// ====================================================================
// Test 5 (plan): should apply size filter during scan when filter is provided
// ====================================================================
#[test]
fn should_apply_size_filter_during_scan() {
    let dir = setup_test_dir();
    let scan_opts = ScanOptions {
        filters: vec![Filter::MinSize(6)],
        ..Default::default()
    };
    let scanner = Scanner::new(scan_opts);
    let opts = ScanOpts::default();
    let tree = scanner.scan(dir.path(), &opts).unwrap();

    // Files >= 6 bytes: b.txt (7), c.txt (6). a.txt(5) and d.txt(4) filtered out.
    assert_eq!(tree.file_count(), 2);
}

// ====================================================================
// Test 6 (plan): should use cache on second scan when cache is enabled
// ====================================================================
#[test]
fn should_use_cache_on_second_scan() {
    let dir = setup_test_dir();
    let cache_dir = tempfile::tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.redb");
    let cache = RedbCache::open(&cache_path).unwrap();
    let scan_opts = ScanOptions::default();
    let inc = IncrementalScanner::new(scan_opts, cache);
    let opts = ScanOpts::default();

    // First scan — populates cache.
    let tree1 = inc.scan(dir.path(), &opts).unwrap();
    assert_eq!(tree1.file_count(), 4);

    // Re-open cache from same path to verify persistence.
    let cache2 = RedbCache::open(&cache_path).unwrap();
    let inc2 = IncrementalScanner::new(ScanOptions::default(), cache2);

    // Second scan — should return same results.
    let tree2 = inc2.scan(dir.path(), &opts).unwrap();
    assert_eq!(tree2.file_count(), 4);
    assert_eq!(tree2.total_size(), tree1.total_size());
}

// ====================================================================
// Test 7 (plan): should return only changed files on incremental scan
//                when files haven't changed
// ====================================================================
#[test]
fn incremental_scan_returns_full_tree_when_unchanged() {
    let dir = setup_test_dir();
    let cache_dir = tempfile::tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.redb");
    let cache = RedbCache::open(&cache_path).unwrap();
    let inc = IncrementalScanner::new(ScanOptions::default(), cache);
    let opts = ScanOpts::default();

    // First scan populates cache.
    let tree1 = inc.scan(dir.path(), &opts).unwrap();
    let count1 = tree1.file_count();
    let size1 = tree1.total_size();

    // Second scan (no changes) should return identical tree.
    let cache2 = RedbCache::open(&cache_path).unwrap();
    let inc2 = IncrementalScanner::new(ScanOptions::default(), cache2);
    let tree2 = inc2.scan(dir.path(), &opts).unwrap();

    assert_eq!(tree2.file_count(), count1);
    assert_eq!(tree2.total_size(), size1);
}

// ====================================================================
// Test 8 (plan): should format output as JSON when format is Json
// ====================================================================
#[test]
fn should_format_output_as_json() {
    let node = FileNode::new(
        PathBuf::from("/test/file.txt"),
        "file.txt".into(),
        1024,
        1_700_000_000,
        false,
    ).unwrap();
    let tree = FileTree::new(node);
    let json = format_tree(&tree, OutputFormat::Json).unwrap();

    assert!(json.contains("\"name\":\"file.txt\""));
    assert!(json.contains("\"size\":1024"));
    assert!(json.contains("\"is_dir\":false"));
}

// ====================================================================
// Test 9 (plan): should format output as table when format is Table
// ====================================================================
#[test]
fn should_format_output_as_table() {
    let node = FileNode::new(
        PathBuf::from("/test/data.csv"),
        "data.csv".into(),
        2048,
        1_700_000_000,
        false,
    ).unwrap();
    let tree = FileTree::new(node);
    let table = format_tree(&tree, OutputFormat::Table).unwrap();

    assert!(table.contains("Name\tSize\tModified\tType"));
    assert!(table.contains("data.csv"));
    assert!(table.contains("csv"));
}

// ====================================================================
// Test 10 (plan): should handle permission errors gracefully
// ====================================================================
#[test]
fn should_handle_permission_errors_gracefully() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("readable.txt"), "ok").unwrap();

    // Create a directory with no read permission.
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("secret.txt"), "secret").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    }

    let scanner = Scanner::new(ScanOptions::default());
    let opts = ScanOpts::default();
    let result = scanner.scan(dir.path(), &opts);

    // Should succeed — permission-denied dirs are skipped, not fatal.
    assert!(result.is_ok());
    let tree = result.unwrap();
    assert!(tree.file_count() >= 1); // readable.txt at minimum

    // Restore permissions so TempDir cleanup succeeds.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
