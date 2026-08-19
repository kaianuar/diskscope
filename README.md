# DiskScope

Fast, cross-platform disk space analyzer. Identifies space hogs with parallel scanning, interactive treemap visualization, and safe delete with undo.

## Who It's For

- **Developers** cleaning up `node_modules`, build artifacts, and caches
- **Power users** managing disk space on personal machines
- **System admins** auditing server disk usage

## Build

```bash
cargo build --release
# binary: ./target/release/diskscope
```

For development builds: `cargo build` → `./target/debug/diskscope`.

## Quick Start

```bash
# Scan current directory (table output)
./target/release/diskscope scan .

# Scan another path
./target/release/diskscope scan /home --format json

# Quick summary (total, file count, top 10 largest)
./target/release/diskscope summary .

# Move a file to the system trash (safe delete, not permanent)
./target/release/diskscope delete ./some-file

# Undo the most recent delete
./target/release/diskscope delete --undo

# Generate shell completions
./target/release/diskscope completions bash
```

`cargo run -p diskscope-cli -- <args>` works too (same binary).

## CLI Options

```
diskscope scan [OPTIONS] <PATH>
  -f, --format <FORMAT>      Output format: table (default), json, jsonl, tree
  -s, --sort <SORT>          Sort: name-asc, name-desc, size-asc, size-desc
      --min-size <BYTES>     Minimum file size filter (RAW bytes, e.g. 1048576 = 1MiB)
      --max-size <BYTES>     Maximum file size filter (RAW bytes)
      --max-age <SECS>       Only entries modified within this many seconds
      --name-pattern <P>     Case-insensitive name substring filter
      --max-depth <N>        Maximum tree depth, inclusive (root = 0)
      --no-cache             Bypass the incremental cache
      --quiet                Suppress the summary line (output is still printed)
```

**Note:** size filters take **raw bytes**, not human-readable strings. `--min-size 100MB` will fail — use `--min-size 104857600` instead.

## Example Checks (what to try when testing)

```bash
# 1. Table output — no duplicate root row, sizes are recursive (dir size = sum of contents)
diskscope scan .

# 2. JSON — machine-readable, root has one "size" equal to total content bytes
diskscope scan . --format json

# 3. JSONL — one JSON object per line (files depth-first)
diskscope scan . --format jsonl

# 4. Tree — indented tree view
diskscope scan . --format tree

# 5. Summary — "total:" should match `du -sb <path>`
diskscope summary .

# 6. Filters — only files >= 1 MiB, up to 2 levels deep
diskscope scan . --min-size 1048576 --max-depth 2

# 7. Sort — largest first
diskscope scan . --sort size-desc

# 8. Safe delete — file goes to trash (restorable), NOT permanently deleted
diskscope delete ./test-file
diskscope delete --undo

# 9. --quiet — output still printed, only the "scanned N entries" line is hidden
diskscope scan . --quiet
```

## How It Works

DiskScope uses `ignore::WalkBuilder` for parallel directory traversal with native `.gitignore` support, `redb` for persistent caching (incremental scans skip unchanged files), and `rayon` for parallelism. Directory sizes are computed as recursive sums of their contents (like `du`). The scan engine runs in a background thread; the Tauri GUI stays responsive during scans.

## Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust 2021 |
| Scanner | `ignore` (parallel walk) + `rayon` |
| Cache | `redb` (embedded key-value store) |
| CLI | `clap` |
| GUI | Tauri v2 + React 18 (egui treemap) |
| Architecture | Hexagonal — domain at center, adapters at edges |

## Project Structure

```
diskscope/
├── domain/               # Pure domain crate: FileNode, Filter, ports, error types
├── scan-engine/          # Scanning library: walker, cache, incremental scanner
├── cli/                  # CLI binary (clap)
├── gui/                  # GUI binary (Tauri + React + egui treemap)
├── tests/                # Pipeline gates (gate.sh, smoke_gate.sh, review_gate.sh)
├── Cargo.toml            # Workspace root
└── plan.md               # Build plan (5 phases)
```

## Development

```bash
# Run all tests
cargo test

# Behavioral smoke gate (binary behaves correctly against a fixed fixture)
bash tests/smoke_gate.sh

# Lint
cargo clippy -- -D warnings

# Build release
cargo build --release
```

## License

MIT OR Apache-2.0
