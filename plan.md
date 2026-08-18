# DiskScope — Implementation Plan

> **Status:** Awaiting Gate 0 (critic) approval. **No code may be written** until this
> plan passes. The `domain/` crate is partially scaffolded (entities + 3 ports + tests)
> and is completed by Phase 1; everything else is greenfield.
>
> Plan-phased build loop: each `Phase N` here is independently buildable, testable,
> and adversarially reviewable — the pipeline (`pipeline.sh`) extracts these
> headings and runs Gate 1 (tests) + Gate 2 (cross-model critic) per phase before
> moving on.

---

## 1. Goal & Scope

**Goal.** Ship a free, open-source, cross-platform disk-space analyzer whose MVP
behavior is: scan a directory fast, present an interactive treemap + tree/table
view with filters and keyboard shortcuts, and safely delete to the system trash
(undoable). MVP performance budget: 100k files in <2 s on modern hardware.

**MVP acceptance covered** (from `requirements.md`):
- Parallel scan + treemap + tree/table + filters (size / type / age / pattern)
- Safe delete (trash + undo), context menu, keyboard shortcuts
- Cross-platform binary (Linux/macOS/Windows); single binary where possible
- Incremental scan; UI responsive during scan; <200 MB peak RSS

**Deferred** (each is its own phase or post-MVP):
- Ably real-time sync across devices → Phase 6, behind `sync` feature flag
- Network drives, cloud storage, scheduled scans, team workspaces, hooks → post-MVP
- Cross-platform packaging matrix → Phase 7 (release/CI only; core stays platform-portable)

---

## 2. Architecture (Hexagonal)

```mermaid
flowchart LR
  subgraph ADAPTERS
    CLI[cli crate\nclap]
    Tauri[gui crate (Rust)\nTauri commands]
    React[gui/web\nReact 18 + TS + Vite]
    Egui[egui WASM\ntreemap/table in canvas]
  end
  subgraph PORTS
    P1[Scanner]
    P2[Trash]
    P3[Cache]
    P4[Watcher]
  end
  subgraph DOMAIN[domain crate — zero external deps]
    D1[FileNode / FileType / Filter / SortSpec / ScanResult]
    D2[DomainError]
  end
  subgraph ENGINE[scan-engine crate]
    E1[JwalkScanner\nrayon + jwalk]
    E2[TrashAdapter\ntrash crate]
    E3[RedbCache\nredb]
    E4[NotifyWatcher\nnotify crate]
    E5[Formatters\njson|jsonl|table|tree]
  end

  CLI --> P1 & P2 & P3
  Tauri --> P1 & P2 & P3 & P4
  React <-->|commands/events| Tauri
  React --> Egui
  P1 -.implemented by.-> E1
  P2 -.implemented by.-> E2
  P3 -.implemented by.-> E3
  P4 -.implemented by.-> E4
  E5 --> P1
  E1 --> D1
  E2 --> D2
```

**Rules.**
- `domain/` depends on **nothing** (already true; preserve). All `pub` items carry
  doc comments; `#![deny(clippy::all)]`, `#![deny(missing_docs)]`, no `unwrap()`
  in non-test code (`GUIDELINES.md` §2.2 / §3.4).
- Adapters (`scan-engine`, `cli`, `gui`) consume the domain **only via the
  ports** (`Scanner`, `Trash`, `Cache`, `Watcher`). No adapter reaches around the
  port into concrete types.
- Workspace root `Cargo.toml` declares 3 members: `domain`, `scan-engine`,
  `cli`, `gui`. Edition 2021, MSRV pinned in `[workspace.package]`.

---

## 3. Module Map

| Module                    | Responsibility                                            | Depends on           |
|---------------------------|-----------------------------------------------------------|----------------------|
| `domain/`                 | Entities, value objects, ports, error type, formatters    | — (zero extern)      |
| `scan-engine/`            | Adapter impls of ports + output formatters + benchmarks   | `domain`             |
| `cli/`                    | `clap` binary; thin adapter over ports                    | `domain`, `scan-engine` |
| `gui/src/` (Rust)         | Tauri commands, IPC handlers, tray, settings              | `domain`, `scan-engine` |
| `gui/web/` (TS)           | React chrome, design tokens, egui canvas, state           | — (frontend only)    |
| `tests/` (existing)       | Gate scripts; new: `tests/perf_scan.rs`, `tests/e2e/`     | —                    |

---

## 4. Phased Plan

> **Conventions used by `pipeline.sh`'s extractor.** Each phase begins with a
> `## Phase N: Title` heading. Deliverable lines start `- `; gate bullets start
> `- **Gate…**` and are filtered by the extractor. TDD test lines live in a
> `### Tests (TDD)` subsection.

---

## Phase 1: Domain Core

### Deliverables
- `domain/Cargo.toml` — adds `#![deny(clippy::all)]`, `#![deny(missing_docs)]`,
  `[lib]` section; pinned MSRV.
- `domain/src/lib.rs` — keeps `FileType`, `FileNode`, `Filter`, `SortSpec`,
  `ScanResult`, `format_size` (already present); **adds**:
  - `DomainError` variants: `Io(String)`, `Trash(String)`, `Cache(String)`,
    `NotFound(String)`. Existing 3 variants retained.
  - `Filter` adds `old_than_seconds: Option<u64>` (age filter from MVP spec).
  - `Filter::matches_age(modified_unix: u64, now_unix: u64) -> bool`.
  - `Filter::apply_filter` extended to call `matches_age` so the existing depth
    test stays green and a new age test passes.
  - `ScanResult::largest_files(n: usize) -> Vec<&FileNode>` (top-N helper for UI).
  - `pub use ports::*;` re-export at crate root.
- `domain/src/ports.rs` — keeps `Scanner`, `Trash`, `Cache` traits (already
  present); **adds**:
  - `pub trait Watcher { fn watch(&self, path: &str) -> Result<WatchHandle, DomainError>; }`
  - `pub struct WatchHandle { /* opaque receiver */ }` (concrete shape decided
    in Phase 6; here only the type slot + trait signature exist).
- `domain/tests/integration.rs` — exercises a fake `Scanner` + `Trash` + `Cache`
  end-to-end through one application flow (scan → filter → move-to-trash →
  invalidate cache) using only the port traits and domain types.

### Tests (TDD)
- `should classify age_filter as pass when file_modified_older_than_threshold`
- `should classify age_filter as fail when file_modified_newer_than_threshold`
- `should return top_n_files_sorted_by_size_desc when scan_result_largest_files_called`
- `should return Io_error when domain_error_constructed_for_io`
- `should return Watcher_required_when_trait_declared_with_watch_method`
- `should complete scan_filter_trash_invalidate_flow when driven_through_ports`

### Gates Satisfied
- **Gate 1** — `cargo test -p domain` green; clippy clean.
- **Gate 2** — Cross-model critic checks: domain has zero extern deps; ports
  are minimal; no `unwrap` in non-test code; new variants are non-breaking.

---

## Phase 2: Scan Engine (`scan-engine` crate)

### Deliverables
- Workspace root `Cargo.toml` (new) — declares members `["domain", "scan-engine",
  "cli", "gui"]`; `[workspace.package]` with version `0.1.0`, edition `2021`,
  rust-version `1.75`, license `MIT OR Apache-2.0`.
- `scan-engine/Cargo.toml` — deps: `domain` (path), `rayon`, `jwalk`, `ignore`,
  `trash`, `redb`, `serde`, `serde_json`, `thiserror`. Feature flag
  `watch = ["dep:notify"]` for Phase 6.
- `scan-engine/src/jwalk_scanner.rs` — implements `Scanner`. Walks with
  `jwalk::WalkDir` (parallel), respects `.gitignore` via `ignore::WalkBuilder`
  fallback, classifies via `FileType::from_extension`, aggregates sizes, reports
  `scan_duration_ms` from `Instant`. Honors `Filter` (size, type, age, pattern,
  depth). Returns `ScanResult`.
- `scan-engine/src/redb_cache.rs` — implements `Cache`. Key: `&str` path;
  value: serialized `ScanResult` (bincode via `redb`). Tables: `scans`, `meta`.
  `put` stores + fsync; `get` returns `Option`; `invalidate` removes key.
- `scan-engine/src/trash_adapter.rs` — implements `Trash` using `trash` crate
  (cross-platform). `undo_last` records path + prior parent, restores on
  supported platforms (best-effort; returns `DomainError::Trash` on
  unsupported restore).
- `scan-engine/src/formatters.rs` — `enum OutputFormat { Json, Jsonl, Table,
  Tree }`; `fn render(&ScanResult, OutputFormat, &mut dyn Write) -> Result<...>`.
  Table is ASCII-aligned, tree is unicode box-drawing, JSON/JSONL via serde_json.
- `scan-engine/src/lib.rs` — re-exports `JwalkScanner`, `RedbCache`,
  `TrashAdapter`, `formatters`.
- `scan-engine/tests/perf_scan.rs` — gated benchmark: synthesize a temp tree of
  N files (default 100k via env `DISKSCOPE_PERF_N=100000`), assert
  `scan_duration_ms < 2000`; allowed to be skipped under
  `DISKSCOPE_SKIP_PERF=1`.
- `scan-engine/tests/integration.rs` — scan a temp dir, assert counts/sizes;
  filter; move-to-trash; cache hit on second scan is faster.

### Tests (TDD)
- `should scan_recursive_tree_when_jwalk_scanner_called_on_temp_dir`
- `should classify_file_types_by_extension_when_scanner_walks_tree`
- `should return DomainError_InvalidPath when scanner_called_with_nonexistent_root`
- `should apply_size_filter_and_prune_subtree_when_filter_min_size_set`
- `should skip_gitignored_paths_when_walking_repo_with_gitignore`
- `should return cached_scan_result_when_cache_get_called_with_known_path`
- `should fsync_cache_writes_when_redb_put_called`
- `should move_file_to_trash_and_record_path_when_trash_adapter_called`
- `should restore_last_trashed_file_when_trash_adapter_undo_last_called`
- `should render_valid_json_when_formatter_called_with_json_format`
- `should render_one_record_per_line_when_formatter_called_with_jsonl_format`
- `should render_box_tree_when_formatter_called_with_tree_format`
- `should finish_under_2_seconds_when_scan_called_on_100k_files`

### Gates Satisfied
- **Gate 1** — `cargo test --workspace` green; perf test passes on builder's
  hardware (or is explicitly skipped with logged reason).
- **Gate 2** — Critic checks: parallel walk is actually parallel (no accidental
  `par_bridge` removed); redb fsync happens before `put` returns; trash restore
  is best-effort and reports unsupported clearly.

---

## Phase 3: CLI Binary (`cli` crate)

### Deliverables
- `cli/Cargo.toml` — deps: `domain`, `scan-engine`, `clap` (derive),
  `anyhow` (binary top-level only), `tracing` (subsystem=cli).
- `cli/src/main.rs` — `diskscope` binary; `clap` subcommands:
  - `scan [PATH] --format <json|jsonl|table|tree> --filter-size-min <B>
    --filter-size-max <B> --filter-type <csv> --filter-pattern <glob>
    --filter-older-than <seconds> --max-depth <n> --no-cache`
  - `summary PATH` — runs scan, prints one-line totals.
  - `completions <bash|zsh|fish|powershell>` — generates via `clap_complete`.
- `cli/src/commands.rs` — wires clap args → `Filter` → `Scanner::scan` →
  formatter → stdout; `--no-cache` skips `Cache::put`; errors print to stderr
  with `DomainError` `Display`.
- `cli/tests/cli.rs` — integration tests via `assert_cmd` against temp dirs.

### Tests (TDD)
- `should print_table_to_stdout_when_scan_invoked_with_format_table`
- `should print_jsonl_one_record_per_line_when_format_jsonl`
- `should print_summary_line_when_summary_subcommand_run`
- `should exit_nonzero_when_scan_path_does_not_exist`
- `should pipe_filter_size_min_into_filter_when_flag_provided`
- `should generate_bash_completion_when_completions_bash_subcommand_run`
- `should hit_cache_when_same_path_scanned_twice` (verifies `--no-cache` off)

### Gates Satisfied
- **Gate 1** — `cargo test --workspace` green.
- **Gate 2** — Critic checks: no domain logic in CLI; clap derives are used (no
  hand-rolled parsing); errors map cleanly from `DomainError`; no silent
  fallbacks.

---

## Phase 4: GUI Backend (Tauri Rust commands)

### Deliverables
- `gui/Cargo.toml` — replaces placeholder; deps: `domain`, `scan-engine`,
  `tauri = { version = "2", features = ["tray-icon"] }`, `serde`, `serde_json`,
  `tokio` (for command runtime), `tracing`. `[[bin]]` `diskscope-gui`.
- `gui/tauri.conf.json` — window dimensions, identifier, `frontendDist`
  pointing at `gui/web/dist`, dev server URL.
- `gui/src/commands.rs` — Tauri commands:
  - `scan(path: String, filter: Filter, use_cache: bool) -> Result<ScanResult, AppError>`
  - `move_to_trash(paths: Vec<String>) -> Result<Vec<String>, AppError>`
  - `undo_last_delete() -> Result<(), AppError>`
  - `subscribe(path: String, sink: EventSink) -> Result<WatchHandle, AppError>`
- `gui/src/state.rs` — `AppState { scanner: Arc<dyn Scanner>, trash: Arc<dyn Trash>,
  cache: Arc<dyn Cache> }`; `FromRef<AppState>` for command injection.
- `gui/src/error.rs` — `AppError` enum (typed, `serde::Serialize`,
  `thiserror`); maps `DomainError` → `AppError` (1:1 by variant).
- `gui/src/main.rs` — builds Tauri app, registers commands, sets up tray
  (`Quit`, `Open window`); loads scan-engine adapters behind `Arc<dyn Port>`.
- `gui/build.rs` — `tauri_build::build()`.

### Tests (TDD)
- `should return_ScanResult_when_scan_command_invoked_with_valid_path`
- `should return_AppError_InvalidPath_when_scan_command_invoked_with_missing_path`
- `should move_all_paths_when_move_to_trash_command_invoked_with_paths`
- `should restore_last_delete_when_undo_last_delete_command_invoked`
- `should serialize_AppError_to_json_when_returned_from_command`
- `should not_block_ui_thread_when_scan_command_invoked` (uses Tokio runtime)
- `should map_every_DomainError_variant_to_AppError_when_converted`

### Gates Satisfied
- **Gate 1** — `cargo test --workspace` green; Tauri command handlers compile
  for the host target.
- **Gate 2** — Critic checks: no `unwrap` in command bodies; `Arc<dyn Port>`
  is the only access path to scan-engine; long scans do not block the main
  thread (documented via `tokio::task::spawn_blocking`).

---

## Phase 5: GUI Frontend (React + egui-in-canvas)

### Deliverables
- `gui/web/package.json` — `react@18`, `react-dom@18`, `typescript@5.5`,
  `vite@6`, `@tauri-apps/api@2`, `zustand` (state), `vitest`, `@testing-library/react`.
- `gui/web/vite.config.ts` — React + Vite + Tauri-aware dev URL.
- `gui/web/tsconfig.json` — `strict: true`, `noImplicitAny: true`,
  `strictNullChecks: true`, `noUncheckedIndexedAccess: true`.
- `gui/web/src/design/tokens.ts` — re-exports `design-system/tokens.json` as
  typed TS module; every screen consumes only token values.
- `gui/web/src/components/`:
  - `<AppShell/>` — toolbar (path picker, filter panel toggle, trash button).
  - `<Treemap/>` — wraps `<canvas>` that hosts egui WASM via `egui_extras::install`;
    receives `ScanResult` from React state, dispatches click → row select.
  - `<TreeTable/>` — virtualized table; sortable columns (name, size, modified,
    type) wired to `SortSpec`.
  - `<FilterPanel/>` — size min/max, type checkboxes (audio/video/image/doc/
    code/archive), age slider, name pattern input — all map to `Filter` struct.
  - `<ContextMenu/>` — open-in-explorer, copy-path, copy-to-clipboard, undo.
- `gui/web/src/hooks/useKeyboard.ts` — `Delete` → `moveToTrash`, `Cmd/Ctrl+Z` →
  `undoLast`, `↑/↓/←/→/Enter/Backspace` for navigation; uses `useEffect` keydown.
- `gui/web/src/store/scan.ts` — zustand store: `result`, `selectedPath`,
  `filter`, `sortSpec`, `pendingOps`.
- `gui/web/tests/` (vitest) — `AppShell.test.tsx`, `FilterPanel.test.tsx`,
  `TreeTable.test.tsx`, `useKeyboard.test.ts`.

### Tests (TDD)
- `should render_token_primary_color_when_treemap_paints_root_node`
- `should sort_table_descending_by_size_when_column_header_clicked`
- `should apply_filter_to_result_when_filter_panel_value_changed`
- `should dispatch_move_to_trash_when_delete_key_pressed_with_selection`
- `should dispatch_undo_when_ctrl_z_key_pressed`
- `should copy_path_to_clipboard_when_context_menu_copy_path_clicked`
- `should open_native_file_manager_when_open_in_explorer_clicked`
- `should render_no_token_violations_when_css_audited`

### Gates Satisfied
- **Gate 1** — vitest green; `tsc --noEmit` clean; `eslint` clean.
- **Gate 2** — Critic checks: no hard-coded colors/sizes (grep audit of
  `gui/web/src`); state stores typed (no `any`); IPC calls go only through the
  typed Tauri command surface.
- **Gate 3** — Playwright drives the Tauri app; vision model audits the treemap
  + table for visual correctness; functional E2E covers scan → filter → delete
  → undo.

---

## Phase 6: Live Updates + Ably Sync (feature-flagged)

### Deliverables
- `scan-engine/src/notify_watcher.rs` — implements `Watcher` from Phase 1 using
  `notify` crate; emits `WatchEvent { kind: Created|Modified|Deleted|Renamed,
  path: String }` via a `mpsc::Receiver`.
- `scan-engine/src/lib.rs` — re-export `NotifyWatcher` under feature `watch`.
- `gui/src/commands.rs` — `subscribe` command spawns a watcher and forwards
  events to the frontend as Tauri window events (`scan://changed`).
- Optional `sync` feature:
  - `gui/src/sync/ably.rs` — `AblyClient` wrapping `ably` crate; subscribes to
    per-user channel `diskscope:{user_id}:{device_id}`; publishes diffs of
    `ScanResult` keyed by `(path, mtime, size)` with last-write-wins timestamp.
  - Reconnect with exponential backoff; offline queue persisted to a tiny
    `redb` table; flush on reconnect. No telemetry without consent — gated by
    user toggle in settings.
- `gui/web/src/store/sync.ts` — surfaces online/offline indicator; merges
  remote `ScanResult` into local store with last-write-wins.

### Tests (TDD)
- `should emit_created_event_when_watcher_detects_new_file`
- `should debounce_rapid_modifications_when_watcher_fires_burst`
- `should resolve_conflict_by_newer_timestamp_when_remote_and_local_diverge`
- `should queue_changes_offline_when_network_down_and_flush_on_reconnect`
- `should opt_out_when_sync_disabled_in_settings`
- `should not_send_any_payload_when_sync_feature_disabled_at_compile_time`

### Gates Satisfied
- **Gate 1** — `cargo test --workspace --features watch,sync` green; without
  features, `cargo test --workspace` still green (zero-cost when off).
- **Gate 2** — Critic checks: feature gate truly compiles out (`#[cfg]`); no
  network calls in default build; consent flow is mandatory before connect.

---

## Phase 7: Cross-Platform Packaging & E2E

### Deliverables
- `.github/workflows/release.yml` — matrix `os: [ubuntu-latest, macos-latest,
  windows-latest]`; `tauri-action` builds `.AppImage`, `.deb`, `.rpm` (Linux),
  `.dmg` (macOS), `.msi` + portable `.exe` (Windows).
- `gui/tauri.conf.json` — bundle config: `appimage`, `deb`, `rpm`, `dmg`,
  `msi`, `nsis`. macOS `minimumSystemVersion: 10.15`. Single binary mode via
  `bundle.targets`.
- Code-signing secrets wired: `APPLE_CERT`, `WINDOWS_CERT`; notarization via
  `tauri-action` `signingIdentity`. Skipped on forks (documented).
- Auto-update config (`tauri-plugin-updater`) reading releases from GitHub.
- `tests/e2e/` — Playwright suite:
  - `scan_flow.spec.ts` — open app → see treemap → click node → row in table.
  - `delete_undo.spec.ts` — select file → `Delete` → confirm toast → `Ctrl+Z`
    restores.
  - `filter.spec.ts` — apply size filter → result shrinks accordingly.
- `tests/visual_gate.sh` (already present) extended to launch packaged build
  in headless mode and capture treemap screenshot.

### Tests (TDD)
- `should produce_appimage_when_release_workflow_runs_on_ubuntu`
- `should produce_dmg_when_release_workflow_runs_on_macos`
- `should produce_msi_when_release_workflow_runs_on_windows`
- `should not_require_electron_when_binary_inspected` (regression guard)
- `should pass_playwright_scan_flow_when_run_against_packaged_app`
- `should pass_playwright_delete_undo_when_run_against_packaged_app`
- `should pass_vision_review_when_treemap_screenshot_captured`

### Gates Satisfied
- **Gate 1** — `cargo test --workspace` + `npm --prefix gui/web test` green.
- **Gate 2** — Critic checks: secrets not hard-coded; bundle config committed;
  release workflow is idempotent.
- **Gate 3** — Visual + functional E2E pass on the host (Linux in dev; macOS
  / Windows artifacts produced by CI but visually verified on Linux only —
  cross-platform vision audit is out of MVP scope, tracked post-MVP).

---

## 5. Risk Register

| Risk                                                               | L   | I   | Mitigation                                                                                          |
|--------------------------------------------------------------------|-----|-----|-----------------------------------------------------------------------------------------------------|
| Builder ≡ critic model produces lenient Gate 2 (PIPELINE_AUDIT §A1) | M   | M   | `tests/review_gate.sh` reads `CRITIC_MODEL` from env; `CONFIG.md` validates critic ≠ builder AND that the resolved model is non-empty + in the endpoint's supported-model allowlist before any provider call.            |
| `CRITIC_MODEL` leaks as the literal string `${CRITIC_MODEL}` into the provider payload → API returns `code:unsupported_model` → Gate 2 aborts with a cryptic provider error | M   | H   | `tests/review_gate.sh` MUST resolve `CRITIC_MODEL` exactly once at the top via `${PIPELINE_CRITIC_MODEL:-${CRITIC_MODEL:-<known-good-default>}}`, then MUST refuse to run if the resolved value is empty OR matches the regex `^\$\{[A-Z_]+\}$` (a literal unset interpolation). The resolved value MUST be checked against the model allowlist in `CONFIG.md` BEFORE any JSON is built; on miss, the script MUST exit non-zero with the actionable message `CRITIC_MODEL unsupported on this endpoint — set PIPELINE_CRITIC_MODEL to one of: <allowlist>`. Every substitution site that puts the model into a JSON payload (the `node` argv on the request-builder line AND any future heredoc interpolations) MUST use the already-resolved shell variable, never `${CRITIC_MODEL}` or `${PIPELINE_CRITIC_MODEL}` directly. The assembled request body MUST be grepped for the pattern `\$\{[A-Z_]+\}` before `curl` sends it — any match is a hard fail with the message `literal ${VAR} leaked into JSON payload — fix substitution site`. |
| Reasoning model returns empty on tight `max_tokens`               | M   | H   | Gate scripts set `max_tokens >= 4000`; first-JSON parser handles concatenated output.               |
| `redb` native dep breaks `cargo test` on contributor machines     | L   | M   | Tied to redb 2.x wheels; CI matrix covers it; `tests/gate.sh` already runs `cargo test` everywhere. |
| `trash` crate restore unsupported on Windows for some filesystems  | M   | L   | Documented in CLI help; `undo_last` returns `DomainError::Trash` with clear message.                |
| Tauri v2 + egui WASM in same binary is an unusual combo            | M   | M   | Phase 5 starts with a build-spike commit before full UI work.                                       |
| Ably requires online + API key; conflicts with offline-first       | M   | H   | Entire sync path is behind `sync` feature flag; defaults off; opt-in only with explicit consent.     |
| Performance target (<2 s / 100k) not met on slow disks             | L   | H   | Phase 2 perf test is gating; `DISKSCOPE_SKIP_PERF=1` allowed but logged; revisit before MVP sign-off. |
| `.gitignore` lets a `*.db` slip into a commit                     | L   | M   | `tests/review_gate.sh` checks `git status` before commit; `.gitignore` already excludes `*.db`.      |
| `extract_phases` regex misses a phase heading                      | L   | H   | All headings use `## Phase N: Title` (single space); verified against the extractor.               |
| `MAX_SUBCHUNKS=5` batches too aggressively                          | L   | M   | Each phase's deliverables list stays short (~6–12 bullets), well under the cap.                      |
| Docker pre-flight unavailable → native deps missing                | M   | M   | Already non-fatal; documented in `pipeline.sh`. WebKit/GTK devs install system libs via package mgr. |

---

## 6. Compliance Checklist (Gate 0 reviewer)

- [ ] **Architecture** — Hexagonal: domain at center, adapters (scan-engine,
      cli, gui) at edges, ports in `domain/ports.rs`.
- [ ] **Domain deps** — `domain/Cargo.toml` has zero extern deps (preserved).
- [ ] **Type safety** — `#![deny(clippy::all)]`, `#![deny(missing_docs)]`,
      no `unwrap()` in non-test code; TS `strict: true` in `gui/web`.
- [ ] **TDD** — Every phase lists its tests in `should <behavior> when
      <condition>` form, in build order.
- [ ] **Commits** — Conventional commits; one logical change per commit
      (per phase's TDD cycle).
- [ ] **Gates** — Each phase declares Gates Satisfied; Gate 1 (tests) on every
      phase; Gate 2 (cross-model critic) on every phase; Gate 3 (visual/E2E)
      on Phases 5 and 7.
- [ ] **Design tokens** — All `gui/web` components consume only
      `design-system/tokens.json` values.
- [ ] **No paid deps** — All deps MIT/Apache-2.0 (rayon, jwalk, ignore, redb,
- [ ] **Critic model resolvable** — `tests/review_gate.sh` resolves `CRITIC_MODEL` exactly once at the top via `${PIPELINE_CRITIC_MODEL:-${CRITIC_MODEL:-<known-good-default>}}`, refuses to run if the resolved value is empty or matches `^\$\{[A-Z_]+\}$`, and checks the resolved value against the `CONFIG.md` allowlist BEFORE any JSON is built (fail-fast with `CRITIC_MODEL unsupported on this endpoint — set PIPELINE_CRITIC_MODEL to one of: <allowlist>` on miss). Every substitution site that puts the model into the JSON payload uses the resolved shell variable (never `${CRITIC_MODEL}` / `${PIPELINE_CRITIC_MODEL}` raw), and the assembled request body is grepped for `\$\{[A-Z_]+\}` before `curl` — any match is a hard fail. No literal `${VAR}` interpolation ever reaches the provider.

      trash, clap, tauri, react, vite, vitest, ably, notify — all OSS).
- [ ] **Privacy** — No telemetry; sync is opt-in and behind feature flag.
- [ ] **Offline-first** — Default build has zero network code; Ably compiled out
      unless `--features sync`.

---

## 7. Build Order Summary

```
Phase 1  Domain Core           → Gate 1 + Gate 2
Phase 2  Scan Engine           → Gate 1 (incl. perf) + Gate 2
Phase 3  CLI                   → Gate 1 + Gate 2
Phase 4  GUI Backend (Tauri)   → Gate 1 + Gate 2
Phase 5  GUI Frontend          → Gate 1 + Gate 2 + Gate 3
Phase 6  Live Updates + Sync   → Gate 1 + Gate 2 (feature-flagged)
Phase 7  Cross-Platform Release→ Gate 1 + Gate 2 + Gate 3
```

`pipeline.sh` runs each phase in turn with `extract_phases` driving the loop.
Phase 6 may be skipped at MVP ship time if Ably credentials and infra are not
ready — its code stays in the tree behind the feature flag for a later release.

---

## 8. Out of Scope (re-stated for clarity)

- Cloud storage scanning (S3 / GCS / Azure Blob).
- RAID / volume management, file recovery, duplicate-file finder.
- Cross-platform vision audit of screenshots (Gate 3 runs on host platform
  only; macOS/Windows artifacts verified by build-success only).
- Pro tier features (network drives, scheduled scans, team workspaces, hooks).

---

*End of plan — awaiting Gate 0 review.*