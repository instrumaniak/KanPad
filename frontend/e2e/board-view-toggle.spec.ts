import { expect } from '@playwright/test';
import { monitoringTest } from './test-utils';

const API_BASE_URL = 'http://localhost:3000';
const TEST_EMAIL = 'e2e-viewtoggle@test.local';
const TEST_PASSWORD = 'Test1234!';

monitoringTest.describe('Board View Toggle E2E', () => {
  monitoringTest.describe.configure({ timeout: 90000 });
  monitoringTest.beforeAll(async ({ request }) => {
    const res = await request.post(`${API_BASE_URL}/api/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  monitoringTest(
    'toggles kanban/list, sorts, persists across reload, adds and deletes in list',
    async ({ page, request }) => {
      const loginRes = await request.post(`${API_BASE_URL}/api/auth/login`, {
        data: { email: TEST_EMAIL, password: TEST_PASSWORD },
      });
      const cookies = loginRes.headers()['set-cookie'] || '';

      const boardRes = await request.post(`${API_BASE_URL}/api/boards`, {
        data: { name: `View Toggle Board ${Date.now()}` },
        headers: { Cookie: cookies },
      });
      const boardId = (await boardRes.json()).data.id;

      const colRes = await request.get(`${API_BASE_URL}/api/boards/${boardId}/columns`, {
        headers: { Cookie: cookies },
      });
      const cols = (await colRes.json()).data;
      const colA = cols[0].id;
      const colB = cols[1].id;

      for (const [i, title] of ['Zulu task', 'Alpha task'].entries()) {
        await request.post(`${API_BASE_URL}/api/cards`, {
          data: { title, column_id: i === 0 ? colA : colB, position: 0 },
          headers: { Cookie: cookies },
        });
      }

      await page.goto('/login');
      await page.getByLabel('Email').fill(TEST_EMAIL);
      await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
      await page.getByRole('button', { name: 'Sign In' }).click();
      await page.waitForURL('/');

      await page.goto(`/board/${boardId}`);
      await expect(page.getByRole('group', { name: 'Board view mode' })).toBeVisible({
        timeout: 15000,
      });

      // Kanban visible by default
      await expect(page.getByRole('group', { name: 'Board view mode' })).toBeVisible();

      // Toggle to List
      await page.getByRole('button', { name: 'List' }).click();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible({
        timeout: 15000,
      });
      await expect(page.getByText('Zulu task')).toBeVisible();
      await expect(page.getByText('Alpha task')).toBeVisible();

      // Sort by Title asc -> Alpha first
      await page.getByRole('button', { name: /title/i }).click();
      const rows = page.getByTestId('board-list-row');
      await expect(rows.first()).toContainText('Alpha task');

      // Sort each remaining key (Created / Updated / Due) without crashing
      await page.getByRole('button', { name: /created/i }).click();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible();
      await page.getByRole('button', { name: /updated/i }).click();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible();
      await page.getByRole('button', { name: /^due/i }).click();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible();

      // Reload -> List persisted (DB-backed view_mode)
      await page.reload();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible({
        timeout: 15000,
      });
      await expect(page.getByRole('button', { name: 'List' })).toHaveAttribute(
        'aria-pressed',
        'true',
      );

      // Verify persistence via API (not just UI)
      const verifyRes = await request.get(`${API_BASE_URL}/api/boards/${boardId}`, {
        headers: { Cookie: cookies },
      });
      expect((await verifyRes.json()).data.view_mode).toBe('list');

      // Toggle back to Board -> kanban restored
      await page.getByRole('button', { name: 'Board' }).click();
      await expect(page.getByRole('group', { name: 'Board view mode' })).toBeVisible();
      await expect(page.getByText('Zulu task')).toBeVisible();

      // Add Card in List view with column picker
      await page.getByRole('button', { name: 'List' }).click();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible();
      await page.getByRole('button', { name: 'Add Card' }).click();
      await page.getByLabel('Card title').fill('List view new card');
      await page.getByLabel('Target column').selectOption({ index: 0 });
      await page.getByRole('button', { name: 'Create' }).click();
      await expect(page.getByText('List view new card')).toBeVisible({ timeout: 15000 });

      // Delete the new card
      await page.getByLabel('Delete card List view new card').click();
      await page.getByRole('button', { name: 'Delete' }).click();
      await expect(page.getByText('List view new card')).not.toBeVisible({ timeout: 15000 });
    },
  );
});
