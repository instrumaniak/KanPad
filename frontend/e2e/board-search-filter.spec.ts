import { expect, test, type Locator, type Page } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

/**
 * T051/T053 pre-dev contract checklist (locked against T048/T049/T050/T053 implementation):
 * - Endpoints: GET /api/v1/boards/:id/columns?embed_limit=&search=&labels=&due_date=&checklist=
 *              GET /api/v1/columns/:id/cards?page=&limit=&search=&labels=&due_date=&checklist=
 *              GET /api/v1/boards/:id/cards?page=&limit=&sort=&order=&search=&labels=&due_date=&checklist=
 * - Params/encoding: search trimmed string; labels repeated i32 AND (labels=1&labels=2);
 *   due_date enum overdue|today|this_week|no_date; checklist enum with|complete|incomplete.
 * - Page sizes: columns embed initial 10 (embed_limit=10), column card hydration 20/page,
 *   board notes 20/page, list view 20/page.
 * - Totals semantics: paginated card endpoints return filtered authoritative Page.total;
 *   column embed keeps total_cards unfiltered authoritative and matching_cards filtered authoritative.
 * - DnD under filter (T050/T053): sort/move-all disabled aria-disabled with titles "Clear search and filters
 *   to sort cards" / "Clear search and filters to move all cards"; board banner "Sorting and move-all are
 *   disabled while search or filters are active. Card drag and drop is available for visible cards.";
 *   list view shows "Drag and drop is disabled while search or filters are active." (list view remains no-DnD);
 *   card DnD allowed for visible anchors under filter. Ambiguous drops (no visible anchor, unloaded matching tail,
 *   hidden-only target, unknown matching_cards for ambiguous end/container) show neutral/default non-assertive
 *   toast title "Can't drop here while filters are active." + persistently mounted polite feedback via
 *   data-testid="dnd-blocked-status" (role="status", aria-live="polite", repeated announcements for identical
 *   drops), no focus steal, no mutation. Guarded mutations (sort, move-all, blocked drop) must not be sent.
 * - A11y/testids: column-cards-sentinel, board-list-sentinel, board-notes-sentinel, virtual-card-row,
 *   board-list-row, board-list-scroll, column-card-list-scroll, role=search aria-label="Search cards",
 *   aria-label="Filter cards", aria-label="Clear search", table aria-label="Board cards list",
 *   aria-live polite status elements.
 */

const TEST_EMAIL = `e2e-board-search-filter-${Date.now()}@test.local`;
const TEST_PASSWORD = 'Test1234!';
let cookies = '';

async function apiLogin(request: import('@playwright/test').APIRequestContext) {
  const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
    data: { email: TEST_EMAIL, password: TEST_PASSWORD },
  });
  if (res.status() !== 201 && res.status() !== 409) {
    throw new Error(`Failed to create test user: ${res.status()}`);
  }
  const loginRes = await request.post(`${E2E_API}/api/v1/auth/login`, {
    data: { email: TEST_EMAIL, password: TEST_PASSWORD },
  });
  if (!loginRes.ok()) throw new Error(`Login failed: ${loginRes.status()}`);
  cookies = loginRes.headers()['set-cookie'] || '';
}

async function uiLogin(page: Page) {
  await page.goto('/login');
  await page.getByLabel('Email').fill(TEST_EMAIL);
  await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
  await page.getByRole('button', { name: 'Sign In' }).click();
  await page.waitForURL('/');
}

async function createBoard(request: import('@playwright/test').APIRequestContext, name: string) {
  const res = await request.post(`${E2E_API}/api/v1/boards`, { data: { name }, headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Create board failed: ${res.status()}`);
  return (await res.json()).data.id as number;
}

async function getColumns(request: import('@playwright/test').APIRequestContext, boardId: number) {
  const res = await request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`, { headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Get columns failed: ${res.status()}`);
  return (await res.json()).data as Array<{ id: number; name: string }>;
}

async function createCard(
  request: import('@playwright/test').APIRequestContext,
  data: { title: string; column_id: number; position?: number; due_date?: string },
) {
  const res = await request.post(`${E2E_API}/api/v1/cards`, { data, headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Create card failed: ${res.status()} ${await res.text()}`);
  return (await res.json()).data as { id: number; title: string };
}

async function createLabel(request: import('@playwright/test').APIRequestContext, name: string, color = 'red') {
  const res = await request.post(`${E2E_API}/api/v1/labels`, { data: { name, color }, headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Create label failed: ${res.status()} ${await res.text()}`);
  return (await res.json()).data as { id: number; name: string; color: string };
}

async function assignLabel(request: import('@playwright/test').APIRequestContext, cardId: number, labelId: number) {
  const res = await request.put(`${E2E_API}/api/v1/cards/${cardId}/labels/${labelId}`, { headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Assign label failed: ${res.status()} ${await res.text()}`);
}

async function createChecklist(request: import('@playwright/test').APIRequestContext, cardId: number, title = 'Checklist') {
  const res = await request.post(`${E2E_API}/api/v1/cards/${cardId}/checklists`, { data: { title }, headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Create checklist failed: ${res.status()} ${await res.text()}`);
  return (await res.json()).data as { id: number };
}

async function createChecklistItem(
  request: import('@playwright/test').APIRequestContext,
  checklistId: number,
  text: string,
  completed = false,
) {
  const res = await request.post(`${E2E_API}/api/v1/checklists/${checklistId}/items`, { data: { text }, headers: { Cookie: cookies } });
  if (!res.ok()) throw new Error(`Create checklist item failed: ${res.status()} ${await res.text()}`);
  const item = (await res.json()).data as { id: number };
  if (completed) {
    const upd = await request.patch(`${E2E_API}/api/v1/checklist-items/${item.id}`, { data: { is_completed: true }, headers: { Cookie: cookies } });
    if (!upd.ok()) throw new Error(`Complete checklist item failed: ${upd.status()}`);
  }
  return item;
}

/**
 * Drags `source` onto `zone` with raw pointer events and PROVES the drag
 * engaged (dnd-kit's live `isOver` outline) before releasing the mouse.
 *
 * Why not locator.dragTo(): it exposes no mid-drag hook, so a drag that never
 * engages (open menu overlaying the source, missed activation distance) only
 * surfaces much later at the drop assertion. That is exactly how the T053
 * blocked-drop test ran a silent no-op drag and hung until the test timeout.
 */
async function pointerDragToZone(
  page: Page,
  source: Locator,
  zone: Locator,
  place: 'center' | 'bottom',
) {
  const fromBox = await source.boundingBox();
  const toBox = await zone.boundingBox();
  if (!fromBox || !toBox) throw new Error('pointerDragToZone: source/target has no bounding box');
  const vp = page.viewportSize() ?? { width: 1280, height: 720 };
  const clampX = (x: number) => Math.max(10, Math.min(x, vp.width - 10));
  const clampY = (y: number) => Math.max(10, Math.min(y, vp.height - 10));
  const startX = clampX(fromBox.x + fromBox.width / 2);
  const startY = clampY(fromBox.y + fromBox.height / 2);

  await page.mouse.move(startX, startY);
  await page.mouse.down();
  // Exceed MouseSensor's distance:5 activation constraint.
  await page.mouse.move(clampX(startX + 10), clampY(startY + 10), { steps: 5 });
  await page.mouse.move(
    clampX(toBox.x + toBox.width / 2),
    clampY(place === 'bottom' ? toBox.y + toBox.height - 30 : toBox.y + toBox.height / 2),
    { steps: 15 },
  );
  // The target column only outlines itself while the dnd-kit drag is live AND
  // over it — fails fast (5s) instead of at the drop assertion.
  await expect(zone).toHaveCSS('outline-style', 'solid', { timeout: 5000 });
  await page.mouse.up();
}

monitoringTest.describe('Board server-driven search/filter (T051)', () => {
  monitoringTest.describe.configure({ timeout: 180_000 });

  monitoringTest.beforeAll(async ({ request }) => {
    await apiLogin(request);
  });

  monitoringTest('search/filter returns out-of-initial-page matches, resets accumulation, clears, and shows empty state', async ({ page, request, monitoring }) => {
    const boardId = await createBoard(request, `T051 Search ${Date.now()}`);
    const [colA, colB] = await getColumns(request, boardId).then(async (cols) => {
      if (cols.length >= 2) return [cols[0], cols[1]];
      const b = await request.post(`${E2E_API}/api/v1/boards/${boardId}/columns`, { data: { name: 'B', board_id: boardId }, headers: { Cookie: cookies } });
      return [cols[0], (await b.json()).data];
    });

    const labelA = await createLabel(request, `T051-A-${Date.now()}`, 'red');
    const labelB = await createLabel(request, `T051-B-${Date.now()}`, 'blue');

    // Seed 12 cards in colA so card 12 is beyond initial 10 embed.
    for (let i = 1; i <= 11; i += 1) {
      await createCard(request, { title: `Filler ${String(i).padStart(2, '0')}`, column_id: colA.id, position: i - 1 });
    }
    const outOfPage = await createCard(request, { title: 'Needle Beyond Page', column_id: colA.id, position: 11 });
    await assignLabel(request, outOfPage.id, labelA.id);
    await assignLabel(request, outOfPage.id, labelB.id);
    const checklist = await createChecklist(request, outOfPage.id, 'Needle CL');
    await createChecklistItem(request, checklist.id, 'item', true);

    await createCard(request, { title: 'Overdue Card', column_id: colB.id, position: 0, due_date: '2000-01-01T00:00:00.000Z' });
    await createCard(request, { title: 'NoDate Card', column_id: colB.id, position: 1 });

    await uiLogin(page);
    await page.goto(`/board/${boardId}`);
    await expect(page.getByLabel('Search cards')).toBeVisible({ timeout: 15000 });

    // Search: server-driven, out-of-initial-page match.
    const searchInput = page.getByLabel('Search cards');
    const searchRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes(`/api/v1/boards/${boardId}/columns`) && r.url().includes('search=Needle'),
      { timeout: 15000 },
    );
    await searchInput.fill('Needle Beyond Page');
    const searchResp = await searchRespPromise;
    expect(searchResp.status()).toBe(200);
    const searchBody = await searchResp.json();
    const colAData = (searchBody.data as Array<{ id: number; total_cards: number; matching_cards: number; cards: Array<{ title: string }> }>).find((c) => c.id === colA.id);
    expect(colAData).toBeTruthy();
    expect(colAData!.total_cards).toBe(12); // unfiltered authoritative
    expect(colAData!.matching_cards).toBe(1); // filtered authoritative
    await expect(page.getByText('Needle Beyond Page')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Filler 01')).not.toBeVisible();

    // Clear search restores results. Default unfiltered key may be served from fresh cache.
    await searchInput.fill('');
    await expect(page.getByText('Filler 01')).toBeVisible({ timeout: 10000 });

    // Labels AND (repeated params) - Needle has both A and B.
    await page.getByLabel('Filter cards').click();
    const popover = page.locator('[data-radix-popper-content-wrapper]').last();
    await expect(popover.getByText('Labels')).toBeVisible();
    const labelsRespPromise = page.waitForResponse(
      (r) => {
        if (r.request().method() !== 'GET' || !r.url().includes(`/api/v1/boards/${boardId}/columns`)) return false;
        const url = new URL(r.url());
        const labels = url.searchParams.getAll('labels');
        return labels.includes(String(labelA.id)) && labels.includes(String(labelB.id));
      },
      { timeout: 15000 },
    );
    await popover.getByRole('button', { name: labelA.name, exact: true }).click();
    if (!(await popover.isVisible().catch(() => false))) {
      await page.getByLabel('Filter cards').click();
    }
    const popoverAfterA = page.locator('[data-radix-popper-content-wrapper]').last();
    await expect(popoverAfterA.getByRole('button', { name: labelB.name, exact: true })).toBeVisible();
    await popoverAfterA.getByRole('button', { name: labelB.name, exact: true }).click();
    await labelsRespPromise;
    await expect(page.getByText('Needle Beyond Page')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Overdue Card')).not.toBeVisible();

    // Change filter resets accumulation / shows server-confirmed results: add due_date filter causing empty.
    let activePopover = page.locator('[data-radix-popper-content-wrapper]').last();
    if (!(await activePopover.isVisible().catch(() => false))) {
      await page.getByLabel('Filter cards').click();
      activePopover = page.locator('[data-radix-popper-content-wrapper]').last();
    }
    const emptyRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes('due_date=overdue') && r.url().includes('labels='),
      { timeout: 15000 },
    );
    await activePopover.getByRole('button', { name: 'Overdue', exact: true }).click();
    await emptyRespPromise;
    await expect(page.getByText('No cards found')).toBeVisible({ timeout: 10000 });

    // Clear filters restores.
    await page.getByRole('button', { name: 'Clear filters' }).first().click();
    await expect(page.getByText('Filler 01')).toBeVisible({ timeout: 10000 });

    // due_date filter alone
    await page.getByLabel('Filter cards').click();
    const pop2 = page.locator('[data-radix-popper-content-wrapper]').last();
    const dueRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes('due_date=no_date'),
      { timeout: 15000 },
    );
    await pop2.getByRole('button', { name: 'No Date', exact: true }).click();
    await dueRespPromise;
    await expect(page.getByText('NoDate Card')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Overdue Card')).not.toBeVisible();

    // checklist filter: Complete should return Needle (has complete checklist) and exclude others.
    // Toggle off due_date: returning to default key may be served from fresh cache.
    let pop2Active = page.locator('[data-radix-popper-content-wrapper]').last();
    if (!(await pop2Active.isVisible().catch(() => false))) {
      await page.getByLabel('Filter cards').click();
      pop2Active = page.locator('[data-radix-popper-content-wrapper]').last();
    }
    await pop2Active.getByRole('button', { name: 'No Date', exact: true }).click();
    await expect(page.getByText('Filler 01')).toBeVisible({ timeout: 10000 });

    pop2Active = page.locator('[data-radix-popper-content-wrapper]').last();
    if (!(await pop2Active.isVisible().catch(() => false))) {
      await page.getByLabel('Filter cards').click();
      pop2Active = page.locator('[data-radix-popper-content-wrapper]').last();
    }
    const clRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes('checklist=complete'),
      { timeout: 15000 },
    );
    await pop2Active.getByRole('button', { name: 'Complete', exact: true }).click();
    await clRespPromise;
    await expect(page.getByText('Needle Beyond Page')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('NoDate Card')).not.toBeVisible();

    // Clear filters restores accumulation baseline; default key may be served from fresh cache.
    await page.getByRole('button', { name: 'Clear all', exact: true }).click();
    await expect(page.getByText('Filler 01')).toBeVisible({ timeout: 10000 });

    assertNoErrors(monitoring);
  });

  monitoringTest('T053: filter-active sort/move-all disabled, card DnD allowed, ambiguous drop blocked', async ({ page, request, monitoring }) => {
    test.setTimeout(120_000);
    const boardId = await createBoard(request, `T053 DnD ${Date.now()}`);
    const cols = await getColumns(request, boardId);
    const [colA, colB, colC] = cols;
    if (!colA || !colB || !colC) throw new Error(`Expected 3 default columns, got ${cols.length}`);

    await createCard(request, { title: 'DnD A1', column_id: colA.id, position: 0 });
    await createCard(request, { title: 'DnD A2', column_id: colA.id, position: 1 });
    // colB: all matching cards fit in the initial embed, so a container drop
    // always resolves to a visible anchor (never the unloaded-tail path).
    for (let i = 1; i <= 3; i += 1) {
      await createCard(request, { title: `DnD B0${i}`, column_id: colB.id, position: i - 1 });
    }
    // colC: cards exist but none match the active search (hidden-only target) —
    // a deterministic ambiguous drop. An unloaded matching tail was NOT
    // deterministic: hydration fills it, which turns the drop into an allowed
    // one (T053 R3).
    await createCard(request, { title: 'Hidden C1', column_id: colC.id, position: 0 });

    await uiLogin(page);
    await page.goto(`/board/${boardId}`);
    await expect(page.getByLabel('Search cards')).toBeVisible({ timeout: 15000 });

    const respPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes('/columns') && r.url().includes('search=DnD'),
      { timeout: 15000 },
    );
    await page.getByLabel('Search cards').fill('DnD');
    await respPromise;

    await expect(page.getByText('DnD A1')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('DnD B01')).toBeVisible({ timeout: 10000 });

    // Banner copy updated for T053.
    await expect(page.getByText('Sorting and move-all are disabled while search or filters are active. Card drag and drop is available for visible cards.')).toBeVisible();

    // Nothing is announced before any drop.
    await expect(page.getByTestId('dnd-blocked-status')).toHaveText('');

    // Sort/move-all still disabled.
    const columnA = page.locator(`[data-column-id="${colA.id}"]`);
    const moreBtn = columnA.getByRole('button').filter({ has: page.locator('svg.lucide-more-horizontal') });
    await moreBtn.click();
    const sortBtn = page.getByRole('button', { name: 'Sort by Date' });
    await expect(sortBtn).toBeDisabled();
    await expect(sortBtn).toHaveAttribute('title', 'Clear search and filters to sort cards');
    const moveBtn = page.getByRole('button', { name: 'Move All Cards' });
    if (await moveBtn.count()) {
      await expect(moveBtn).toBeDisabled();
      await expect(moveBtn).toHaveAttribute('title', 'Clear search and filters to move all cards');
    }
    // The column menu is a manual popover with no Escape handler and no
    // outside-click close: toggle it shut via its trigger and PROVE it is gone
    // before dragging — an open menu overlays the drag source, the pointer never
    // reaches the card and the drag silently never starts (T053 R2).
    await moreBtn.click();
    await expect(page.getByRole('button', { name: 'Sort by Date' })).toHaveCount(0);

    const guardedRequests: string[] = [];
    const cardPatches: string[] = [];
    const onRequest = (req: import('@playwright/test').Request) => {
      const url = req.url();
      const method = req.method();
      if (method === 'POST' && /\/api\/v1\/columns\/\d+\/move-cards/.test(url)) guardedRequests.push(`${method} ${url}`);
      if (method === 'PATCH' && /\/api\/v1\/columns\/\d+/.test(url)) guardedRequests.push(`${method} ${url}`);
      // A card PATCH IS the allowed DnD move. Tracking it turns "a blocked drop
      // sends no mutation" into a real assertion instead of a coverage gap (T053 R5).
      if (method === 'PATCH' && /\/api\/v1\/cards\/\d+/.test(url)) cardPatches.push(`${method} ${url}`);
    };
    page.on('request', onRequest);

    const a1 = columnA.locator('[data-testid="virtual-card-row"]').filter({ hasText: 'DnD A1' }).first();
    const colBDroppable = page.locator(`[data-column-id="${colB.id}"]`).locator('[data-testid="column-droppable"]').first();
    const colCDroppable = page.locator(`[data-column-id="${colC.id}"]`).locator('[data-testid="column-droppable"]').first();

    // Allowed filtered DnD: A1 -> end of colB (container drop over a fully
    // loaded column resolves to the last visible anchor).
    const moveRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'PATCH' && /\/api\/v1\/cards\/\d+/.test(r.url()) && r.status() === 200,
      { timeout: 15000 },
    );
    await pointerDragToZone(page, a1, colBDroppable, 'bottom');
    await moveRespPromise; // no .catch(): a drag that never fires the PATCH must fail here
    await expect(page.getByTestId('dnd-blocked-status')).toHaveText('');
    expect(cardPatches).toHaveLength(1);
    expect(guardedRequests).toEqual([]);

    const colBLoc = page.locator(`[data-column-id="${colB.id}"]`);
    const a1InB = colBLoc.locator('[data-testid="virtual-card-row"]').filter({ hasText: 'DnD A1' }).first();
    await expect(a1InB).toBeVisible({ timeout: 10000 });

    // Blocked ambiguous drop: colC exists (total_cards > 0) but shows nothing
    // that matches the search, so there is no visible anchor to drop against.
    const blockedText = "Can't drop here while filters are active.";
    await pointerDragToZone(page, a1InB, colCDroppable, 'center');
    await expect(page.getByTestId('dnd-blocked-status')).toHaveText(blockedText, { timeout: 15000 });
    await expect(page.getByText(blockedText).first()).toBeVisible(); // neutral, non-assertive toast
    expect(cardPatches).toHaveLength(1); // blocked drop sent no card PATCH
    expect(guardedRequests).toEqual([]);

    // The blocked drop rolled back to server state: A1 is back in colB.
    await expect(a1InB).toBeVisible({ timeout: 15000 });
    const firstAnnouncement = (await page.getByTestId('dnd-blocked-status').textContent()) ?? '';

    // An identical blocked drop must RE-announce (nonce/ZWSP): a no-op write
    // would leave the text identical and the polite region silent.
    await pointerDragToZone(page, a1InB, colCDroppable, 'center');
    await expect
      .poll(async () => (await page.getByTestId('dnd-blocked-status').textContent()) ?? '', { timeout: 15000 })
      .not.toBe(firstAnnouncement);
    expect(await page.getByTestId('dnd-blocked-status').textContent()).toContain(blockedText);
    expect(cardPatches).toHaveLength(1);
    expect(guardedRequests).toEqual([]);

    // Verify persistence via reload under same filter.
    await page.reload();
    await expect(page.getByLabel('Search cards')).toBeVisible({ timeout: 15000 });
    await page.getByLabel('Search cards').fill('DnD');
    await page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes('/columns') && r.url().includes('search=DnD'),
      { timeout: 15000 },
    );
    await expect(page.locator(`[data-column-id="${colB.id}"]`).getByText('DnD A1')).toBeVisible({ timeout: 10000 });
    expect(guardedRequests).toEqual([]);

    // List view remains no-DnD with accurate copy.
    await page.getByRole('button', { name: 'List' }).click();
    await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('Drag and drop is disabled while search or filters are active.')).toBeVisible();

    page.off('request', onRequest);
    assertNoErrors(monitoring);
  });

  monitoringTest('list view filter/sort resets accumulation and reports filtered authoritative Page.total', async ({ page, request, monitoring }) => {
    const boardId = await createBoard(request, `T051 List Filter ${Date.now()}`);
    const cols = await getColumns(request, boardId);
    const colA = cols[0];
    const colB = cols[1] ?? (await (async () => {
      const r = await request.post(`${E2E_API}/api/v1/boards/${boardId}/columns`, { data: { name: 'B', board_id: boardId }, headers: { Cookie: cookies } });
      return (await r.json()).data;
    })());

    const total = 22;
    for (let i = 1; i <= total; i += 1) {
      const columnId = i % 2 === 0 ? colB.id : colA.id;
      await createCard(request, { title: `LF ${String(i).padStart(2, '0')}`, column_id: columnId, position: i });
    }
    // One extra card beyond initial page matching a unique search
    await createCard(request, { title: 'LF Needle', column_id: colA.id, position: 999 });

    await uiLogin(page);
    await page.goto(`/board/${boardId}`);
    await expect(page.getByRole('group', { name: 'Board view mode' })).toBeVisible({ timeout: 15000 });

    const listRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes(`/api/v1/boards/${boardId}/cards`) && r.url().includes('limit=20') && r.status() === 200,
      { timeout: 15000 },
    );
    await page.getByRole('button', { name: 'List' }).click();
    const listResp = await listRespPromise;
    const firstBody = await listResp.json();
    expect(firstBody.total).toBe(23);
    expect(firstBody.data.length).toBe(20);

    // Load all pages to create accumulation, then verify stable order by checking all ids seen.
    const seen = new Set<number>((firstBody.data as Array<{ id: number }>).map((c) => c.id));
    const scrollContainer = page.locator('[data-testid="board-list-scroll"]');
    const sentinel = page.locator('[data-testid="board-list-sentinel"]');
    for (let i = 0; i < 10 && seen.size < 23; i += 1) {
      if ((await sentinel.count()) === 0) break;
      const before = seen.size;
      const respPromise = page.waitForResponse(
        (r) => r.request().method() === 'GET' && r.url().includes(`/api/v1/boards/${boardId}/cards`) && /page=[2-9]\d*/.test(r.url()) && r.status() === 200,
        { timeout: 10000 },
      );
      await scrollContainer.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const resp = await respPromise;
      const body = await resp.json();
      for (const item of body.data as Array<{ id: number }>) seen.add(item.id);
      expect(seen.size).toBeGreaterThan(before);
    }
    expect(seen.size).toBe(23);

    // Filtered search resets accumulation and returns filtered authoritative Page.total.
    const searchRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes(`/api/v1/boards/${boardId}/cards`) && (r.url().includes('search=LF+Needle') || r.url().includes('search=LF%20Needle')) && r.status() === 200,
      { timeout: 15000 },
    );
    await page.getByLabel('Search cards').fill('LF Needle');
    const searchResp = await searchRespPromise;
    const searchBody = await searchResp.json();
    expect(searchBody.total).toBe(1); // filtered authoritative
    expect(searchBody.data.length).toBe(1);
    expect(searchBody.data[0].title).toBe('LF Needle');
    await expect(page.getByText('LF Needle')).toBeVisible({ timeout: 10000 });

    // Clear search resets accumulation to unfiltered; default list key may be served from fresh cache.
    await page.getByLabel('Search cards').fill('');
    await expect(page.getByText('LF 01')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('LF Needle')).not.toBeVisible();

    // Sort change resets accumulation.
    const sortRespPromise = page.waitForResponse(
      (r) => r.request().method() === 'GET' && r.url().includes(`/api/v1/boards/${boardId}/cards`) && r.url().includes('sort=title') && r.url().includes('page=1') && r.status() === 200,
      { timeout: 15000 },
    );
    await page.getByRole('button', { name: 'Title', exact: true }).click();
    const sortResp = await sortRespPromise;
    const sortBody = await sortResp.json();
    expect(sortBody.page).toBe(1);
    expect(sortBody.total).toBe(23);
    expect(sortBody.data.length).toBe(20);
    // Order should be ascending by title for newly selected sort key.
    const titles = (sortBody.data as Array<{ title: string }>).map((c) => c.title);
    const sorted = [...titles].sort((a, b) => a.localeCompare(b));
    expect(titles).toEqual(sorted);

    assertNoErrors(monitoring);
  });

  monitoringTest('board view column sort resets paginated card accumulation', async ({ page, request, monitoring }) => {
    const boardId = await createBoard(request, `T051 Sort Reset ${Date.now()}`);
    const cols = await getColumns(request, boardId);
    const colA = cols[0];

    for (let i = 1; i <= 12; i += 1) {
      await createCard(request, { title: `SortCard ${String(i).padStart(2, '0')}`, column_id: colA.id, position: i - 1 });
    }

    await uiLogin(page);
    const columnsResponsePromise = page.waitForResponse(
      (r) =>
        r.request().method() === 'GET' &&
        r.url().includes(`/api/v1/boards/${boardId}/columns`) &&
        r.url().includes('embed_limit=10') &&
        r.status() === 200,
      { timeout: 15000 },
    );
    await page.goto(`/board/${boardId}`);
    await columnsResponsePromise;

    const column = page.locator(`[data-column-id="${colA.id}"]`);
    const scrollContainer = column.locator('[data-testid="column-card-list-scroll"]');
    const sentinel = column.locator('[data-testid="column-cards-sentinel"]');
    const seenIds = new Set<number>();
    const onCardsResponse = async (r: import('@playwright/test').Response) => {
      if (r.request().method() !== 'GET') return;
      if (!r.url().includes(`/api/v1/columns/${colA.id}/cards`)) return;
      if (!r.url().includes('page=')) return;
      if (r.status() !== 200) return;
      try {
        const body = await r.json();
        for (const card of (body.data ?? []) as Array<{ id: number }>) seenIds.add(card.id);
      } catch {
        // ignore
      }
    };
    page.on('response', onCardsResponse);
    try {
      for (let i = 0; i < 10 && (await sentinel.count()) > 0; i += 1) {
        const respPromise = page.waitForResponse(
          (r) =>
            r.request().method() === 'GET' &&
            r.url().includes(`/api/v1/columns/${colA.id}/cards`) &&
            r.url().includes('page=') &&
            r.status() === 200,
          { timeout: 10000 },
        );
        await scrollContainer.evaluate((el) => { el.scrollTop = el.scrollHeight; });
        await respPromise;
      }
      await expect(sentinel).toHaveCount(0, { timeout: 10000 });
    } finally {
      page.off('response', onCardsResponse);
    }
    expect(seenIds.size).toBeGreaterThanOrEqual(2); // loaded beyond initial embed

    await scrollContainer.evaluate((el) => { el.scrollTop = 0; });
    // Sort by date triggers invalidation; accumulation should reset to fresh page 1 embed.
    await column.getByRole('button').filter({ has: page.locator('svg.lucide-more-horizontal') }).click();
    await expect(page.getByRole('button', { name: 'Sort by Date' })).toBeVisible({ timeout: 5000 });
    await page.getByRole('button', { name: 'Sort by Date' }).click();
    await expect(page.getByRole('button', { name: 'Ascending (Oldest first)' })).toBeVisible({ timeout: 5000 });
    const sortRespPromise = page.waitForResponse(
      (r) =>
        r.request().method() === 'PATCH' &&
        r.url().includes(`/api/v1/columns/${colA.id}`) &&
        r.status() === 200,
      { timeout: 15000 },
    );
    const columnsAfterSortPromise = page.waitForResponse(
      (r) =>
        r.request().method() === 'GET' &&
        r.url().includes(`/api/v1/boards/${boardId}/columns`) &&
        r.url().includes('embed_limit=10') &&
        r.status() === 200,
      { timeout: 15000 },
    );
    await page.getByRole('button', { name: 'Ascending (Oldest first)' }).click();
    await sortRespPromise;
    const columnsAfterSort = await columnsAfterSortPromise;
    const body = await columnsAfterSort.json();
    const sortedCol = (body.data as Array<{ id: number; cards: Array<{ id: number }>; total_cards: number }>).find((c) => c.id === colA.id);
    expect(sortedCol).toBeTruthy();
    expect(sortedCol!.total_cards).toBe(12);
    expect(sortedCol!.cards.length).toBe(10); // accumulation reset to initial embed
    await expect(column.locator('[data-testid="virtual-card-row"]')).toHaveCount(10, { timeout: 10000 });

    assertNoErrors(monitoring);
  });
});
