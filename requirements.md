# DiskScope — Requirements

## Problem Statement
Users need a fast, cross-platform disk space analyzer that can quickly identify space hogs, visualize disk usage with an interactive treemap, and safely delete unwanted files. Existing tools are either paid (DaisyDisk), platform-specific (WinDirStat, GrandPerspective), slow (ncdu), or lack a modern GUI.

## Goal
Build a **free, open-source, cross-platform disk space analyzer** with:
- Fast parallel scanning (parallel walk + rayon)
- Interactive treemap visualization (egui)
- Safe delete (move to trash, with undo)
- Cross-platform (Linux, macOS, Windows)
- Free forever, with optional Pro tier for advanced features

## Target Users
- Developers cleaning up build artifacts, node_modules, caches
- Power users managing disk space on personal machines
- System admins auditing server disk usage

## Acceptance Criteria

### Core Functionality (MVP)
- [ ] Scan a directory and display results in <2 seconds for typical home directories (~100k files)
- [ ] Display interactive treemap visualization (egui + egui_extras)
- [ ] Tree/table view with sortable columns (name, size, modified, type)
- [ ] Filter by: size range, file type, age, name pattern
- [ ] Safe delete: move to system trash (not permanent delete), with undo
- [ ] Filter by: size range, file type (audio/video/image/doc/code/archive), age, name pattern
- [ ] Context menu: open in file explorer, copy path, copy to clipboard
- [ ] Keyboard shortcuts for navigation (arrows, enter, backspace, delete)
- [ ] Keyboard shortcut: `Delete` → move to trash, `Cmd/Ctrl+Z` → undo

### Real-time Sync (via Ably)
- [ ] Multi-device sync: scan results sync across devices in real-time
- [ ] Live updates: when files change on disk, UI updates automatically
- [ ] Conflict resolution: last-write-wins with timestamp

### Cross-Platform
- [ ] Linux (AppImage, .deb, .rpm, .tar.gz)
- [ ] macOS (universal binary, .dmg, notarized)
- [ ] Windows (MSI installer, portable .exe)
- [ ] Single binary distribution where possible

### Performance
- [ ] Scan 100k files in <2 seconds on modern hardware
- [ ] Memory usage <200MB during scan
- [ ] UI remains responsive during scan (background thread)
- [ ] Incremental scan: re-scan only changed files

### Pro Features (Post-MVP)
- [ ] Network drives (SMB/NFS) scanning
- [ ] Scheduled scans with notifications
- [ ] Cloud storage index (Google Drive, Dropbox, S3)
- [ ] Team workspaces with shared scans
- [ ] Custom scripts/hooks for automated cleanup

## Technical Requirements

### Architecture
- **Language**: Rust (2021 edition)
- **Architecture**: Hexagonal (domain at center, adapters at edges)
- **Workspace**: Cargo workspace with 3 crates
  - `scan-engine` — core scanning logic (library)
  - `gui` — Tauri + React + egui frontend (binary)
  - `cli` — CLI binary
- **Domain**: Pure Rust, zero external dependencies
- **Adapters**: Tauri (GUI), CLI, scan engine

### Scan Engine
- Parallel scanning using `rayon` + `jwalk`
- Respects `.gitignore` via `ignore` crate
- Caching with `redb` embedded database
- Filters: size, type, age, depth, pattern
- Output formats: JSON, JSONL, table, tree
- Incremental scan support (re-scan only changed files)

### GUI (Tauri + React + egui)
- **Framework**: Tauri v2 + React 18 + TypeScript + Vite
- **UI**: egui + egui_extras (treemap, tables)
- **State**: React for chrome, egui for canvas-heavy views
- **IPC**: Tauri commands for scan control
- **Real-time**: Ably for live sync across devices

### CLI
- `diskscope scan [path] --format table|json|jsonl|tree`
- `diskscope summary <path>` — quick summary
- `diskscope completions <shell>` — shell completions
- Output formats: table, JSON, JSONL, tree

### Quality Gates (enforced by pipeline)
- **Gate 0**: Plan review (architecture, TDD plan, risk assessment)
- **Gate 1**: Tests pass (unit + integration)
- **Gate 2**: Adversarial code review (different model)
- **Gate 3**: Visual + functional E2E (Playwright + vision model)

### Deployment
- **CI/CD**: GitHub Actions
- **Artifacts**: `.dmg`, `.msi`, `.AppImage`, `.deb`, `.rpm`, `.tar.gz`
- **Auto-update**: Tauri updater
- **Code signing**: macOS (Developer ID), Windows (EV cert)

## Constraints
- **No Electron** — Tauri only (smaller binary, less memory)
- **No paid dependencies** — all OSS, MIT/Apache-2.0 compatible
- **No telemetry without consent** — privacy first
- **Offline-first** — works offline, sync when online

## Out of Scope (v1)
- Cloud storage scanning (S3, GCS, Azure Blob)
- RAID/volume management
- File recovery/undelete
- Duplicate file finder (separate tool)

## Success Metrics
- 10k+ downloads in first month
- <5% crash rate
- 4.5+ star rating on GitHub
- 100+ contributors in first year