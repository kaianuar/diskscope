# DiskScope Implementation Plan

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    gui (Tauri)                        │
│  ┌────────────────┐  ┌──────────────────────────┐  │
│  │ React (chrome)  │  │ egui (canvas/treemap)    │  │
│  └────────┬───────┘  └────────────┬─────────────┘  │
│           └───────────┬───────────┘                  │
│                       │ IPC / commands               │
├───────────────────────┼─────────────────────────────┤
│                       │                              │
├───────────────────────┼─────────────────────────────┤
│                  scan-engine (lib)                    │
│  ┌────────────────────┴──────────────────────────┐  │
│  │ parallel walker (jwalk) │ cache (redb)        │  │
│  │ rayon pool              │ filters             │  │
│  │ incremental scan        │ trash integration   │  │
│  └────────────────────┬──────────────────────────┘  │
│                       │                              │
├───────────────────────┼─────────────────────────────┤
│                   domain (pure)                      │
│  FileNode │ ScanResult │ Filter │ SortSpec │ Trash  │
└─────────────────────────────────────────────────────┘
```

**Workspace crates:**
- `domain` — pure types + logic, zero external deps
- `scan-engine` — parallel scanner, caching, trash; depends on `domain`
- `cli` — CLI binary; depends on `scan-engine`
- `gui` — Tauri v2 binary; depends on `scan-engine`

---

## Phase 1: Domain Core

> **Goal**: Pure Rust domain with zero external dependencies. All types, validation, and business rules. Independently testable without I/O.

### Deliverables
1. `FileNode` — path, size (u64), modified (u64 epoch), file type enum, children
2. `ScanResult` — root `FileNode`, total size, file count, scan duration
3. `Filter` — size range, file type set, age range, name glob pattern, depth limit
4. `SortSpec` — column enum (Name, Size, Modified, Type), direction (Asc, Desc)
5. `FileType` enum — Audio, Video, Image, Document, Code, Archive, Directory, Other; `from_extension()` classification
6. `format_size(bytes) -> String` — human-readable (B, KB, MB, GB, TB)
7. Domain error type (`DomainError`) — invalid path, invalid filter, permission denied

### Tests
- `should classify .mp3 as Audio when from_extension called`
- `should classify .rs as Code when from_extension called`
- `should classify unknown extension as Other when from_extension called`
- `should format 1024 as "1.0 KB" when format_size called`
- `should format 0 as "0 B" when format_size called`
- `should format 1_073_741_824 as "1.0 GB" when format_size called`
- `should reject negative size range when Filter validated`
- `should reject empty name pattern when Filter validated`
- `should sort descending by size when SortSpec direction is Desc`
- `should compute total size recursively when ScanResult built from tree`
- `should respect depth limit when Filter applied to FileNode tree`

### Gates
- Gate 0: plan review ✓
- Gate 1: unit tests pass ✓

---

## Phase 2: Scan Engine

> **Goal**: Parallel directory scanner with caching and trash integration. All I/O lives here.

### Deliverables
1. `Scanner` — `scan(root: &Path, config: ScanConfig) -> Result<ScanResult, ScanError>`
   - Parallel walk via `jwalk` + `rayon` thread pool
   - Respects `.gitignore` via `ignore` crate
   - Applies `Filter` during walk (skip early, don't collect then filter)
2. `Cache` — `redb` embedded DB; keyed by `(path, mtime, size)`; incremental re-scan
3. `TrashService` — `trash::delete(path)`, `trash::list()`, `trash::restore(id)`
   - Uses `trash` crate (cross-platform)
4. `ScanConfig` — root path, filter, max depth, cache enabled, follow symlinks
5. `ScanError` — wraps `DomainError` + I/O errors (permission, not found, broken symlink)
6. `Progress` — callback channel: `(files_scanned, current_path)` for UI updates
7. JSON / JSONL / table / tree output formatters on `ScanResult`

### Tests
- `should scan directory recursively when Scanner.scan called`
- `should skip .gitignore entries when scan respects gitignore config`
- `should respect filter by file type when Filter includes only Code`
- `should respect filter by size range when Filter specifies min_bytes`
- `should return cached result when file mtime unchanged and cache enabled`
- `should update cache entry when file modified since last scan`
- `should delete file to trash when TrashService.delete called`
- `should list trashed items when TrashService.list called`
- `should restore file from trash when TrashService.restore called with valid id`
- `should emit progress events during scan when callback provided`
- `should handle permission denied gracefully when scan encounters unreadable dir`
- `should format as JSON when OutputFormat::Json requested`
- `should format as tree when OutputFormat::Tree requested`
- `should complete 100k files in <2s when scanning modern SSD` *(performance gate)*

### Gates
- Gate 0: TDD plan review ✓
- Gate 1: unit + integration tests pass ✓

---

## Phase 3: CLI

> **Goal**: Fully functional CLI binary. Scannable, filterable, deletable — no GUI dependency.

### Deliverables
1. `diskscope scan [path] --format table|json|jsonl|tree [--min-size] [--max-size] [--type] [--pattern] [--depth]`
2. `diskscope summary <path>` — quick total size + top-10 largest items
3. `diskscope trash list` / `diskscope trash restore <id>`
4. `diskscope completions <shell>` — bash/zsh/fish completions (via `clap`)
5. `--no-cache` flag, `--no-gitignore` flag
6. `clap` derive CLI with colored help, version, error display
7. `Ctrl+C` graceful cancel (propagates to scanner)

### Tests
- `should print table output when --format table specified`
- `should print JSON when --format json specified`
- `should exit 0 when scan completes successfully`
- `should exit 1 when path does not exist`
- `should print top 10 largest when summary called`
- `should filter by --type audio when flag passed`
- `should generate completions for bash when completions bash requested`
- `should cancel cleanly on SIGINT when scan in progress`

### Gates
- Gate 0: plan review ✓
- Gate 1: tests pass ✓
- Gate 2: adversarial code review ✓

---

## Phase 4: GUI — Tauri Shell + React Chrome

> **Goal**: Tauri v2 app with React frontend. Window, routing, state, IPC skeleton. No egui canvas yet.

### Deliverables
1. Tauri v2 project with `tauri.conf.json` (window size, title, bundle config)
2. React 18 + TypeScript + Vite frontend
3. Tauri IPC commands: `start_scan`, `get_scan_state`, `cancel_scan`, `trash_delete`, `trash_restore`
4. React state management for scan lifecycle (idle → scanning → complete → error)
5. Scan progress bar + file counter (binds to `Progress` channel)
6. Sidebar: drive/volume selector + recent scans
7. Top bar: breadcrumb path + filter controls
8. Settings page: cache path, gitignore toggle, theme

### Tests
- `should display scan progress when start_scan IPC called`
- `should transition to complete state when scan finishes`
- `should show error state when scan path invalid`
- `should cancel scan when cancel_scan IPC called during active scan`
- `should persist filter settings when user changes filter controls`

### Gates
- Gate 0: plan review ✓
- Gate 1: tests pass ✓
- Gate 2: adversarial code review ✓

---

## Phase 5: GUI — Treemap + Tree/Table View

> **Goal**: Interactive visualization. Treemap canvas (egui via Tauri) and sortable table.

### Deliverables
1. egui treemap component — `egui_extras` treemap; clickable rectangles, hover tooltip (name, size, %), click to zoom
2. Table view — sortable columns (name, size, modified, type); virtual scroll for >10k rows
3. View toggle: treemap ↔ table ↔ split
4. Selection sync: selecting in treemap highlights in table and vice versa
5. Right-click context menu: open in file explorer, copy path, copy size
6. Keyboard navigation: arrows, enter (drill down), backspace (drill up), delete (trash)
7. `Cmd/Ctrl+Z` undo — restores last trashed item

### Tests
- `should render treemap rectangles when scan data present`
- `should zoom into subdirectory when treemap rectangle clicked`
- `should sort by size descending when Size column header clicked`
- `should navigate up when backspace pressed at subdirectory level`
- `should move to trash when delete key pressed on selected item`
- `should undo last trash when Cmd+Z pressed`
- `should sync selection between treemap and table when item selected in either`

### Gates
- Gate 0: plan review ✓
- Gate 1: tests pass ✓
- Gate 2: adversarial code review ✓
- Gate 3: visual + functional E2E (Playwright + vision model) ✓

---

## Phase 6: Real-time Sync (Ably)

> **Goal**: Multi-device scan sync. Offline-first with sync-when-online.

### Deliverables
1. Ably channel per scan (keyed by `(device_id, scan_path_hash)`)
2. Scan result diff → publish on change; subscribe on other devices
3. Last-write-wins conflict resolution (timestamp)
4. Offline queue: pending diffs buffered, flushed on reconnect
5. UI indicator: sync status (connected / syncing / offline)

### Tests
- `should publish scan diff when local scan completes`
- `should apply remote diff when received on subscribed device`
- `should resolve conflict with last-write-wins when concurrent edits detected`
- `should buffer diffs offline when Ably disconnected`
- `should flush buffered diffs when connection restored`

### Gates
- Gate 0: plan review ✓
- Gate 1: tests pass ✓
- Gate 2: adversarial code review ✓
- Gate 3: visual + functional E2E ✓

---

## Phase 7: Packaging & CI/CD

> **Goal**: Cross-platform builds, code signing, auto-update. Artifacts ready for distribution.

### Deliverables
1. GitHub Actions workflow: build + test on Linux, macOS, Windows
2. Artifacts: `.dmg` (macOS universal), `.msi` (Windows), `.AppImage` + `.deb` + `.rpm` + `.tar.gz` (Linux)
3. Tauri auto-update: configured with update endpoint
4. Code signing: macOS Developer ID, Windows EV cert (secrets in CI)
5. Release workflow: tag → build → sign → upload → publish

### Tests
- `should produce valid AppImage when Linux build completes`
- `should produce signed DMG when macOS build completes`
- `should produce MSI installer when Windows build completes`
- `should trigger auto-update when new version published`

### Gates
- Gate 0: plan review ✓
- Gate 1: all platform builds pass ✓

---

## Risk Register

| Risk | Mitigation |
|------|-----------|
| egui treemap in Tauri is non-trivial (embedding) | Prototype in Phase 5; fallback to pure React treemap |
| `jwalk` + `ignore` crate interaction on symlinks | Explicit test coverage; follow_symlinks opt-in only |
| `trash` crate inconsistency across Linux distros | Test on Ubuntu, Fedora, Arch; document limitations |
| Ably SDK Rust support immature | Evaluate `ably-rs` crate early; fallback to REST API |
| Scan performance <2s for 100k files | Benchmark early in Phase 2; tune rayon thread count |
| macOS code signing CI complexity | Start with unsigned builds; signing in Phase 7 only |

---

## Gate Summary

| Gate | Criteria | Phases |
|------|----------|--------|
| Gate 0 | Architecture review, TDD plan, risk assessment | 1–7 |
| Gate 1 | Unit + integration tests pass | 1–7 |
| Gate 2 | Adversarial code review (different model) | 3–6 |
| Gate 3 | Visual + functional E2E (Playwright + vision) | 5–6 |

---

## Dependencies Between Phases

```
Phase 1 (Domain)
    ↓
Phase 2 (Scan Engine)
    ↓
Phase 3 (CLI) ← can start once Phase 2 done
Phase 4 (GUI Shell) ← can start once Phase 2 done (parallel with CLI)
    ↓
Phase 5 (GUI Visualization) ← after Phase 4
    ↓
Phase 6 (Sync) ← after Phase 5
    ↓
Phase 7 (Packaging) ← after Phase 5 minimum; after Phase 6 for full release
```

Phases 3 and 4 can proceed in parallel after Phase 2 completes.
