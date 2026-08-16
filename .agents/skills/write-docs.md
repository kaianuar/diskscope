# Skill: Write Documentation

## Purpose
Write documentation following project conventions and Karpathy principles.

## Usage
```
/write-docs <topic>
```

## Documentation Standards

### 1. Explicit > Implicit
- Document WHY, not just WHAT
- Explain the WHY behind decisions
- Document assumptions and constraints

### 2. Types & Contracts
- Document public APIs with types
- Document preconditions and postconditions
- Document error cases and expected behavior

### 3. Small & Focused
- One concept per document
- Link to related docs instead of repeating
- Cross-reference related concepts

### 4. No Cleverness
- Clear, direct language
- No jargon without definition
- Examples over abstract explanations

### 5. Explicit Examples
- Every public API has a usage example
- Examples are runnable (copy-pasteable)
- Edge cases documented

## Documentation Structure

```
docs/
├── architecture.md          # High-level architecture
├── api/
│   ├── endpoints.md         # API endpoints
│   └── schemas.md           # Request/response schemas
├── domain/
│   ├── entities.md          # Domain entities
│   └── value-objects.md     # Value objects
├── guides/
│   ├── getting-started.md
│   ├── development.md
│   └── deployment.md
└── decisions/
    └── adr-<number>-<topic>.md  # Architecture Decision Records
```

## Checklist
- [ ] Public APIs documented with types
- [ ] Preconditions/postconditions documented
- [ ] Error cases documented
- [ ] Usage examples for public APIs
- [ ] Cross-references to related docs
- [ ] ADR for significant architectural decisions
- [ ] Commit: `docs: <description>`