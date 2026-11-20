/* eslint-disable @typescript-eslint/no-explicit-any */
// Perf measurement helpers (T046 F1/F2/F3).

export interface FrameSample { t: number; dt: number; }

export interface LoafEntry {
  firstUIEventTimestamp: number;
  presentationTime: number | null;
  paintTime: number | null;
  startTime: number;
  duration: number;
}

export interface LongTaskEntry {
  startTime: number;
  duration: number;
}

export interface PerfMeasurement {
  frames: FrameSample[];
  loafEntries: LoafEntry[];
  longTaskEntries: LongTaskEntry[];
  eventSamples: number[];
}

// Install browser-side perf instrumentation once per page. Idempotent.
// Observers/listeners run in the page (not Node) and only record while a
// measurement window is active.
export async function installPerfInstrumentation(page: any): Promise<void> {
  await page.evaluate(() => {
    const w = window as any;
    if (w.__kanpadPerf?.installed) return;

    const state = {
      installed: true,
      recording: false,
      frames: [] as FrameSample[],
      loaf: [] as LoafEntry[],
      longtasks: [] as LongTaskEntry[],
      eventSamples: [] as number[],
      frameLast: 0,
      loafObs: null as PerformanceObserver | null,
      longTaskObs: null as PerformanceObserver | null,
    };
    w.__kanpadPerf = state;

    const frameTick = () => {
      const now = performance.now();
      if (state.recording) {
        if (state.frameLast !== 0) state.frames.push({ t: now, dt: now - state.frameLast });
        state.frameLast = now;
      } else {
        state.frameLast = 0;
      }
      requestAnimationFrame(frameTick);
    };
    requestAnimationFrame(frameTick);

    document.addEventListener(
      'pointermove',
      (e) => {
        if (!state.recording) return;
        const t = e.timeStamp;
        requestAnimationFrame((now) => {
          // Attribute to the drag window that produced the event.
          state.eventSamples.push(now - t);
        });
      },
      { passive: true },
    );

    const supported = (typeof PerformanceObserver !== 'undefined' &&
      (PerformanceObserver as any).supportedEntryTypes) || [];

    if (supported.includes('long-animation-frame')) {
      try {
        const obs = new PerformanceObserver((list) => {
          if (!state.recording) return;
          for (const e of list.getEntries() as any[]) {
            state.loaf.push({
              firstUIEventTimestamp: e.firstUIEventTimestamp ?? 0,
              presentationTime: e.presentationTime ?? null,
              paintTime: e.paintTime ?? null,
              startTime: e.startTime ?? 0,
              duration: e.duration ?? 0,
            });
          }
        });
        obs.observe({ type: 'long-animation-frame', buffered: false });
        state.loafObs = obs;
      } catch {
        state.loafObs = null;
      }
    }

    if (supported.includes('longtask')) {
      try {
        const obs = new PerformanceObserver((list) => {
          if (!state.recording) return;
          for (const e of list.getEntries() as any[]) {
            state.longtasks.push({
              startTime: e.startTime ?? 0,
              duration: e.duration ?? 0,
            });
          }
        });
        obs.observe({ type: 'longtask', buffered: false });
        state.longTaskObs = obs;
      } catch {
        state.longTaskObs = null;
      }
    }
  });
}

// Reset buffers and begin recording a measurement window.
export async function startPerfMeasurement(page: any): Promise<void> {
  await installPerfInstrumentation(page);
  await page.evaluate(() => {
    const state = (window as any).__kanpadPerf;
    state.frames.length = 0;
    state.loaf.length = 0;
    state.longtasks.length = 0;
    state.eventSamples.length = 0;
    state.frameLast = 0;
    state.recording = true;
  });
}

// Stop recording and return serializable entries collected during the window.
// Includes a short settle so pending rAF/observer callbacks from the window flush.
export async function stopPerfMeasurement(page: any): Promise<PerfMeasurement> {
  // Allow in-flight frame/observer work from the measured interaction to land.
  await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));

  return page.evaluate(() => {
    const state = (window as any).__kanpadPerf;
    state.recording = false;

    // Flush any observer records queued before recording was disabled.
    try {
      for (const e of (state.loafObs?.takeRecords?.() ?? []) as any[]) {
        state.loaf.push({
          firstUIEventTimestamp: e.firstUIEventTimestamp ?? 0,
          presentationTime: e.presentationTime ?? null,
          paintTime: e.paintTime ?? null,
          startTime: e.startTime ?? 0,
          duration: e.duration ?? 0,
        });
      }
    } catch { /* ignore */ }
    try {
      for (const e of (state.longTaskObs?.takeRecords?.() ?? []) as any[]) {
        state.longtasks.push({
          startTime: e.startTime ?? 0,
          duration: e.duration ?? 0,
        });
      }
    } catch { /* ignore */ }

    state.frameLast = 0;
    return {
      frames: [...state.frames] as FrameSample[],
      loafEntries: [...state.loaf] as LoafEntry[],
      longTaskEntries: [...state.longtasks] as LongTaskEntry[],
      eventSamples: [...state.eventSamples] as number[],
    } satisfies PerfMeasurement;
  });
}

export async function cpuThrottle(page: any, rate: number, fn: () => Promise<void>) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate });
  try {
    await fn();
  } finally {
    await cdp.send('Emulation.setCPUThrottlingRate', { rate: 1 });
    await cdp.detach();
  }
}

export function median(nums: number[]) {
  const s = [...nums].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

export function p95(nums: number[]) {
  if (nums.length === 0) return 0;
  const s = [...nums].sort((a, b) => a - b);
  const idx = Math.ceil(0.95 * s.length) - 1;
  return s[Math.max(0, idx)];
}

export async function warmupAndMeasure<T>(run: () => Promise<T>, warmups = 1, measured = 3): Promise<T[]> {
  for (let i = 0; i < warmups; i++) await run();
  const out: T[] = [];
  for (let i = 0; i < measured; i++) out.push(await run());
  return out;
}

// Input-latency probe — PRIMARY (LoAF): `firstUIEventTimestamp → presentationTime`
// (falls back to `paintTime` when `presentationTime` is null — per MDN,
// paintTime is the more reliably populated value; `firstUIEventTimestamp == 0`
// = "no UI event this frame" → null). This is a WORST-CASE/LONG-FRAME probe:
// LoAF only fires for frames >50 ms, so it samples long frames only.
export function inputLatencyFromLoaf(entry: any): number | null {
  const first = entry.firstUIEventTimestamp;
  if (!first || first === 0) return null; // no UI event this frame
  const presentation = entry.presentationTime || entry.paintTime;
  if (!presentation) return null;
  return presentation - first;
}

// Pooled p95 — spec: "p95 over pooled pointermove samples across the measured
// runs, ≥100 moves/drag so p95 is meaningful."
export function pooledP95(pooled: number[]): number {
  return p95(pooled);
}
