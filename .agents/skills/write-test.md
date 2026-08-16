# Skill: Write Test (TDD + AAA Pattern)

## Purpose
Write a failing test first (RED), then implement to make it pass (GREEN), then refactor.

## Usage
```
/write-test <behavior-description>
```

## Template

```typescript
// Test file: <feature>.test.ts
import { describe, it, expect } from 'vitest';

describe('<FeatureName>', () => {
  it('should <behavior> when <condition>', () => {
    // ARRANGE
    // Given: set up the initial state
    const input = ...;
    const expected = ...;
    
    // ACT
    // When: execute the behavior under test
    const result = functionUnderTest(input);
    
    // ASSERT
    // Then: verify the expected outcome
    expect(result).toEqual(expected);
  });
});
```

## Rules
1. **Test FIRST** — Write the test BEFORE any implementation
2. **One behavior per test** — Each `it()` tests ONE behavior
3. **AAA Pattern** — Arrange, Act, Assert clearly separated
4. **Descriptive names** — `should <behavior> when <condition>`
4. **No implementation in test** — Test only specifies expected behavior
5. **Run test first** — Confirm it fails (RED) before implementing

## Checklist
- [ ] Test file created with `.test.ts` suffix
- [ ] `describe` block with feature name
- [ ] `it` block with descriptive name: `should <behavior> when <condition>`
- [ ] AAA pattern clearly separated
- [ ] Test fails before implementation (RED)
- [ ] Test passes after implementation (GREEN)
- [ ] Refactor if needed (REFACTOR)
- [ ] Commit with message: `test: <description>`