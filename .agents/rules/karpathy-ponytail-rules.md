# Ponytail Rules — Karpathy Principles (Core)

# These rules are the CORE coding standards derived from Andrej Karpathy's principles.
# They are enforced by the critic at Gate 0 (Plan Review) and Gate 2 (Adversarial Review).
# Exported to .agents/rules/ so any IDE agent can enforce them.

---
# KARPATHY PRINCIPLES (Core Rules)

## 1. Explicit > Implicit
- **Rule**: No magic, no hidden behavior, no hidden state.
- **Enforcement**: All dependencies explicit. No global state mutations. No implicit side effects.
- **Checker**: Grep for global mutations, implicit imports, hidden side effects.

## 2. Types Everywhere
- **Rule**: Type hints on ALL public functions, variables, returns. No `any` / `Any` without explicit comment.
- **Enforcement**: TypeScript `strict: true`, `noImplicitAny: true`. Python `mypy --strict`.
- **Exception**: `// @ts-expect-error` with comment explaining why.

## 3. Small Functions
- **Rule**: One thing per function, < 50 lines ideal. If it does two things, split it.
- **Enforcement**: Lint rule for function length > 50 lines = warning.

## 4. No Cleverness
- **Rule**: Boring code > clever code. Readability > cleverness.
- **Enforcement**: Flag clever tricks, clever one-liners, obscure syntax.

## 4. No Cleverness
- **Rule**: Boring code > clever code. Readability > cleverness.
- **Enforcement**: Flag clever tricks, clever one-liners, obscure syntax.

## 5. Explicit Error Handling
- **Rule**: No bare `catch` / `except:`. Handle errors explicitly with context.
- **Enforcement**: Flag bare `catch (e) { }`, `except:`, bare `except:`.

## 6. Tests as Documentation
- **Rule**: Test names describe BEHAVIOR (`should <do> when <condition>`), not implementation.
- **Pattern**: `it('should <behavior> when <condition>', ...)`
- **Enforcement**: Test naming convention check.

## 7. No Magic Numbers
- **Rule**: Named constants with descriptive names. No magic numbers/strings.
- **Enforcement**: Flag literal numbers/strings in non-test code.

## 8. Explicit Dependencies
- **Rule**: Dependencies injected, not imported globally. Explicit constructor injection.
- **Enforcement**: Flag global imports, service locator patterns.

## 9. Fail Fast
- **Rule**: Validate early, fail fast with descriptive messages.
- **Enforcement**: Input validation at boundaries.

## 10. Explicit Returns
- **Rule**: Return types explicit. No implicit returns.
- **Enforcement**: TypeScript `noImplicitReturns: true`.

---

# PONYTAIL SPECIFIC RULES

## 11. Hexagonal Architecture
- Domain at center, adapters at edges.
- Domain has ZERO external dependencies.
- Adapters implement domain ports (Repository, Presenter, etc.).

## 12. Dependency Inversion
- Domain depends on abstractions (ports), not concretions.
- Adapters implement domain ports (Repository, Presenter, etc.).

## 13. Single Responsibility
- Each module/class/function has one reason to change.

## 13. Composition over Inheritance
- Prefer composition, interfaces, strategy pattern.

## 4. Type Safety
- **Strict TypeScript** — `strict: true`, `noImplicitAny: true`, `strictNullChecks: true`.
- **Python** — `mypy --strict`, type hints on ALL public functions.
- **No `any` / `Any`** without explicit `// @ts-expect-error` + comment explaining why.

## 5. Naming Conventions
| Construct | Convention |
|---|---|
| Files | `kebab-case.ts` / `snake_case.py` |
| Classes/Interfaces | `PascalCase` |
| Functions/Variables | `camelCase` / `snake_case` |
| Constants | `UPPER_SNAKE_CASE` |
| Types/Interfaces | `PascalCase` (prefer `interface` over `type` for objects) |
| Private | `_prefix` (or `#private` in JS) |
| Boolean predicates | `is...`, `has...`, `can...`, `should...` |

## 6. Error Handling
- **No bare `catch` / `except:`** — Always catch specific errors.
- **Result/Option types** — Prefer `Result<T, E>` over throwing for expected errors.
- **Fail fast** — Validate inputs early, fail fast with descriptive messages.
- **Error context** — Wrap errors with context: `throw new Error('context', { cause: original })`.

## 7. Testing Standards
- **TDD is mandatory** — Test written FIRST (RED), then implementation (GREEN), then refactor.
- **Test naming** — `describe('feature', () => { it('should <behavior> when <condition>', ... })`
- **AAA Pattern** — Arrange, Act, Assert — clearly separated.
- **One assertion per test** (ideally) — or logically grouped assertions.
- **Deterministic** — No flaky tests, no time-based flakiness, no external dependencies.
- **Unit tests** — Fast, isolated, no I/O. **Integration tests** — Real DB, real API, clearly separated.
- **Coverage** — Aim for >80% on domain logic. 100% on critical paths.

## 8. Commit & Workflow Standards

### 8.1 Conventional Commits (enforced)
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
- `feat(auth): add JWT refresh token rotation`
- `fix(api): handle 401 on expired token gracefully`
- `refactor(db): extract repository base class`
- `test(auth): add integration tests for login flow`

### 8.2 TDD Workflow (enforced)
```
1. WRITE TEST (RED)     → Write failing test for the next behavior
2. IMPLEMENT (GREEN)    → Minimal code to make test pass
3. REFACTOR             → Clean up, extract, optimize (tests stay green)
4. COMMIT               → One logical change = one commit
```
- **One TDD cycle = one commit** (or small batch of tightly related cycles).
- No "test later" — test is written BEFORE implementation.

---

*End of Ponytail Rules*