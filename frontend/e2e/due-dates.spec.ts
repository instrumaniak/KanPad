import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const API_BASE_URL = E2E_API;
const TEST_EMAIL = 'e2e-duedates@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Due Dates E2E', () => {
  test.beforeAll(async ({ request }) => {
    const res = await request.post(`${API_BASE_URL}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  test('sets, displays, and clears a due date', async ({ page, request }) => {
    const loginRes = await request.post(`${API_BASE_URL}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const boardRes = await request.post(`${API_BASE_URL}/api/v1/boards`, {
      data: { name: `Due Date Board ${Date.now()}` },
      headers: { Cookie: cookies },
    });
    const boardId = (await boardRes.json()).data.id;

    const columnRes = await request.post(`${API_BASE_URL}/api/v1/boards/${boardId}/columns`, {
      data: { name: 'To Do' },
      headers: { Cookie: cookies },
    });
    const columnId = (await columnRes.json()).data.id;

    await request.post(`${API_BASE_URL}/api/v1/cards`, {
      data: { title: 'Due Date Test Card', column_id: columnId, position: 0 },
      headers: { Cookie: cookies },
    });

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    const cardButton = page.locator('div[role="button"][aria-label="Open card details"]', { hasText: /Due Date Test Card/i });
    await expect(cardButton).toBeVisible();
    await cardButton.click();
    await expect(page.getByRole('dialog', { name: /card details/i })).toBeVisible();

    const setDueDateButton = page.getByRole('button', { name: /set due date/i });
    await expect(setDueDateButton).toBeVisible();
    await setDueDateButton.click();

    const calendarGrid = page.locator('[data-slot="calendar"]');
    await expect(calendarGrid).toBeVisible();

    const dayButton = page.getByRole('button', { name: /November 15th, 2026/ });
    await dayButton.focus();
    await dayButton.press('Enter');

    await page.waitForTimeout(500);
    await expect(page.getByRole('dialog', { name: /card details/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /clear due date/i })).toBeVisible();

    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);

    const cardBadge = cardButton.locator('span').filter({ hasText: /2026/ });
    await expect(cardBadge).toBeVisible();

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    const persistedCardButton = page.locator('div[role="button"][aria-label="Open card details"]', { hasText: /Due Date Test Card/i });
    await expect(persistedCardButton).toBeVisible();

    const persistedBadge = persistedCardButton.locator('span').filter({ hasText: /2026/ });
    await expect(persistedBadge).toBeVisible();

    await persistedCardButton.click();
    await expect(page.getByRole('dialog', { name: /card details/i })).toBeVisible();

    const clearButton = page.getByRole('button', { name: /clear due date/i });
    await expect(clearButton).toBeVisible();
    await clearButton.click();

    await page.waitForTimeout(500);
    const triggerButton = page.getByRole('button', { name: /set due date/i });
    await expect(triggerButton).toBeVisible();
    await expect(triggerButton).toContainText(/no due date/i);

    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);

    await expect(persistedBadge).not.toBeVisible();
  });
});
