import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_EMAIL = 'e2e-shell@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('App shell & navigation', () => {
  // Board fixture for the sidebar tests — the Projects sidebar was removed
  // (2026 Q4 app-layout rework); the living collapsible sidebar is the Notes
  // sidebar, which only exists on board pages. Assertion intent is unchanged:
  // "shell chrome is sane by default and toggleable on desktop + mobile".
  let boardId: number;

  test.beforeAll(async ({ request }) => {
    // Ensure test user exists via API
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
    // Login + create a board via API for /board/:id navigation
    const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';
    const boardRes = await request.post(`${E2E_API}/api/v1/boards`, {
      data: { name: `app-shell test board ${Date.now()}` },
      headers: { Cookie: cookies },
    });
    if (boardRes.status() !== 201) {
      throw new Error(`Failed to create test board: ${boardRes.status()}`);
    }
    const boardBody = await boardRes.json();
    boardId = boardBody.data.id;
  });

  test.beforeEach(async ({ page }) => {
    // Login before each test
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL((url) => url.pathname !== '/login', { timeout: 10000 });
  });

  test('renders header with app name', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'KanPad' })).toBeVisible();
  });

  test('renders user email in header', async ({ page }) => {
    await expect(page.getByText(TEST_EMAIL)).toBeVisible();
  });

  test('renders theme toggle button', async ({ page }) => {
    await expect(page.getByRole('button', { name: 'Toggle theme' })).toBeVisible();
  });

  test('sidebar is collapsed by default', async ({ page }) => {
    await page.goto(`/board/${boardId}`);
    const sidebar = page.locator('aside');
    await expect(sidebar).toHaveClass(/w-0/);
  });

  test('sidebar expands and collapses via toggle buttons', async ({ page }) => {
    await page.goto(`/board/${boardId}`);
    const sidebar = page.locator('aside');

    // Collapsed by default
    await expect(sidebar).toHaveClass(/w-0/);

    // Expand via the floating "Show notes sidebar" button (only rendered when collapsed)
    await page.getByRole('button', { name: 'Show notes sidebar' }).click();
    await expect(sidebar).toHaveClass(/w-\[280px\]/);
    // Let the 300ms width transition settle — clicking the header toggle
    // mid-animation loses the race against React's re-render
    await page.waitForTimeout(350);
    await expect(sidebar.getByText('Notes', { exact: true })).toBeVisible();

    // Collapse via the sidebar header toggle
    await sidebar.getByRole('button', { name: 'Toggle notes sidebar' }).click();
    await expect(sidebar).toHaveClass(/w-0/);
  });

  test('notes sidebar shows empty state when board has no notes', async ({ page }) => {
    await page.goto(`/board/${boardId}`);
    await page.getByRole('button', { name: 'Show notes sidebar' }).click();
    await expect(page.getByText('No notes for this board')).toBeVisible();
  });

  test('header toggle button opens the notes sheet on mobile', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto(`/board/${boardId}`);

    const openButton = page.locator('header').getByRole('button', { name: 'Open sidebar' });
    await expect(openButton).toBeVisible();

    await openButton.click();
    const sheet = page.getByRole('dialog');
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole('heading', { name: 'Notes' })).toBeVisible();
  });

  test('theme toggle switches between light and dark', async ({ page }) => {
    const html = page.locator('html');
    const themeToggle = page.getByRole('button', { name: 'Toggle theme' });

    // Should start in light mode
    await expect(html).toHaveClass(/light/);

    // Switch to dark
    await themeToggle.click();
    await expect(html).toHaveClass(/dark/);

    // Switch back to light
    await themeToggle.click();
    await expect(html).toHaveClass(/light/);
  });

  test('user dropdown shows logout option', async ({ page }) => {
    await page.getByRole('button', { name: TEST_EMAIL }).click();
    await expect(page.getByRole('menuitem', { name: 'Log out' })).toBeVisible();
  });

  test('renders main content area', async ({ page }) => {
    await page.goto('/');
    if (page.url().includes('/login')) {
      await page.getByLabel('Email').fill(TEST_EMAIL);
      await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
      await page.getByRole('button', { name: 'Sign In' }).click();
      await page.waitForURL((url) => url.pathname !== '/login', { timeout: 10000 });
    }

    const boardsHeading = page.getByRole('heading', { name: 'My Boards' });
    const projectsHeading = page.getByRole('heading', { name: 'My Projects' });

    await expect(boardsHeading.or(projectsHeading)).toBeVisible();
  });
});
