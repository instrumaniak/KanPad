import { test, expect } from '@playwright/test';

const TEST_EMAIL = 'e2e-shell@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('App shell & navigation', () => {
  test.beforeAll(async ({ request }) => {
    // Ensure test user exists via API
    const res = await request.post('http://localhost:3000/api/auth/register', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
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
    const sidebar = page.locator('aside');
    await expect(sidebar).toHaveClass(/w-0/);
  });

  test('sidebar expands and collapses via toggle button', async ({ page }) => {
    const sidebar = page.locator('aside');
    const headerToggle = page.locator('header').getByRole('button', { name: 'Toggle sidebar' });

    // Collapsed by default
    await expect(sidebar).toHaveClass(/w-0/);

    // Expand via header toggle
    await headerToggle.click();
    await expect(sidebar).toHaveClass(/w-\[240px\]/);
    await expect(sidebar.getByText('Projects', { exact: true })).toBeVisible();

    // Collapse via sidebar toggle (now visible)
    const sidebarToggle = sidebar.getByRole('button', { name: 'Toggle sidebar' });
    await sidebarToggle.click();
    await expect(sidebar).toHaveClass(/w-0/);
  });

  test('sidebar shows "No projects yet" when empty', async ({ page }) => {
    const headerToggle = page.locator('header').getByRole('button', { name: 'Toggle sidebar' });

    await headerToggle.click();
    await expect(page.getByText('No projects yet')).toBeVisible();
  });

  test('header toggle button opens sidebar on mobile', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });

    const headerToggle = page.locator('header').getByRole('button', { name: 'Toggle sidebar' });
    await expect(headerToggle).toBeVisible();

    const sidebar = page.locator('aside');
    await headerToggle.click();
    await expect(sidebar).toHaveClass(/w-\[240px\]/);
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
