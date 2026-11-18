import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_EMAIL = 'e2e-checklists@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Checklist Feature', () => {
  let boardId: number;
  let columnId: number;
  let cardId: number;

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
      data: { name: `Checklist Test Board ${Date.now()}` },
      headers: { Cookie: cookies },
    });
    const boardData = await boardRes.json();
    boardId = boardData.data.id;

    // Create column
    const columnRes = await request.post(`${E2E_API}/api/v1/boards/${boardId}/columns`, {
      data: { name: 'To Do' },
      headers: { Cookie: cookies },
    });
    const columnData = await columnRes.json();
    columnId = columnData.data.id;

    // Create card
    const cardRes = await request.post(`${E2E_API}/api/v1/cards`, {
      data: { title: 'Checklist Card', column_id: columnId, position: 0 },
      headers: { Cookie: cookies },
    });
    const cardData = await cardRes.json();
    cardId = cardData.data.id;
  });

  test('creates a checklist, adds items, toggles completion, and persists', async ({ page, request }) => {
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

    // Click on card to open detail panel
    const cardButton = page.locator('div[role="button"][aria-label="Open card details"]', {
      hasText: /Checklist Card/i,
    });
    await expect(cardButton).toBeVisible();
    await cardButton.click();
    await expect(page.getByRole('dialog')).toBeVisible();

    // Verify "No checklists yet" and Add Checklist button
    await expect(page.getByText('No checklists yet')).toBeVisible();
    await expect(page.getByRole('button', { name: /add checklist/i })).toBeVisible();

    // Click "+ Add Checklist"
    await page.getByRole('button', { name: /add checklist/i }).click();

    // The form appears with an input and submit button
    const titleInput = page.getByPlaceholder('Checklist title...');
    await expect(titleInput).toBeVisible();
    await titleInput.clear();
    await titleInput.fill('My Checklist');

    // Click Add
    await page.getByRole('button', { name: 'Add', exact: true }).click();
    await page.waitForTimeout(1000);

    // Verify checklist appears with its title
    const checklistTitle = page.getByText('My Checklist');
    await expect(checklistTitle).toBeVisible();

    // Verify progress shows 0/0 (0%)
    await expect(page.getByRole('dialog').getByText('0/0 (0%)')).toBeVisible();

    // Click "+ Add item"
    await page.getByRole('button', { name: /add item/i }).click();

    // Type first item and submit
    const itemInput = page.getByPlaceholder('Add an item...');
    await expect(itemInput).toBeVisible();
    await itemInput.fill('First item');
    await page.getByRole('button', { name: 'Add', exact: true }).click();
    await page.waitForTimeout(500);

    // Verify item appears
    await expect(page.getByText('First item')).toBeVisible();

    // Add second item (input stays focused, text resets)
    await itemInput.fill('Second item');
    await page.getByRole('button', { name: 'Add', exact: true }).click();
    await page.waitForTimeout(500);

    await expect(page.getByText('Second item')).toBeVisible();

    // Verify progress shows 0/2 (0%) (add-item form stays open, doesn't block visibility)
    await expect(page.getByRole('dialog').getByText('0/2 (0%)')).toBeVisible();
    await expect(page.getByRole('dialog').getByLabel('Checklist progress: 0/2 (0%)')).toBeVisible();

    // Toggle first item completion - click its checkbox
    const checkboxes = page.getByRole('checkbox');
    await expect(checkboxes).toHaveCount(2);
    await checkboxes.nth(0).click();
    await page.waitForTimeout(500);

    // Verify progress bar updates to 1/2 (50%)
    await expect(page.getByRole('dialog').getByText('1/2 (50%)')).toBeVisible();
    await expect(page.getByRole('dialog').getByLabel('Checklist progress: 1/2 (50%)')).toBeVisible();

    // Close panel
    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);

    // Reload and verify persistence
    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Click card again to open panel
    const cardAfterReload = page.locator('div[role="button"][aria-label="Open card details"]', {
      hasText: /Checklist Card/i,
    });
    await expect(cardAfterReload).toBeVisible();
    await cardAfterReload.click();
    await expect(page.getByRole('dialog')).toBeVisible();

    // Verify checklist still exists with title
    await expect(page.getByText('My Checklist')).toBeVisible();

    // Verify progress persists at 1/2 (50%)
    await expect(page.getByRole('dialog').getByText('1/2 (50%)')).toBeVisible();
    await expect(page.getByRole('dialog').getByLabel('Checklist progress: 1/2 (50%)')).toBeVisible();

    // Verify the first item checkbox is checked
    const persistedCheckboxes = page.getByRole('checkbox');
    await expect(persistedCheckboxes.nth(0)).toBeChecked();
    await expect(persistedCheckboxes.nth(1)).not.toBeChecked();

    // Verify persistence via API - use single card endpoint which returns checklists
    const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';
    const cardRes = await request.get(`${E2E_API}/api/v1/cards/${cardId}`, {
      headers: { Cookie: cookies },
    });
    const cardData = await cardRes.json();
    expect(cardData.data.checklists).toBeDefined();
    expect(cardData.data.checklists.length).toBeGreaterThanOrEqual(1);
    expect(cardData.data.checklists[0].title).toBe('My Checklist');
    const completedItems = cardData.data.checklists[0].items.filter(
      (item: { is_completed: boolean }) => item.is_completed,
    );
    expect(completedItems.length).toBe(1);
  });
});
