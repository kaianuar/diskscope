# DiskScope — Implementation Plan

> Hexagonal architecture: domain core → scan-engine adapter → CLI/GUI adapters.
> Each phase is independently buildable, testable, and gate-passable.

---

## Phase 1: Domain Core

**Goal:** Pure domain logic with zero external dependencies. Foundation for all adapters.

### Deliverables

- `scan-engine/src/domain/` — file tree entities, size units, file types, filter predicates
- `scan-engine/src/domain/tree.rs` — `FileNode` (name, path, size, children, modified), `FileTree` (root, total_size, file_count)
- `scan-engine/src/domain/filter.rs` — `Filter` enum (SizeRange, FileType, Age, NamePattern), `matches(node, filter) -> bool`
- `scan-engine/src/domain/size.rs` — `Size(bytes)` with human-readable display (B/KB/MB/GB)
- `scan-engine/src/domain/file_type.rs` — `FileType` enum (Audio, Video, Image, Document, Code, Archive, Other), classify by extension

### Tests (TDD — write first)

| # | Test | Type |
|---|------|------|
| 1 | should create FileNode with valid fields when given path and size | unit |
| 2 | should calculate total_size when building FileTree from children | unit |
| 3 | should count files recursively when tree has nested children | unit |
| 4 | should format Size as human-readable string when bytes > 1024 | unit |
| 5 | should classify file type by extension when extension is known | unit |
| 6 | should return Other when extension is unknown | unit |
| 7 | should match file when size within range filter | unit |
| 8 | should reject file when size outside range filter | unit |
| 9 | should match file when name matches glob pattern | unit |
| 10 | should combine multiple filters with AND logic | unit |

### Gates Satisfied

- **Gate 0:** Plan review (this document)
- **Gate 1:** `cargo test -p scan-engine` — domain unit tests pass

---

## Phase 2: Scan Engine Adapter

**Goal:** Parallel directory scanning with caching. First adapter wrapping domain.

### Deliverables

- `scan-engine/src/scanner.rs` — `Scanner` struct with `scan(path, options) -> Result<FileTree>`
- `scan-engine/src/scanner/options.rs` — `ScanOptions` (max_depth, follow_symlinks, respect_gitignore, filters)
- `scan-engine/src/scanner/walker.rs` — parallel walk via `jwalk` + `rayon`, builds `FileTree` from filesystem
- `scan-engine/src/scanner/cache.rs` — `Cache` trait + `RedbCache` impl using `redb` for incremental scans
- `scan-engine/src/scanner/incremental.rs` — `IncrementalScanner` — re-scan only changed files (mtime + size check)
- `scan-engine/src/output.rs` — `OutputFormat` (Table, Json, Jsonl, Tree), `format(tree, format) -> String`
- Integration tests: scan real temp directories, verify file counts and sizes

### Tests (TDD — write first)

| # | Test | Type |
|---|------|------|
| 1 | should scan directory and return correct file count when path contains files | integration |
| 2 | should scan directory and calculate total size when path contains nested dirs | integration |
| 3 | should respect max_depth option when depth limit is set | integration |
| 4 | should respect .gitignore when ignore option is true | integration |
| 5 | should apply size filter during scan when filter is provided | integration |
| 6 | should use cache on second scan when cache is enabled | integration |
| 7 | should return only changed files on incremental scan when files haven't changed | integration |
| 8 | should format output as JSON when format is Json | unit |
| 9 | should format output as table when format is Table | unit |
| 10 | should handle permission errors gracefully when directory is unreadable | integration |

### Gates Satisfied

- **Gate 1:** `cargo test -p scan-engine` — all unit + integration tests pass
- **Gate 2:** Adversarial review of scan-engine diff (correctness, safety, performance)

---

## Phase 3: CLI Binary

**Goal:** Command-line interface for scanning and outputting results.

### Deliverables

- `cli/src/main.rs` — entry point with `clap` argument parsing
- `cli/src/commands/scan.rs` — `diskscope scan [path] --format table|json|jsonl|tree`
- `cli/src/commands/summary.rs` — `diskscope summary <path>` — quick summary (total size, file count, top 10 largest)
- `cli/src/commands/completions.rs` — `diskscope completions <shell>` — shell completions (bash, zsh, fish)
- `cli/src/output.rs` — pretty-print to terminal, color support, progress indicator during scan
- `cli/src/error.rs` — user-friendly error messages (permission denied, path not found)

### Tests (TDD — write first)

| # | Test | Type |
|---|------|------|
| 1 | should parse scan command with path when invoked correctly | unit |
| 2 | should parse scan command with format flag when --format is provided | unit |
| 3 | should default to table format when no --format flag | unit |
| 4 | should parse summary command with path when invoked correctly | unit |
| 5 | should print JSON output when format is json | integration |
| 6 | should print table output when format is table | integration |
| 7 | should show progress indicator when scanning large directories | integration |
| 8 | should exit with error when path doesn't exist | integration |
| 9 | should generate completions for bash when shell is bash | unit |
| 10 | should respect size filter in CLI when --min-size is provided | integration |

### Gates Satisfied

- **Gate 1:** `cargo test -p cli` — all tests pass
- **Gate 2:** Adversarial review of cli diff (argument parsing, error handling, UX)
- **Gate 3:** Functional E2E — run CLI commands and verify output format/exit codes

---

## Phase 4: GUI — Tauri + React + egui

**Goal:** Desktop app with interactive treemap visualization.

### Deliverables

**Backend (Tauri commands):**
- `gui/src-tauri/src/main.rs` — Tauri app entry, register commands
- `gui/src-tauri/src/commands/scan.rs` — `scan_directory(path, options) -> ScanResult`
- `gui/src-tauri/src/commands/filter.rs` — `apply_filter(tree, filter) -> FileTree`
- `gui/src-tauri/src/commands/delete.rs` — `move_to_trash(path) -> Result<()>`, `undo_delete() -> Result<()>`
- `gui/src-tauri/src/commands/sync.rs` — Ably integration stubs (post-MVP)

**Frontend (React + TypeScript + Vite):**
- `gui/src/` — React app shell, routing, state management
- `gui/src/components/Treemap.tsx` — egui-based treemap visualization (egui_extras)
- `gui/src/components/FileTree.tsx` — tree/table view with sortable columns (name, size, modified, type)
- `gui/src/components/FilterBar.tsx` — filter controls (size range, type, age, pattern)
- `gui/src/components/ContextMenu.tsx` — right-click menu (open in explorer, copy path, copy to clipboard)
- `gui/src/hooks/useScan.ts` — scan state management (loading, results, error)
- `gui/src/hooks/useKeyboard.ts` — keyboard shortcuts (arrows, enter, backspace, delete, cmd/ctrl+z)
- `gui/src/types.ts` — TypeScript types mirroring Rust domain types

**Shared:**
- `gui/src/design-system/` — consume tokens from `design-system/tokens.json`

### Tests (TDD — write first)

| # | Test | Type |
|---|------|------|
| 1 | should return scan results when scan_directory is called with valid path | unit (Rust) |
| 2 | should apply size filter when filter is provided | unit (Rust) |
| 3 | should move file to trash when delete is called | unit (Rust) |
| 4 | should undo last delete when undo_delete is called | unit (Rust) |
| 5 | should display treemap when scan results are provided | e2e (Playwright) |
| 6 | should sort table by size when size column header is clicked | e2e (Playwright) |
| 7 | should filter results when filter bar is used | e2e (Playwright) |
| 8 | should show context menu when right-clicking file | e2e (Playwright) |
| 9 | should navigate with keyboard when arrow keys are pressed | e2e (Playwright) |
| 10 | should delete file when Delete key is pressed | e2e (Playwright) |
| 11 | should undo delete when Cmd/Ctrl+Z is pressed | e2e (Playwright) |

### Gates Satisfied

- **Gate 1:** `cargo test -p gui` — Rust unit tests pass
- **Gate 1:** `cd gui && npm test` — Frontend tests pass
- **Gate 2:** Adversarial review of gui diff (Tauri commands, React components, egui integration)
- **Gate 3:** Visual E2E — Playwright tests + vision model screenshot review

---

## Phase 5: Integration, Packaging, CI/CD

**Goal:** Cross-platform builds, auto-update, and release pipeline.

### Deliverables

- `.github/workflows/ci.yml` — test + build on push (Linux, macOS, Windows)
- `.github/workflows/release.yml` — build artifacts on tag: `.dmg`, `.msi`, `.AppImage`, `.deb`, `.rpm`, `.tar.gz`
- `gui/src-tauri/tauri.conf.json` — Tauri config (app name, icon, updater, code signing)
- `gui/src-tauri/icons/` — App icons for all platforms
- `README.md` — updated with install instructions, usage, screenshots
- `CHANGELOG.md` — release notes template

### Tests

| # | Test | Type |
|---|------|------|
| 1 | should build AppImage on Linux when release workflow runs | ci |
| 2 | should build .dmg on macOS when release workflow runs | ci |
| 3 | should build .msi on Windows when release workflow runs | ci |
| 4 | should auto-update app when new version is available | e2e |
| 5 | should pass all gates when run-gates.sh is executed | ci |

### Gates Satisfied

- **Gate 1:** All platform builds pass in CI
- **Gate 2:** Adversarial review of CI/CD pipeline (security, signing, update mechanism)
- **Gate 3:** Visual E2E — app launches and displays correctly on all platforms

---

## Phase Summary

| Phase | Scope | Key Deliverables | Gates |
|-------|-------|------------------|-------|
| 1 | Domain Core | `FileNode`, `FileTree`, `Filter`, `Size`, `FileType` | Gate 0, 1 |
| 2 | Scan Engine | `Scanner`, parallel walk, `Cache`, incremental, output formats | Gate 1, 2 |
| 3 | CLI | `clap` commands, pretty output, shell completions | Gate 1, 2, 3 |
| 4 | GUI | Tauri backend, React frontend, egui treemap, keyboard shortcuts | Gate 1, 2, 3 |
| 5 | Packaging | CI/CD, cross-platform builds, auto-update, docs | Gate 1, 2, 3 |

---

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `jwalk` performance on large dirs | Low | High | Benchmark early; fallback to `walkdir` if needed |
| egui treemap rendering perf | Medium | High | Prototype with 100k nodes; consider WebGL fallback |
| Tauri IPC overhead for real-time | Low | Medium | Batch updates; use channels not polling |
| Redb cache corruption | Low | Medium | WAL mode; cache is disposable (re-scan) |
| Cross-platform build matrix | Medium | Medium | Start CI early; test each platform incrementally |

---

## Success Criteria

- [ ] Scan 100k files in <2 seconds on modern hardware
- [ ] Memory usage <200MB during scan
- [ ] UI remains responsive during scan (background thread)
- [ ] Treemap renders smoothly with 100k+ nodes
- [ ] Safe delete works on Linux, macOS, Windows
- [ ] All gates pass (0-3)
- [ ] Single binary distribution per platform
