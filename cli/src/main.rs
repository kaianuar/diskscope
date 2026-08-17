use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};

use scan_engine::format;
use scan_engine::scanner::Scanner;
use scan_engine::{OutputFormat, ScanOptions, SortDir, SortKey};

#[derive(Parser)]
#[command(name = "diskscope", version, about = "Fast disk space analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a directory and display results
    Scan {
        /// Path to scan (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value_t = FormatArg::Table)]
        format: FormatArg,

        /// Sort key
        #[arg(short, long, value_enum)]
        sort: Option<SortArg>,

        /// Sort direction
        #[arg(long, value_enum)]
        sort_dir: Option<SortDirArg>,

        /// Minimum file size in bytes
        #[arg(long)]
        min_size: Option<u64>,

        /// Maximum directory depth
        #[arg(long)]
        max_depth: Option<u32>,

        /// Name pattern filter (substring match)
        #[arg(short, long)]
        pattern: Option<String>,

        /// Respect .gitignore files
        #[arg(long, default_value_t = true)]
        gitignore: bool,
    },

    /// Show a quick summary of disk usage for a path
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

#[derive(ValueEnum, Clone, Copy)]
enum FormatArg {
    Table,
    Json,
    Jsonl,
    Tree,
}

#[derive(ValueEnum, Clone, Copy)]
enum SortArg {
    Size,
    Name,
    Modified,
}

#[derive(ValueEnum, Clone, Copy)]
enum SortDirArg {
    Asc,
    Desc,
}

impl From<FormatArg> for OutputFormat {
    fn from(f: FormatArg) -> Self {
        match f {
            FormatArg::Table => OutputFormat::Table,
            FormatArg::Json => OutputFormat::Json,
            FormatArg::Jsonl => OutputFormat::Jsonl,
            FormatArg::Tree => OutputFormat::Tree,
        }
    }
}

impl From<SortArg> for SortKey {
    fn from(s: SortArg) -> Self {
        match s {
            SortArg::Size => SortKey::Size,
            SortArg::Name => SortKey::Name,
            SortArg::Modified => SortKey::Modified,
        }
    }
}

impl From<SortDirArg> for SortDir {
    fn from(d: SortDirArg) -> Self {
        match d {
            SortDirArg::Asc => SortDir::Asc,
            SortDirArg::Desc => SortDir::Desc,
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
            sort_dir,
            min_size,
            max_depth,
            pattern,
            gitignore,
        } => {
            let opts = ScanOptions {
                max_depth,
                min_size,
                respect_gitignore: gitignore,
                pattern,
                ..Default::default()
            };
            let scanner = Scanner::new(opts);
            let result = match scanner.scan(&path) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };

            let output = match format.into() {
                OutputFormat::Table => {
                    let sk = sort.map(SortKey::from);
                    let sd = sort_dir.map(SortDir::from);
                    format::table::format(&result, sk, sd)
                }
                OutputFormat::Json => format::json::format(&result),
                OutputFormat::Jsonl => format::jsonl::format(&result),
                OutputFormat::Tree => format::tree::format(&result),
            };
            print!("{output}");
        }

        Commands::Summary { path } => {
            let scanner = Scanner::new(ScanOptions::default());
            let result = match scanner.scan(&path) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };

            println!(
                "Path:        {}",
                path.canonicalize()
                    .unwrap_or(path)
                    .display()
            );
            println!("Total size:  {} bytes", result.total_size);
            println!("Entries:     {}", result.entry_count);
        }

        Commands::Completions { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            generate(shell, &mut cmd, "diskscope", &mut std::io::stdout());
        }
    }
}
