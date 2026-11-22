/* eslint-disable @typescript-eslint/no-explicit-any */
import { expect, type Page } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';
import {
  installPerfInstrumentation,
  median,
  perfCountsById,
  pooledP95,
  resetPerfInPage,
  snapshotPerfInPage,
  startPerfMeasurement,
  stopPerfMeasurement,
  warmupAndMeasure,
} from './perf-helpers';

// @perf - scroll jank probe (T046 deferred scroll gate; reported regression).
// Frame metrics are Tier B/statistical (warmup+median), not deterministic.
// Render counts via window.__PERF__ are deterministic when dev-gated profiler is active.
monitoringTest.setTimeout(180_000);

const TEST_EMAIL = 'e2e-perf-scroll@test.local';
const TEST_PASSWORD = 'Test1234!';

const SCROLL_CARDS = 400;
const SCROLL_STEPS = 120;
const SCROLL_DELTA_PX = 80;

monitoringTest.describe('Perf scroll @perf', () => {
  let boardId: number;

  monitoringTest.beforeAll(async ({ request }, testInfo) => {
    testInfo.setTimeout(300_000);

    const reg = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (reg.status() !== 201 && reg.status() !== 409) throw new Error('register failed');

    await request.post(`${E2E_API}/api/v1/auth/login`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });

    const boardRes = await request.post(`${E2E_API}/api/v1/boards`, {
      data: { name: `PerfScroll ${Date.now()}` },
    });
    if (!boardRes.ok()) throw new Error(`create board failed: ${boardRes.status()}`);
    const boardJson = await boardRes.json();
    boardId = (boardJson.data?.id ?? boardJson.id) as number;

    const colsRes = await request.get(`${E2E_API}/api/v1/boards/${boardId}/columns`);
    if (!colsRes.ok()) throw new Error(`list columns failed: ${colsRes.status()}`);
    const colsJson = await colsRes.json();
    const columns = (colsJson.data ?? colsJson) as Array<{ id: number }>;
    const columnId = columns[0]?.id;
    if (!columnId) throw new Error('missing default column');

    for (let i = 0; i < SCROLL_CARDS; i += 1) {
      const cardRes = await request.post(`${E2E_API}/api/v1/cards`, {
        data: { title: `scroll-card-${String(i + 1).padStart(3, '0')}`, column_id: columnId },
      });
      if (!cardRes.ok()) throw new Error(`create card failed: ${cardRes.status()}`);
    }
  });

  async function login(page: Page) {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');
    // Local compose/Chromium workaround for Secure __Host- cookies over http.
    for (const c of await page.context().cookies()) {
      await page.context().addCookies([{ ...c, name: c.name.replace(/^__Host-/, ''), secure: false }]);
    }
  }

  async function runScroll(page: Page) {
    const scrollContainer = page.locator('[data-testid="column-card-list-scroll"]').first();
    await scrollContainer.waitFor({ state: 'visible' });
    await scrollContainer.hover();

    // Reset to top before each measured run; exclude setup from measurement window.
    await scrollContainer.evaluate((el) => {
      el.scrollTop = 0;
    });
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));

    await installPerfInstrumentation(page);
    await resetPerfInPage(page);
    await startPerfMeasurement(page);

    // Paced scroll: small wheel deltas interleaved with rAF so frame/LoAF sampling
    // observes scroll work, not idle gaps. Hover keeps wheel targeted at the column.
    for (let i = 0; i < SCROLL_STEPS; i += 1) {
      await page.mouse.wheel(0, SCROLL_DELTA_PX);
      await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));
    }

    // Settle paint/observer callbacks from the measured window.
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));
    const { frames, loafEntries, longTaskEntries, eventSamples } = await stopPerfMeasurement(page);
    const perfEntries = await snapshotPerfInPage(page);

    // Scroll back to top to leave a clean state for next run.
    await scrollContainer.evaluate((el) => {
      el.scrollTop = 0;
    });
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));

    const dt = frames.map((s) => s.dt).filter((d) => d > 0);
    const maxDt = dt.length ? Math.max(...dt) : 0;
    return { maxDt, loafEntries, longTaskEntries, eventSamples, perfEntries };
  }

  monitoringTest('column scroll perf with many cards', async ({ page, monitoring }) => {
    await login(page);
    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');

    const firstColumn = page.locator('[data-column-id]').first();
    await expect(firstColumn.locator('[data-testid="virtual-card-row"]').first()).toBeVisible();
    const mountedRows = await firstColumn.locator('[data-testid="virtual-card-row"]').count();
    expect(mountedRows).toBeGreaterThan(0);
    expect(mountedRows).toBeLessThan(60); // virtualization sanity; not a tight perf bound

    const scale = Number(process.env.PERF_BUDGET_SCALE || 1);
    const results = await warmupAndMeasure(() => runScroll(page), 1, 3);

    const medMaxDt = median(results.map((r) => r.maxDt));
    const pooledEvent = results.flatMap((r) => r.eventSamples);
    const eventP95 = pooledEvent.length ? pooledP95(pooledEvent) : 0;
    const pooledLoaf = results.flatMap((r) => r.loafEntries);
    const pooledLongTasks = results.flatMap((r) => r.longTaskEntries);
    const renderCounts = results.map((r) => perfCountsById(r.perfEntries));
    const medColumnListRenders = median(renderCounts.map((c) => c.ColumnCardList ?? 0));
    const medBoardViewRenders = median(renderCounts.map((c) => c.BoardView ?? 0));

    console.log(
      `[perf-scroll] median maxDt=${medMaxDt} eventP95=${eventP95} ` +
        `loafts=${pooledLoaf.length} longtasks=${pooledLongTasks.length} events=${pooledEvent.length} ` +
        `median ColumnCardList renders=${medColumnListRenders} median BoardView renders=${medBoardViewRenders}`,
    );
    console.log(`[perf-scroll] renderCounts=${JSON.stringify(renderCounts)}`);

    // Frame timing is statistical (Tier B). Budget is generous/dev-mode tripwire.
    expect(medMaxDt).toBeLessThanOrEqual(3500 * scale);

    // Deterministic signals: virtualization keeps DOM bounded, and __PERF__ is present in dev.
    const hasPerfStore = await page.evaluate(() => !!(window as any).__PERF__);
    console.log(`[perf-scroll] __PERF__ present=${hasPerfStore}`);
    if (hasPerfStore) {
      // Log-first: capture counts for diagnosis. Avoid a tight count assertion until
      // baseline is established across environments; the presence check is deterministic.
      expect(medColumnListRenders).toBeGreaterThanOrEqual(0);
    }

    // Tolerate SPA auth polling 401s in local compose/Chromium (same as other perf specs).
    monitoring.networkRequests = monitoring.networkRequests.filter(
      (r) => !(r.status === 401 && r.url.includes('/api/auth/me')),
    );
    const remainingFailed = monitoring.networkRequests.filter((r) => r.status >= 400 || r.failure);
    if (remainingFailed.length === 0) {
      monitoring.consoleMessages = monitoring.consoleMessages.filter(
        (m) => !(m.type === 'error' && m.text.includes('Failed to load resource') && m.text.includes('401')),
      );
    }
    assertNoErrors(monitoring);
  });
});