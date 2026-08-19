# DiskScope

Fast, cross-platform disk space analyzer. Identifies space hogs with parallel scanning, interactive treemap visualization, and safe delete with undo.

## Who It's For

- **Developers** cleaning up `node_modules`, build artifacts, and caches
- **Power users** managing disk space on personal machines
- **System admins** auditing server disk usage

## Quick Start

```bash
# Scan current directory
cargo run -p diskscope-cli -- scan .

# Scan with filters and output format
cargo run -p diskscope-cli -- scan /home --format json --min-size 100MB

# Quick summary
cargo run -p diskscope-cli -- summary .

# Generate shell completions
cargo run -p diskscope-cli -- completions bash
```

The built binary is named `diskscope` (e.g. `./target/debug/diskscope scan .`).

### CLI Options

```
diskscope scan [OPTIONS] [PATH]
  -f, --format <FORMAT>    Output format: table (default), json, jsonl, tree
  -s, --sort <SORT>        Sort: name-asc, name-desc, size-asc, size-desc
      --min-size <SIZE>    Minimum file size filter (e.g. 100MB)
      --max-size <SIZE>    Maximum file size filter
      --min-depth <N>      Minimum directory depth
      --max-depth <N>      Maximum directory depth
```

## How It Works

DiskScope uses `ignore::WalkBuilder` for parallel directory traversal with native `.gitignore` support, `redb` for persistent caching (incremental scans skip unchanged files), and `rayon` for parallelism. The scan engine runs in a background thread; the Tauri GUI stays responsive during scans.

## Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust 2021 |
| Scanner | `ignore` (parallel walk) + `rayon` |
| Cache | `redb` (embedded key-value store) |
| CLI | `clap` |
| GUI | Tauri v2 + egui + React 18 |
| Architecture | Hexagonal — domain at center, adapters at edges |

## Project Structure

```
diskscope/
├── domain/               # Pure domain crate: FileNode, Filter, ports, error types
├── scan-engine/          # Scanning library: walker, cache, incremental scanner
│   └── src/              # cache.rs, filters.rs, scanner.rs, sort.rs, trash.rs
├── cli/                  # CLI binary (clap)
├── gui/                  # GUI binary (Tauri + React + egui treemap)
├── tests/                # Pipeline gates (gate.sh, review_gate.sh, visual_gate.sh)
├── Cargo.toml            # Workspace root
└── plan.md               # Build plan (5 phases)
```

## Development

```bash
# Run all tests
cargo test

# Run scan-engine tests only
cargo test -p scan-engine

# Run CLI tests only
cargo test -p diskscope-cli

# Lint
cargo clippy -- -D warnings

# Build release
cargo build --release
```

## License

MIT OR Apache-2.0
