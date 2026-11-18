import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_EMAIL = 'e2e-boards@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Boards CRUD', () => {
  test.beforeAll(async ({ request }) => {
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  test('shows My Boards heading when navigating to root', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    // Navigate to root which shows boards
    await page.goto('/');
    await page.waitForTimeout(500);
    
    await expect(page.getByRole('heading', { name: 'My Boards' })).toBeVisible();
  });

  test('shows Create Board button for logged in user', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    // Navigate to root which shows boards
    await page.goto('/');

    // Logged-in users always get the header's Create Board button (the
    // empty state adds a second, aria-labeled "Create your first board" CTA —
    // keep this strict, no .or()).
    await expect(page.getByRole('button', { name: 'Create Board', exact: true })).toBeVisible();
  });

  test('creates a board via Create Board button', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    // Navigate to root which shows boards
    await page.goto('/');
    await page.waitForTimeout(500);

    const boardName = `Test Board ${Date.now()}`;

    const emptyStateButton = page.getByRole('button', { name: 'Create your first board' });
    const createButton = page.getByRole('button', { name: 'Create Board' });

    if (await emptyStateButton.isVisible().catch(() => false)) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    const nameInput = page.getByLabel('Board name');
    await expect(nameInput).toBeVisible();
    await nameInput.fill(boardName);
    await page.getByRole('button', { name: 'Create' }).click();

    await expect(page.getByRole('main').getByText(boardName)).toBeVisible();
  });

  test('cancels board creation', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    // Navigate to root which shows boards
    await page.goto('/');
    await page.waitForTimeout(500);

    const emptyStateButton = page.getByRole('button', { name: 'Create your first board' });
    const createButton = page.getByRole('button', { name: 'Create Board' });

    if (await emptyStateButton.isVisible().catch(() => false)) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    const nameInput = page.getByLabel('Board name');
    await expect(nameInput).toBeVisible();
    await nameInput.fill('Should not create');

    await page.getByRole('button', { name: 'Cancel' }).click();
    await expect(nameInput).not.toBeVisible();
  });
});

test.describe('Board Archiving', () => {
  const boardNameToArchive = `Board to Archive ${Date.now()}`;
  test.beforeAll(async ({ request }) => {
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }

    // Create a board to archive
    const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const createRes = await request.post(`${E2E_API}/api/v1/boards`, {
      data: { name: boardNameToArchive },
      headers: { Cookie: cookies },
    });
    await createRes.json();
  });

  test('archives a board from board card', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/');
    await page.waitForTimeout(500);

    // Find the board and click the delete/archive button
    const boardCard = page.locator('.group').filter({ hasText: boardNameToArchive });
    await expect(boardCard).toBeVisible();

    // Click the delete button (trash icon) on the board card
    const deleteButton = boardCard.getByLabel(new RegExp(`Delete board ${boardNameToArchive}`));
    await deleteButton.click();

    // Verify the archive dialog appears
    await expect(page.getByRole('heading', { name: 'Archive board' })).toBeVisible();

    // Click Archive button
    await page.getByRole('button', { name: 'Archive' }).click();

    // Wait for the board to disappear from the main list
    await page.waitForTimeout(1000);
    await expect(boardCard).not.toBeVisible();
  });

  test('views archived boards', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/');
    await page.waitForTimeout(500);

    // Click on "Archived Boards" link
    const archivedBoardsLink = page.getByRole('link', { name: 'Archived Boards' });
    await archivedBoardsLink.click();

    // Verify we're on the archived boards page
    await expect(page.getByRole('heading', { name: 'Archived Boards' })).toBeVisible();

    // Verify the archived board is visible
    await expect(page.getByText(boardNameToArchive)).toBeVisible();
  });

  test('restores an archived board', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/archived-boards');
    await page.waitForTimeout(500);

    const restoreButton = page.getByRole('button', { name: /restore/i }).first();
    await expect(restoreButton).toBeVisible();
    await restoreButton.click();

    await page.waitForTimeout(1000);
  });

  test('permanently deletes an archived board', async ({ page, request }) => {
    // Create and archive a specific board for this test
    const boardName = `Board to Permanent Delete ${Date.now()}`;
    const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const createRes = await request.post(`${E2E_API}/api/v1/boards`, {
      data: { name: boardName },
      headers: { Cookie: cookies },
    });
    const boardData = await createRes.json();
    const boardId = boardData.data.id;

    // Archive the board
    await request.patch(`${E2E_API}/api/v1/boards/${boardId}`, {
      headers: { Cookie: cookies },
      data: { is_archived: true },
    });

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/archived-boards');
    await page.waitForTimeout(500);

    // The name also appears in the permanent-delete confirmation dialog
    // (and in the post-delete toast), so scope to the card's heading element
    // (h3) — unique per board, gone when the card unmounts.
    const boardNameLocator = page.getByRole('heading', { name: boardName });
    await expect(boardNameLocator).toBeVisible();
    const boardCard = boardNameLocator.locator('..');

    const deleteButton = boardCard.getByRole('button', { name: /delete/i });
    await deleteButton.click();

    // Verify the permanent delete dialog appears
    await expect(page.getByRole('heading', { name: 'Delete board permanently' })).toBeVisible();

    // Click Delete Permanently
    await page.getByRole('button', { name: 'Delete Permanently' }).click();

    // Wait for the board to disappear
    await page.waitForTimeout(1000);
    await expect(boardCard).not.toBeVisible();
  });
});
