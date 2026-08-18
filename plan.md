# DiskScope — Build Plan

## 1. PROBLEM & GOAL

Users need a fast, cross-platform disk space analyzer. Existing tools are paid (DaisyDisk), platform-specific (WinDirStat), or slow (ncdu).

**Goal:** A Rust/Tauri disk analyzer that scans 100k files in <2s, renders an interactive treemap, and safely deletes via system trash — cross-platform, single binary, free.

---

## 2. ARCHITECTURE

### 2.1 High-Level (Hexagonal)

```
┌─────────────────────────────────────────────────────┐
│                     Adapters                         │
│  ┌─────────┐   ┌──────────────┐   ┌──────────────┐  │
│  │   CLI   │   │  scan-engine │   │  GUI (Tauri) │  │
│  │ (clap)  │   │(rayon+jwalk) │   │ (egui+React) │  │
│  └────┬────┘   └──────┬───────┘   └──────┬───────┘  │
│       │               │                   │          │
│       └───────────────┼───────────────────┘          │
│                       ▼                              │
│               ┌──────────────┐                       │
│               │    domain    │  ← zero dependencies  │
│               │  (FileNode,  │                       │
│               │   Filter,    │                       │
│               │   ScanResult)│                       │
│               └──────────────┘                       │
└─────────────────────────────────────────────────────┘
```

### 2.2 Workspace Crates

| Crate | Path | Depends On | External Deps |
|---|---|---|---|
| `domain` | `domain/` | — | none |
| `scan-engine` | `scan-engine/` | `domain` | rayon, jwalk, ignore, redb |
| `cli` | `cli/` | `domain`, `scan-engine` | clap |
| `gui` | `gui/` | `domain`, `scan-engine` | tauri, egui, egui_extras |

### 2.3 Key Ports (domain defines, adapters implement)

```rust
// domain/src/ports.rs — scan port trait
pub trait Scanner {
    fn scan(&self, path: &Path) -> Result<ScanResult, DomainError>;
}

// domain/src/ports.rs — trash port trait
pub trait Trash {
    fn move_to_trash(&self, path: &Path) -> Result<(), DomainError>;
    fn undo_last(&self) -> Result<(), DomainError>;
}

// domain/src/ports.rs — cache port trait
pub trait Cache {
    fn get(&self, path: &Path) -> Option<ScanResult>;
    fn put(&self, path: &Path, result: &ScanResult) -> Result<(), DomainError>;
    fn invalidate(&self, path: &Path) -> Result<(), DomainError>;
}
```

### 2.4 Data Flow — Scan

```
CLI/GUI → scan_engine::scan(path, filter, cache)
            → jwalk parallel walk (rayon thread pool)
            → build FileNode tree in memory
            → apply Filter (size/type/age/name/depth)
            → compute ScanResult (total_size, file_count, duration)
            → cache in redb
            → return ScanResult to adapter
```

### 2.5 Data Flow — Safe Delete

```
GUI → scan_engine::delete(path)
        → trash::move_to_trash(path)  // OS trash via `trash` crate
        → push (path, original_location) onto undo stack
        → return Ok(()) or DomainError::PermissionDenied

GUI → scan_engine::undo()
        → pop from undo stack
        → move file back to original location
        → return Ok(()) or DomainError::NothingToUndo
```

---

## 3. PHASED BUILD PLAN

### Phase 0: Workspace + Domain Foundation
**Status: ~80% done.** `domain/src/lib.rs` has FileType, FileNode, ScanResult, Filter, SortSpec, DomainError, format_size with 12 passing tests.

**Remaining deliverables:**
- Root `Cargo.toml` workspace manifest (members: domain, scan-engine, cli, gui)
- `domain/src/ports.rs` — Scanner, Trash, Cache port traits
- Port trait tests (mock impls verifying contract)

**Tests (TDD):**
| # | Test | Type |
|---|---|---|
| 1 | `should require scan method when Scanner trait implemented` | unit |
| 2 | `should require move_to_trash and undo_last when Trash trait implemented` | unit |
| 3 | `should return cached result when Cache::get called with known path` | unit |
| 4 | `should invalidate entry when Cache::invalidate called` | unit |

**Gates:** Gate 0 (plan review — this file).

---

### Phase 1: Scan Engine Adapter
**Deliverables:**
- `scan-engine/` crate with `Cargo.toml` (deps: domain, rayon, jwalk, ignore, redb)
- `scan-engine/src/lib.rs` — `EngineScanner` impl of `Scanner` port
- `scan-engine/src/walk.rs` — parallel directory walk (jwalk + rayon)
- `scan-engine/src/cache.rs` — `RedbCache` impl of `Cache` port (redb)
- `scan-engine/src/incremental.rs` — re-scan only changed files (mtime check vs cache)
- `scan-engine/src/filter.rs` — age filter support (extend domain Filter with `min_age`/`max_age`)

**Tests (TDD):**
| # | Test | Type |
|---|---|---|
| 1 | `should build FileNode tree with correct sizes when scanning a flat directory` | integration |
| 2 | `should walk nested directories in parallel when scanning a tree` | integration |
| 3 | `should respect .gitignore when scanning a git repo` | integration |
| 4 | `should return ScanResult with total_size and file_count when scan completes` | integration |
| 5 | `should complete scan of 1000 files in under 2 seconds when hardware is modern` | perf |
| 6 | `should cache scan result in redb when cache miss occurs` | integration |
| 7 | `should return cached result on second scan when files unchanged` | integration |
| 8 | `should re-scan only changed files when incremental scan runs` | integration |
| 9 | `should filter by age when min_age/max_age set in filter` | unit |
| 10 | `should return PermissionDenied when scanning unreadable directory` | integration |
| 11 | `should scan to memory under 200MB when walking 100k synthetic files` | perf |

**Gate:** Gate 1 (cargo test scan-engine).

---

### Phase 2: CLI Adapter
**Deliverables:**
- `cli/` crate with `Cargo.toml` (deps: domain, scan-engine, clap)
- `cli/src/main.rs` — clap-based arg parsing
- `cli/src/commands/scan.rs` — `diskscope scan [path] --format table|json|jsonl|tree`
- `cli/src/commands/summary.rs` — `diskscope summary <path>` (quick stats)
- `cli/src/commands/completions.rs` — `diskscope completions <shell>`
- `cli/src/output.rs` — formatters: table (prettytable), JSON, JSONL, tree (indented)

**Tests (TDD):**
| # | Test | Type |
|---|---|---|
| 1 | `should parse scan command with path and format when args provided` | unit |
| 2 | `should default to table format when no --format flag given` | unit |
| 3 | `should emit valid JSON when --format json requested` | integration |
| 4 | `should emit one JSON object per line when --format jsonl requested` | integration |
| 5 | `should display indented tree when --format tree requested` | integration |
| 6 | `should show top-10 largest entries when summary command runs` | integration |
| 7 | `should generate shell completions when completions command runs` | unit |
| 8 | `should exit with code 1 when scan path does not exist` | integration |

**Gates:** Gate 1 (cargo test + cargo test cli), Gate 2 (adversarial review diff).

---

### Phase 3: GUI Foundation (Tauri + egui)
**Deliverables:**
- Tauri v2 project scaffold in `gui/` (Rust backend + frontend)
- `gui/src-tauri/` — Tauri commands: `start_scan`, `get_results`, `apply_filter`, `delete_file`, `undo_delete`
- `gui/src/main.rs` — Tauri command handlers wrapping scan-engine + trash port
- `gui/frontend/` — React 18 + TypeScript + Vite scaffold
- `gui/frontend/src/App.tsx` — shell with sidebar (tree view) + main pane (treemap placeholder)
- egui WASM integration: `<canvas>` embedding for treemap/table views
- Keyboard shortcut handling (Delete → trash, Ctrl+Z → undo)

**Tests (TDD):**
| # | Test | Type |
|---|---|---|
| 1 | `should return scan results when start_scan Tauri command called` | integration |
| 2 | `should apply filter and return subset when apply_filter called` | integration |
| 3 | `should move file to trash when delete_file called` | integration |
| 4 | `should restore file when undo_delete called after delete` | integration |
| 5 | `should return error when delete_file called on nonexistent path` | integration |
| 6 | `should keep scan running in background when UI renders during scan` | integration |

**Gates:** Gate 1, Gate 2 (adversarial review), Gate 3 (Playwright e2e + vision model).

---

### Phase 4: Interactive Treemap + Views
**Deliverables:**
- `gui/frontend/src/components/Treemap.tsx` — egui treemap rendering (egui_extras::Tree)
- `gui/frontend/src/components/TableView.tsx` — sortable table (name, size, modified, type)
- `gui/frontend/src/components/FilterPanel.tsx` — size range, type, age, name pattern filters
- `gui/frontend/src/components/ContextMenu.tsx` — right-click: open in explorer, copy path, copy to clipboard
- `gui/frontend/src/components/Breadcrumb.tsx` — path navigation with backspace support
- Sort: click column headers toggles asc/desc (wire to domain SortSpec)
- Selection: arrow keys navigate, Enter drills into directory, Backspace goes up

**Tests (TDD):**
| # | Test | Type |
|---|---|---|
| 1 | `should render treemap rectangles proportional to file sizes when results loaded` | e2e |
| 2 | `should sort table by size descending when size column header clicked twice` | e2e |
| 3 | `should filter treemap to show only files over 10MB when size filter applied` | e2e |
| 4 | `should open context menu on right-click when treemap node selected` | e2e |
| 5 | `should navigate into subdirectory when Enter pressed on selected node` | e2e |
| 6 | `should navigate up when Backspace pressed while inside subdirectory` | e2e |
| 7 | `should show file count and total size in breadcrumb when directory selected` | e2e |

**Gates:** Gate 1, Gate 2, Gate 3 (visual + functional E2E).

---

### Phase 5: Real-time Sync + Incremental + Polish
**Deliverables:**
- Ably integration behind `sync` feature flag (optional compile)
- `scan-engine/src/sync.rs` — publish scan deltas via Ably channels
- Conflict resolution: last-write-wins with timestamp
- Incremental scan: file-system watcher triggers re-scan on changes
- CI/CD: GitHub Actions workflow (build + test + release on tag)
- Cross-platform packaging: .deb, .rpm, .AppImage (Linux), .dmg (macOS), .msi (Windows)

**Tests (TDD):**
| # | Test | Type |
|---|---|---|
| 1 | `should publish scan delta to Ably channel when file added` | integration |
| 2 | `should apply remote delta to local state when Ably message received` | integration |
| 3 | `should use last-write-wins when two devices modify same entry` | unit |
| 4 | `should re-scan only changed subtree when file watcher fires` | integration |
| 5 | `should build .deb package when CI runs on ubuntu` | ci |
| 6 | `should build .dmg when CI runs on macos` | ci |

**Gates:** Gate 1, Gate 2, Gate 3.

---

## 4. DESIGN DECISIONS

| Concern | Decision | Rationale |
|---|---|---|
| Domain deps | Zero | Hexagonal — domain is pure logic, testable without I/O |
| Scan parallelism | jwalk + rayon | jwalk handles dir iteration, rayon thread pool for CPU work |
| Cache | redb | Embedded, single-file, no server, cross-platform |
| Trash | `trash` crate | OS-native trash on Linux (gio), macOS (NSFileManager), Windows (SHFileOperation) |
| GUI framework | Tauri v2 + egui WASM | Small binary, native perf, egui for canvas-heavy treemap |
| Sync | Ably (optional) | Pub/sub, free tier sufficient, offline-first with feature flag |
| CLI parsing | clap derive | Type-safe, auto-generated help/completions |
| Error types | DomainError (domain) + AppError (per binary) | Domain stays pure; adapters add I/O variants |

---

## 5. RISK ASSESSMENT

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| jwalk performance on NFS/remote mounts | Medium | Medium | Detect mount type; fall back to sequential walk; document limitation |
| egui WASM treemap rendering perf at 100k+ nodes | Medium | High | Level-of-detail: aggregate small nodes into "N files, X MB"; virtualize |
| redb corruption on crash during write | Low | High | Write-ahead: write new entry, then atomically swap pointer; checksum |
| Ably free tier rate limits | Low | Low | Feature-flagged; local-first; batch deltas |
| Cross-platform trash API differences | Medium | Medium | `trash` crate handles this; test on all 3 platforms in CI |
| Tauri v2 + egui WASM integration complexity | Medium | Medium | Prototype in Phase 3; if blocking, fall back to pure egui native window |

---

## 6. COMPLIANCE CHECKLIST

- [x] Architecture — Hexagonal, domain at center, adapters at edges
- [x] Dependencies — Domain has ZERO external deps
- [x] Types — All public functions typed, no `any`/`Any`
- [x] TDD — Tests listed per phase, test-first approach
- [x] Karpathy — Explicit types, small functions, no cleverness, explicit errors
- [x] Ponytail — Rules in `/.agents/rules/`, portable
- [x] Commits — Conventional commits, one logical change per commit
- [x] TDD Cycle — Test → Implement → Refactor → Commit per cycle

---

## 7. GATE MAPPING

| Phase | Gate 0 (Plan) | Gate 1 (Tests) | Gate 2 (Review) | Gate 3 (E2E) |
|---|---|---|---|---|
| Phase 0: Domain | ✓ (this plan) | ✓ `cargo test` | — | — |
| Phase 1: Scan Engine | — | ✓ `cargo test` | ✓ diff review | — |
| Phase 2: CLI | — | ✓ `cargo test` | ✓ diff review | — |
| Phase 3: GUI Foundation | — | ✓ `cargo test` + vitest | ✓ diff review | ✓ Playwright + vision |
| Phase 4: Treemap + Views | — | ✓ `cargo test` + vitest | ✓ diff review | ✓ Playwright + vision |
| Phase 5: Sync + Polish | — | ✓ `cargo test` + vitest | ✓ diff review | ✓ Playwright + vision |

---

## 8. ESTIMATION

| Phase | Effort | Notes |
|---|---|---|
| Phase 0: Domain | ~0.5 unit | 80% done; ports + workspace remaining |
| Phase 1: Scan Engine | ~2 units | Core complexity: parallel walk, cache, incremental |
| Phase 2: CLI | ~1 unit | Thin adapter, formatters are mechanical |
| Phase 3: GUI Foundation | ~2 units | Tauri scaffold + egui WASM integration |
| Phase 4: Treemap + Views | ~2 units | Canvas rendering, interaction, keyboard nav |
| Phase 5: Sync + Polish | ~1.5 units | Ably, CI/CD, cross-platform packaging |
| Gates (all phases) | ~1 unit | Distributed across phases |
| **Total** | **~10 units** | |
