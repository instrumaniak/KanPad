import { expect } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

const API_BASE_URL = E2E_API;
const TEST_EMAIL = 'e2e-cardsearch@test.local';
const TEST_PASSWORD = 'Test1234!';

monitoringTest.describe('Card Search E2E', () => {
  monitoringTest.describe.configure({ timeout: 90000 });
  monitoringTest.beforeAll(async ({ request }) => {
    const res = await request.post(`${API_BASE_URL}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  monitoringTest(
    'searches cards by title, clears, shows empty state, preserves across list toggle',
    async ({ page, request, monitoring }) => {
      const loginRes = await request.post(`${API_BASE_URL}/api/v1/auth/login`, {
        data: { email: TEST_EMAIL, password: TEST_PASSWORD },
      });
      const cookies = loginRes.headers()['set-cookie'] || '';

      const boardRes = await request.post(`${API_BASE_URL}/api/v1/boards`, {
        data: { name: `Card Search Board ${Date.now()}` },
        headers: { Cookie: cookies },
      });
      const boardId = (await boardRes.json()).data.id;

      const colResA = await request.post(`${API_BASE_URL}/api/v1/boards/${boardId}/columns`, {
        data: { name: 'To Do', board_id: boardId },
        headers: { Cookie: cookies },
      });
      const colResB = await request.post(`${API_BASE_URL}/api/v1/boards/${boardId}/columns`, {
        data: { name: 'Done', board_id: boardId },
        headers: { Cookie: cookies },
      });
      const colA = (await colResA.json()).data.id;
      const colB = (await colResB.json()).data.id;

      const seed: [string, number][] = [
        ['Alpha task', colA],
        ['Beta task', colB],
        ['Zulu task', colA],
      ];
      for (const [title, column_id] of seed) {
        await request.post(`${API_BASE_URL}/api/v1/cards`, {
          data: { title, column_id, position: 0 },
          headers: { Cookie: cookies },
        });
      }

      await page.goto('/login');
      await page.getByLabel('Email').fill(TEST_EMAIL);
      await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
      await page.getByRole('button', { name: 'Sign In' }).click();
      await page.waitForURL('/');

      await page.goto(`/board/${boardId}`);
      const search = page.getByLabel('Search cards');
      await expect(search).toBeVisible({ timeout: 15000 });
      await expect(page.getByText('Alpha task')).toBeVisible();
      await expect(page.getByText('Beta task')).toBeVisible();
      await expect(page.getByText('Zulu task')).toBeVisible();

      const searchResp = (q: string) =>
        page.waitForResponse(
          (r) =>
            r.request().method() === 'GET' &&
            r.url().includes(`/api/v1/boards/${boardId}/columns`) &&
            r.status() === 200 &&
            (q ? r.url().includes(`search=${encodeURIComponent(q)}`) : !r.url().includes('search=')),
          { timeout: 15000 },
        );

      let resp = searchResp('Al');
      await search.fill('Al');
      await resp;
      await expect(page.getByText('Alpha task')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Beta task')).not.toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Zulu task')).not.toBeVisible({ timeout: 5000 });

      // "A" is < 2 chars: server filter not applied; may resolve from fresh cache (staleTime 5min).
      await search.fill('A');
      await expect(page.getByText('Alpha task')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Beta task')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Zulu task')).toBeVisible({ timeout: 5000 });

      // Clear to empty: default unfiltered key may be served from fresh cache.
      await search.fill('');
      await expect(page.getByText('Alpha task')).toBeVisible({ timeout: 5000 });

      resp = searchResp('zzz-no-match');
      await search.fill('zzz-no-match');
      await resp;
      await expect(page.getByText('No cards found')).toBeVisible({ timeout: 5000 });
      // Clear search button: default unfiltered key may be served from fresh cache.
      await page.getByText('Clear search', { exact: true }).click();
      await expect(page.getByText('Alpha task')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Beta task')).toBeVisible({ timeout: 5000 });

      await page.getByRole('button', { name: 'List' }).click();
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible({
        timeout: 15000,
      });
      const listSearchResp = (q: string) =>
        page.waitForResponse(
          (r) =>
            r.request().method() === 'GET' &&
            r.url().includes(`/api/v1/boards/${boardId}/cards`) &&
            r.status() === 200 &&
            (q ? r.url().includes(`search=${encodeURIComponent(q)}`) : !r.url().includes('search=')),
          { timeout: 15000 },
        );
      resp = listSearchResp('Beta');
      await search.fill('Beta');
      await resp;
      await expect(page.getByText('Beta task')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Alpha task')).not.toBeVisible({ timeout: 5000 });
      // Clear in list view: default unfiltered list key may be served from fresh cache.
      await search.fill('');
      await expect(page.getByText('Alpha task')).toBeVisible({ timeout: 5000 });

      assertNoErrors(monitoring);
    },
  );
});
