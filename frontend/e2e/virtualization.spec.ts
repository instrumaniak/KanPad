import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

// T021 — virtualization + render-perf smoke.
// Seeds throwaway boards via the API (never the UI), then checks:
//   1. a 120-card column keyboard-drags across the virtual window boundary
//      (source row stays mounted while pinned in the rangeExtractor; focus
//      preserved; full-list index semantics)
//   2. a 500-card board renders a bounded DOM window (P3 rendering half)
//   3. the list view virtualizes rows the same way
const TEST_EMAIL = 'e2e-virtualization@test.local';
const TEST_PASSWORD = 'Test1234!';
const KEYBOARD_BOARD_CARDS = 120;
const SMOKE_BOARD_CARDS = 500;

monitoringTest.describe('Virtualized lists (T021)', () => {
  test.setTimeout(180_000);

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
    // Sequential seeding keeps insertion order = sort_key order, which the
    // keyboard-drag position assertion relies on.
    for (let i = 0; i < count; i += 1) {
      const res = await page.request.post(`${E2E_API}/api/v1/cards`, {
        data: { title: `${titlePrefix} ${String(i + 1).padStart(3, '0')}`, column_id: columnId },
      });
      if (!res.ok()) {
        throw new Error(`Failed to seed card ${i + 1}: ${res.status()}`);
      }
    }
  }

  monitoringTest('keyboard drag progresses stepwise across a 120-card virtual window and persists', async ({ page, monitoring }) => {
    const boardId = await createBoard(page, `T021 Keyboard ${Date.now()}`);
    const columnId = await firstColumnId(page, boardId);
    await seedCards(page, columnId, KEYBOARD_BOARD_CARDS, 'Keyboard card');

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');

    const firstColumn = page.locator('[data-column-id]').first();
    // Bounded DOM: the 120-card column mounts only a window of rows
    const rowCount = await firstColumn.locator('[data-testid="virtual-card-row"]').count();
    expect(rowCount).toBeGreaterThan(0);
    expect(rowCount).toBeLessThan(40);

    // Pick up the first card with the keyboard (target = the sortable node itself)
    const sortable = firstColumn.locator('div[aria-roledescription="sortable"]').first();
    await sortable.focus();
    await expect(sortable).toBeFocused();
    await page.keyboard.press('Space');
    // Source row must stay mounted (dimmed) while pinned in the rendered range
    await expect(firstColumn.locator('div[aria-roledescription="sortable"]').first()).toBeVisible();
    // 24 steps crosses the default virtual window (overscan ≈ 5 + ~15 visible rows):
    // auto-scroll mounts each successive row; the pinned source row must stay reachable.
    for (let i = 0; i < 24; i += 1) {
      await page.keyboard.press('ArrowDown');
    }
    // Focus is still on the dragged node (source row pinned, not virtualized away)
    const focusedRole = await page.evaluate(() => document.activeElement?.getAttribute('aria-roledescription'));
    expect(focusedRole).toBe('sortable');
    const focusedText = await page.evaluate(() => document.activeElement?.textContent ?? '');
    expect(focusedText).toContain('Keyboard card 001');
    // Wait for the drop's PATCH to reach the server before asserting persisted state
    const patchDone = page.waitForResponse(
      (r) => r.request().method() === 'PATCH' && /\/api\/v1\/cards\/\d+/.test(r.url()),
    );
    await page.keyboard.press('Space');
    await patchDone;

    // Verify persisted order via the API (independent source of truth)
    const res = await page.request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`);
    const body = await res.json();
    const columns = body.data ?? body;
    const col = columns.find((c: { id: number }) => c.id === columnId);
    expect(col.cards[24].title).toBe('Keyboard card 001');

    await page.reload();
    await page.waitForLoadState('networkidle');
    // After reload the card is persisted at index 24 — the first mounted row is
    // therefore card 002 (the virtualized list only mounts a window of rows).
    const reloaded = page.locator('[data-column-id]').first().locator('[data-testid="virtual-card-row"]');
    await expect(reloaded.first()).toContainText('Keyboard card 002');

    assertNoErrors(monitoring);
  });

  monitoringTest('500-card board renders a bounded DOM window (P3 rendering half)', async ({ page, monitoring }) => {
    const boardId = await createBoard(page, `T021 Smoke ${Date.now()}`);
    const columnId = await firstColumnId(page, boardId);
    await seedCards(page, columnId, SMOKE_BOARD_CARDS, 'Smoke card');

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');

    const firstColumn = page.locator('[data-column-id]').first();
    const rowCount = await firstColumn.locator('[data-testid="virtual-card-row"]').count();
    expect(rowCount).toBeGreaterThan(0);
    expect(rowCount).toBeLessThanOrEqual(30);
    // navToNetworkIdleMs is asserted in e2e/perf-load-heap.spec.ts (T046 F2) —
    // the old ad-hoc ttInteractiveMs log was promoted into that budget.
    console.log(`[T021] 500-card board: mounted rows=${rowCount}`);

    // List view: same bounded behavior + full-list aria bookkeeping
    await page.getByRole('button', { name: 'List view' }).click();
    await page.waitForLoadState('networkidle');
    const listRows = page.locator('[data-testid="board-list-row"]');
    const listRowCount = await listRows.count();
    expect(listRowCount).toBeGreaterThan(0);
    expect(listRowCount).toBeLessThanOrEqual(30);
    await expect(page.getByRole('table', { name: 'Board cards list' })).toHaveAttribute('aria-rowcount', String(SMOKE_BOARD_CARDS + 1));
    console.log(`[T021] list view: mounted rows=${listRowCount}`);

    assertNoErrors(monitoring);
  });
});
