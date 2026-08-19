import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';

// Auto-cleanup DOM after each test (RTL default with vitest globals).
afterEach(() => {
  cleanup();
});
