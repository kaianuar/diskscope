# DiskScope — Implementation Plan

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                      Binary Crates                      │
│  ┌──────────┐   ┌─────────────────────────────────────┐ │
│  │   cli    │   │                gui                   │ │
│  └────┬─────┘   │  ┌──────────┐  ┌────────────────┐   │ │
│       │         │  │  React   │  │  egui canvas   │   │ │
│       │         │  │  (chrome)│  │  (treemap)     │   │ │
│       │         │  └────┬─────┘  └───────┬────────┘   │ │
│       │         └───────┼────────────────┼────────────┘ │
│       └────────┬────────┘                │              │
│                │                         │              │
│  ┌─────────────▼─────────────────────────▼──────────┐   │
│  │              scan-engine (library)                │   │
│  │  ┌──────────┐  ┌─────────┐  ┌────────────────┐  │   │
│  │  │ domain   │  │ walker  │  │ cache (redb)   │  │   │
│  │  │ (pure)   │  │ (jwalk) │  │                │  │   │
│  │  └──────────┘  └─────────┘  └────────────────┘  │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**Workspace crates:** `scan-engine` (lib), `gui` (bin), `cli` (bin)

---

## Phase 1: Domain Core (`scan-engine` — pure domain layer)

**Goal:** Pure Rust types and logic with zero external dependencies. Everything else builds on this.

### Deliverables

- `scan-engine/src/domain/mod.rs` — module re-exports
- `scan-engine/src/domain/node.rs` — `FileNode` struct:
  - `name: String`, `path: PathBuf`, `size: u64`, `modified: SystemTime`
  - `kind: NodeKind` (File | Directory | Symlink)
  - `children: Vec<FileNode>` (directories only)
  - `extension()` → `Option<&str>`, `file_type()` → `FileType` (audio/video/image/doc/code/archive/other)
  - `total_size()` → recursive sum for directories
- `scan-engine/src/domain/tree.rs` — `FileTree`:
  - `root: FileNode`, `total_size: u64`, `file_count: usize`, `dir_count: usize`
  - `flatten()` → `Vec<&FileNode>` (DFS order)
  - `by_extension()` → `HashMap<String, Vec<&FileNode>>`
  - `top_n(n: usize)` → largest N entries
- `scan-engine/src/domain/filter.rs` — `Filter` enum:
  - `MinSize(u64)`, `MaxSize(u64)`
  - `FileType(FileType)`, `Extension(String)`
  - `ModifiedBefore(SystemTime)`, `ModifiedAfter(SystemTime)`
  - `NamePattern(Regex)`, `MaxDepth(usize)`
  - `apply(&self, &FileNode) -> bool`
  - `FilterSet(Vec<Filter>)` — all-match combinator
- `scan-engine/src/domain/mod.rs` — `FileType` enum with `from_extension(&str) -> FileType`

### Tests

| # | Test |
|---|------|
| 1 | should classify extensions correctly when mapping from extension string |
| 2 | should compute recursive total_size when node is directory with nested children |
| 3 | should return empty vec when flatten called on single-file tree |
| 4 | should group files by extension when by_extension called on mixed tree |
| 5 | should return N largest files when top_n called with n < file_count |
| 6 | should filter by min size when filter node with size below threshold |
| 7 | should filter by file type when filter node is audio file |
| 8 | should filter by date range when filter node modified outside range |
| 9 | should filter by depth when filter node exceeds max depth |
| 10 | should combine filters (all-match) when FilterSet contains multiple criteria |

### Gates

- **Gate 1** — All unit tests pass (`cargo test -p scan-engine`)
- **Gate 2** — Zero `unsafe`, zero external deps in domain module; review confirms domain purity

---

## Phase 2: Scan Engine Adapters (walker + cache)

**Goal:** Parallel filesystem walking, incremental caching, and output formatting.

### Deliverables

- `scan-engine/src/walker.rs` — `Scanner`:
  - `new(root: &Path, config: ScanConfig) -> Self`
  - `ScanConfig`: `respect_gitignore: bool`, `follow_symlinks: bool`, `max_depth: Option<usize>`
  - `scan() -> Result<FileTree>` — parallel walk via `jwalk` + `rayon`
  - `scan_incremental(cache: &Cache) -> Result<FileTree>` — skip unchanged files (mtime + size match)
  - Build `FileTree` bottom-up from walk results
- `scan-engine/src/cache.rs` — `Cache`:
  - `open(path: &Path) -> Result<Self>` — redb embedded DB
  - `store(&self, tree: &FileTree) -> Result<()>` — persist snapshot
  - `load(&self) -> Result<FileTree>` — restore last snapshot
  - `is_stale(&self, entry: &FileNode) -> bool` — mtime/size diff check
  - Schema: table `entries` keyed by path, value = `{size, mtime, kind}`
- `scan-engine/src/output.rs` — `OutputFormat` enum:
  - `Json`, `Jsonl`, `Table`, `Tree`
  - `format(&self, tree: &FileTree, filters: &FilterSet) -> Result<String>`
  - Table: tabwriter-aligned; Tree: indented ASCII art
- `scan-engine/src/lib.rs` — public API: `Scanner`, `Cache`, `FileTree`, `Filter`, `FilterSet`, `OutputFormat`, `ScanConfig`

### Tests

| # | Test |
|---|------|
| 1 | should scan directory and return correct file count when walking a flat directory |
| 2 | should scan nested directories recursively when max_depth is None |
| 3 | should respect max_depth when scan config specifies depth limit |
| 4 | should respect .gitignore when respect_gitignore is true |
| 5 | should skip unchanged files when incremental scan matches cache mtime+size |
| 6 | should rescan stale files when incremental scan detects mtime change |
| 7 | should persist and restore tree when cache store then load |
| 8 | should output valid JSON when format is Json |
| 9 | should output indented tree when format is Tree |
| 10 | should apply filters to output when filter set is non-empty |
| 11 | should complete scan of 100k synthetic files in <2 seconds on CI hardware |

### Gates

- **Gate 1** — All unit + integration tests pass (`cargo test -p scan-engine`)
- **Gate 2** — Review confirms: no blocking I/O on main thread; rayon parallelism verified; redb schema stable; no unwrap in public API
- **Gate 0** — Architecture review: hexagonal boundaries clean, domain has zero dep leakage from adapters

---

## Phase 3: CLI Binary

**Goal:** Fully functional CLI that exercises the scan engine with all output formats and filters.

### Deliverables

- `cli/src/main.rs` — `clap`-based CLI:
  - `diskscope scan <PATH> [--format table|json|jsonl|tree] [--min-size] [--max-size] [--type] [--depth] [--pattern] [--no-gitignore]`
  - `diskscope summary <PATH>` — quick stats: total size, file count, top 10 by size, breakdown by type
  - `diskscope cache <PATH>` — show/manage cache (path, age, entry count)
  - `diskscope completions <shell>` — generate shell completions (bash/zsh/fish/powershell)
- Progress bar during scan (via `indicatif`)
- Exit codes: 0 success, 1 scan error, 2 invalid args

### Tests

| # | Test |
|---|------|
| 1 | should output table format when --format table specified |
| 2 | should output JSON when --format json specified |
| 3 | should filter by min-size when --min-size 1MB specified |
| 4 | should show summary with top 10 when summary subcommand used |
| 5 | should generate valid completions when completions bash specified |
| 6 | should exit with code 2 when invalid path provided |
| 7 | should show progress bar when scanning directory with >1000 files |

### Gates

- **Gate 1** — All tests pass; `cargo test -p cli` + manual smoke test
- **Gate 2** — Review: clap derive usage clean, error messages user-friendly, no panics on bad input

---

## Phase 4: GUI — Tauri + React Shell

**Goal:** Desktop app with React chrome (sidebar, toolbar, status bar) and Tauri IPC bridge to scan engine.

### Deliverables

- `gui/` — Tauri v2 project scaffold:
  - `gui/src-tauri/` — Rust backend with Tauri commands:
    - `scan(path: String, config: ScanConfig) -> Result<ScanResult, String>`
    - `scan_incremental(path: String) -> Result<ScanResult, String>`
    - `apply_filters(tree_id: String, filters: Vec<Filter>) -> Result<ScanResult, String>`
    - `delete_entries(paths: Vec<String>) -> Result<DeleteResult, String>` — move to trash
    - `undo_delete(trash_ids: Vec<String>) -> Result<(), String>`
    - `get_cache_info(path: String) -> Result<CacheInfo, String>`
  - `gui/src/` — React + TypeScript + Vite:
    - `App.tsx` — layout: sidebar (tree view) + main (treemap/table) + status bar
    - `components/Toolbar.tsx` — scan controls, format toggle, filter bar
    - `components/TreeView.tsx` — expandable directory tree (name, size, %)
    - `components/Statusbar.tsx` — scan progress, file count, total size
    - `hooks/useScan.ts` — Tauri command wrappers with loading/error state
    - `lib/types.ts` — TypeScript mirrors of Rust domain types
    - `lib/format.ts` — size formatting (B/KB/MB/GB), date formatting
  - Auto-update: Tauri updater configured
  - Window: 1200×800 default, min 800×600, title "DiskScope"

### Tests

| # | Test |
|---|------|
| 1 | should return scan results when scan command invoked with valid path |
| 2 | should return error when scan command invoked with invalid path |
| 3 | should move files to trash when delete_entries command invoked |
| 4 | should restore files when undo_delete command invoked |
| 5 | should display tree view with correct hierarchy when scan results loaded |
| 6 | should update status bar when scan progress changes |
| 7 | should toggle between tree and table view when format toggle clicked |

### Gates

- **Gate 1** — `cargo build -p gui` succeeds; `npm run build` in `gui/` succeeds; TypeScript type-checks clean
- **Gate 3** — Manual E2E: launch app, scan home directory, verify tree view renders, status bar updates

---

## Phase 5: GUI — egui Treemap & Interactivity

**Goal:** Interactive treemap visualization, context menus, keyboard navigation, and safe delete with undo.

### Deliverables

- `gui/src-tauri/src/treemap/` — egui treemap renderer:
  - `layout.rs` — squarified treemap algorithm (input: `&FileTree`, output: `Vec<TreemapRect>`)
  - `render.rs` — egui painting: colored rectangles by file type, labels on hover, click to zoom
  - `interaction.rs` — click-to-drill-down, breadcrumb navigation, right-click context menu
- `gui/src-tauri/src/commands.rs` — extend Tauri commands:
  - `navigate_to(path: String) -> Result<ScanResult, String>` — drill into subdirectory
  - `navigate_up() -> Result<ScanResult, String>`
  - `copy_path(path: String) -> Result<(), String>` — clipboard
  - `open_in_explorer(path: String) -> Result<(), String>` — native file manager
- `gui/src/components/Treemap.tsx` — egui canvas wrapper:
  - Mount egui in Tauri webview (via tauri-plugin-egui or custom bridge)
  - Keyboard: Arrow keys navigate, Enter drills down, Backspace goes up, Delete moves to trash
  - Cmd/Ctrl+Z undoes last delete
- `gui/src/components/FilterPanel.tsx`:
  - Size range slider, file type checkboxes, date range picker, name pattern input
  - Real-time filter application (debounced 300ms)
- Color scheme by file type: video=blue, image=green, audio=purple, code=orange, archive=red, doc=yellow, other=gray

### Tests

| # | Test |
|---|------|
| 1 | should produce non-overlapping rectangles when squarified layout applied to tree |
| 2 | should drill down when treemap cell clicked and node is directory |
| 3 | should navigate up when backspace pressed at subdirectory level |
| 4 | should move to trash when delete pressed on selected file |
| 5 | should undo delete when cmd+z pressed after delete |
| 6 | should open native file manager when "Open in Explorer" selected from context menu |
| 7 | should apply size filter when slider changed and update treemap |
| 8 | should filter by file type when checkbox toggled |
| 9 | should copy path to clipboard when "Copy Path" selected from context menu |
| 10 | should resize treemap correctly when window resized |

### Gates

- **Gate 1** — All layout + interaction tests pass
- **Gate 3** — Visual E2E: treemap renders with correct proportions; drill-down, filter, delete, undo all functional; keyboard navigation works; context menu appears on right-click

---

## Phase 6: Real-time Sync (Ably)

**Goal:** Multi-device scan result sync with live updates and conflict resolution.

### Deliverables

- `scan-engine/src/sync.rs` — sync domain types:
  - `SyncEvent` enum: `ScanUpdate { path, tree }`, `DeleteEvent { paths }`, `FilterChange { filters }`
  - `SyncConfig`: `ably_api_key: String`, `channel_prefix: String`, `device_id: String`
- `gui/src-tauri/src/sync.rs` — Ably adapter:
  - `SyncManager::new(config: SyncConfig) -> Result<Self>`
  - `publish(&self, event: SyncEvent) -> Result<()>`
  - `subscribe(&self, callback: impl Fn(SyncEvent)) -> Result<Subscription>`
  - `resolve_conflict(local: SyncEvent, remote: SyncEvent) -> SyncEvent` — last-write-wins by timestamp
  - Channel naming: `diskscope:{user_id}:{device_group}`
- `gui/src/hooks/useSync.ts`:
  - Connect/disconnect Ably on app start/stop
  - Incoming events update local state
  - Outgoing events published on scan completion, delete, filter change
- `gui/src-tauri/src/commands.rs` — extend:
  - `configure_sync(config: SyncConfig) -> Result<(), String>`
  - `get_sync_status() -> Result<SyncStatus, String>` (connected, last sync, peer count)

### Tests

| # | Test |
|---|------|
| 1 | should publish scan update when scan completes and sync is configured |
| 2 | should apply remote scan update when received via subscription |
| 3 | should resolve conflict using last-write-wins when two devices update same path |
| 4 | should update peer count when device connects/disconnects |
| 5 | should gracefully degrade when Ably connection lost (queue locally, retry) |

### Gates

- **Gate 1** — Sync unit tests pass (mocked Ably transport)
- **Gate 2** — Review: no API keys in source, offline-first verified (app works without sync configured), conflict resolution correct

---

## Phase 7: Packaging & Distribution

**Goal:** Cross-platform installers, code signing, auto-update.

### Deliverables

- `.github/workflows/release.yml` — triggered on tag push (`v*`):
  - Matrix: linux (ubuntu-latest), macos (macos-latest), windows (windows-latest)
  - Linux: `.AppImage`, `.deb`, `.rpm`, `.tar.gz`
  - macOS: universal binary (x86_64 + aarch64), `.dmg`, notarized
  - Windows: `.msi`, portable `.exe`
  - Upload artifacts to GitHub Release
- `.github/workflows/ci.yml` — on push/PR:
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo fmt --check`
  - `npm run lint` + `npm run typecheck` in `gui/`
- Code signing:
  - macOS: Developer ID certificate (via `APPLE_SIGNING_IDENTITY` secret)
  - Windows: EV certificate (via `WINDOWS_CERTIFICATE` secret)
- Tauri auto-update: configured with update endpoint pointing to GitHub Releases
- `INSTALL.md` — per-platform install instructions (generated from CI artifacts)

### Tests

| # | Test |
|---|------|
| 1 | should produce .AppImage on ubuntu-latest when release workflow runs |
| 2 | should produce .dmg on macos-latest when release workflow runs |
| 3 | should produce .msi on windows-latest when release workflow runs |
| 4 | should pass all gates (test + clippy + fmt) when CI runs on PR |
| 5 | should trigger auto-update when new version published and app checks for updates |

### Gates

- **Gate 1** — CI green on all platforms; release matrix produces all artifacts
- **Gate 3** — E2E on each platform: install from artifact, launch, scan, delete, verify auto-update prompt

---

## Gate Summary

| Gate | Criteria | Applied In |
|------|----------|------------|
| **Gate 0** | Plan review — architecture, risk assessment, TDD plan | Before Phase 1 |
| **Gate 1** | All tests pass (unit + integration) | Every phase |
| **Gate 2** | Adversarial code review (different model) | Phases 2, 4, 6 |
| **Gate 3** | Visual + functional E2E | Phases 4, 5, 7 |

---

## Dependencies & Risks

| Risk | Mitigation |
|------|------------|
| `jwalk` performance on network drives | Phase 2 benchmark; fallback to sequential walk if needed |
| egui in Tauri webview — plugin maturity | Prototype in Phase 4; fallback: pure canvas + JS treemap |
| Ably free tier limits (6k msg/min) | Batch sync events; debounced publish; offline queue |
| macOS notarization latency | CI caches notarization; test in staging before release |
| Windows EV cert availability | Procure cert early; HSM-backed signing in CI |

---

## TDD Commitment

Every phase writes tests **before** implementation:
1. Define test cases from acceptance criteria
2. Write failing tests
3. Implement minimum code to pass
4. Refactor with green suite
5. Gate 2 review on non-trivial phases

---

## Phase Dependencies

```
Phase 1 (Domain)
    │
    ├──► Phase 2 (Scan Engine) ──► Phase 3 (CLI)
    │                                 │
    └──► Phase 4 (GUI Shell) ────────┘
              │
              ▼
         Phase 5 (Treemap)
              │
              ▼
         Phase 6 (Sync)
              │
              ▼
         Phase 7 (Packaging)
```

Phases 3 and 4 can proceed in parallel after Phase 2.
