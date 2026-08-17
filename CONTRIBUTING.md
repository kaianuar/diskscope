# Contributing to DiskScope

## Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Git

## Development Setup

```bash
git clone https://github.com/kaianuar/diskscope.git
cd diskscope
cargo test                    # verify everything works
cargo clippy -- -D warnings   # verify lint passes
```

## Branching

All work happens on topic branches off `main`:

- `feat/<description>` — new features
- `fix/<description>` — bug fixes
- `refactor/<description>` — code restructuring
- `docs/<description>` — documentation
- `chore/<description>` — tooling/dependency updates

## Commit Convention

Single-line Conventional Commits:

```
feat(scanner): add symlink traversal option
fix(cache): handle corrupt redb gracefully
test(domain): add filter composition edge cases
docs(readme): add CLI usage examples
```

Each commit must:
1. Pass `cargo test`
2. Pass `cargo clippy -- -D warnings`
3. Represent one logical change (atomic)

## Running Tests

```bash
# All tests (unit + integration, all crates)
cargo test

# Scan engine only
cargo test -p scan-engine

# CLI only
cargo test -p diskscope-cli

# Domain unit tests only
cargo test -p scan-engine --lib

# Integration tests only
cargo test -p scan-engine --test scan_engine_tests
```

## Code Style

- `clippy::all` and `missing_docs` lints enforced
- All public items must have doc comments (`///`)
- Domain layer (`scan-engine/src/domain/`) must remain zero external dependencies
- Adapters (`scanner/`, `cli/`, `gui/`) may use external crates

## Pull Requests

1. Create a topic branch from `main`
2. Make atomic commits with conventional commit messages
3. Ensure `cargo test` and `cargo clippy` pass
4. Open a PR with a clear description of what changed and why
5. PR title follows the same conventional commit format

## Architecture

DiskScope uses hexagonal architecture:

```
Domain (pure) ← Port Traits ← Adapters (scanner, cache, trash)
                    ↑
              CLI / GUI shells
```

- **Domain**: `FileNode`, `FileTree`, `Filter`, `FileType`, `Size`, `SortKey`, `OutputFormat`, `ScanOpts`, error types, port traits
- **Adapters**: `RedbCache`, `IncrementalScanner`, walker (all in `scan-engine/src/scanner/`)
- **Shells**: CLI (`cli/`), GUI (`gui/`)

When adding features, start with the domain (port trait), then implement the adapter, then wire into the shell.

## License

By contributing, you agree that your contributions will be licensed under MIT OR Apache-2.0.
