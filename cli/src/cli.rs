//! Clap definitions for the DiskScope CLI.
//!
//! Pure argument parsing: value types and defaults live here, while the
//! command dispatch lives in `main.rs`. Keeping the clap layer separate
//! lets the unit tests in `main.rs` exercise parse behavior without
//! touching the filesystem.

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use domain::{Filter, SortColumn, SortDirection, SortSpec};
use scan_engine::format::OutputFormat;

/// DiskScope — see where your disk space is going and clean it up safely.
#[derive(Debug, Parser)]
#[command(name = "diskscope", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan a directory and render the result tree.
    Scan(ScanArgs),
    /// Print total size, file count, and the ten largest entries.
    Summary(SummaryArgs),
    /// Move a file to the system trash, or undo the most recent move.
    Delete(DeleteArgs),
    /// Emit a shell completion script.
    Completions(CompletionsArgs),
    /// Find duplicate files by content hash.
    Dupes(DupesArgs),
}

#[derive(Debug, clap::Args)]
pub struct ScanArgs {
    /// Directory to scan.
    #[arg(value_name = "PATH")]
    pub path: String,

    /// Output format: table, json, jsonl, or tree. Defaults to table.
    #[arg(long, value_name = "FORMAT", value_enum)]
    pub format: Option<FormatArg>,

    /// Sort column and direction: name|size|modified|type + asc|desc.
    #[arg(long, value_name = "SORT", value_enum)]
    pub sort: Option<SortArg>,

    /// Minimum entry size in bytes.
    #[arg(long, value_name = "BYTES")]
    pub min_size: Option<u64>,

    /// Maximum entry size in bytes.
    #[arg(long, value_name = "BYTES")]
    pub max_size: Option<u64>,

    /// Maximum age in seconds (relative to now).
    #[arg(long, value_name = "SECS")]
    pub max_age: Option<u64>,

    /// Case-insensitive name substring filter.
    #[arg(long, value_name = "PATTERN")]
    pub name_pattern: Option<String>,

    /// Maximum tree depth, inclusive (root = 0).
    #[arg(long, value_name = "DEPTH")]
    pub max_depth: Option<usize>,

    /// Skip the embedded cache and always re-walk.
    #[arg(long)]
    pub no_cache: bool,

    /// Suppress the summary line written to stderr after the result.
    #[arg(long)]
    pub quiet: bool,

    /// Export an interactive HTML snapshot to FILE (self-contained treemap + table).
    /// When present, `--format` is ignored and the result is written to FILE.
    #[arg(long, value_name = "FILE")]
    pub export: Option<std::path::PathBuf>,
}

impl ScanArgs {
    /// Resolve the output format, defaulting to [`OutputFormat::Table`]
    /// when `--format` was not passed.
    pub fn format(&self) -> OutputFormat {
        self.format.map(FormatArg::into).unwrap_or(OutputFormat::Table)
    }

    /// Assemble the `--filter` arguments into a [`Filter`].
    ///
    /// `now` is set to the current Unix time so `max_age` behaves as a
    /// wall-clock age bound; the default `now = 0` in the domain would
    /// otherwise accept every entry (see `domain::Filter::matches`).
    pub fn filter(&self) -> Option<Filter> {
        if self.min_size.is_none()
            && self.max_size.is_none()
            && self.max_age.is_none()
            && self.name_pattern.is_none()
            && self.max_depth.is_none()
        {
            return None;
        }
        Some(Filter {
            min_size: self.min_size,
            max_size: self.max_size,
            file_types: None,
            name_pattern: self.name_pattern.clone(),
            max_age: self.max_age,
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            max_depth: self.max_depth,
        })
    }
}

/// Wrapper so the CLI owns its `ValueEnum` impls (the `format` and
/// `sort` domain types live in `scan-engine`, which must stay free of
/// clap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FormatArg {
    /// Aligned table columns (default).
    Table,
    /// Single JSON object with embedded children.
    Json,
    /// One JSON object per line.
    Jsonl,
    /// Indented tree-style output.
    Tree,
}

impl From<FormatArg> for OutputFormat {
    fn from(arg: FormatArg) -> Self {
        match arg {
            FormatArg::Table => OutputFormat::Table,
            FormatArg::Json => OutputFormat::Json,
            FormatArg::Jsonl => OutputFormat::Jsonl,
            FormatArg::Tree => OutputFormat::Tree,
        }
    }
}

/// Sort spec wrapper parsed from `column:direction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SortArg {
    /// name:asc
    NameAsc,
    /// name:desc
    NameDesc,
    /// size:asc
    SizeAsc,
    /// size:desc
    SizeDesc,
    /// modified:asc
    ModifiedAsc,
    /// modified:desc
    ModifiedDesc,
    /// type:asc
    TypeAsc,
    /// type:desc
    TypeDesc,
}

impl From<SortArg> for SortSpec {
    fn from(arg: SortArg) -> Self {
        match arg {
            SortArg::NameAsc => {
                SortSpec { column: SortColumn::Name, direction: SortDirection::Ascending }
            }
            SortArg::NameDesc => {
                SortSpec { column: SortColumn::Name, direction: SortDirection::Descending }
            }
            SortArg::SizeAsc => {
                SortSpec { column: SortColumn::Size, direction: SortDirection::Ascending }
            }
            SortArg::SizeDesc => {
                SortSpec { column: SortColumn::Size, direction: SortDirection::Descending }
            }
            SortArg::ModifiedAsc => {
                SortSpec { column: SortColumn::Modified, direction: SortDirection::Ascending }
            }
            SortArg::ModifiedDesc => {
                SortSpec { column: SortColumn::Modified, direction: SortDirection::Descending }
            }
            SortArg::TypeAsc => {
                SortSpec { column: SortColumn::Type, direction: SortDirection::Ascending }
            }
            SortArg::TypeDesc => {
                SortSpec { column: SortColumn::Type, direction: SortDirection::Descending }
            }
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct SummaryArgs {
    /// Directory to summarize.
    #[arg(value_name = "PATH")]
    pub path: String,
}

#[derive(Debug, clap::Args)]
pub struct DeleteArgs {
    /// Path to move to the trash.
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Restore the most recently trashed item instead of deleting.
    #[arg(long)]
    pub undo: bool,
}

#[derive(Debug, clap::Args)]
pub struct CompletionsArgs {
    /// Target shell.
    #[arg(value_name = "SHELL")]
    pub shell: Shell,
}

#[derive(Debug, clap::Args)]
pub struct DupesArgs {
    /// Directory to scan for duplicates.
    #[arg(value_name = "PATH")]
    pub path: String,

    /// Minimum file size in bytes to consider (default: 1 MiB).
    #[arg(long, default_value_t = 1_048_576)]
    pub min_size: u64,
}
