import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

// T044 follow-up — "+ Add a card" placement contract (board view):
//   * short/empty columns: the footer rides the bottom of the visible CONTENT
//     (it must NOT be pinned to the column's full height),
//   * overflowing columns: the footer pins to the bottom of the visible column
//     while the card list keeps its internal scroll (virtualized),
//   * the droppable drop zone always spans header → column bottom — including
//     empty columns, whose drop target must not collapse to a stub.
// Seeded via the API; assertions are geometry via page.evaluate
// (AGENTS §Frontend E2E Test Patterns — no waitForTimeout, no UI setup).

const TEST_EMAIL = 'e2e-addcard-placement@test.local';
const TEST_PASSWORD = 'Test1234!';
const SHORT_CARDS = 3;
const TALL_CARDS = 14;

monitoringTest.describe('Add-a-card placement (board view)', () => {
  test.setTimeout(120_000);

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

  monitoringTest('footer rides content bottom on short columns, visible bottom on overflowing ones', async ({
    page,
    monitoring,
  }) => {
    // Seed a board via the API — new boards ship 3 default columns:
    // [To Do, In Progress, Done].
    const boardRes = await page.request.post(`${E2E_API}/api/v1/boards`, {
      data: { name: `AddCard placement ${Date.now()}` },
    });
    expect(boardRes.ok()).toBe(true);
    const boardId = (await boardRes.json()).data.id;

    const colsRes = await page.request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`);
    const columns = (await colsRes.json()).data as Array<{ id: number }>;
    expect(columns).toHaveLength(3);
    const [shortCol, emptyCol, tallCol] = columns;

    const seed = async (columnId: number, count: number) => {
      for (let i = 0; i < count; i += 1) {
        const res = await page.request.post(`${E2E_API}/api/v1/cards`, {
          data: {
            title: `Placement card ${i + 1} — varying length titles shape card heights for the layout check`,
            column_id: columnId,
          },
        });
        expect(res.ok()).toBe(true);
      }
    };
    await seed(shortCol.id, SHORT_CARDS);
    await seed(tallCol.id, TALL_CARDS);

    // Guard the seed shape: the middle column must stay empty — the layout
    // assertion below depends on it.
    const verifyRes = await page.request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`);
    const verified = (await verifyRes.json()).data as Array<{ id: number; cards: unknown[] }>;
    expect(verified.find((c) => c.id === emptyCol.id)?.cards).toHaveLength(0);

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');

    const geom = () =>
      page.evaluate(() => {
        const fmt = (n: number) => Math.round(n * 10) / 10;
        const out = [] as Array<Record<string, unknown>>;
        for (const col of document.querySelectorAll('div[data-column-id]')) {
          const cr = col.getBoundingClientRect();
          const header = col.children[0].getBoundingClientRect();
          const zoneEl = col.querySelector('[data-testid="column-droppable"]');
          const zone = zoneEl ? zoneEl.getBoundingClientRect() : null;
          const btn = [...col.querySelectorAll('button')].find(
            (b) => (b.textContent || '').replace(/\s+/g, ' ').trim() === '+ Add a card',
          );
          const br = btn ? btn.getBoundingClientRect() : null;
          const rows = [...col.querySelectorAll('[data-testid="virtual-card-row"]')];
          const last = rows.length ? rows[rows.length - 1].getBoundingClientRect() : null;
          const scroller = col.querySelector('[data-testid="column-card-list-scroll"]');
          out.push({
            colTop: fmt(cr.top),
            colBottom: fmt(cr.bottom),
            colH: fmt(cr.height),
            headerBottom: fmt(header.bottom),
            zoneBottom: zone ? fmt(zone.bottom) : null,
            zoneH: zone ? fmt(zone.height) : null,
            btnTop: br ? fmt(br.top) : null,
            btnBottom: br ? fmt(br.bottom) : null,
            lastRowBottom: last ? fmt(last.bottom) : null,
            rows: rows.length,
            scroller: scroller ? { c: scroller.clientHeight, s: scroller.scrollHeight } : null,
          });
        }
        return out;
      });

    const [gShort, gEmpty, gTall] = (await geom()) as [
      {
        colTop: number; colBottom: number; colH: number; headerBottom: number;
        zoneBottom: number | null; zoneH: number | null;
        btnTop: number | null; btnBottom: number | null;
        lastRowBottom: number | null; rows: number;
        scroller: { c: number; s: number } | null;
      },
      {
        colTop: number; colBottom: number; colH: number; headerBottom: number;
        zoneBottom: number | null; zoneH: number | null;
        btnTop: number | null; btnBottom: number | null;
        lastRowBottom: number | null; rows: number;
        scroller: { c: number; s: number } | null;
      },
      {
        colTop: number; colBottom: number; colH: number; headerBottom: number;
        zoneBottom: number | null; zoneH: number | null;
        btnTop: number | null; btnBottom: number | null;
        lastRowBottom: number | null; rows: number;
        scroller: { c: number; s: number } | null;
      },
    ];
    expect(gShort).toBeDefined();
    expect(gEmpty).toBeDefined();
    expect(gTall).toBeDefined();

    // Short column: footer immediately after the last card (measured ~24 px),
    // far from the column's full-height bottom (was pinned: gap ≤ 16 px).
    expect(gShort.lastRowBottom).not.toBeNull();
    expect(gShort.btnTop! - gShort.lastRowBottom!).toBeLessThanOrEqual(60);
    expect(gShort.colBottom - gShort.btnBottom!).toBeGreaterThan(100);

    // Empty column: footer right below the header (measured ~24 px), far from
    // the column bottom.
    expect(gEmpty.btnTop! - gEmpty.headerBottom).toBeLessThanOrEqual(60);
    expect(gEmpty.colBottom - gEmpty.btnBottom!).toBeGreaterThan(100);

    // Overflowing column: footer pinned to the visible bottom, list scrolls
    // internally, and the virtual window stays bounded.
    expect(gTall.colBottom - gTall.btnBottom!).toBeLessThanOrEqual(20);
    expect(gTall.scroller).not.toBeNull();
    expect(gTall.scroller!.s).toBeGreaterThan(gTall.scroller!.c);
    expect(gTall.rows).toBeGreaterThan(0);
    expect(gTall.rows).toBeLessThan(40);

    // Drop zone spans header → column bottom in EVERY column (incl. empty):
    // bottom within 2 px of the column bottom and at least colH − 80 tall
    // (pre-fix it stopped at the pinned footer: colH − ~88).
    for (const g of [gShort, gEmpty, gTall]) {
      expect(g.zoneBottom).not.toBeNull();
      expect(g.zoneBottom!).toBeGreaterThanOrEqual(g.colBottom - 2);
      expect(g.zoneH!).toBeGreaterThan(g.colH - 80);
    }

    // T044 Stage-A containment guard: neither the document nor <main> scrolls —
    // the board keeps its bounded height chain with the new flex layout.
    const doc = await page.evaluate(() => ({
      scrollH: document.documentElement.scrollHeight,
      inner: window.innerHeight,
      mainScroll: (document.querySelector('main') as HTMLElement).scrollHeight,
      mainClient: (document.querySelector('main') as HTMLElement).clientHeight,
    }));
    expect(doc.scrollH).toBe(doc.inner);
    expect(doc.mainScroll).toBe(doc.mainClient);

    assertNoErrors(monitoring);
  });
});
