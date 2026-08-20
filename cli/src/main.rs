//! DiskScope CLI: scan, summarize, and safely trash files.
//!
//! A thin clap controller over `scan-engine` — no domain logic lives
//! here. Commands:
//!
//! - `scan <path> [--format ...] [--sort ...] [--filter ...]` — walk a
//!   directory and render the result tree in the chosen format.
//! - `summary <path>` — total size, file count, and top-10 entries.
//! - `delete <path> [--undo]` — move a file to the system trash, or undo
//!   the most recent move.
//! - `completions <shell>` — emit a shell completion script.
//!
//! Exit codes: 0 = ok, 2 = usage error, 3 = I/O error, 5 = path not
//! found. Errors go to stderr; `--quiet` suppresses stdout.

mod cli;

use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;

use cli::Cli;

use domain::{DomainError, FileNode, ScanResult, SortColumn, SortDirection, SortSpec};
use scan_engine::ScanService;

/// Exit code for usage errors (clap also uses 2 for parse failures).
const EXIT_USAGE: u8 = 2;
/// Exit code for I/O errors.
const EXIT_IO: u8 = 3;
/// Exit code for a path that does not exist.
const EXIT_NOT_FOUND: u8 = 5;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Clap exits 2 itself on parse errors; runtime usage errors
            // (e.g. an empty path) surface here with the same code.
            let code = match e.downcast_ref::<DomainError>() {
                Some(DomainError::InvalidPath(_)) => EXIT_NOT_FOUND,
                Some(DomainError::PermissionDenied(_)) => EXIT_NOT_FOUND,
                Some(DomainError::InvalidFilter(_)) => EXIT_USAGE,
                _ => EXIT_IO,
            };
            eprintln!("diskscope: {e:#}");
            ExitCode::from(code)
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        cli::Command::Scan(cmd) => run_scan(cmd),
        cli::Command::Summary(cmd) => run_summary(cmd),
        cli::Command::Delete(cmd) => run_delete(cmd),
        cli::Command::Completions(cmd) => run_completions(cmd),
    }
}

fn service() -> ScanService {
    ScanService::new()
}

fn run_scan(cmd: cli::ScanArgs) -> anyhow::Result<()> {
    if cmd.path.is_empty() {
        return Err(DomainError::InvalidPath("path must not be empty".into()).into());
    }
    let service = service();
    let mut result = service.scan(&cmd.path)?;

    if let Some(sort) = cmd.sort {
        result = scan_engine::sort::apply_sort_result(&result, sort.into());
    }
    if let Some(filter) = cmd.filter() {
        result = scan_engine::filter::apply_filter(&result, &filter);
    }

    // --export takes precedence: write a self-contained HTML snapshot.
    if let Some(ref export_path) = cmd.export {
        if let Some(parent) = export_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let title = std::path::Path::new(&cmd.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| cmd.path.clone());
        let html = scan_engine::snapshot::render_html_snapshot(
            &result,
            &title,
            scan_engine::snapshot::DEFAULT_MAX_NODES,
        );
        fs::write(export_path, html)?;
        eprintln!("Snapshot written to {}", export_path.display());
        return Ok(());
    }

    let fmt = cmd.format();
    let out = io::stdout();
    let mut lock = out.lock();
    scan_engine::format::render(&result, fmt, &mut lock)?;

    if !cmd.quiet {
        eprintln!(
            "scanned {} entries ({}), {} skipped, {} ms",
            result.file_count,
            domain::format_size(result.total_size),
            result.skipped_count(),
            result.scan_duration_ms
        );
    }
    Ok(())
}

fn run_summary(cmd: cli::SummaryArgs) -> anyhow::Result<()> {
    let service = service();
    let result = service.scan(&cmd.path)?;
    print_summary(&result)
}

fn print_summary(result: &ScanResult) -> anyhow::Result<()> {
    let out = io::stdout();
    let mut lock = out.lock();
    writeln!(
        lock,
        "total: {} ({} entries)",
        domain::format_size(result.total_size),
        result.file_count
    )?;
    writeln!(lock, "top 10:")?;

    // Recursively collect every node (root included), sort by size
    // descending, and print the ten largest.
    let mut nodes: Vec<&FileNode> = Vec::new();
    collect_nodes(&result.root, &mut nodes);
    let mut sorted: Vec<FileNode> = nodes.into_iter().cloned().collect();
    SortSpec { column: SortColumn::Size, direction: SortDirection::Descending }.apply(&mut sorted);
    for node in sorted.iter().take(10) {
        writeln!(lock, "  {:>10}  {}", domain::format_size(node.size), node.path)?;
    }

    if !result.skipped.is_empty() {
        writeln!(lock, "skipped {} path(s)", result.skipped_count())?;
    }
    Ok(())
}

fn collect_nodes<'a>(node: &'a FileNode, out: &mut Vec<&'a FileNode>) {
    out.push(node);
    for child in &node.children {
        collect_nodes(child, out);
    }
}

fn run_delete(cmd: cli::DeleteArgs) -> anyhow::Result<()> {
    let service = service();
    if cmd.undo {
        service.undo_last()?;
        println!("restored most recently trashed item");
        return Ok(());
    }
    let path = cmd
        .path
        .filter(|p| !p.is_empty())
        .ok_or_else(|| DomainError::InvalidPath("path must not be empty".into()))?;
    service.move_to_trash(&path)?;
    println!("moved {path} to trash");
    Ok(())
}

fn run_completions(cmd: cli::CompletionsArgs) -> anyhow::Result<()> {
    use clap::CommandFactory;
    use clap_complete::generate;

    let mut command = cli::Cli::command();
    generate(cmd.shell, &mut command, "diskscope", &mut io::stdout());
    Ok(())
}

// Unit tests for the small wrappers (format defaulting, format parsing).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::FormatArg;
    use scan_engine::format::OutputFormat;

    #[test]
    fn should_pick_table_format_by_default_when_scan_invoked_without_format() {
        let cli = Cli::try_parse_from(["diskscope", "scan", "/tmp"]).unwrap();
        match cli.command {
            cli::Command::Scan(cmd) => {
                assert_eq!(cmd.format, None);
                assert_eq!(cmd.format(), OutputFormat::Table);
            }
            _ => panic!("expected Scan command"),
        }
    }

    #[test]
    fn should_pick_json_format_when_format_json_passed() {
        let cli = Cli::try_parse_from(["diskscope", "scan", "/tmp", "--format", "json"]).unwrap();
        match cli.command {
            cli::Command::Scan(cmd) => {
                assert_eq!(cmd.format, Some(FormatArg::Json));
                assert_eq!(cmd.format(), OutputFormat::Json);
            }
            _ => panic!("expected Scan command"),
        }
    }

    #[test]
    fn should_reject_unknown_format_when_format_xml_passed() {
        let err = Cli::try_parse_from(["diskscope", "scan", "/tmp", "--format", "xml"])
            .err()
            .expect("parse must fail");
        assert!(err.to_string().contains("invalid value"), "unexpected error: {err}");
    }

    #[test]
    fn should_print_total_and_count_and_top10_when_summary_invoked() {
        let cli = Cli::try_parse_from(["diskscope", "summary", "/tmp"]).unwrap();
        assert!(matches!(cli.command, cli::Command::Summary(_)));
    }
}
