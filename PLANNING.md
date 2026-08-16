# PLANNING.md — Plan Template

> Copy this to `plan.md` and fill in. The critic reviews this at **Gate 0**.
> Be specific. Vague plans get rejected.

---

## 1. PROBLEM & GOAL

**Problem Statement:**
<!-- One paragraph: what problem are we solving? For whom? What's the outcome? -->

**Goal:**
<!-- One sentence: the concrete outcome (e.g., "User can create, toggle, delete, and share todo lists in real-time") -->

---

## 2. ARCHITECTURE

### 2.1 High-Level Architecture
```mermaid
<!-- Architecture diagram (mermaid) or ASCII sketch -->
<!-- Show: domain core, adapters (API, DB, UI), boundaries -->
```

### 2.2 Module Boundaries
| Module | Responsibility | Depends On |
|---|---|---|
| `domain/` | Pure domain logic (entities, value objects, domain services) | — (zero external deps) |
| `application/` | Use cases, application services | `domain/` |
| `infrastructure/` | Adapters: DB, API, UI, External services | `domain/`, `application/` |
| `api/` | HTTP handlers, routing, serialization | `application/` |
| `ui/` | React components, state, hooks | `api/` |

### 2.3 Key Interfaces (Ports)
```typescript
// Example ports the domain defines
interface TaskRepository {
  save(task: Task): Promise<Task>;
  findById(id: TaskId): Promise<Task | null>;
  findByListId(listId: ListId): Promise<Task[]>;
  delete(id: TaskId): Promise<void>;
}
```

### 2.4 Data Flow (Critical Paths)
```
User Action → API Handler → Application Service → Domain Service → Repository → DB
                    ↑                                                      ↓
              Response ← Serialization ← Domain Events ← Persistence ←─┘
```

---

## 3. TDD TEST PLAN

> **List every test you will write, IN ORDER (TDD sequence).**
> Format: `test: <description>`

| Order | Test Description | Type | Notes |
|---|---|---|---|
| 1 | `should create a task with valid input` | unit | domain |
| 2 | `should reject task with empty title` | unit | domain |
| 3 | `should toggle task completion` | unit | domain |
| 4 | `should delete task` | unit | domain |
| 5 | `should list tasks by listId` | unit | domain |
| 6 | `should create task via API` | integration | api |
| 7 | `should reject invalid task input` | integration | api |
| 8 | `should toggle task via API` | integration | api |
| ... | ... | ... | ... |

> **Rule:** Tests written IN THIS ORDER. One test → implement → refactor → commit. Repeat.

---

## 4. DESIGN PATTERNS & PRINCIPLES

| Concern | Pattern / Principle | Rationale |
|---|---|---|
| **Data Access** | Repository Pattern | Domain depends on abstraction, not DB |
| **Domain Logic** | Domain Services + Entities | Keep business logic in domain |
| **API Layer** | Thin Handlers + Application Services | Thin controllers, logic in app services |
| **Validation** | Zod / Pydantic at boundaries | Fail fast, explicit errors |
| **Error Handling** | Result<T, E> / Exceptions | No bare catch, explicit errors |
| **Testing** | TDD + AAA | Tests as documentation |

---

## 5. RISK ASSESSMENT

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| e.g., "Third-party API rate limits" | Medium | High | Implement client-side rate limiter + exponential backoff |
| e.g., "Database migration complexity" | Low | High | Write migration scripts first, test on staging |

---

## 6. PONYTAIL / KARPATHY COMPLIANCE CHECKLIST

> The critic will verify these at Gate 0. Check each before submitting.

- [ ] **Architecture** — Hexagonal, domain at center, adapters at edges
- [ ] **Dependencies** — Domain has ZERO external deps
- [ ] **Types** — All public functions typed, no `any`/`Any`
- [ ] **TDD** — Tests listed in order, test-first approach
- [ ] **Karpathy** — Explicit types, small functions, no cleverness, explicit errors
- [ ] **Ponytail** — Rules exported to `/.agents/rules/`, portable
- [ ] **TDD** — Tests listed in order, test-first approach
- [ ] **Commits** — Conventional commits, one logical change per commit
- [ ] **TDD Cycle** — Test → Implement → Refactor → Commit (per cycle)

---

## 7. RISK MITIGATION

| Risk | Mitigation |
|---|---|
| | |

---

## 8. ESTIMATION

| Phase | Estimated Effort |
|---|---|
| Plan & Gate 0 | |
| Build (TDD cycles) | |
| Gates 1-3 | |
| Steer | |

---

*Delete this template section and replace with your actual plan.*