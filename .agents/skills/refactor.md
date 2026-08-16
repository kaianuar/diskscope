# Skill: Refactor (Karpathy Principles)

## Purpose
Refactor code following Karpathy principles: explicit, typed, small, no cleverness.

## Usage
```
/refactor <file-or-directory>
```

## Refactoring Checklist (Karpathy Principles)

### 1. Explicit > Implicit
- [ ] No magic, no hidden behavior, no hidden state
- [ ] All dependencies explicit
- [ ] No global state mutations
- [ ] No implicit side effects

### 2. Types Everywhere
- [ ] Type hints on ALL public functions, variables, returns
- [ ] No `any` / `Any` without explicit comment
- [ ] TypeScript `strict: true`, `noImplicitAny: true`

### 3. Small Functions
- [ ] One thing per function
- [ ] Functions < 50 lines (ideally < 30)
- [ ] If it does two things, split it

### 4. No Cleverness
- [ ] Boring code > clever code
- [ ] Readability > cleverness
- [ ] No clever one-liners, obscure syntax

### 5. Explicit Error Handling
- [ ] No bare `catch` / `except:`
- [ ] Handle errors explicitly with context
- [ ] Error messages are descriptive

### 6. Tests as Documentation
- [ ] Test names describe BEHAVIOR (`should <do> when <condition>`)
- [ ] AAA pattern (Arrange, Act, Assert)
- [ ] Test names describe BEHAVIOR, not implementation

### 7. No Magic Numbers
- [ ] Named constants with descriptive names
- [ ] No magic numbers/strings in non-test code

### 8. Explicit Dependencies
- [ ] Dependencies injected, not imported globally
- [ ] No global state mutations
- [ ] No hidden side effects

### 9. Explicit Returns
- [ ] Return types explicit
- [ ] No implicit returns
- [ ] TypeScript `noImplicitReturns: true`

### 9. Tests as Documentation
- [ ] Test names describe BEHAVIOR (`should <do> when <condition>`)
- [ ] AAA pattern (Arrange, Act, Assert)
- [ ] Test names describe BEHAVIOR, not implementation

## Checklist
- [ ] All functions < 50 lines
- [ ] All public functions have explicit type hints
- [ ] No `any` / `Any` without comment
- [ ] No bare `catch` / `except:`
- [ ] All error messages descriptive
- [ ] No magic numbers/strings
- [ ] Dependencies explicit (injected, not global)
- [ ] Test names follow `should <behavior> when <condition>`
- [ ] AAA pattern in all tests
- [ ] Commit: `refactor: <description>`