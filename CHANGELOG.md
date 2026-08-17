# Changelog

## v0.1.0 — 2026-08-18

Initial release. Core scan engine and CLI are functional.

### scan-engine

- **Parallel walker** — directory traversal via `ignore::WalkBuilder` with native `.gitignore` support, optional symlink following, and configurable `max_depth`
- **RedbCache** — persistent file metadata cache backed by `redb`; `get`/`put`/`invalidate` cycle for incremental scanning
- **IncrementalScanner** — reuses cached metadata for files whose size and mtime haven't changed since last scan
- **Domain core** — `FileNode`, `FileTree`, `Filter` (size, extension, age, name pattern), `FileType`, `Size` (human-readable formatting), `SortKey`, `OutputFormat` (JSON, JSONL, table, tree), `ScanOpts`, error types, port traits (`Scanner`, `Cache`, `Trash`), mock implementations
- 25 domain unit tests, 10 integration tests

### CLI

- `diskscope scan [path]` — scan with format/sort/filter flags
- `diskscope summary <path>` — quick directory stats
- `diskscope completions <shell>` — shell completion generation
- Error handling: nonexistent path exits 1 with message

### Known Limitations

- GUI is a stub (Phase 4)
- Trash adapter uses mock (Phase 3)
- No real-time sync (Phase 7)
- No cross-platform packaging (Phase 8)
