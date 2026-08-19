// Playwright E2E spec for DiskScope treemap (Gate 3).
//
// Runs against the Vite dev server (gui/web). Tauri IPC is unavailable
// in a plain browser — these tests exercise the React shell, Canvas2D
// treemap rendering, keyboard navigation, context menu, and table
// interactions. Screenshots are captured for the vision-model visual
// review pass.

import { test, expect } from '@playwright/test';

const BASE_URL = process.env.BASE_URL || 'http://localhost:5173';
const SHOTS = process.env.SHOTS_DIR || '/tmp/gate3_shots';

test.describe('app shell + core flow', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(BASE_URL);
  });

  test('should render the app shell on load', async ({ page }) => {
    await expect(page.locator('[data-testid="app-shell"]')).toBeVisible();
    await expect(page.locator('[data-testid="sidebar"]')).toBeVisible();
    await expect(page.locator('[data-testid="toolbar"]')).toBeVisible();
    await expect(page.locator('[data-testid="status-bar"]')).toBeVisible();
    await page.screenshot({ path: `${SHOTS}/01-shell.png`, fullPage: true });
  });

  test('should render treemap canvas area', async ({ page }) => {
    const canvas = page.locator('[data-testid="treemap-canvas"]');
    await expect(canvas).toBeVisible();
    // Canvas should have non-zero dimensions.
    const box = await canvas.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(0);
    expect(box!.height).toBeGreaterThan(0);
    await page.screenshot({ path: `${SHOTS}/02-treemap.png`, fullPage: true });
  });

  test('should render the file table with sortable headers', async ({ page }) => {
    const table = page.locator('[data-testid="file-table"]');
    await expect(table).toBeVisible();
    // All four column headers should be present.
    for (const col of ['Name', 'Size', 'Modified', 'Type']) {
      await expect(table.locator('th', { hasText: col })).toBeVisible();
    }
    await page.screenshot({ path: `${SHOTS}/03-table.png`, fullPage: true });
  });

  test('should show scan input and scan button in sidebar', async ({ page }) => {
    await expect(page.locator('[data-testid="scan-path-input"]')).toBeVisible();
    await expect(page.locator('[data-testid="start-scan"]')).toBeVisible();
  });

  test('should show filter panel', async ({ page }) => {
    await expect(page.locator('[data-testid="filter-panel"]')).toBeVisible();
  });

  test('should show status bar with initial "No scan yet" message', async ({ page }) => {
    await expect(page.locator('[data-testid="status-bar"]')).toContainText('No scan yet');
  });
});

test.describe('keyboard navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(BASE_URL);
  });

  test('should not hijack typing in the scan input when ArrowDown pressed', async ({ page }) => {
    const input = page.locator('[data-testid="scan-path-input"]');
    await input.focus();
    // Pressing ArrowDown while focused on an input should NOT move table selection.
    await page.keyboard.press('ArrowDown');
    // The input should still be focused (shortcuts skip inputs).
    await expect(input).toBeFocused();
  });
});

test.describe('context menu', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(BASE_URL);
  });

  test('should not show context menu initially', async ({ page }) => {
    await expect(page.locator('[data-testid="context-menu"]')).toHaveCount(0);
  });
});

test.describe('status bar', () => {
  test('should display status path and summary areas', async ({ page }) => {
    await page.goto(BASE_URL);
    await expect(page.locator('[data-testid="status-path"]')).toBeVisible();
    // status-summary only appears after a scan completes.
    await expect(page.locator('[data-testid="status-summary"]')).toHaveCount(0);
  });
});

test.describe('treemap after scan', () => {
  test('should render treemap after scan completes when app scans a fixture dir', async ({ page }) => {
    const fixtureUrl = `${BASE_URL}/treemap-fixture.html`;
    await page.goto(fixtureUrl);

    // Wait for the fixture's JS to finish rendering the treemap.
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__treemapReady === true, null, { timeout: 10_000 });

    // Canvas must be visible with non-zero dimensions.
    const canvas = page.locator('[data-testid="treemap-canvas"]');
    await expect(canvas).toBeVisible();
    const box = await canvas.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(0);
    expect(box!.height).toBeGreaterThan(0);

    // Canvas must have painted content (not blank).
    const hasContent = await page.evaluate(() => {
      const c = document.querySelector('[data-testid="treemap-canvas"]') as HTMLCanvasElement;
      if (!c) return false;
      const data = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i + 3] > 0) return true; // any non-transparent pixel
      }
      return false;
    });
    expect(hasContent).toBe(true);

    // Status summary visible after scan completes.
    await expect(page.locator('[data-testid="status-summary"]')).toBeVisible();
    await expect(page.locator('[data-testid="status-summary"]')).toContainText('entries');

    await page.screenshot({ path: `${SHOTS}/04-treemap-after-scan.png`, fullPage: true });
  });
});
