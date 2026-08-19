# Changelog

## v0.1.0 — 2026-08-19

Initial release. Core scan engine, CLI, and Tauri GUI are functional. Built autonomously by the omp-agent pipeline (5 phases, all gates passing).

### scan-engine

- **Parallel walker** — directory traversal via `ignore::WalkBuilder` with native `.gitignore` support, optional symlink following, and configurable `max_depth`
- **RedbCache** — persistent file metadata cache backed by `redb`; `get`/`put`/`invalidate` cycle for incremental scanning
- **IncrementalScanner** — reuses cached metadata for files whose size and mtime haven't changed since last scan
- **Accurate sizes** — directory sizes are recursive sums of contents (du-style); no double counting
- **Cross-platform trash** — move-to-trash works on Linux/Windows/macOS; undo (trash listing) is Windows/Linux only (the `trash` crate exposes no `os_limited` on macOS)
- **Domain core** — `FileNode`, `FileTree`, `Filter` (size, extension, age, name pattern), `FileType`, `Size`, `SortKey`, `OutputFormat` (JSON, JSONL, table, tree), `ScanOpts`, error types, port traits (`Scanner`, `Cache`, `Trash`)
- 34 domain tests, 10 CLI tests passing

### CLI

- `diskscope scan [path]` — table/json/jsonl/tree output, `--sort`, `--min-size`/`--max-size` (raw bytes), `--max-age`, `--name-pattern`, `--max-depth`, `--no-cache`, `--quiet`
- `diskscope summary <path>` — total size, file count, top 10
- `diskscope delete [path]` / `--undo` — safe move-to-trash with undo
- `diskscope completions <shell>` — shell completion generation
- `--quiet` suppresses the summary line only; rendered output is always printed

### GUI (Tauri v2)

- Tauri v2 + React 18 frontend, flat treemap-style visualization
- Bundles: `.deb`, `.AppImage`, `.rpm` (Linux), `.msi`/`.exe` (Windows), `.dmg` (macOS)
- GitHub Actions CI builds all platforms on tag pushes

### Fixes

- `--quiet` no longer suppresses rendered output
- Root directory no longer appears as its own child in scans
- `total_size()` no longer double-counts directory sizes
- `trash::os_limited` usage gated for macOS (crate limitation)

## Known Limitations

- Undo-by-item (trash restore) is unavailable on macOS (upstream `trash` crate limitation)
- Real-time sync (Ably) is implemented in `scan-engine` but not yet wired into the GUI
- No signed/notarized macOS builds yet
