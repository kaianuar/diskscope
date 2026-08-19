import { defineConfig } from '@playwright/test';

// TEMPLATE Playwright config for GATE 3 (tests/visual_gate.sh).
// Set baseURL via env (BASE_URL) at runtime; use a local chromium.
export default defineConfig({
  testDir: './',
  timeout: 60_000,
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:5173',
    viewport: { width: 390, height: 844 },  // mobile-first, matches the app's intent
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
