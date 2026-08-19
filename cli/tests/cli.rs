//! End-to-end CLI tests via `assert_cmd`.
//!
//! Each test runs the real `diskscope` binary against a tempfile
//! fixture tree. Trash tests mutate the real system trash and restore
//! after themselves via `trash::os_limited::restore_all` so they leave
//! no residue.

use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

/// A fixture directory tree:
///
/// ```text
/// <root>/
///   a.txt       11 bytes   (document)
///   c.md         4 bytes   (document)
///   sub/
///     b.bin    300 bytes   (other)
/// ```
fn fixture_tree() -> TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    std::fs::write(dir.path().join("a.txt"), b"hello world").expect("write a.txt");
    std::fs::write(dir.path().join("c.md"), b"data").expect("write c.md");
    std::fs::create_dir(dir.path().join("sub")).expect("create sub");
    std::fs::write(dir.path().join("sub").join("b.bin"), vec![b'x'; 300]).expect("write b.bin");
    dir
}

fn bin() -> Command {
    Command::cargo_bin("diskscope").expect("binary exists")
}

// ── scan ──────────────────────────────────────────────────────────────────

#[test]
fn should_emit_json_array_when_scan_format_json_runs_against_fixture_tree() {
    let dir = fixture_tree();
    let out = bin()
        .args(["scan", dir.path().to_str().unwrap(), "--format", "json", "--quiet"])
        .output()
        .expect("run scan --format json");
    eprintln!(
        "DIAG status={} stdout={} bytes stderr={} bytes",
        out.status,
        out.stdout.len(),
        out.stderr.len()
    );
    eprintln!("DIAG stdout={:?}", String::from_utf8_lossy(&out.stdout));
    eprintln!("DIAG stderr={:?}", String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}\n{stdout}"));
    assert_eq!(v["file_type"], "dir", "root should be a directory");
    assert_eq!(v["path"], dir.path().to_str().unwrap());
}

#[test]
fn should_emit_one_json_line_per_file_when_scan_format_jsonl_runs() {
    let dir = fixture_tree();
    let out = bin()
        .args(["scan", dir.path().to_str().unwrap(), "--format", "jsonl", "--quiet"])
        .output()
        .expect("run scan --format jsonl");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    // The renderer emits one JSON object per tree node (depth-first),
    // i.e. at least one line per file, with every line parseable JSON.
    assert!(lines.len() >= 3, "expected one line per file, got {}", lines.len());
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line is not valid JSON: {line}: {e}"));
        assert!(v.get("path").is_some(), "line missing path: {line}");
    }
    assert!(
        lines.iter().any(|l| l.contains("b.bin")),
        "expected b.bin among the lines"
    );
}

#[test]
fn should_emit_tree_style_output_when_scan_format_tree_runs() {
    let dir = fixture_tree();
    let out = bin()
        .args(["scan", dir.path().to_str().unwrap(), "--format", "tree", "--quiet"])
        .output()
        .expect("run scan --format tree");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");

    // Every file appears on its own indented line, with a size label.
    for name in ["a.txt", "c.md", "sub", "b.bin"] {
        let line = stdout
            .lines()
            .find(|l| l.contains(name))
            .unwrap_or_else(|| panic!("tree output missing {name}: {stdout}"));
        assert!(line.contains('[') && line.contains(']'), "line lacks size label: {line}");
    }
}

#[test]
fn should_print_nothing_on_stdout_and_exit_0_when_quiet_passed() {
    let dir = fixture_tree();
    let out = bin()
        .args(["scan", dir.path().to_str().unwrap(), "--quiet"])
        .output()
        .expect("run scan --quiet");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        out.stdout.is_empty(),
        "expected empty stdout, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn should_print_to_stderr_and_exit_2_when_no_path_given() {
    bin()
        .args(["scan"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("required arguments"));
}

// ── delete / undo ─────────────────────────────────────────────────────────

/// Remove every `TrashItem` whose original path lives under `root`
/// (restoring it to its original location). Returns the number restored.
fn cleanup_trash_under(root: &Path) -> usize {
    let items = match trash::os_limited::list() {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let ours: Vec<_> = items
        .into_iter()
        .filter(|it| it.original_path().starts_with(root))
        .collect();
    let count = ours.len();
    if count > 0 {
        let _ = trash::os_limited::restore_all(ours);
    }
    count
}

#[test]
fn should_move_file_to_trash_when_delete_invoked_against_a_real_file() {
    let dir = fixture_tree();
    let target = dir.path().join("a.txt");

    bin()
        .args(["delete", target.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("moved"));

    // File must be gone from its original location.
    assert!(!target.exists(), "file should be gone from origin");

    // And present in the system trash, keyed by its original path.
    let items = trash::os_limited::list().expect("list trash");
    let in_trash = items
        .iter()
        .any(|it| it.original_path() == target);
    assert!(in_trash, "file should be present in trash");

    cleanup_trash_under(dir.path());
}

#[test]
fn should_restore_file_when_delete_undo_runs_after_a_delete() {
    let dir = fixture_tree();
    let target = dir.path().join("c.md");

    // `delete --undo` pops the in-process undo stack. The stack is
    // per-process, so a single `diskscope` invocation cannot both delete
    // and undo. Instead, drive the same `ScanService` directly to
    // exercise the exact TrashBin logic the CLI uses, then verify the
    // CLI's `--undo` path fails cleanly with an empty stack.
    let svc = scan_engine::ScanService::new();
    svc.move_to_trash(target.to_str().unwrap()).expect("move to trash");
    assert!(!target.exists(), "file should be gone after move");

    // Undo through the service (same code path the CLI calls).
    svc.undo_last().expect("undo last");
    assert!(target.exists(), "file should be restored after undo");

    // Now verify the CLI surface: with an empty in-process stack, the
    // `delete --undo` invocation reports a clean error and exit code 5.
    bin()
        .args(["delete", "--undo"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("nothing to undo"));

    cleanup_trash_under(dir.path());
}

// ── completions ───────────────────────────────────────────────────────────

#[test]
fn should_emit_bash_script_when_completions_bash_invoked() {
    bin()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_diskscope"));
}

// ── summary ───────────────────────────────────────────────────────────────

#[test]
fn should_print_total_and_count_and_top10_when_summary_invoked() {
    let dir = fixture_tree();
    bin()
        .args(["summary", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("total:")
                .and(predicate::str::contains("top 10:"))
                .and(predicate::str::contains("b.bin")),
        );
}

// ── exit codes ────────────────────────────────────────────────────────────

#[test]
fn should_exit_5_when_scan_path_does_not_exist() {
    bin()
        .args(["scan", "/nonexistent/diskscope/does/not/exist"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("not found"));
}
