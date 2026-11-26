import { expect, type Page } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

const TEST_EMAIL = `e2e-board-pagination-${Date.now()}@test.local`;
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
  const body = await res.json();
  return body.data.id as number;
}

async function createColumn(
  request: import('@playwright/test').APIRequestContext,
  boardId: number,
  name: string,
) {
  const res = await request.post(`${E2E_API}/api/v1/boards/${boardId}/columns`, {
    data: { name, board_id: boardId },
    headers: { Cookie: cookies },
  });
  if (!res.ok()) throw new Error(`Create column failed: ${res.status()}`);
  return (await res.json()).data.id as number;
}

async function seedCards(
  request: import('@playwright/test').APIRequestContext,
  columnId: number,
  count: number,
  prefix: string,
) {
  const ids: number[] = [];
  for (let i = 0; i < count; i += 1) {
    const title = `${prefix} ${String(i + 1).padStart(2, '0')}`;
    const res = await request.post(`${E2E_API}/api/v1/cards`, {
      data: { title, column_id: columnId, position: i },
      headers: { Cookie: cookies },
    });
    if (!res.ok()) throw new Error(`Seed card failed ${i + 1}: ${res.status()} ${await res.text()}`);
    ids.push((await res.json()).data.id as number);
  }
  return ids;
}

async function seedNotes(
  request: import('@playwright/test').APIRequestContext,
  boardId: number,
  count: number,
  prefix: string,
) {
  const ids: number[] = [];
  for (let i = 0; i < count; i += 1) {
    const title = `${prefix} ${String(i + 1).padStart(2, '0')}`;
    const res = await request.post(`${E2E_API}/api/v1/notes`, {
      data: { title, content: `Content for ${title}`, board_id: boardId },
      headers: { Cookie: cookies },
    });
    if (!res.ok()) throw new Error(`Seed note failed ${i + 1}: ${res.status()} ${await res.text()}`);
    ids.push((await res.json()).data.id as number);
  }
  return ids;
}

async function waitForVisibleCardTitles(
  column: import('@playwright/test').Locator,
  scrollContainer: import('@playwright/test').Locator,
  expectedTitles: string[],
) {
  const seenTitles = new Set<string>();
  const total = expectedTitles.length;
  await scrollContainer.evaluate((el) => { el.scrollTop = 0; });
  for (let pass = 0; pass < 80 && seenTitles.size < total; pass += 1) {
    const texts = await column.locator('[data-testid="virtual-card-row"]').evaluateAll((rows) =>
      rows.map((r) => (r.textContent ?? '').trim()),
    );
    for (const text of texts) {
      for (const title of expectedTitles) {
        if (text.includes(title)) seenTitles.add(title);
      }
    }
    if (seenTitles.size === total) break;
    await scrollContainer.evaluate((el) => { el.scrollTop += Math.max(el.clientHeight * 0.75, 160); });
    await expect(column.locator('[data-testid="virtual-card-row"]').first()).toBeVisible();
  }
  return seenTitles;
}

monitoringTest.describe('Board pagination / infinite scroll (T051)', () => {
  monitoringTest.describe.configure({ timeout: 180_000 });

  monitoringTest.beforeAll(async ({ request }) => {
    await apiLogin(request);
  });

  monitoringTest('board column loads 10 cards initially and infinite scroll loads remaining without duplicates', async ({ page, request, monitoring }) => {
    const boardId = await createBoard(request, `T051 Board Cards ${Date.now()}`);
    const columnsRes = await request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`, { headers: { Cookie: cookies } });
    const cols = (await columnsRes.json()).data as Array<{ id: number }>;
    const colA = cols[0].id;

    const totalCards = 12; // > initial 10, triggers one 20/page load (page 2)
    const cardIds = await seedCards(request, colA, totalCards, 'ColCard');
    const expectedTitles = cardIds.map((_, i) => `ColCard ${String(i + 1).padStart(2, '0')}`);

    await uiLogin(page);

    const seenIds = new Set<number>();
    const paginatedSeenIds = new Set<number>();
    const duplicateIds: number[] = [];
    const onCardsResponse = async (r: import('@playwright/test').Response) => {
      if (r.request().method() !== 'GET') return;
      if (!r.url().includes(`/api/v1/columns/${colA}/cards`)) return;
      if (!r.url().includes('page=')) return;
      if (r.status() !== 200) return;
      try {
        const body = await r.json();
        for (const card of (body.data ?? []) as Array<{ id: number }>) {
          if (paginatedSeenIds.has(card.id)) duplicateIds.push(card.id);
          paginatedSeenIds.add(card.id);
          seenIds.add(card.id);
        }
      } catch {
        // ignore non-JSON/aborted responses
      }
    };
    page.on('response', onCardsResponse);

    const columnsResponsePromise = page.waitForResponse(
      (r) =>
        r.request().method() === 'GET' &&
        r.url().includes(`/api/v1/boards/${boardId}/columns`) &&
        r.url().includes('embed_limit=10') &&
        r.status() === 200,
      { timeout: 15000 },
    );
    await page.goto(`/board/${boardId}`);
    const columnsResponse = await columnsResponsePromise;
    const columnsBody = await columnsResponse.json();
    const colData = (columnsBody.data as Array<{
      id: number;
      cards: Array<{ id: number; title: string }>;
      total_cards: number;
      matching_cards?: number;
    }>).find((c) => c.id === colA);
    expect(colData).toBeTruthy();
    expect(colData!.cards.length).toBe(10); // T050 initial page size
    expect(colData!.total_cards).toBe(totalCards);
    expect(colData!.matching_cards ?? colData!.total_cards).toBe(totalCards);
    for (const card of colData!.cards) seenIds.add(card.id);

    const column = page.locator(`[data-column-id="${colA}"]`);
    await expect(column).toBeVisible();
    await expect(column.locator('[data-testid="virtual-card-row"]').first()).toBeVisible();

    const scrollContainer = column.locator('[data-testid="column-card-list-scroll"]');
    const sentinel = column.locator('[data-testid="column-cards-sentinel"]');
    try {
      for (let i = 0; i < 10 && seenIds.size < totalCards; i += 1) {
        if ((await sentinel.count()) === 0) break;
        const before = seenIds.size;
        const respPromise = page.waitForResponse(
          (r) =>
            r.request().method() === 'GET' &&
            r.url().includes(`/api/v1/columns/${colA}/cards`) &&
            r.url().includes('page=') &&
            r.status() === 200,
          { timeout: 10000 },
        );
        await scrollContainer.evaluate((el) => { el.scrollTop = el.scrollHeight; });
        await respPromise;
        await expect.poll(() => seenIds.size, { timeout: 5000 }).toBeGreaterThan(before);
      }
      await expect(sentinel).toHaveCount(0, { timeout: 10000 });
    } finally {
      page.off('response', onCardsResponse);
    }
    expect(duplicateIds, 'no duplicate card IDs across paginated responses').toEqual([]);
    expect(seenIds.size, 'all seeded column cards should be loaded via UI pagination/IO').toBe(totalCards);
    for (const id of cardIds) expect(seenIds.has(id)).toBe(true);

    const seenTitles = await waitForVisibleCardTitles(column, scrollContainer, expectedTitles);
    expect(seenTitles.size).toBe(totalCards);

    assertNoErrors(monitoring);
  });

  monitoringTest('notes sidebar loads 20 initially and infinite scroll loads remaining without duplicates', async ({ page, request, monitoring }) => {
    const boardId = await createBoard(request, `T051 Notes ${Date.now()}`);
    const totalNotes = 22; // > 20, triggers page 2
    const noteIds = await seedNotes(request, boardId, totalNotes, 'T051 Note');

    await uiLogin(page);
    await page.goto(`/board/${boardId}`);
    await expect(page.getByRole('group', { name: 'Board view mode' })).toBeVisible({ timeout: 15000 });

    // Reload for a clean state, then ensure sidebar is open and capture notes page 1.
    const notesPage1Promise = page.waitForResponse(
      (r) =>
        r.request().method() === 'GET' &&
        r.url().includes('/api/v1/notes') &&
        r.url().includes(`board_id=${boardId}`) &&
        r.url().includes('page=1') &&
        r.status() === 200,
      { timeout: 15000 },
    );
    await page.reload();
    const showBtn = page.locator('[aria-label="Show notes sidebar"]');
    const toggleBtn = page.locator('[aria-label="Toggle notes sidebar"]');
    await expect(showBtn.or(toggleBtn).first()).toBeVisible({ timeout: 15000 });
    if (await showBtn.isVisible().catch(() => false)) {
      await showBtn.click();
    }
    const firstNotesResp = await notesPage1Promise;
    const firstBody = await firstNotesResp.json();
    expect(firstBody.data.length).toBe(20);
    expect(firstBody.total).toBe(totalNotes);

    const seenNoteIds = new Set<number>((firstBody.data as Array<{ id: number }>).map((n) => n.id));
    await scrollNotesToLoadAll(page, boardId, seenNoteIds, totalNotes);
    expect(seenNoteIds.size).toBe(totalNotes);
    for (const id of noteIds) expect(seenNoteIds.has(id)).toBe(true);

    await assertAllNoteTitlesAccessible(page, totalNotes);
    assertNoErrors(monitoring);
  });

  monitoringTest('list view loads 20 initially and infinite scroll loads remaining with stable order and no duplicates', async ({ page, request, monitoring }) => {
    const boardId = await createBoard(request, `T051 List ${Date.now()}`);
    const columnsRes = await request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`, { headers: { Cookie: cookies } });
    const cols = (await columnsRes.json()).data as Array<{ id: number }>;
    const colA = cols[0].id;
    const colB = cols[1]?.id ?? (await createColumn(request, boardId, 'B'));

    const total = 22;
    const ids: number[] = [];
    for (let i = 0; i < total; i += 1) {
      const columnId = i % 2 === 0 ? colA : colB;
      const res = await request.post(`${E2E_API}/api/v1/cards`, {
        data: { title: `ListCard ${String(i + 1).padStart(2, '0')}`, column_id: columnId, position: i },
        headers: { Cookie: cookies },
      });
      if (!res.ok()) throw new Error(`Seed list card failed: ${res.status()}`);
      ids.push((await res.json()).data.id as number);
    }

    await uiLogin(page);
    await page.goto(`/board/${boardId}`);
    await expect(page.getByRole('group', { name: 'Board view mode' })).toBeVisible({ timeout: 15000 });

    const listRespPromise = page.waitForResponse(
      (r) =>
        r.request().method() === 'GET' &&
        r.url().includes(`/api/v1/boards/${boardId}/cards`) &&
        r.url().includes('limit=20') &&
        r.status() === 200,
      { timeout: 15000 },
    );
    await page.getByRole('button', { name: 'List' }).click();
    const listResp = await listRespPromise;
    const firstBody = await listResp.json();
    expect(firstBody.data.length).toBe(20);
    expect(firstBody.total).toBe(total);

    const orderedIds: number[] = [];
    const seen = new Set<number>();
    for (const item of firstBody.data as Array<{ id: number; title: string }>) {
      expect(seen.has(item.id)).toBe(false);
      seen.add(item.id);
      orderedIds.push(item.id);
    }

    const scrollContainer = page.locator('[data-testid="board-list-scroll"]');
    const sentinel = page.locator('[data-testid="board-list-sentinel"]');
    for (let i = 0; i < 10 && seen.size < total; i += 1) {
      if ((await sentinel.count()) === 0) break;
      const before = seen.size;
      const respPromise = page.waitForResponse(
        (r) =>
          r.request().method() === 'GET' &&
          r.url().includes(`/api/v1/boards/${boardId}/cards`) &&
          /page=[2-9]\d*/.test(r.url()) &&
          r.status() === 200,
        { timeout: 10000 },
      );
      await scrollContainer.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const resp = await respPromise;
      const body = await resp.json();
      for (const item of body.data as Array<{ id: number; title: string }>) {
        expect(seen.has(item.id), `duplicate list card ${item.id}`).toBe(false);
        seen.add(item.id);
        orderedIds.push(item.id);
      }
      expect(seen.size).toBeGreaterThan(before);
    }

    expect(seen.size).toBe(total);
    for (const id of ids) expect(seen.has(id)).toBe(true);

    // Stable server-driven sort/order across loads: compare accumulated order to API truth.
    const apiIds: number[] = [];
    for (let pageNum = 1; pageNum <= 5; pageNum += 1) {
      const res = await request.get(`${E2E_API}/api/v1/boards/${boardId}/cards?page=${pageNum}&limit=20&sort=created_at&order=asc`, { headers: { Cookie: cookies } });
      if (!res.ok()) throw new Error(`board cards API failed: ${res.status()}`);
      const body = await res.json();
      apiIds.push(...(body.data as Array<{ id: number }>).map((c) => c.id));
      if (apiIds.length >= body.total) break;
    }
    expect(orderedIds).toEqual(apiIds);

    await scrollContainer.evaluate((el) => { el.scrollTop = 0; });
    const seenTitles = new Set<string>();
    for (let pass = 0; pass < 100 && seenTitles.size < total; pass += 1) {
      const rows = page.locator('[data-testid="board-list-row"]');
      const texts = await rows.evaluateAll((els) => els.map((e) => (e.textContent ?? '').trim()));
      for (const text of texts) {
        for (let n = 1; n <= total; n += 1) {
          const title = `ListCard ${String(n).padStart(2, '0')}`;
          if (text.includes(title)) seenTitles.add(title);
        }
      }
      if (seenTitles.size === total) break;
      await scrollContainer.evaluate((el) => { el.scrollTop += Math.max(el.clientHeight * 0.75, 160); });
      await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible();
    }
    expect(seenTitles.size).toBe(total);

    assertNoErrors(monitoring);
  });
});

async function scrollNotesToLoadAll(
  page: Page,
  boardId: number,
  seenNoteIds: Set<number>,
  totalNotes: number,
) {
  const sentinel = page.locator('[data-testid="board-notes-sentinel"]');
  const scrollViewport = page.locator('[data-radix-scroll-area-viewport]').first();
  for (let i = 0; i < 10 && seenNoteIds.size < totalNotes; i += 1) {
    if ((await sentinel.count()) === 0) break;
    const before = seenNoteIds.size;
    const respPromise = page.waitForResponse(
      (r) =>
        r.request().method() === 'GET' &&
        r.url().includes('/api/v1/notes') &&
        r.url().includes(`board_id=${boardId}`) &&
        /page=\d+/.test(r.url()) &&
        !r.url().includes('page=1') &&
        r.status() === 200,
      { timeout: 10000 },
    );
    await scrollViewport.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    const resp = await respPromise;
    const body = await resp.json();
    for (const note of body.data as Array<{ id: number }>) {
      expect(seenNoteIds.has(note.id), `duplicate note ${note.id}`).toBe(false);
      seenNoteIds.add(note.id);
    }
    expect(seenNoteIds.size).toBeGreaterThan(before);
  }
}

async function assertAllNoteTitlesAccessible(page: Page, totalNotes: number) {
  const scrollViewport = page.locator('[data-radix-scroll-area-viewport]').first();
  await scrollViewport.evaluate((el) => { el.scrollTop = 0; });
  const seenTitles = new Set<string>();
  for (let pass = 0; pass < 80 && seenTitles.size < totalNotes; pass += 1) {
    const texts = await page.locator('[data-radix-scroll-area-viewport]').first().evaluateAll((els) =>
      Array.from(els).flatMap((el) => Array.from(el.querySelectorAll('button, [role="article"]')).map((n) => (n.textContent ?? '').trim())),
    );
    for (const text of texts) {
      for (let n = 1; n <= totalNotes; n += 1) {
        const title = `T051 Note ${String(n).padStart(2, '0')}`;
        if (text.includes(title)) seenTitles.add(title);
      }
    }
    if (seenTitles.size === totalNotes) break;
    await scrollViewport.evaluate((el) => { el.scrollTop += Math.max(el.clientHeight * 0.75, 160); });
    await expect(page.locator('[aria-label="Toggle notes sidebar"], [aria-label="Show notes sidebar"]').first()).toBeVisible();
  }
  expect(seenTitles.size).toBe(totalNotes);
}
