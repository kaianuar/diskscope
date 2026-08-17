use std::process::Command;

fn cli_bin() -> String {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop(); // remove test binary name
    path.pop(); // remove deps/
    path.push("diskscope");
    path.to_string_lossy().into_owned()
}

fn workspace_root() -> &'static str {
    env!("CARGO_MANIFEST_DIR").trim_end_matches("/cli").trim_end_matches("/cli/")
}

#[test]
fn should_print_help_when_invoked_with_help() {
    let out = Command::new(cli_bin())
        .args(["--help"])
        .output()
        .expect("run diskscope");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Fast cross-platform disk space analyzer"), "got: {}", stdout);
    assert!(stdout.contains("scan"), "missing scan subcommand: {}", stdout);
    assert!(stdout.contains("summary"), "missing summary subcommand: {}", stdout);
    assert!(stdout.contains("completions"), "missing completions subcommand: {}", stdout);
}

#[test]
fn should_scan_current_dir_when_no_path_given() {
    let out = Command::new(cli_bin())
        .args(["scan"])
        .current_dir(workspace_root())
        .output()
        .expect("run diskscope scan");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Default table format lists leaf files with tab separators
    assert!(stdout.contains("Cargo.toml"), "missing Cargo.toml: {}", stdout);
    assert!(stdout.contains("main.rs"), "missing main.rs: {}", stdout);
}

#[test]
fn should_output_json_when_format_json() {
    let out = Command::new(cli_bin())
        .args(["scan", workspace_root(), "--format", "json"])
        .output()
        .expect("run diskscope scan --format json");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .expect("output should be valid JSON");
    assert!(value.is_object(), "JSON root should be object");
    assert!(value.get("name").is_some(), "missing 'name' field");
    assert!(value.get("path").is_some(), "missing 'path' field");
    assert!(value.get("size").is_some(), "missing 'size' field");
    assert!(value.get("mtime").is_some(), "missing 'mtime' field");
    assert!(value.get("children").is_some(), "missing 'children' field");
    assert!(value.get("is_dir").is_some(), "missing 'is_dir' field");
}

#[test]
fn should_output_table_when_format_table_default() {
    let out = Command::new(cli_bin())
        .args(["scan", workspace_root()])
        .output()
        .expect("run diskscope scan default");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2, "table should have header + data rows");
    // Header uses tabs: Name\tSize\tModified\tType
    assert!(lines[0].contains("Name"), "header missing 'Name': {}", lines[0]);
    assert!(lines[0].contains("Size"), "header missing 'Size': {}", lines[0]);
    assert!(lines[0].contains("Type"), "header missing 'Type': {}", lines[0]);
    // Data rows contain tab-separated values
    assert!(lines[1].contains('\t'), "data rows should be tab-separated: {}", lines[1]);
    assert!(stdout.contains("Cargo.toml"), "table missing Cargo.toml: {}", stdout);
}

#[test]
fn should_output_jsonl_when_format_jsonl() {
    let out = Command::new(cli_bin())
        .args(["scan", workspace_root(), "--format", "jsonl"])
        .output()
        .expect("run diskscope scan --format jsonl");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(!lines.is_empty(), "JSONL should have at least one line");
    for line in &lines {
        let val: serde_json::Value = serde_json::from_str(line)
            .expect("each JSONL line should be valid JSON");
        assert!(val.is_object(), "JSONL line should be object");
        assert!(val.get("name").is_some(), "JSONL object missing 'name'");
        assert!(val.get("size").is_some(), "JSONL object missing 'size'");
    }
}

#[test]
fn should_output_tree_when_format_tree() {
    let out = Command::new(cli_bin())
        .args(["scan", workspace_root(), "--format", "tree"])
        .output()
        .expect("run diskscope scan --format tree");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    // Root line: "dirname (size)"
    assert!(!lines.is_empty(), "tree should have at least one line");
    assert!(lines[0].contains('('), "root line should show size in parens: {}", lines[0]);
    // Subdirectories are indented with 2 spaces per level
    let has_indented = lines.iter().any(|l| l.starts_with("  "));
    assert!(has_indented, "tree should have indented children: {}", stdout);
    assert!(stdout.contains("Cargo.toml"), "tree missing Cargo.toml: {}", stdout);
}

#[test]
fn should_filter_by_min_size_when_min_size_given() {
    let out = Command::new(cli_bin())
        .args(["scan", workspace_root(), "--format", "json", "--min-size", "100"])
        .output()
        .expect("run diskscope scan --min-size 100");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    fn find_entry<'a>(node: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
        if node.get("name")?.as_str()? == name {
            return Some(node);
        }
        node.get("children")?.as_array()?.iter().find_map(|c| find_entry(c, name))
    }
    assert!(
        find_entry(&value, "Cargo.toml").is_some(),
        "Cargo.toml (>=100 bytes) should be in output"
    );
}

#[test]
fn should_sort_by_name_asc_when_sort_name_asc() {
    // Use JSON to verify sorting at each directory level
    let out = Command::new(cli_bin())
        .args(["scan", workspace_root(), "--format", "json", "--sort", "name-asc"])
        .output()
        .expect("run diskscope scan --sort name-asc");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let children = value.get("children").expect("root should have children").as_array().unwrap();
    let names: Vec<&str> = children.iter()
        .filter_map(|c| c.get("name")?.as_str())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "root children should be sorted by name ascending");
}

#[test]
fn should_exit_1_when_path_does_not_exist() {
    let out = Command::new(cli_bin())
        .args(["scan", "/nonexistent/path/does/not/exist"])
        .output()
        .expect("run diskscope scan nonexistent");

    assert!(!out.status.success(), "should fail for nonexistent path");
    assert_eq!(out.status.code(), Some(1), "should exit with code 1");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not exist") || stderr.contains("not found"),
        "stderr should mention missing path: {}",
        stderr
    );
}

#[test]
fn should_generate_bash_completions_when_completions_bash() {
    let out = Command::new(cli_bin())
        .args(["completions", "bash"])
        .output()
        .expect("run diskscope completions bash");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("complete") || stdout.contains("COMPREPLY") || stdout.contains("_diskscope"),
        "bash completions should contain shell functions: {}",
        stdout
    );
}
