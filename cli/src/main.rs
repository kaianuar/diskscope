//! DiskScope CLI — fast cross-platform disk space analyzer.

use std::path::PathBuf;
use std::process;

use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{generate, Shell};

use scan_engine::domain::filter::Filter;
use scan_engine::scanner::Scanner;
use scan_engine::scanner::options::ScanOptions;
use scan_engine::domain::opts::ScanOpts;
use scan_engine::domain::format::OutputFormat;
use scan_engine::domain::sort::SortKey;

#[derive(Parser)]
#[command(name = "diskscope", version, about = "Fast cross-platform disk space analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a directory and display disk usage
    Scan {
        /// Path to scan (defaults to current directory)
        #[arg(value_hint = ValueHint::DirPath, default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = FormatArg::Table)]
        format: FormatArg,

        /// Sort order
        #[arg(long, value_enum)]
        sort: Option<SortArg>,

        /// Minimum file size in bytes
        #[arg(long)]
        min_size: Option<u64>,

        /// Maximum directory depth
        #[arg(long)]
        max_depth: Option<u32>,

        /// Glob pattern to match file names
        #[arg(long)]
        pattern: Option<String>,
    },

    /// Show a quick summary of disk usage
    Summary {
        /// Path to summarize
        path: PathBuf,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum FormatArg {
    Json,
    Table,
    Jsonl,
    Tree,
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum SortArg {
    #[value(name = "size-desc")]
    SizeDesc,
    #[value(name = "size-asc")]
    SizeAsc,
    #[value(name = "name-asc")]
    NameAsc,
    #[value(name = "name-desc")]
    NameDesc,
}

impl From<FormatArg> for OutputFormat {
    fn from(arg: FormatArg) -> Self {
        match arg {
            FormatArg::Json => OutputFormat::Json,
            FormatArg::Table => OutputFormat::Table,
            FormatArg::Jsonl => OutputFormat::Jsonl,
            FormatArg::Tree => OutputFormat::Tree,
        }
    }
}

impl From<SortArg> for SortKey {
    fn from(arg: SortArg) -> Self {
        match arg {
            SortArg::SizeDesc => SortKey::SizeDesc,
            SortArg::SizeAsc => SortKey::SizeAsc,
            SortArg::NameAsc => SortKey::NameAsc,
            SortArg::NameDesc => SortKey::NameDesc,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            format,
            sort,
            min_size,
            max_depth,
            pattern,
        } => {
            run_scan(path, format, sort, min_size, max_depth, pattern);
        }
        Commands::Summary { path } => {
            run_summary(path);
        }
        Commands::Completions { shell } => {
            run_completions(shell);
        }
    }
}

fn run_scan(
    path: PathBuf,
    format: FormatArg,
    sort: Option<SortArg>,
    min_size: Option<u64>,
    max_depth: Option<u32>,
    pattern: Option<String>,
) {
    if !path.exists() {
        eprintln!("error: path does not exist: {}", path.display());
        process::exit(1);
    }

    let scanner = Scanner::new(ScanOptions::default());

    let mut opts = ScanOpts::new();
    opts.format = format.into();

    if let Some(s) = sort {
        opts.sort = Some(s.into());
    }

    if let Some(min) = min_size {
        opts.filters.push(Filter::MinSize(min));
    }

    if let Some(depth) = max_depth {
        opts.depth = Some(depth);
    }

    if let Some(pat) = pattern {
        opts.filters.push(Filter::NamePattern(pat));
    }

    match scanner.scan(&path, &opts) {
        Ok(tree) => {
            // Apply domain-level filters and sorting
            let root = opts.apply(&tree.root).unwrap_or(tree.root);
            match root.format(opts.format) {
                Ok(output) => println!("{}", output),
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(1);
        }
    }
}

fn run_summary(path: PathBuf) {
    if !path.exists() {
        eprintln!("error: path does not exist: {}", path.display());
        process::exit(1);
    }

    let scanner = Scanner::new(ScanOptions::default());
    let mut opts = ScanOpts::new();
    opts.format = OutputFormat::Table;

    match scanner.scan(&path, &opts) {
        Ok(tree) => {
            let total = tree.total_size();
            let count = tree.file_count();
            println!("Disk usage summary for: {}", path.display());
            println!("  Total size:  {} bytes ({})", total, human_size(total));
            println!("  File count:  {}", count);
            println!();
            println!("Top 10 largest entries:");
            let mut entries: Vec<_> = tree.root.children.iter().collect();
            entries.sort_by_key(|e| std::cmp::Reverse(e.total_size()));
            for entry in entries.iter().take(10) {
                let marker = if entry.is_dir { "[dir] " } else { "      " };
                println!(
                    "  {}{}  {} ({})",
                    marker,
                    entry.name,
                    entry.total_size(),
                    human_size(entry.total_size())
                );
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(1);
        }
    }
}

fn run_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, &bin_name, &mut std::io::stdout());
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}
