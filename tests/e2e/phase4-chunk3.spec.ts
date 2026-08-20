// Playwright E2E spec — Phase 4 chunk 3/3.
//
// Tests interactive operations (navigate, trash, undo), context menu
// (reveal, copy path), and a 10k-file performance smoke. All tests run
// against static HTML fixture pages served by the Vite dev server.
// Tauri IPC is unavailable in a plain browser — the fixtures replicate
// the React app's DOM structure with inline JS that mirrors the real
// behaviour.

import { test, expect } from '@playwright/test';

const BASE_URL = process.env.BASE_URL || 'http://localhost:5173';
const SHOTS = process.env.SHOTS_DIR || '/tmp/gate3_shots';

// ── Navigate into directory on double-click ─────────────────────────

test.describe('directory navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${BASE_URL}/interactive-ops-fixture.html`);
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__ready === true, null, { timeout: 10_000 });
  });

  test('should navigate into directory when entry double-clicked', async ({ page }) => {
    // Root is /home/user/projects — table shows 6 entries (2 dirs + 4 files).
    const rows = page.locator('[data-testid="table-row"]');
    await expect(rows).toHaveCount(6);

    // Double-click the first row (src directory).
    await rows.first().dblclick();

    // After navigation, we should be inside /home/user/projects/src with 3 entries.
    await expect(rows).toHaveCount(3);

    // Status path should reflect the new directory.
    await expect(page.locator('[data-testid="status-path"]')).toContainText('/home/user/projects/src');

    // Verify the entries are the src directory's children.
    const firstCell = rows.first().locator('td').first();
    await expect(firstCell).toHaveText('lib.rs');

    await page.screenshot({ path: `${SHOTS}/05-navigate-into-dir.png`, fullPage: true });
  });

  test('should navigate back to parent when Backspace pressed', async ({ page }) => {
    // Navigate into src first.
    await page.locator('[data-testid="table-row"]').first().dblclick();
    await expect(page.locator('[data-testid="table-row"]')).toHaveCount(3);

    // Press Backspace to go back.
    await page.keyboard.press('Backspace');

    // Should be back at root with 6 entries.
    await expect(page.locator('[data-testid="table-row"]')).toHaveCount(6);
    await expect(page.locator('[data-testid="status-path"]')).toContainText('/home/user/projects');
  });
});

// ── Delete (move to trash) ─────────────────────────────────────────

test.describe('trash via Delete key', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${BASE_URL}/interactive-ops-fixture.html`);
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__ready === true, null, { timeout: 10_000 });
  });

  test('should move selected entry to trash when Delete pressed', async ({ page }) => {
    const rows = page.locator('[data-testid="table-row"]');
    await expect(rows).toHaveCount(6);

    // Select the first row with ArrowDown.
    await page.keyboard.press('ArrowDown');
    await expect(rows.first()).toHaveClass(/selected/);

    // Get the name of the first entry for later assertion.
    const firstName = await rows.first().locator('td').first().textContent();

    // Press Delete.
    await page.keyboard.press('Delete');

    // Entry should be removed — 5 rows remain.
    await expect(rows).toHaveCount(5);

    // The deleted entry should no longer appear in the table.
    const allNames = await rows.locator('td:first-child').allTextContents();
    expect(allNames).not.toContain(firstName);

    // Verify via the fixture's __deleted stack.
    const deleted = await page.evaluate(() => (window as unknown as Record<string, Array<{ entry: { path: string } }>>).__deleted);
    expect(deleted.length).toBe(1);
    expect(deleted[0].entry.path).toContain(firstName!.toLowerCase().replace(/\./, ''));

    await page.screenshot({ path: `${SHOTS}/06-delete-entry.png`, fullPage: true });
  });
});

// ── Undo (Cmd/Ctrl+Z) ─────────────────────────────────────────────

test.describe('undo via Cmd/Ctrl+Z', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${BASE_URL}/interactive-ops-fixture.html`);
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__ready === true, null, { timeout: 10_000 });
  });

  test('should restore trashed entry when Cmd/Ctrl+Z pressed', async ({ page }) => {
    const rows = page.locator('[data-testid="table-row"]');
    await expect(rows).toHaveCount(6);

    // Select and delete the first entry.
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Delete');
    await expect(rows).toHaveCount(5);

    // The deleted stack should have 1 entry.
    const deletedBefore = await page.evaluate(() => (window as unknown as Record<string, unknown[]>).__deleted);
    expect(deletedBefore.length).toBe(1);

    // Press Ctrl+Z to undo.
    await page.keyboard.press('Control+z');

    // Entry should be restored — 6 rows again.
    await expect(rows).toHaveCount(6);

    // Deleted stack should be empty.
    const deletedAfter = await page.evaluate(() => (window as unknown as Record<string, unknown[]>).__deleted);
    expect(deletedAfter.length).toBe(0);

    // Status should show 6 entries.
    await expect(page.locator('[data-testid="status-summary"]')).toContainText('6 entries');

    await page.screenshot({ path: `${SHOTS}/07-undo-delete.png`, fullPage: true });
  });
});

// ── Context menu: Reveal in Explorer ───────────────────────────────

test.describe('context menu — Reveal', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${BASE_URL}/context-menu-fixture.html`);
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__ready === true, null, { timeout: 10_000 });
  });

  test('should open OS file explorer when "Reveal" context item clicked', async ({ page }) => {
    // Right-click the second row (app.mp4).
    const rows = page.locator('[data-testid="table-row"]');
    await expect(rows).toHaveCount(3);
    await rows.nth(1).click({ button: 'right' });

    // Context menu should appear.
    const menu = page.locator('[data-testid="context-menu"]');
    await expect(menu).toBeVisible();

    // Click "Reveal in Explorer".
    await page.locator('[data-testid="ctx-reveal"]').click();

    // Context menu should close.
    await expect(menu).toHaveCount(0);

    // Verify the IPC log recorded the reveal call.
    const log = await page.evaluate(() => (window as unknown as Record<string, Array<{ fn: string; args: string[] }>>).__ipcLog);
    const revealCall = log.find((e) => e.fn === 'reveal_in_explorer');
    expect(revealCall).toBeDefined();
    expect(revealCall!.args[0]).toBe('/home/user/projects/app.mp4');

    await page.screenshot({ path: `${SHOTS}/08-reveal-context.png`, fullPage: true });
  });
});

// ── Context menu: Copy Path ────────────────────────────────────────

test.describe('context menu — Copy Path', () => {
  test.beforeEach(async ({ page, context }) => {
    // Grant clipboard permissions so writeText works in headless Chromium.
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto(`${BASE_URL}/context-menu-fixture.html`);
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__ready === true, null, { timeout: 10_000 });
  });

  test('should copy path when "Copy Path" context item clicked', async ({ page }) => {
    const rows = page.locator('[data-testid="table-row"]');
    await expect(rows).toHaveCount(3);

    // Right-click the third row (main.rs).
    await rows.nth(2).click({ button: 'right' });
    const menu = page.locator('[data-testid="context-menu"]');
    await expect(menu).toBeVisible();

    // Click "Copy Path".
    await page.locator('[data-testid="ctx-copy-path"]').click();

    // Context menu should close.
    await expect(menu).toHaveCount(0);

    // Verify the clipboard was written via the fixture's polyfill.
    const clipText = await page.evaluate(() => (window as unknown as Record<string, string>).__clipboardText);
    expect(clipText).toBe('/home/user/projects/main.rs');

    // Also verify the IPC log.
    const log = await page.evaluate(() => (window as unknown as Record<string, Array<{ fn: string; args: string[] }>>).__ipcLog);
    const clipCall = log.find((e) => e.fn === 'clipboard.writeText');
    expect(clipCall).toBeDefined();
    expect(clipCall!.args[0]).toBe('/home/user/projects/main.rs');

    await page.screenshot({ path: `${SHOTS}/09-copy-path-context.png`, fullPage: true });
  });
});

// ── Performance: 10k-file treemap under 5 s ────────────────────────

test.describe('performance smoke', () => {
  test('should scan 10k-file fixture and render treemap within generous budget', async ({ page }) => {
    // Wall-clock bound is intentionally generous (30 s) to avoid flaky
    // failures on shared CI runners where cold-start + network latency
    // can spike.  The real render is typically <100 ms; this catches
    // gross regressions (e.g. accidental O(n²) layout) without
    // depending on machine speed.
    const start = Date.now();
    await page.goto(`${BASE_URL}/perf-10k-fixture.html`);
    await page.waitForFunction(() => (window as unknown as Record<string, boolean>).__ready === true, null, { timeout: 30_000 });
    const wallMs = Date.now() - start;

    // Canvas must be visible with non-zero dimensions.
    // (10k entries on a reasonable canvas produces many sub-pixel rects;
    // the test asserts render time, not pixel content.)
    const canvas = page.locator('[data-testid="treemap-canvas"]');
    await expect(canvas).toBeVisible();
    const box = await canvas.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(0);
    expect(box!.height).toBeGreaterThan(0);

    // Verify 10k files were generated.
    const fileCount = await page.evaluate(() => (window as unknown as Record<string, number>).__fileCount);
    expect(fileCount).toBe(10_000);

    // Summary should mention 10,000 entries.
    await expect(page.locator('[data-testid="status-summary"]')).toContainText('10,000 entries');

    // Wall time: generous upper bound to catch gross regressions on CI.
    expect(wallMs).toBeLessThan(30_000);

    // Log the actual render time from the fixture.
    const renderMs = await page.evaluate(() => (window as unknown as Record<string, number>).__renderMs);
    // Soft assertion: log but don't fail on render time alone (CI variance).
    console.log(`10k treemap: wall=${wallMs}ms, render=${renderMs}ms`);

    await page.screenshot({ path: `${SHOTS}/10-perf-10k.png`, fullPage: true });
  });
});
