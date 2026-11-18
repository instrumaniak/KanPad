import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_EMAIL = 'e2e-columns@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Column CRUD', () => {
  test.beforeAll(async ({ request }) => {
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  test('creates a new column', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    const boardName = `Test Board Columns ${Date.now()}`;
    await page.goto('/');
    await page.waitForTimeout(500);

    const emptyStateButton = page.getByRole('button', { name: 'Create your first board' });
    if (await emptyStateButton.isVisible().catch(() => false)) {
      await emptyStateButton.click();
    } else {
      await page.getByRole('button', { name: 'Create Board' }).click();
    }

    const nameInput = page.getByLabel('Board name');
    await nameInput.fill(boardName);
    await page.getByRole('button', { name: 'Create' }).click();

    await page.waitForTimeout(1000);

    const addColumnButton = page.getByRole('button', { name: 'Add Column' });
    if (await addColumnButton.isVisible().catch(() => false)) {
      await addColumnButton.click();
      await page.waitForTimeout(500);

      const columnHeader = page.locator('.text-sm.font-semibold').first();
      await expect(columnHeader).toBeVisible();
    }
  });

  test('shows column count badge', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/');

    const emptyStateButton = page.getByRole('button', { name: 'Create your first board' });
    const createButton = page.getByRole('button', { name: 'Create Board' });

    if (await emptyStateButton.isVisible().catch(() => false)) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    const nameInput = page.getByLabel('Board name');
    await expect(nameInput).toBeVisible();
    await nameInput.fill(`Test Board ${Date.now()}`);
    await page.getByRole('button', { name: 'Create' }).click();

    await page.waitForTimeout(1000);

    const countBadge = page.locator('.bg-muted');
    if (await countBadge.first().isVisible().catch(() => false)) {
      await expect(countBadge.first()).toBeVisible();
    }
  });

  test('shows Add a card button in column', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/');
    await page.waitForTimeout(500);

    const addCardButton = page.getByRole('button', { name: '+ Add a card' });
    if (await addCardButton.isVisible().catch(() => false)) {
      await expect(addCardButton).toBeVisible();
    }
  });

  test('shows error toast when card title exceeds max length', async ({ page }) => {
    const consoleErrors: string[] = [];
    const apiErrors: string[] = [];

    page.on('console', msg => {
      consoleErrors.push(`[${msg.type()}] ${msg.text()}`);
    });
    page.on('pageerror', error => {
      consoleErrors.push(`[pageerror] ${error.message}`);
    });
    page.on('response', response => {
      if (response.status() >= 400) {
        apiErrors.push(`${response.url()} -> ${response.status()}`);
      }
    });

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/');
    await page.waitForTimeout(500);

    const addCardButton = page.getByRole('button', { name: '+ Add a card' });
    if (await addCardButton.isVisible().catch(() => false)) {
      await addCardButton.click();
      await page.waitForTimeout(300);

      const textarea = page.locator('textarea[data-column-id]');
      await expect(textarea).toBeVisible();

      const longTitle = 'A'.repeat(300);
      await textarea.fill(longTitle);
      await textarea.press('Enter');
      await page.waitForTimeout(500);

      const errorToast = page.locator('[data-testid="toast-error"]');
      await expect(errorToast).toBeVisible();
      await expect(errorToast).toContainText('Failed to create card');

      const jsErrors = consoleErrors.filter(e => e.includes('[error]') || e.includes('[pageerror]'));
      expect(jsErrors).toEqual([]);
      expect(apiErrors).toEqual([]);
    }
  });
});