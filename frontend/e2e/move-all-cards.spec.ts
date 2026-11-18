import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_EMAIL = 'e2e-move-all-cards@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Move All Cards', () => {
  let boardId: number;
  let column1Id: number;
  let column2Id: number;
  let column2Name: string;
  let cardTitle: string;

  test.beforeAll(async ({ request }) => {
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }

    // Login via API
    const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    // Create board
    const boardRes = await request.post(`${E2E_API}/api/v1/boards`, {
      data: { name: `Move All Cards Board ${Date.now()}` },
      headers: { Cookie: cookies },
    });
    const boardData = await boardRes.json();
    boardId = boardData.data.id;

    // Create column 1
    const col1Res = await request.post(`${E2E_API}/api/v1/boards/${boardId}/columns`, {
      data: { name: 'Source Column' },
      headers: { Cookie: cookies },
    });
    column1Id = (await col1Res.json()).data.id;

    // Create column 2
    column2Name = 'Target Column';
    const col2Res = await request.post(`${E2E_API}/api/v1/boards/${boardId}/columns`, {
      data: { name: column2Name },
      headers: { Cookie: cookies },
    });
    column2Id = (await col2Res.json()).data.id;

    // Create a card in column 1
    cardTitle = 'Moving Card';
    const cardRes = await request.post(`${E2E_API}/api/v1/cards`, {
      data: { title: cardTitle, column_id: column1Id, position: 0 },
      headers: { Cookie: cookies },
    });
    expect(cardRes.status()).toBe(201);
  });

  test('moves all cards from one column to another', async ({ page, request }) => {
    // Login via UI
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    // Navigate to board
    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Verify card is visible in column 1
    const column1 = page.locator(`[data-column-id="${column1Id}"]`);
    const cardInCol1 = column1.locator('div[role="button"][aria-label="Open card details"]', {
      hasText: new RegExp(cardTitle, 'i'),
    });
    await expect(cardInCol1).toBeVisible();

    // Verify card is NOT in column 2 initially
    const column2 = page.locator(`[data-column-id="${column2Id}"]`);
    const cardInCol2 = column2.locator('div[role="button"][aria-label="Open card details"]', {
      hasText: new RegExp(cardTitle, 'i'),
    });
    await expect(cardInCol2).not.toBeVisible();

    // Open column 1's header menu (MoreHorizontal button)
    // The MoreHorizontal button is the second <button> in the column (after column name)
    const menuButton = column1.locator('button').nth(1);
    await menuButton.click();
    await page.waitForTimeout(300);

    // Click "Move All Cards" to show the submenu
    const moveMenuButton = page.getByRole('button', { name: /move all cards/i });
    await moveMenuButton.click();
    await page.waitForTimeout(300);

    // Click the target column name in the submenu (scoped to avoid matching column header)
    await moveMenuButton.locator('..').getByRole('button', { name: column2Name }).click();
    await page.waitForTimeout(1000);

    // Verify success toast appears
    const successToast = page.locator('[data-testid="toast-success"]');
    await expect(successToast).toBeVisible({ timeout: 5000 });

    // Verify the card moved to column 2
    await expect(cardInCol2).toBeVisible();

    // Verify column 1 has no cards (the card div should not exist)
    await expect(
      column1.locator('div[role="button"][aria-label="Open card details"]'),
    ).toHaveCount(0);

    // Reload and verify persistence
    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    const col2AfterReload = page.locator(`[data-column-id="${column2Id}"]`);
    const persistedCard = col2AfterReload.locator('div[role="button"][aria-label="Open card details"]', {
      hasText: new RegExp(cardTitle, 'i'),
    });
    await expect(persistedCard).toBeVisible();

    const col1AfterReload = page.locator(`[data-column-id="${column1Id}"]`);
    await expect(
      col1AfterReload.locator('div[role="button"][aria-label="Open card details"]'),
    ).toHaveCount(0);

    // Verify via API that the card's column_id changed
    const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';
    const boardRes = await request.get(`${E2E_API}/api/v1/boards/${boardId}`, {
      headers: { Cookie: cookies },
    });
    const boardData = await boardRes.json();
    const columns = boardData.data.columns ?? [];
    const col2 = columns.find((c: { id: number }) => c.id === column2Id);
    expect(col2).toBeDefined();
  });
});
