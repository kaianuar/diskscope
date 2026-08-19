import { test, expect } from '@playwright/test';

// TEMPLATE e2e spec — adapt to YOUR app.
// GATE 3 runs this via tests/visual_gate.sh. Use it to assert core flows work
// end-to-end AND to capture screenshots for the vision review.
//
// Env: BASE_URL (server under test), SHOTS_DIR (where to save screenshots).
const BASE_URL = process.env.BASE_URL || 'http://localhost:5173';
const SHOTS = process.env.SHOTS_DIR || '/tmp/gate3_shots';

test.describe('app smoke + core flow', () => {
  test('loads and renders', async ({ page }) => {
    await page.goto(BASE_URL);
    // Expect SOME meaningful UI (adjust selector to your app).
    await expect(page.locator('body')).toBeVisible();
    await page.screenshot({ path: `${SHOTS}/01-load.png`, fullPage: true });
  });

  // Add your app's real flows here, e.g.:
  // test('add a task and see it', async ({ page }) => {
  //   await page.goto(BASE_URL);
  //   await page.getByPlaceholder('Add a task').fill('Milk');
  //   await page.getByRole('button', { name: 'Add' }).click();
  //   await expect(page.getByText('Milk')).toBeVisible();
  //   await page.screenshot({ path: `${SHOTS}/02-added.png`, fullPage: true });
  // });
  //
  // test('toggle a task', async ({ page }) => { ... });
  // test('delete a task', async ({ page }) => { ... });
  // test('share link opens in a second tab and is in sync', async ({ browser }) => { ... });
});
