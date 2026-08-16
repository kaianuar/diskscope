# GUIDELINES.md — DiskScope Project Quality Constitution

> This file defines the **non-negotiable quality standards** for the DiskScope project.
> It is the single source of truth for **Gate 0 (Plan Review)**, **Gate 2 (Adversarial Code Review)**,
> and **pre-commit hooks**. The builder, critic, and pre-commit hooks all reference this file.

---

## 1. CORE PHILOSOPHY — Karpathy Principles

> **Explicit > Implicit** — No magic, no hidden behavior, no hidden state.
> **Types Everywhere** — Type hints on all functions, variables, returns. No `any`/`Any` without explicit comment.
> **Small Functions** — One thing per function, <50 lines ideal. If it does two things, split it.
> **No Cleverness** — Boring code > clever code. Readability > cleverness.
> **Explicit Error Handling** — No bare `except:` / `catch (e) { }`. Handle errors explicitly.
> **Tests as Documentation** — Test names describe behavior, not implementation. AAA pattern.
> **No Magic Numbers** — Constants with descriptive names.

---

## 2. PORTABLE RULES — Ponytail (`/.agents/rules/`)

> These rules are exported to `/.agents/rules/` so they work across **any IDE agent**:
> Cursor, OpenCode, Claude Code, Codex, Devin, Qoder, Kiro, Grok, etc.

### 2.1 Architecture & Structure
- **Hexagonal Architecture** — Domain at center, adapters at edges (Repository, API, UI).
- **Dependency Inversion** — Domain depends on abstractions (ports), not concretions.
- **Single Responsibility** — Each module/class/function has one reason to change.
- **Composition over Inheritance** — Prefer composition, interfaces, strategy pattern.

### 2.2 Type Safety
- **Strict TypeScript** — `strict: true`, `noImplicitAny: true`, `strictNullChecks: true`.
- **Python** — `mypy --strict`, type hints on ALL public functions.
- **Rust** — `#![deny(clippy::all)]`, `#![deny(missing_docs)]`, no `unwrap()` in production code.
- **No `any` / `Any`** without explicit comment explaining why.

### 2.3 Naming Conventions
| Construct | Convention |
|---|---|
| Files | `kebab-case.rs` / `snake_case.py` / `kebab-case.ts` / `kebab-case.tsx` |
| Classes/Interfaces | `PascalCase` |
| Functions/Variables | `camelCase` / `snake_case` |
| Constants | `UPPER_SNAKE_CASE` |
| Types/Interfaces | `PascalCase` (prefer `interface` over `type` for objects) |
| Private | `_prefix` (or `#private` in JS) |
| Boolean predicates | `is...`, `has...`, `can...`, `should...` |

### 2.4 Error Handling
- **No bare `catch` / `except:`** — Always catch specific errors.
- **Result/Option types** — Prefer `Result<T, E>` over throwing for expected errors.
- **Fail fast** — Validate inputs early, fail fast with descriptive messages.
- **Error context** — Wrap errors with context: `throw new Error('context', { cause: original })`.

### 2.5 Testing Standards
- **TDD is mandatory** — Test written FIRST (RED), then implementation (GREEN), then refactor.
- **Test naming** — `describe('feature', () => { it('should <behavior> when <condition>', ... })`
- **AAA Pattern** — Arrange, Act, Assert — clearly separated.
- **One assertion per test** (ideally) — or logically grouped assertions.
- **Deterministic** — No flaky tests, no time-based flakiness, no external dependencies.
- **Unit tests** — Fast, isolated, no I/O. **Integration tests** — Real DB, real API, clearly separated.
- **Coverage** — Aim for >80% on domain logic. 100% on critical paths.

### 2.6 Commit & Workflow Standards

### 4.1 Conventional Commits (enforced)
| Type | When to use |
|---|---|
| `feat:` | New feature for user |
| `fix:` | Bug fix |
| `refactor:` | Code restructuring, no behavior change |
| `test:` | Adding/updating tests |
| `docs:` | Documentation only |
| `chore:` | Maintenance (deps, config, CI) |
| `perf:` | Performance improvement |

**Format:** `type(scope): imperative description`
- `feat(scan): add parallel walk with rayon`
- `fix(gui): handle null path in treemap`
- `refactor(engine): extract cache trait`
- `test(scan): add integration tests for incremental scan`

### 4.2 TDD Workflow (enforced)
```
1. WRITE TEST (RED)     → Write failing test for the next behavior
2. IMPLEMENT (GREEN)    → Minimal code to make test pass
3. REFACTOR             → Clean up, extract, optimize (tests stay green)
4. COMMIT               → One logical change = one commit
```
- **One TDD cycle = one commit** (or small batch of tightly related cycles).
- No "test later" — test is written BEFORE implementation.

---

## 3. PROJECT-SPECIFIC RULES — DiskScope

### 3.1 Technology Stack
```yaml
language: "Rust"
edition: "2021"
framework: "Tauri v2.1 + React 18.3 + TypeScript 5.5 + Vite 6.0"
gui_framework: "egui 0.31 (WASM in <canvas>) + egui_extras (treemap, tables)"
scan_engine: "rayon 1.10 + jwalk 0.6 + ignore 0.4 + redb 2.1"
cli_framework: "clap 4.5"
error_handling: "DomainError (manual impl Display + Error) + AppError (typed enum, per binary)"
trash: "trash 4.0 (cross-platform move-to-trash)"
sync: "ably 1.0 (optional, behind sync feature flag)"
testing: "cargo test + vitest + playwright"
linting: "clippy + rustfmt + eslint + prettier"
typecheck: "cargo check --workspace + tsc --strict"
```

### 3.2 Project-Specific Patterns
```markdown
- **Agent Framework**: omp (Oh My Pi) for multi-agent orchestration
- **Providers**: OpenRouter, Xiaomi MiMo, OpenCode Go
- **Models**: deepseek-v4-flash (builder), GLM-5.2 (critic), MiMo (plan)
- **Testing**: cargo test (unit) + vitest (frontend) + playwright (e2e)
- **CI**: run-gates.sh (Gate 0: Plan, Gate 1: Tests, Gate 2: Review, Gate 3: Visual/E2E)
```

### 3.3 Domain-Specific Rules
```markdown
- All agent interactions go through omp CLI
- Model roles configured in .omp/config.yml
- OpenRouter key in ~/.hermes/.env (loaded by run-gates.sh)
- Critic uses z-ai/glm-5.2 via OpenRouter
- Builder uses xiaomi-token-plan-sgp/mimo-v2.5-pro
- Plan phase: Gate 0 (plan review) before any code
- Build phase: TDD enforced, incremental commits
- Gate 0: Plan review (adversarial)
- Gate 1: Tests (auto-detect stack)
- Gate 2: Adversarial review (critic model different from builder)
- Gate 3: Visual/E2E (Playwright + vision model)
- Pre-commit hooks: clippy + rustfmt + eslint + prettier + commitizen
```

### 3.4 Scan Engine Specific Rules
```markdown
- Parallel scanning using rayon + jwalk (mandatory)
- Respects .gitignore via ignore crate (mandatory)
- Caching with redb embedded database (mandatory)
- Filters: size, type, age, depth, pattern (all implemented)
- Output formats: JSON, JSONL, table, tree (all implemented)
- Incremental scan support (re-scan only changed files)
- No unwrap() in production code — use ? or expect with context
- All public functions have doc comments
- No unwrap() or expect() in hot paths