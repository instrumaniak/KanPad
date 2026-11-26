import { expect } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

// T021/T051 - virtualization + pagination smoke.
// T050 changed column embed to 10 initial + 20/page infinite scroll. These tests
// preserve intent under paged loading: bounded DOM, keyboard DnD across virtual
// boundary with paged data loaded, and list-view virtualization.
const TEST_EMAIL = 'e2e-virtualization@test.local';
const TEST_PASSWORD = 'Test1234!';
const KEYBOARD_BOARD_CARDS = 120;
const SMOKE_BOARD_CARDS = 60; // > initial page, enough for bounded-DOM smoke under pagination

monitoringTest.describe('Virtualized lists (T021/T051)', () => {
  monitoringTest.setTimeout(240_000);

  monitoringTest.beforeAll(async ({ request }) => {
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  monitoringTest.beforeEach(async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');
  });

  async function createBoard(page: import('@playwright/test').Page, name: string) {
    const res = await page.request.post(`${E2E_API}/api/v1/boards`, { data: { name } });
    const body = await res.json();
    return body.data.id as number;
  }

  async function firstColumnId(page: import('@playwright/test').Page, boardId: number) {
    const res = await page.request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`);
    const body = await res.json();
    const columns = body.data ?? body;
    return columns[0].id as number;
  }

  async function seedCards(
    page: import('@playwright/test').Page,
    columnId: number,
    count: number,
    titlePrefix: string,
  ) {
    for (let i = 0; i < count; i += 1) {
      const res = await page.request.post(`${E2E_API}/api/v1/cards`, {
        data: { title: `${titlePrefix} ${String(i + 1).padStart(3, '0')}`, column_id: columnId },
      });
      if (!res.ok()) {
        throw new Error(`Failed to seed card ${i + 1}: ${res.status()}`);
      }
    }
  }

  async function loadAllColumnCards(
    page: import('@playwright/test').Page,
    column: import('@playwright/test').Locator,
    columnId: number,
    expectedTotal: number,
  ) {
    const scrollContainer = column.locator('[data-testid="column-card-list-scroll"]');
    const sentinel = column.locator('[data-testid="column-cards-sentinel"]');
    for (let i = 0; i < 40; i += 1) {
      if ((await sentinel.count()) === 0) break;
      const beforeCount = await page.request
        .get(`${E2E_API}/api/v1/columns/${columnId}/cards?page=1&limit=1`)
        .then(async (r) => (await r.json()).total as number)
        .catch(() => expectedTotal);
      void beforeCount;
      const respPromise = page.waitForResponse(
        (r) =>
          r.request().method() === 'GET' &&
          r.url().includes(`/api/v1/columns/${columnId}/cards`) &&
          /page=\d+/.test(r.url()) &&
          r.status() === 200,
        { timeout: 15000 },
      );
      await scrollContainer.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      await respPromise;
    }
    await expect.poll(async () => await sentinel.count(), { timeout: 15000 }).toBe(0);
  }

  async function fetchAllColumnCardTitles(
    page: import('@playwright/test').Page,
    columnId: number,
  ): Promise<string[]> {
    const titles: string[] = [];
    for (let pageNum = 1; pageNum <= 20; pageNum += 1) {
      const res = await page.request.get(`${E2E_API}/api/v1/columns/${columnId}/cards?page=${pageNum}&limit=100`);
      if (!res.ok()) throw new Error(`Failed to fetch cards page ${pageNum}: ${res.status()}`);
      const body = await res.json();
      const data = body.data as Array<{ title: string }>;
      titles.push(...data.map((c) => c.title));
      if (titles.length >= (body.total as number)) break;
    }
    return titles;
  }

  monitoringTest('keyboard drag progresses stepwise across a 120-card virtual window and persists', async ({ page, monitoring }) => {
    const boardId = await createBoard(page, `T021 Keyboard ${Date.now()}`);
    const columnId = await firstColumnId(page, boardId);
    await seedCards(page, columnId, KEYBOARD_BOARD_CARDS, 'Keyboard card');

    await page.goto(`/board/${boardId}`);
    await expect(page.locator('[data-column-id]').first()).toBeVisible({ timeout: 15000 });

    const firstColumn = page.locator('[data-column-id]').first();
    const rowCount = await firstColumn.locator('[data-testid="virtual-card-row"]').count();
    expect(rowCount).toBeGreaterThan(0);
    expect(rowCount).toBeLessThan(40);

    // Load all pages so keyboard drag can traverse the virtual boundary with data present.
    await loadAllColumnCards(page, firstColumn, columnId, KEYBOARD_BOARD_CARDS);

    const scrollContainer = firstColumn.locator('[data-testid="column-card-list-scroll"]');
    await scrollContainer.evaluate((el) => { el.scrollTop = 0; });
    await expect(firstColumn.locator('[data-testid="virtual-card-row"]').first()).toContainText('Keyboard card 001');

    const sortable = firstColumn.locator('div[aria-roledescription="sortable"]').first();
    await sortable.focus();
    await expect(sortable).toBeFocused();
    await page.keyboard.press('Space');
    await expect(firstColumn.locator('div[aria-roledescription="sortable"]').first()).toBeVisible();

    for (let i = 0; i < 24; i += 1) {
      await page.keyboard.press('ArrowDown');
    }

    const focusedRole = await page.evaluate(() => document.activeElement?.getAttribute('aria-roledescription'));
    expect(focusedRole).toBe('sortable');
    const focusedText = await page.evaluate(() => document.activeElement?.textContent ?? '');
    expect(focusedText).toContain('Keyboard card 001');

    const patchDone = page.waitForResponse(
      (r) => r.request().method() === 'PATCH' && /\/api\/v1\/cards\/\d+/.test(r.url()),
      { timeout: 15000 },
    );
    await page.keyboard.press('Space');
    await patchDone;

    const titles = await fetchAllColumnCardTitles(page, columnId);
    expect(titles[24]).toBe('Keyboard card 001');

    await page.reload();
    await expect(page.locator('[data-column-id]').first()).toBeVisible({ timeout: 15000 });
    await loadAllColumnCards(page, page.locator('[data-column-id]').first(), columnId, KEYBOARD_BOARD_CARDS);
    const titlesAfterReload = await fetchAllColumnCardTitles(page, columnId);
    expect(titlesAfterReload[24]).toBe('Keyboard card 001');

    assertNoErrors(monitoring);
  });

  monitoringTest('paged board renders a bounded DOM window (P3 rendering half, paged)', async ({ page, monitoring }) => {
    const boardId = await createBoard(page, `T021 Smoke ${Date.now()}`);
    const columnId = await firstColumnId(page, boardId);
    await seedCards(page, columnId, SMOKE_BOARD_CARDS, 'Smoke card');

    await page.goto(`/board/${boardId}`);
    await expect(page.locator('[data-column-id]').first()).toBeVisible({ timeout: 15000 });

    const firstColumn = page.locator('[data-column-id]').first();
    // Initial page is 10 (T050); DOM remains bounded.
    await expect(firstColumn.locator('[data-testid="virtual-card-row"]').first()).toBeVisible();
    const initialRowCount = await firstColumn.locator('[data-testid="virtual-card-row"]').count();
    expect(initialRowCount).toBeGreaterThan(0);
    expect(initialRowCount).toBeLessThanOrEqual(30);

    // Load remaining pages; DOM window should remain bounded.
    await loadAllColumnCards(page, firstColumn, columnId, SMOKE_BOARD_CARDS);
    const loadedRowCount = await firstColumn.locator('[data-testid="virtual-card-row"]').count();
    expect(loadedRowCount).toBeGreaterThan(0);
    expect(loadedRowCount).toBeLessThanOrEqual(30);
    console.log(`[T021/T051] ${SMOKE_BOARD_CARDS}-card board: mounted rows after paged load=${loadedRowCount}`);

    // List view: paged + virtualized, bounded DOM.
    await page.getByRole('button', { name: 'List' }).click();
    await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible({ timeout: 15000 });
    const listRows = page.locator('[data-testid="board-list-row"]');
    await expect(listRows.first()).toBeVisible();
    const listRowCount = await listRows.count();
    expect(listRowCount).toBeGreaterThan(0);
    expect(listRowCount).toBeLessThanOrEqual(30);

    // Load all list pages through sentinel/scroll.
    const listScroll = page.locator('[data-testid="board-list-scroll"]');
    const listSentinel = page.locator('[data-testid="board-list-sentinel"]');
    for (let i = 0; i < 20; i += 1) {
      if ((await listSentinel.count()) === 0) break;
      const respPromise = page.waitForResponse(
        (r) => r.request().method() === 'GET' && r.url().includes(`/api/v1/boards/${boardId}/cards`) && /page=\d+/.test(r.url()) && r.status() === 200,
        { timeout: 15000 },
      );
      await listScroll.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      await respPromise;
    }
    await expect.poll(async () => await listSentinel.count(), { timeout: 15000 }).toBe(0);
    const finalListRowCount = await page.locator('[data-testid="board-list-row"]').count();
    expect(finalListRowCount).toBeGreaterThan(0);
    expect(finalListRowCount).toBeLessThanOrEqual(30);
    console.log(`[T021/T051] list view: mounted rows=${finalListRowCount}`);

    assertNoErrors(monitoring);
  });
});
