# DiskScope — Build Plan

## Current State

| Crate | Status |
|---|---|
| `scan-engine/domain/` | ✅ Complete — FileNode, FileTree, Filter, FileType, Size, SortKey, OutputFormat, ScanOpts, port traits (Scanner, Cache, Trash), error types, mocks, 14 unit tests |
| `scan-engine/scanner/` | ✅ Complete — parallel walker (ignore crate), RedbCache (redb), IncrementalScanner, 10 integration tests |
| `cli/` | ✅ Complete — clap CLI (scan, summary, completions), all output formats, filters, sorting, 10 integration tests |
| `gui/` | ❌ Stub (`println!("not yet implemented")`) |
| Trash adapter | ❌ Port trait defined, no real adapter |
| Ably sync | ❌ Not started |
| CI/CD + packaging | ❌ Not started |

## Architecture (Hexagonal)

```
┌─────────────────────────────────────────────────┐
│                  Domain (Pure)                   │
│  FileNode · FileTree · Filter · FileType · Size  │
│  SortKey · OutputFormat · ScanOpts               │
│  Error types · Port traits (Scanner, Cache, Trash)│
├──────────┬──────────┬──────────┬─────────────────┤
│  Scanner │  Cache   │  Trash   │    Sync         │
│ (ignore) │ (redb)   │(trash-rs)│   (ably-rs)     │
├──────────┴──────────┴──────────┴─────────────────┤
│            Adapters                               │
│   CLI (clap)  │  GUI (Tauri + egui + React)      │
└─────────────────────────────────────────────────┘
```

---

## Phase 0: Domain Core — Harden & Verify

**Status:** Already built. This phase verifies existing code meets the spec and closes gaps.

### Deliverables
- All existing domain unit tests pass (`cargo test -p scan-engine --lib`)
- `clippy::all` and `missing_docs` lints pass clean
- Port traits `Scanner`, `Cache`, `Trash` documented with contracts
- `Cargo.toml` workspace has `edition = "2021"` and correct license

### Tests (all must pass — already written)
| # | Test | Status |
|---|---|---|
| 1 | should create FileNode with valid path and size | ✅ |
| 2 | should reject FileNode with empty path | ✅ |
| 3 | should compute total_size recursively | ✅ |
| 4 | should filter by MinSize | ✅ |
| 5 | should filter by Extension | ✅ |
| 6 | should filter by MaxAge | ✅ |
| 7 | should filter by NamePattern | ✅ |
| 8 | should sort children by size descending | ✅ |
| 9 | should sort children by name ascending | ✅ |
| 10 | should format as JSON | ✅ |
| 11 | should format as table | ✅ |
| 12 | should display DomainError variants correctly | ✅ |
| 13 | should compose multiple filters | ✅ |
| 14 | should respect max_depth | ✅ |
| 15 | should format Size as human-readable string | ✅ |
| 16 | should classify file type by extension | ✅ |
| 17 | should return Other for unknown extension | ✅ |
| 18 | should calculate total_size in FileTree | ✅ |
| 19 | should count files recursively in FileTree | ✅ |
| 20 | should match file when size within range | ✅ |
| 21 | should reject file when size outside range | ✅ |
| 22 | should match file when name matches glob | ✅ |
| 23 | should combine multiple filters with AND logic | ✅ |
| 24 | should format as JSONL | ✅ |
| 25 | should format as tree | ✅ |

### Gates
- **Gate 1** — `cargo test -p scan-engine --lib` (25 tests)
- **Gate 2** — Adversarial review of domain contracts

---

## Phase 1: Scan Engine Adapters — Verify & Close Gaps

**Status:** Already built. Verifies walker, cache, incremental scanner.

### Deliverables
- Parallel walker respects .gitignore, follows symlinks option, max_depth
- RedbCache: get/put/invalidate cycle works with real redb on disk
- IncrementalScanner: cache hit on unchanged files, cache miss on modified
- All existing integration tests pass (`cargo test -p scan-engine`)

### Tests (all must pass — already written)
| # | Test |
|---|---|
| 1 | should scan directory and return correct file count |
| 2 | should scan directory and calculate total size |
| 3 | should respect max_depth option when depth limit is set |
| 4 | should respect .gitignore when ignore option is true |
| 5 | should apply size filter during scan when filter is provided |
| 6 | should use cache on second scan when cache is enabled |
| 7 | should return full tree on incremental scan when unchanged |
| 8 | should format output as JSON when format is Json |
| 9 | should format output as table when format is Table |
| 10 | should handle permission errors gracefully |

### Gates
- **Gate 1** — `cargo test -p scan-engine` (10 integration + 25 unit = 35 tests)
- **Gate 2** — Adversarial review of adapter correctness

---

## Phase 2: CLI — Verify & Close Gaps

**Status:** Already built. Verifies all commands and output formats.

### Deliverables
- `diskscope scan [path]` works with all format/sort/filter flags
- `diskscope summary <path>` prints quick stats
- `diskscope completions <shell>` generates valid completions
- Error handling: nonexistent path → exit 1 with message

### Tests (all must pass — already written)
| # | Test |
|---|---|
| 1 | should print help when invoked with --help |
| 2 | should scan current dir when no path given |
| 3 | should output JSON when --format json |
| 4 | should output table when --format table (default) |
| 5 | should output JSONL when --format jsonl |
| 6 | should output tree when --format tree |
| 7 | should filter by min-size when --min-size given |
| 8 | should sort by name-asc when --sort name-asc |
| 9 | should exit 1 when path does not exist |
| 10 | should generate bash completions when completions bash |

### Gates
- **Gate 1** — `cargo test -p diskscope-cli` (10 tests)
- **Gate 2** — Adversarial review of CLI ergonomics

---

## Phase 3: Trash Adapter (Real Implementation)

**Status:** Not started. Port trait exists (`domain::ports::Trash`).

### Deliverables
- `scan-engine/src/scanner/trash.rs` — real `Trash` implementation using `trash` crate
- `TrashTicket` populated with real `deleted_at` timestamp
- `undo` restores file from system trash
- MockTrash updated to verify ticket timestamps
- Wire into CLI: `diskscope delete <path>` and `diskscope undo <path>` subcommands
- Cargo.toml: add `trash = "4"` dependency to scan-engine

### New Tests
| # | Test |
|---|---|
| 1 | should move file to system trash when delete called |
| 2 | should return TrashTicket with path and timestamp when delete succeeds |
| 3 | should restore file from trash when undo called with valid ticket |
| 4 | should return TrashError when undo called with invalid ticket |
| 5 | should not permanently delete file when trash is used |
| 6 | should print deletion confirmation when CLI delete runs |
| 7 | should print undo confirmation when CLI undo runs |

### Gates
- **Gate 1** — `cargo test` (all crates, including new trash tests)
- **Gate 2** — Adversarial review of trash safety (permanent delete must be impossible)

---

## Phase 4: GUI Foundation (Tauri + egui Scaffold)

**Status:** Not started. GUI crate is a stub.

### Deliverables
- `gui/Cargo.toml`: add `tauri`, `eframe`, `egui`, `egui_extras` dependencies
- `gui/src/main.rs`: Tauri app shell with egui canvas inside webview
- `gui/src-tauri/tauri.conf.json`: Tauri v2 config (window size, title, bundle)
- `gui/package.json`: React 18 + TypeScript + Vite setup for chrome
- Tauri command `scan(path: String)` → calls `scan_engine::Scanner::new().scan()`
- Tauri command `get_tree()` → returns last scan result as JSON
- Basic window: title bar shows "DiskScope", egui canvas renders placeholder

### New Tests
| # | Test |
|---|---|
| 1 | should compile Tauri app when cargo build runs |
| 2 | should invoke scan command when scan called via IPC |
| 3 | should return scan results as JSON when get_tree called |
| 4 | should display egui window when app launches |

### Gates
- **Gate 1** — `cargo test -p diskscope-gui` (compilation + IPC tests)
- **Gate 2** — Adversarial review of Tauri/egui integration
- **Gate 3** — Visual: window renders, title visible

---

## Phase 5: GUI Interactive (Treemap + Tree/Table)

**Status:** Depends on Phase 4.

### Deliverables
- **Treemap view**: `egui_extras` treemap rendering scan results (size proportional)
- **Tree/table view**: `egui_extras` table with sortable columns (name, size, modified, type)
- **View toggle**: button/shortcut to switch treemap ↔ table
- **Navigation**: click treemap cell or table row to drill into subdirectory
- **Breadcrumb**: path bar showing current location, click to navigate up
- **Progress indicator**: egui spinner/progress bar during scan
- Tauri command `scan_with_progress(path)` → emits progress events to frontend
- `FilterPanel` component: size range sliders, file type checkboxes, name pattern input

### New Tests
| # | Test |
|---|---|
| 1 | should render treemap with correct proportions when data present |
| 2 | should render table with sortable columns when data present |
| 3 | should drill into subdirectory when treemap cell clicked |
| 4 | should navigate up when breadcrumb segment clicked |
| 5 | should show progress bar when scan in progress |
| 6 | should filter results when filter panel values change |
| 7 | should toggle view when view toggle button clicked |

### Gates
- **Gate 1** — `cargo test -p diskscope-gui` + `npm test` (frontend)
- **Gate 2** — Adversarial review of UI correctness
- **Gate 3** — Visual: treemap renders with colored blocks, table shows columns

---

## Phase 6: GUI Polish (Keyboard, Context Menus, Delete/Undo)

**Status:** Depends on Phase 5.

### Deliverables
- **Keyboard shortcuts**:
  - Arrow keys: navigate treemap/table cells
  - Enter: drill into selected item
  - Backspace: navigate up
  - Delete: move selected file to trash (confirmation dialog)
  - `Cmd/Ctrl+Z`: undo last trash operation
- **Context menu** (right-click):
  - "Open in file explorer" (`xdg-open` / `open` / `explorer`)
  - "Copy path" (clipboard)
  - "Copy size" (clipboard)
  - "Delete" (move to trash)
- **Undo stack**: in-memory stack of `TrashTicket`s, `Cmd/Ctrl+Z` pops and calls undo
- **Status bar**: current path, total size, file count, selected item info

### New Tests
| # | Test |
|---|---|
| 1 | should navigate treemap when arrow keys pressed |
| 2 | should drill into item when Enter pressed |
| 3 | should navigate up when Backspace pressed |
| 4 | should move to trash when Delete pressed and confirmed |
| 5 | should undo last delete when Cmd/Ctrl+Z pressed |
| 6 | should open file explorer when context menu "Open" clicked |
| 7 | should copy path when context menu "Copy path" clicked |
| 8 | should show status bar with current path and stats |

### Gates
- **Gate 1** — `cargo test -p diskscope-gui` + `npm test`
- **Gate 2** — Adversarial review of keyboard/mouse interaction safety
- **Gate 3** — Functional: keyboard navigation works in Playwright E2E

---

## Phase 7: Ably Sync Integration

**Status:** Not started. Optional feature behind `sync` feature flag.

### Deliverables
- `scan-engine/Cargo.toml`: add `ably = "1"` behind `sync` feature
- `scan-engine/src/sync.rs`: Ably client wrapper
  - `SyncPort` trait: `publish_scan(result)`, `subscribe_updates(callback)`
  - `AblySync` adapter: real Ably implementation
  - `MockSync`: for testing
- `gui/src-tauri/src/sync.rs`: Tauri command `start_sync(api_key)`
- Conflict resolution: last-write-wins with timestamp
- Offline-first: scan works without network, syncs when connected

### New Tests
| # | Test |
|---|---|
| 1 | should publish scan result when sync enabled |
| 2 | should receive scan update when remote device publishes |
| 3 | should resolve conflict with last-write-wins when concurrent edits |
| 4 | should work offline when no network available |
| 5 | should queue updates when offline and send when reconnected |

### Gates
- **Gate 1** — `cargo test --features sync` (all crates + sync tests)
- **Gate 2** — Adversarial review of sync safety (no data loss, offline-first)
- **Gate 3** — Functional: two instances sync scan results

---

## Phase 8: Cross-Platform Packaging

**Status:** Not started.

### Deliverables
- `.github/workflows/release.yml`: GitHub Actions release pipeline
- **Linux**: AppImage, .deb, .rpm, .tar.gz (via `cargo-appimage` or `tauri-bundler`)
- **macOS**: universal binary (.dmg), notarized (via `tauri-bundler` + Apple Developer ID)
- **Windows**: MSI installer, portable .exe (via `tauri-bundler` + WiX)
- Auto-update: Tauri updater configured in `tauri.conf.json`
- Code signing: macOS Developer ID + Windows EV cert (secrets in GitHub Actions)

### Tests
| # | Test |
|---|---|
| 1 | should build AppImage when CI runs on Linux |
| 2 | should build DMG when CI runs on macOS |
| 3 | should build MSI when CI runs on Windows |
| 4 | should notarize DMG when Apple credentials present |
| 5 | should check for updates when app launches |

### Gates
- **Gate 1** — CI pipeline runs green on all 3 platforms
- **Gate 2** — Adversarial review of build reproducibility

---

## Phase 9: CI/CD Pipeline

**Status:** Not started.

### Deliverables
- `.github/workflows/ci.yml`: runs on every PR
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo fmt --check`
  - `npm test` (frontend)
  - `npm run build` (frontend build)
- `.github/workflows/release.yml`: runs on tag push
  - Builds all platform artifacts
  - Creates GitHub Release with artifacts
- `.github/workflows/review.yml`: Gate 2 integration
  - Runs adversarial review on PR diff
  - Posts review as PR comment

### Tests
| # | Test |
|---|---|
| 1 | should run cargo test when PR opened |
| 2 | should run clippy when PR opened |
| 3 | should run npm test when PR opened |
| 4 | should create release when tag pushed |
| 5 | should post review comment when PR opened |

### Gates
- **Gate 1** — Pipeline runs green on test PR
- **Gate 2** — Adversarial review of CI config

---

## Phase 10: Documentation & Launch

**Status:** Not started.

### Deliverables
- `README.md`: real app description (what it does, who it's for, how to run, stack)
- `CONTRIBUTING.md`: development setup, test instructions, PR guidelines
- `CHANGELOG.md`: release notes for v0.1.0
- GitHub repository: description, topics, website link
- First release: v0.1.0 tag with all platform artifacts

### Tests
| # | Test |
|---|---|
| 1 | should have accurate README when reviewed |
| 2 | should have working development setup when CONTRIBUTING followed |

### Gates
- **Gate 0** — Plan review (this document)
- **Gate 1** — All tests green
- **Gate 2** — Adversarial review of documentation accuracy

---

## Summary: Gate Matrix

| Phase | Gate 0 (Plan) | Gate 1 (Tests) | Gate 2 (Review) | Gate 3 (Visual/E2E) |
|---|---|---|---|---|
| 0: Domain Core | ✅ | 25 unit tests | Domain contracts | — |
| 1: Scan Adapters | ✅ | 35 tests (unit+int) | Adapter correctness | — |
| 2: CLI | ✅ | 10 integration tests | CLI ergonomics | — |
| 3: Trash Adapter | ✅ | 7 new tests | Trash safety | — |
| 4: GUI Foundation | ✅ | 4 new tests | Tauri/egui integration | Window renders |
| 5: GUI Interactive | ✅ | 7 new tests | UI correctness | Treemap + table |
| 6: GUI Polish | ✅ | 8 new tests | Keyboard/mouse safety | Keyboard E2E |
| 7: Ably Sync | ✅ | 5 new tests | Sync safety | Two-instance sync |
| 8: Packaging | ✅ | 5 new tests | Build reproducibility | — |
| 9: CI/CD | ✅ | 5 new tests | CI config | — |
| 10: Launch | ✅ | 2 new tests | Docs accuracy | — |

**Total new tests: ~43** (phases 3–10) + existing 35 tests = **~78 tests**

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Tauri + egui WASM integration complexity | High | Medium | Prototype Phase 4 early, verify egui renders in webview before building UI |
| `trash` crate cross-platform behavior | Medium | Medium | Test on all 3 platforms in CI; document platform-specific quirks |
| Ably sync offline-first edge cases | Medium | High | Feature-gate behind `sync` flag; default to offline mode |
| macOS code signing + notarization | Medium | High | Use Tauri's built-in bundler; document manual signing steps |
| egui treemap performance with 100k+ nodes | Medium | Medium | Implement level-of-detail rendering; only show top N by size |
| Windows MSI build complexity | Low | Medium | Use Tauri's WiX integration; test in CI early |

---

## Decision Log

| Date | Decision | Rationale |
|---|---|---|
| 2026-08-17 | Keep existing domain/scanner/CLI code | All 35 tests pass, code is clean, no refactoring needed |
| 2026-08-17 | Use `trash` crate for trash operations | Cross-platform, well-maintained, MIT license |
| 2026-08-17 | Feature-gate Ably sync | Not core functionality; offline-first is a hard requirement |
| 2026-08-17 | egui WASM inside Tauri webview | Required by spec; egui_extras treemap is the only viable OSS option |

---

## Commit Strategy

Each phase = one PR. Within a phase:
- One TDD cycle = one commit (`feat(scope): description` or `test(scope): description`)
- Each commit must pass `cargo test` and `cargo clippy`
- No giant blobs; history reads as logical sequence

---

## Next Step

**Phase 0–2 are already built.** Start at **Phase 3 (Trash Adapter)** — implement real trash operations and wire into CLI.
