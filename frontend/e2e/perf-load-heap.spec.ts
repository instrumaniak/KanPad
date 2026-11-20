import { expect } from '@playwright/test';
import { E2E_API } from './api-base';
import { monitoringTest, assertNoErrors } from './test-utils';

// @perf
monitoringTest.setTimeout(180_000);

const TEST_EMAIL = 'e2e-perf-heap@test.local';
const TEST_PASSWORD = 'Test1234!';

// F3: boards created by the perf specs accumulate (React Query cache growth
// skews later heap/load numbers) — deleted in afterAll.
let loadBoardId: number | null = null;
let heapBoardId: number | null = null;

monitoringTest.afterAll(async ({ request }) => {
  for (const id of [loadBoardId, heapBoardId].filter((id): id is number => id !== null)) {
    await request.delete(`${E2E_API}/api/v1/boards/${id}`);
  }
});

monitoringTest.describe('Perf load+heap @perf', () => {
  monitoringTest.beforeAll(async ({ request }) => {
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) throw new Error('register failed');
  });

  monitoringTest('navToNetworkIdleMs budget', async ({ page, monitoring }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');
    // Local compose/Chromium workaround for Secure __Host- cookies over http.
    for (const c of await page.context().cookies()) {
      await page.context().addCookies([{ ...c, name: c.name.replace(/^__Host-/, ''), secure: false }]);
    }

    const boardRes = await page.request.post(`${E2E_API}/api/v1/boards`, { data: { name: `PerfLoad ${Date.now()}` } });
    const boardId = (await boardRes.json()).data.id as number;
    loadBoardId = boardId;

    const startedAt = Date.now();
    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    const navToNetworkIdleMs = Date.now() - startedAt;
    console.log(`[F2] navToNetworkIdleMs=${navToNetworkIdleMs}ms`);
    const scale = Number(process.env.PERF_BUDGET_SCALE || 1);
    // dev-mode tripwire, generous
    expect(navToNetworkIdleMs).toBeLessThanOrEqual(3000 * scale);
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

  monitoringTest('heap growth guard', async ({ page, monitoring }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');
    // Local compose/Chromium workaround for Secure __Host- cookies over http.
    for (const c of await page.context().cookies()) {
      await page.context().addCookies([{ ...c, name: c.name.replace(/^__Host-/, ''), secure: false }]);
    }

    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    await cdp.send('HeapProfiler.enable');
    const forceGC = async () => {
      await cdp.send('HeapProfiler.collectGarbage');
      await cdp.send('HeapProfiler.collectGarbage');
      await page.evaluate(() => new Promise((r) => setTimeout(r, 200)));
    };

    const boardRes = await page.request.post(`${E2E_API}/api/v1/boards`, { data: { name: `PerfHeap ${Date.now()}` } });
    const boardId = (await boardRes.json()).data.id as number;
    heapBoardId = boardId;

    await forceGC();
    const before = await cdp.send('Performance.getMetrics');
    const heapBefore = (before.metrics || []).find((m: {name:string;value:number}) => m.name === 'JSHeapUsedSize')?.value || 0;

    for (let i = 0; i < 10; i++) {
      await page.goto(`/board/${boardId}`);
      await page.waitForLoadState('networkidle');
      await page.goto('/');
      await page.waitForLoadState('networkidle');
    }

    await forceGC();
    const after = await cdp.send('Performance.getMetrics');
    const heapAfter = (after.metrics || []).find((m: {name:string;value:number}) => m.name === 'JSHeapUsedSize')?.value || 0;
    const delta = heapAfter - heapBefore;
    console.log(`[F3] heap delta=${(delta / 1024 / 1024).toFixed(2)}MB`);
    const scale = Number(process.env.PERF_BUDGET_SCALE || 1);
    expect(delta).toBeLessThanOrEqual(5 * 1024 * 1024 * scale);

    await cdp.detach();
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
