import { test, expect } from '@playwright/test';
import { monitoringTest, assertNoErrors } from './test-utils';

const TEST_EMAIL = 'e2e-dnd@test.local';
const TEST_PASSWORD = 'Test1234!';

monitoringTest.describe('Drag and Drop', () => {
  monitoringTest.beforeAll(async ({ request }) => {
    const res = await request.post('http://localhost:3000/api/auth/register', {
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

  async function createBoardWithCards(page: import('@playwright/test').Page, boardName: string) {
    await page.goto('/');
    await page.waitForTimeout(1000);

    await page.getByRole('button', { name: 'Create Board' }).click();
    await page.getByLabel('Board name').fill(boardName);
    await page.getByRole('button', { name: 'Create' }).click();
    await page.waitForTimeout(2000);

    const columns = page.locator('[data-column-id]');
    await expect(columns.first()).toBeVisible({ timeout: 10000 });
  }

  async function addCardInColumn(page: import('@playwright/test').Page, columnIndex: number, cardTitle: string) {
    const column = page.locator('[data-column-id]').nth(columnIndex);

    let textarea = column.locator('textarea[placeholder="Enter a title..."]');
    const isTextareaVisible = await textarea.isVisible().catch(() => false);

    if (!isTextareaVisible) {
      const addCardBtn = column.locator('button', { hasText: /Add a card/i });
      await expect(addCardBtn).toBeVisible({ timeout: 10000 });
      await addCardBtn.click();
      textarea = column.locator('textarea[placeholder="Enter a title..."]');
    }

    await expect(textarea).toBeVisible({ timeout: 5000 });
    await textarea.fill(cardTitle);
    await textarea.press('Enter');
    await page.waitForTimeout(500);
  }

  // NOTE: dnd-kit uses pointer events with activation constraints (distance: 5px).
  // Playwright's built-in dragTo() uses HTML5 Drag API which doesn't trigger dnd-kit's sensors.
  // We use page.mouse to emit the exact pointer event sequence dnd-kit expects:
  // mouse.down() + small delay + mouse.move() with {steps} to exceed the distance constraint + mouse.up()

  async function dragCardTo(page: import('@playwright/test').Page, fromBox: { x: number; y: number; width: number; height: number }, toBox: { x: number; y: number; width: number; height: number }) {
    const fromCenterX = fromBox.x + fromBox.width / 2;
    const fromCenterY = fromBox.y + fromBox.height / 2;
    const toCenterX = toBox.x + toBox.width / 2;
    const toCenterY = toBox.y + toBox.height / 2;

    await page.mouse.move(fromCenterX, fromCenterY);
    await page.mouse.down();
    await page.waitForTimeout(100);
    await page.mouse.move(fromCenterX + 5, fromCenterY + 5, { steps: 5 });
    await page.waitForTimeout(200);
    await page.mouse.move(toCenterX, toCenterY, { steps: 20 });
    await page.waitForTimeout(200);
    await page.mouse.up();
    await page.waitForTimeout(1000);
  }

  monitoringTest('can drag and drop a card within a column', async ({ page, monitoring }) => {
    test.setTimeout(60000);
    const boardName = `DnD Internal ${Date.now()}`;
    await createBoardWithCards(page, boardName);

    await addCardInColumn(page, 0, 'Card 1');
    await addCardInColumn(page, 0, 'Card 2');

    // Dismiss any open textarea by clicking elsewhere
    await page.click('[data-column-id]', { position: { x: 5, y: 5 } });
    await page.waitForTimeout(500);

    const column = page.locator('[data-column-id]').first();
    const cards = column.locator('div[role="button"][aria-label="Open card details"]');
    await expect(cards).toHaveCount(2, { timeout: 10000 });
    await expect(cards.nth(0)).toHaveText(/Card 1/);
    await expect(cards.nth(1)).toHaveText(/Card 2/);

    const card1 = cards.nth(0);
    const card2 = cards.nth(1);

    const card1Box = await card1.boundingBox();
    const card2Box = await card2.boundingBox();
    expect(card1Box).toBeTruthy();
    expect(card2Box).toBeTruthy();

    await dragCardTo(page, card1Box!, card2Box!);

    await expect(cards.nth(0)).toHaveText(/Card 2/, { timeout: 5000 });
    await expect(cards.nth(1)).toHaveText(/Card 1/, { timeout: 5000 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    const reloadedCards = page.locator('[data-column-id]').first().locator('div[role="button"][aria-label="Open card details"]');
    await expect(reloadedCards.nth(0)).toHaveText(/Card 2/, { timeout: 10000 });
    await expect(reloadedCards.nth(1)).toHaveText(/Card 1/, { timeout: 10000 });

    assertNoErrors(monitoring);
  });

  monitoringTest('can drag and drop a card between columns', async ({ page, monitoring }) => {
    test.setTimeout(60000);
    const boardName = `DnD Between ${Date.now()}`;
    await createBoardWithCards(page, boardName);

    const boardUrl = page.url();
    const boardId = parseInt(boardUrl.split('/board/')[1]?.split('/')[0] || '0', 10);

    const columnsRes = await page.request.get(`/api/boards/${boardId}/columns`);
    const columnsData = await columnsRes.json();
    const columnIds: number[] = (columnsData.data ?? columnsData).map((c: { id: number }) => c.id);

    expect(columnIds.length).toBeGreaterThanOrEqual(2);

    await page.request.post('/api/cards', {
      data: { title: 'Card A', column_id: columnIds[0] },
    });
    await page.request.post('/api/cards', {
      data: { title: 'Card B', column_id: columnIds[1] },
    });

    await page.reload();
    await page.waitForTimeout(2000);

    const columns = page.locator('[data-column-id]');
    const firstColumnCards = columns.nth(0).locator('div[role="button"][aria-label="Open card details"]');
    const secondColumnCards = columns.nth(1).locator('div[role="button"][aria-label="Open card details"]');

    await expect(firstColumnCards).toHaveCount(1, { timeout: 10000 });
    await expect(secondColumnCards).toHaveCount(1, { timeout: 10000 });

    const cardA = firstColumnCards.first();
    const cardB = secondColumnCards.first();

    const cardABox = await cardA.boundingBox();
    const cardBBox = await cardB.boundingBox();
    expect(cardABox).toBeTruthy();
    expect(cardBBox).toBeTruthy();

    await dragCardTo(page, cardABox!, cardBBox!);

    await expect(firstColumnCards).toHaveCount(0, { timeout: 5000 });
    await expect(secondColumnCards).toHaveCount(2, { timeout: 5000 });

    // Verify server state via API before reload
    const verifyRes = await page.request.get(`/api/boards/${boardId}/columns`);
    const verifyData = await verifyRes.json();
    const verifyColumns = verifyData.data ?? verifyData;
    expect(verifyColumns[0].cards).toHaveLength(0);
    expect(verifyColumns[1].cards).toHaveLength(2);

    await page.reload();
    await page.waitForLoadState('networkidle');
    const reloadedSecondColumn = page.locator('[data-column-id]').nth(1).locator('div[role="button"][aria-label="Open card details"]');
    await expect(reloadedSecondColumn).toHaveCount(2, { timeout: 10000 });

    assertNoErrors(monitoring);
  });

  monitoringTest('drag overlay shows the same labels and description as the source card', async ({ page, monitoring }) => {
    test.setTimeout(60000);

    const loginRes = await page.request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const labelsRes = await page.request.get('http://localhost:3000/api/labels', {
      headers: { Cookie: cookies },
    });
    const labelsData = await labelsRes.json();
    const bugLabel = labelsData.data.find((l: { id: number; name: string }) => l.name === 'Bug');
    expect(bugLabel).toBeDefined();
    if (!bugLabel) {
      throw new Error('Expected Bug label to exist');
    }

    const boardName = `DnD Preview ${Date.now()}`;
    const boardRes = await page.request.post('http://localhost:3000/api/boards', {
      data: { name: boardName },
      headers: { Cookie: cookies },
    });
    const boardId = (await boardRes.json()).data.id;

    const columnRes = await page.request.post(`http://localhost:3000/api/boards/${boardId}/columns`, {
      data: { name: 'To Do' },
      headers: { Cookie: cookies },
    });
    const columnId = (await columnRes.json()).data.id;

    const cardTitle = 'Labeled Preview Card';
    const cardDescription = `Preview body ${Date.now()}`;
    const cardRes = await page.request.post('http://localhost:3000/api/cards', {
      data: { title: cardTitle, description: cardDescription, column_id: columnId, position: 0 },
      headers: { Cookie: cookies },
    });
    const cardId = (await cardRes.json()).data.id;

    const assignRes = await page.request.post(`http://localhost:3000/api/cards/${cardId}/labels`, {
      data: { labelId: bugLabel.id },
      headers: { Cookie: cookies },
    });
    expect(assignRes.status()).toBe(200);

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    const cardButton = page.locator('div[role="button"][aria-label="Open card details"]', {
      hasText: new RegExp(cardTitle, 'i'),
    });
    await expect(cardButton).toBeVisible({ timeout: 10000 });

    const cardBox = await cardButton.boundingBox();
    expect(cardBox).toBeTruthy();
    if (!cardBox) {
      throw new Error('Expected card to have a bounding box');
    }

    const cardCenterX = cardBox.x + cardBox.width / 2;
    const cardCenterY = cardBox.y + cardBox.height / 2;

    await page.mouse.move(cardCenterX, cardCenterY);
    await page.mouse.down();
    await page.waitForTimeout(100);
    await page.mouse.move(cardCenterX + 10, cardCenterY + 10, { steps: 5 });
    await page.waitForTimeout(300);

    const labelInstances = page.getByText('Bug', { exact: true });
    await expect(labelInstances).toHaveCount(2);

    const originalOpacity = await cardButton.evaluate((el) => getComputedStyle(el.parentElement!).opacity);
    expect(originalOpacity).toBe('0.5');

    await page.mouse.up();
    await page.waitForTimeout(800);

    await expect(labelInstances).toHaveCount(1);

    assertNoErrors(monitoring);
  });
});
