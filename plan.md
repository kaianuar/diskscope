# DiskScope — Build Plan

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                    Adapters                          │
│  ┌─────────┐   ┌─────────┐   ┌──────────────────┐  │
│  │   CLI   │   │  GUI    │   │  Real-time Sync  │  │
│  │ (clap)  │   │ (Tauri) │   │     (Ably)       │  │
│  └────┬────┘   └────┬────┘   └────────┬─────────┘  │
│       │              │                 │             │
│  ┌────┴──────────────┴─────────────────┴──────────┐ │
│  │              scan-engine (lib)                 │ │
│  │  ┌──────────┐  ┌──────────┐  ┌─────────────┐  │ │
│  │  │  domain  │  │  ports   │  │  adapters    │  │ │
│  │  │  (pure)  │  │ (traits) │  │ (rayon/jwalk)│  │ │
│  │  └──────────┘  └──────────┘  └─────────────┘  │ │
│  └────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

**Crate layout:** Cargo workspace  
- `scan-engine` — domain + ports + scan adapters (lib)  
- `cli` — clap binary  
- `gui` — Tauri v2 binary  

---

## Phase 1: Domain Core

**Goal:** Pure domain types and ports — zero external deps, compiles in isolation.

### Deliverables

**Domain types (`scan-engine/src/domain/`):**
```
mod.rs          — re-exports
node.rs         — FileNode { path, name, size, modified, node_type, children }
filter.rs       — Filter { min_size, max_size, file_types, name_pattern, max_depth, since }
scan_result.rs  — ScanResult { root, total_size, file_count, dir_count, scan_duration }
```

**Ports (`scan-engine/src/ports/`):**
```
scanner.rs      — trait Scanner { async fn scan(&self, path: &Path, filter: Filter) -> Result<ScanResult> }
trash.rs        — trait Trash { async fn delete(&self, path: &Path) -> Result<TrashId>; async fn undo(&self, id: TrashId) -> Result<()> }
```

**Error types:** `ScanError` enum with `NotFound`, `PermissionDenied`, `Cancelled`, `Io`.

### Tests

| Test | Behavior | Condition |
|------|----------|-----------|
| `test_node_size_calculation` | Sum of children sizes equals parent | Directory with 3 files |
| `test_node_sort_by_size` | Largest child first | Nodes with different sizes |
| `test_filter_size_range` | Only files in range returned | Filter `min_size=1MB, max_size=100MB` |
| `test_filter_by_type` | Only matching extensions | Filter `file_types=[Image, Video]` |
| `test_filter_by_pattern` | Only matching names | Filter `name_pattern="*.log"` |
| `test_scan_result_totals` | Correct aggregates | Scan of test directory |
| `test_error_permission_denied` | `ScanError::PermissionDenied` | Scan protected directory |

### Gate
- ✅ Gate 0: Architecture review (pure domain, trait-based ports)
- ✅ Gate 1: Unit tests pass (domain + port contract tests)

---

## Phase 2: Scan Engine

**Goal:** Parallel scanner + filters + caching — functional CLI smoke test.

### Deliverables

**Scanner adapter (`scan-engine/src/adapters/`):**
```
parallel_scanner.rs  — RayonThreadPoolScanner: impl Scanner
                      Uses jwalk for parallel walk, rayon for CPU work
                      Respects .gitignore via ignore crate
                      Progress callback: Fn(ScanProgress)

redb_cache.rs        — RedbCache: impl ScanCache
                      Caches last N scans, incremental diff support
```

**Filter implementation:**  
Apply Filter in pipeline: size → type → pattern → depth → time.

**Caching:**  
- `ScanCache` trait: `get(root_path) -> Option<ScanResult>`, `put(root_path, result)`
- Incremental: walk new tree, diff with cached, re-scan only changed subtrees

**Public API (`scan-engine/src/lib.rs`):**
```rust
pub async fn scan(path: &Path, filter: Filter, cache: Option<&dyn ScanCache>) -> Result<ScanResult>
```

### Tests

| Test | Behavior | Condition |
|------|----------|-----------|
| `test_parallel_scan_small_dir` | All files discovered | Scan `tests/fixtures/small/` (50 files) |
| `test_parallel_scan_respects_gitignore` | Ignored files excluded | Scan dir with `.gitignore` |
| `test_parallel_scan_symlinks` | No cycles, sizes correct | Dir with circular symlinks |
| `test_filter_combined` | All filters applied | Size + type + pattern filter |
| `test_scan_progress_callback` | Called with correct counts | Scan 1000-file directory |
| `test_cache_hit` | Returns cached result | Re-scan same path |
| `test_incremental_scan` | Only changed subtree re-scanned | Modify 1 file, re-scan |
| `test_scan_cancel` | Returns `ScanError::Cancelled` | Cancel during large scan |
| `test_scan_100k_files` | <2 seconds | `tests/fixtures/large/` (100k files) |

### Performance targets
- 100k files < 2s on modern hardware (4+ cores)
- Memory < 200MB during scan
- UI thread never blocked (background scanner)

### Gate
- ✅ Gate 1: Integration tests pass (real filesystem, no mocks)

---

## Phase 3: CLI

**Goal:** Fully functional CLI with all output formats.

### Deliverables

**CLI binary (`cli/src/`):**
```
main.rs        — entry point, clap args
commands/
  scan.rs      — `diskscope scan [path] --format table|json|jsonl|tree`
  summary.rs   — `diskscope summary <path>` (quick size report)
  completions.rs — shell completions (bash/zsh/fish/powershell)
```

**Output formats:**
- `table` — human-readable, sorted by size
- `json` — single ScanResult object
- `jsonl` — one FileNode per line (streaming)
- `tree` — indented tree with sizes

**Commands:**
```bash
diskscope scan ~/projects --format table --min-size 1MB
diskscope summary /var/log
diskscope completions bash > ~/.bash_completion.d/diskscope
```

### Tests

| Test | Behavior | Condition |
|------|----------|-----------|
| `test_scan_table_output` | Human-readable table | `--format table` |
| `test_scan_json_output` | Valid JSON, correct fields | `--format json` |
| `test_scan_jsonl_output` | One object per line | `--format jsonl` |
| `test_scan_tree_output` | Indented tree structure | `--format tree` |
| `test_scan_filter_min_size` | Only large files shown | `--min-size 10MB` |
| `test_scan_filter_type` | Only matching types | `--type image --type video` |
| `test_scan_cancel_ctrl_c` | Graceful exit | Send SIGINT during scan |
| `test_summary_quick` | Returns in <100ms | Small directory |
| `test_completions_bash` | Valid bash completion | `--completions bash` |

### Gate
- ✅ Gate 1: CLI integration tests (spawn process, assert output)
- ✅ Gate 2: Adversarial review (CLI UX, error handling)

---

## Phase 4: GUI

**Goal:** Tauri v2 app with interactive treemap + table view.

### Deliverables

**Tauri integration (`gui/src-tauri/`):**
```
src/
  main.rs           — Tauri app setup, register commands
  commands/
    scan.rs         — #[tauri::command] fn start_scan(path, filter)
    delete.rs       — #[tauri::command] fn delete_file(path) -> Result<()>
    undo_delete.rs  — #[tauri::command] fn undo_delete(id) -> Result<()>
  state.rs          — AppState { active_scans, trash_log }
```

**Frontend (`gui/src/`):**
```
main.tsx              — React entry
App.tsx               — Layout: sidebar + main panel
components/
  Sidebar.tsx         — Directory picker, recent scans
  Treemap.tsx         — egui treemap (WebAssembly)
  Table.tsx           — Sortable file list (React)
  FilterBar.tsx       — Size/type/pattern filters
  ContextMenu.tsx     — Right-click: open, copy path, delete
  Toolbar.tsx         — Back, forward, up, refresh
hooks/
  useScan.ts          — Tauri invoke wrapper, progress state
  useKeyboard.ts      — Arrow keys, delete, ctrl+z
ipc.ts                — Type-safe Tauri invoke wrappers
```

**Treemap (egui WebAssembly):**
- Render FileNode hierarchy as nested rectangles
- Hover: show name + size + percentage
- Click: drill into directory
- Color: by file type (images=blue, video=red, code=green, etc.)
- Responsive: resize with window

**Table view:**
- Columns: Name, Size, Modified, Type
- Sortable by click
- Virtualized (egui_extras TableBuilder)
- Multi-select with shift/ctrl

**Keyboard shortcuts:**
- Arrow keys: navigate
- Enter: drill into directory
- Backspace: go up
- Delete: move to trash
- Cmd/Ctrl+Z: undo delete
- Cmd/Ctrl+F: focus search/filter

**Context menu:**
- Open in file explorer
- Copy path
- Copy size
- Move to trash

### Tests

| Test | Behavior | Condition |
|------|----------|-----------|
| `test_tauri_scan_command` | Returns ScanResult | Valid path |
| `test_tauri_scan_cancel` | Stops gracefully | Cancel during scan |
| `test_delete_to_trash` | File moved to trash | Delete file |
| `test_undo_delete` | File restored | Undo after delete |
| `test_treemap_renders` | Canvas shows rectangles | Scan complete |
| `test_treemap_hover_tooltip` | Tooltip appears | Hover over rectangle |
| `test_treemap_click_drill` | Navigates into dir | Click directory |
| `test_table_sort_by_size` | Largest first | Click Size column |
| `test_filter_updates_view` | View filtered | Change filter |
| `test_keyboard_navigation` | Selection moves | Arrow keys |
| `test_context_menu_delete` | Triggers delete | Right-click → Delete |

### Gate
- ✅ Gate 1: Unit tests (Tauri commands, state management)
- ✅ Gate 3: Visual E2E (Playwright + vision model)

---

## Phase 5: Real-time Sync

**Goal:** Multi-device scan sync via Ably.

### Deliverables

**Ably adapter (`scan-engine/src/adapters/ably_sync.rs`):**
```rust
pub struct AblySync { client: ably::Rest, channel: String }

impl SyncPort for AblySync {
    async fn publish_scan(&self, result: &ScanResult) -> Result<()>;
    async fn subscribe_scans(&self, callback: Box<dyn Fn(ScanResult)>) -> Result<()>;
    async fn resolve_conflict(&self, local: &ScanResult, remote: &ScanResult) -> ScanResult;
}
```

**Port:**
```rust
trait SyncPort {
    async fn publish_scan(&self, result: &ScanResult) -> Result<()>;
    async fn subscribe_scans(&self, callback: Box<dyn Fn(ScanResult)>) -> Result<()>;
    async fn resolve_conflict(&self, local: &ScanResult, remote: &ScanResult) -> ScanResult;
}
```

**Conflict resolution:** Last-write-wins with timestamp (simple, predictable).

**GUI integration:**
- Real-time indicator (connected/syncing/offline)
- Merge remote scans into local view
- Show "Last synced: 2 min ago"

### Tests

| Test | Behavior | Condition |
|------|----------|-----------|
| `test_publish_scan` | Published to Ably channel | Valid scan result |
| `test_subscribe_receives` | Callback called | Scan published |
| `test_conflict_resolution` | Latest timestamp wins | Two conflicting scans |
| `test_offline_queue` | Queued, syncs on reconnect | Publish while offline |
| `test_concurrent_edits` | No data loss | Two devices scan same dir |

### Gate
- ✅ Gate 1: Integration tests (Ably sandbox)
- ✅ Gate 2: Adversarial review (security, conflict edge cases)

---

## Phase 6: Packaging & Distribution

**Goal:** Cross-platform installers + auto-update.

### Deliverables

**CI/CD (`.github/workflows/`):**
```
ci.yml          — Test on push (linux, macos, windows)
release.yml     — Build installers on tag
```

**Installers:**
- Linux: `.AppImage`, `.deb`, `.rpm`, `.tar.gz`
- macOS: Universal `.dmg` (arm64 + x86_64), notarized
- Windows: `.msi`, portable `.exe`

**Auto-update:**
- Tauri updater (checks GitHub releases)
- Graceful: download in background, apply on restart

**Code signing:**
- macOS: Developer ID certificate
- Windows: EV code signing certificate

### Tests

| Test | Behavior | Condition |
|------|----------|-----------|
| `test_appimage_runs` | Opens on Ubuntu 22.04 | Fresh install |
| `test_dmg_installs` | Drag to Applications works | macOS 13+ |
| `test_msi_installs` | Installed to Program Files | Windows 10+ |
| `test_auto_update` | Prompts for update | New version available |
| `test_cli_help` | Shows help text | `diskscope --help` |

### Gate
- ✅ Gate 1: CI passes on all platforms
- ✅ Gate 3: E2E on each platform (Playwright + vision model)

---

## Gate Summary

| Gate | Criteria | Phases |
|------|----------|--------|
| **Gate 0** | Plan reviewed, architecture sound | Phase 1 (this plan) |
| **Gate 1** | Unit + integration tests pass | All phases |
| **Gate 2** | Adversarial code review | Phase 3, 5 |
| **Gate 3** | Visual + functional E2E | Phase 4, 6 |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| jwalk + ignore crate conflict | Test with real `.gitignore` files early |
| egui treemap performance | Benchmark 100k rectangles, optimize batch rendering |
| Tauri IPC overhead | Measure round-trip time, use event streams for progress |
| Ably rate limits | Implement exponential backoff, offline queue |
| Windows long paths | Use `\\?\` prefix, test with 260+ char paths |

---

## Dependencies

```toml
# scan-engine
rayon = "1.10"
jwalk = "0.8"
ignore = "0.4"
redb = "2"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
trash = "5"
ably-rest = "0.5"  # Phase 5

# cli
clap = { version = "4", features = ["derive"] }
colored = "2"
tabled = "0.17"

# gui (Tauri v2)
tauri = { version = "2", features = ["api-all"] }
egui = "0.30"
eframe = "0.30"
```

---

## Timeline

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Domain Core | 2 days | None |
| Phase 2: Scan Engine | 3 days | Phase 1 |
| Phase 3: CLI | 2 days | Phase 2 |
| Phase 4: GUI | 5 days | Phase 2 |
| Phase 5: Real-time Sync | 3 days | Phase 4 |
| Phase 6: Packaging | 3 days | Phase 4 |
| **Total** | **18 days** | |

*Parallel: Phase 5 and 6 can run concurrently after Phase 4.*
