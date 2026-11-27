import { expect, test, type APIRequestContext } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_PASSWORD = 'Test1234!';

async function apiLogin(request: APIRequestContext, email: string) {
  const register = await request.post(`${E2E_API}/api/v1/auth/register`, {
    data: { email, password: TEST_PASSWORD },
  });
  if (register.status() !== 201 && register.status() !== 409) {
    throw new Error(`Failed to create test user: ${register.status()}`);
  }
  const login = await request.post(`${E2E_API}/api/v1/auth/login`, {
    data: { email, password: TEST_PASSWORD },
  });
  if (!login.ok()) throw new Error(`Login failed: ${login.status()}`);
  return login.headers()['set-cookie'] || '';
}

async function seedDenseBoard(request: APIRequestContext, cookies: string) {
  const headers = { Cookie: cookies };
  const boardRes = await request.post(`${E2E_API}/api/v1/boards`, {
    data: { name: `No vertical scroll ${Date.now()}` },
    headers,
  });
  if (!boardRes.ok()) throw new Error(`Create board failed: ${boardRes.status()}`);
  const boardId = (await boardRes.json()).data.id as number;

  const columnsRes = await request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`, { headers });
  if (!columnsRes.ok()) throw new Error(`Get columns failed: ${columnsRes.status()}`);
  const columns = (await columnsRes.json()).data as Array<{ id: number }>;

  for (const column of columns) {
    for (let i = 0; i < 8; i++) {
      const card = await request.post(`${E2E_API}/api/v1/cards`, {
        data: { column_id: column.id, title: `Card ${i} ${'content '.repeat(8)}` },
        headers,
      });
      if (!card.ok()) throw new Error(`Create card failed: ${card.status()}`);
    }
  }

  return boardId;
}

test('board view does not add a vertical scrollbar to the kanban surface', async ({ page, request }) => {
  const email = `board-no-vertical-scroll-${Date.now()}@test.local`;
  const cookies = await apiLogin(request, email);
  const boardId = await seedDenseBoard(request, cookies);

  await page.goto('/login');
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
  await page.getByRole('button', { name: 'Sign In' }).click();
  await page.waitForURL('/');

  await page.goto(`/board/${boardId}`);
  await page.waitForLoadState('networkidle');
  await expect(page.locator('[data-column-id]').first()).toBeVisible();

  const main = page.locator('main').first();
  await expect(main).toHaveJSProperty('scrollHeight', await main.evaluate((el) => el.clientHeight));

  const kanbanScroller = page.locator('main .overflow-x-auto').first();
  await expect(kanbanScroller).toBeVisible();
  await expect(kanbanScroller).toHaveCSS('overflow-y', 'hidden');
  await expect(kanbanScroller).toHaveCSS('overflow-x', 'auto');
});

test('board list view does not add a second vertical scrollbar', async ({ page, request }) => {
  const email = `board-list-no-second-scroll-${Date.now()}@test.local`;
  const cookies = await apiLogin(request, email);
  const boardId = await seedDenseBoard(request, cookies);

  await page.goto('/login');
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
  await page.getByRole('button', { name: 'Sign In' }).click();
  await page.waitForURL('/');

  await page.goto(`/board/${boardId}`);
  await page.waitForLoadState('networkidle');
  await expect(page.locator('[data-column-id]').first()).toBeVisible();

  await page.getByRole('button', { name: 'List view' }).click();
  await page.waitForLoadState('networkidle');

  const listScroll = page.getByTestId('board-list-scroll');
  await expect(listScroll).toBeVisible();
  // Wait for the virtualized rows — the escaping live region only exists once
  // the list has rendered; measuring during "Loading cards..." is a false pass.
  await expect(page.getByRole('table', { name: 'Board cards list' })).toBeVisible();
  await expect(page.getByTestId('board-list-row').first()).toBeVisible();

  const main = page.locator('main').first();
  await expect(main).toHaveJSProperty('scrollHeight', await main.evaluate((el) => el.clientHeight));

  // Only board-list-scroll owns vertical scrolling; wrappers are clipped.
  await expect(listScroll).toHaveCSS('overflow-y', 'auto');
  await expect(listScroll).toHaveCSS('overflow-x', 'hidden');

  const outerList = page.locator('main [data-testid="board-list-scroll"]').first();
  const outerHeights = await outerList.evaluate((el) => ({ scrollHeight: el.scrollHeight, clientHeight: el.clientHeight }));
  expect(outerHeights.scrollHeight).toBeGreaterThanOrEqual(outerHeights.clientHeight);

  const pageMain = await main.evaluate((el) => ({ scrollHeight: el.scrollHeight, clientHeight: el.clientHeight }));
  expect(pageMain.scrollHeight).toBeLessThanOrEqual(pageMain.clientHeight + 1);

  // The virtualized list ends with an absolutely-positioned `sr-only` live
  // region. If no ancestor is positioned it escapes the scroller's clipping
  // and extends the DOCUMENT scroll area — a second vertical scrollbar. The
  // `main` assertion above cannot catch that, so assert the root scroller too.
  const doc = await page.evaluate(() => ({
    scrollHeight: document.documentElement.scrollHeight,
    clientHeight: document.documentElement.clientHeight,
  }));
  expect(doc.scrollHeight).toBeLessThanOrEqual(doc.clientHeight + 1);
});
