# AGENTS.md — DiskScope

Guidance for AI agents (omp, Claude Code, Copilot, etc.) working in this repository. Read this first; it overrides generic assumptions.

## What This Repo Is

DiskScope is a **cross-platform disk space analyzer**: it scans directories, shows which files/folders use the most space (treemap + table), and supports safe delete-with-undo. It is a PRODUCT repo — the build pipeline that generated it lives in a separate repo (`omp-agent`) and is NOT part of this codebase. Do not add pipeline machinery here.

Three deliverables exist: a **CLI**, a **Tauri v2 desktop GUI**, and **CI** that builds installers for Linux/macOS/Windows.

## Architecture

Rust workspace (Cargo.toml at root) with four crates — hexagonal layout, domain at the center:

| Crate | Path | Responsibility |
|---|---|---|
| `domain` | `domain/` | Pure domain: `FileNode`, `FileTree`, `Filter`, `FileType`, `SortKey`, `OutputFormat`, `ScanOpts`, `DomainError`, port traits (`Scanner`, `Cache`, `Trash`). **Zero dependencies on other crates.** No I/O. |
| `scan-engine` | `scan-engine/` | Adapters: parallel walker (`ignore` + `rayon`), `redb` cache, incremental scanner, size normalization, trash integration, sync (Ably, unused by GUI yet). Depends on `domain`. |
| `cli` | `cli/` | `diskscope` binary (clap). Thin controller over scan-engine. |
| `gui` | `gui/` | Tauri v2 app. `gui/src-tauri/` = Rust backend (commands, DTOs, scan runner). `gui/web/` = React 18 + TypeScript frontend (Vite). |

**Frontend layout (`gui/web/src/`):**
- `App.tsx` — shell: owns scan state, navigation (breadcrumb/history), selection, filters, context menu
- `components/` — Toolbar, Sidebar (path input + quick paths), Breadcrumb, FilterPanel, TableView, TreemapCanvas2D, StatusBar, ContextMenu
- `hooks/` — useScan (IPC to Rust), useSelection, useShortcuts
- `ipc.ts` — typed wrappers over Tauri invoke
- `lib/` — formatSize, treemapLayout
- `styles.css` — design tokens (CSS custom properties) + all styling

**Design tokens** (styles.css `:root`): `--ds-primary: #2563eb`, `--ds-background: #0f172a`, `--ds-surface: #1e293b`, `--ds-text: #e2e8f0`, `--ds-muted: #94a3b8`, `--ds-accent: #f59e0b`, `--ds-success: #22c55e`, `--ds-danger: #ef4444`. Dark theme only. Mirror `design-system/tokens.json` (file-type colors).

## Conventions

**Rust:**
- Edition 2021, MSRV 1.75
- `#![forbid(unsafe_code)]` in gui lib; `unsafe` is forbidden unless truly required — prefer safe refactors
- Errors: `DomainError` enum with typed variants; return `Result`, never `.expect()`/`.unwrap()` in non-test code
- No panics on user input; invalid paths → `DomainError::InvalidPath` or `PermissionDenied`
- Directory `size` fields are **recursive sums of children** (normalize_sizes post-pass in scanner). Never set a dir's size from `metadata.len()` — that's the raw inode size and double-counts.
- `total_size()` returns `self.size` (already the sum for dirs post-normalize); do NOT add children again
- Cross-platform: `trash::os_limited` is **cfg-gated OUT on macOS** — any new use must go through the cfg-gated helpers in `scan-engine/src/trash.rs` (`list_trash`/`restore_items`) which return `DomainError::Unsupported` on macOS
- Path separators: handle both `/` and `\` (Windows)

**TypeScript/React:**
- Strict mode: `noUnusedLocals`, `noUnusedParameters`, `noUnusedLocals` all on — **no unused imports/vars** (tsc fails)
- `node:url`/Node types available via `@types/node`; tsconfig `types` includes `"node"`
- Functional components, hooks, no classes
- All interactive elements get `data-testid` attributes (tests rely on them)
- Import path alias `@/*` → `src/*`
- No `any`; type IPC boundaries via `ipc.ts`
- Buttons/inputs styled with `--ds-*` tokens, not hardcoded colors

## Code Style (Karpathy / Ponytail principles)

The repo follows Andrej Karpathy's coding principles + Ponytail style (originally enforced by the pipeline critic; now expected of any agent writing code here):

1. **Explicit > implicit** — no magic, no hidden state, no implicit side effects. Dependencies explicit, no global mutations.
2. **Types everywhere** — TypeScript `strict` (on), no `any` without an explicit `// @ts-expect-error` + comment. Rust: explicit types on public functions.
3. **Small functions** — one thing per function, <50 lines ideal. Split anything doing two things.
4. **No cleverness** — boring readable code beats clever one-liners. Readability > brevity.
5. **Explicit error handling** — no bare `catch`/`except`. Handle errors with context. Rust: return `Result`, never swallow errors.
6. **Tests as documentation** — test names describe behavior: `should_<do>_when_<condition>` (Rust) / `should <do> when <condition>` (TS).
7. **No magic numbers** — named constants for literals in non-test code.
8. **Fail fast** — validate inputs at boundaries with descriptive messages.
9. **Explicit returns** — return types explicit (TS `noImplicitReturns`).
10. **Hexagonal architecture** — domain at center with ZERO external deps; adapters implement domain ports. Domain depends on abstractions, never concretions.
11. **Naming** — files kebab-case, types/interfaces PascalCase (prefer `interface` for objects), functions/vars camelCase, constants UPPER_SNAKE_CASE, boolean predicates `is...`/`has...`/`can...`.
12. **Single responsibility + composition over inheritance** — one reason to change per module; prefer composition/interfaces over class hierarchies.

## Testing

- **Rust:** `cargo test` (domain 34, cli 10, scan-engine). Tests use `should_<behavior>_when_<condition>` naming.
- **Frontend:** `npx vitest run` (14 tests, components + hooks). Tests in `src/**/__tests__/`.
- **E2E:** Playwright specs in `tests/e2e/` (self-contained, run against `localhost:5173` dev server; `BASE_URL` env to override).
- **CLI behavior:** verify manually against real dirs — sizes must match `du -sb`, no duplicate root rows, `--quiet` must keep output (these were the historical bug classes).

## Build & Run

```bash
# CLI
cargo build --release                    # binary: ./target/release/diskscope
./target/release/diskscope scan ~/code --format tree

# GUI dev (hot reload)
cd gui && cargo tauri dev                # requires Tauri Linux deps installed

# GUI release + installers
cd gui && cargo tauri build              # deb/AppImage/rpm in target/release/bundle/

# Frontend only
cd gui/web && npm run build              # tsc --noEmit && vite build
```

## CLI Surface

`diskscope scan <path> [--format table|json|jsonl|tree] [--sort name|size asc|desc] [--min-size BYTES] [--max-size BYTES] [--max-age SECS] [--name-pattern P] [--max-depth N] [--no-cache] [--quiet]`
`diskscope summary <path>` — total, count, top 10
`diskscope delete <path> [--undo]` — move to trash / undo last
`diskscope completions <shell>`

Note: `--min-size`/`--max-size` take **raw bytes**, not human strings ("100MB" fails).

## Known Limitations (do not "fix" without product sign-off)

- Trash undo-by-item unavailable on macOS (upstream `trash` crate limitation)
- Ably real-time sync implemented in scan-engine but not wired into the GUI
- macOS builds not signed/notarized
- GUI UX is intentionally evolving — see the UI backlog for planned improvements

## Workflow Rules

- Commit with conventional messages (`fix(scope):`, `feat(scope):`); the repo uses `--no-verify` on commits by convention (pre-commit hooks are unreliable here)
- Default branch: `feat/phase1-domain-core`
- Verify with `tsc --noEmit` + `npm run build` + `npx vitest run` for frontend, `cargo test` for Rust, before finishing
- Do NOT add omp-agent pipeline files (gate scripts, plan.md, requirements.md, ponytail.yaml) — they were intentionally removed
- Do NOT commit `.env` files (gitignored; API keys live outside the repo)
