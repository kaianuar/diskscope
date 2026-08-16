# Ponytail Templates

## Write Test Template
```typescript
// {{FEATURE_NAME}}.test.ts
import { describe, it, expect } from 'vitest';

describe('{{FEATURE_NAME}}', () => {
  it('should {{BEHAVIOR}} when {{CONDITION}}', () => {
    // ARRANGE
    const input = {{INPUT}};
    const expected = {{EXPECTED}};
    
    // ACT
    const result = {{FUNCTION_UNDER_TEST}}(input);
    
    // ASSERT
    expect(result).toEqual(expected);
  });
});
```

## Refactor Template
```markdown
## Refactor Plan for {{FILE_OR_DIRECTORY}}

### Current Issues
- [ ] {{ISSUE_1}}
- [ ] {{ISSUE_2}}

### Karpathy Compliance Checklist
- [ ] Explicit > Implicit
- [ ] Types everywhere
- [ ] Small functions (< 50 lines)
- [ ] No cleverness
- [ ] Explicit error handling
- [ ] Tests as documentation
- [ ] No magic numbers
- [ ] Explicit dependencies
- [ ] Explicit returns

### Plan
1. {{STEP_1}}
2. {{STEP_2}}
3. {{STEP_3}}

### Validation
- [ ] All tests pass
- [ ] TypeScript compiles with `strict: true`
- [ ] Lint passes
- [ ] No new warnings