/* eslint-disable react-refresh/only-export-components */
import { Profiler, type ProfilerOnRenderCallback, type ReactNode } from 'react';

export interface PerfRenderEntry {
  id: string;
  phase: 'mount' | 'update' | 'nested-update';
  actualDuration: number;
  baseDuration: number;
  startTime: number;
  commitTime: number;
  meta?: Record<string, unknown>;
}

export interface PerfMarkEntry {
  name: string;
  at: number;
  data?: Record<string, unknown>;
}

export interface PerfStore {
  entries: PerfRenderEntry[];
  marks: PerfMarkEntry[];
  record: (entry: PerfRenderEntry) => void;
  mark: (name: string, data?: Record<string, unknown>) => void;
  reset: () => void;
  snapshot: () => PerfRenderEntry[];
  snapshotMarks: () => PerfMarkEntry[];
}

declare global {
  interface Window {
    __PERF__?: PerfStore;
  }
}

function createPerfStore(): PerfStore {
  const entries: PerfRenderEntry[] = [];
  const marks: PerfMarkEntry[] = [];
  return {
    entries,
    marks,
    record: (entry) => {
      entries.push(entry);
    },
    mark: (name, data) => {
      marks.push({ name, at: typeof performance !== 'undefined' ? performance.now() : Date.now(), data });
    },
    reset: () => {
      entries.length = 0;
      marks.length = 0;
    },
    snapshot: () => [...entries],
    snapshotMarks: () => [...marks],
  };
}

function getStore(): PerfStore {
  if (typeof window !== 'undefined') {
    if (!window.__PERF__) {
      window.__PERF__ = createPerfStore();
    }
    // Back-compat for stores created before marks existed.
    if (!window.__PERF__.marks) {
      window.__PERF__.marks = [];
      window.__PERF__.mark = (name: string, data?: Record<string, unknown>) => {
        window.__PERF__!.marks.push({
          name,
          at: typeof performance !== 'undefined' ? performance.now() : Date.now(),
          data,
        });
      };
      window.__PERF__.snapshotMarks = () => [...(window.__PERF__?.marks ?? [])];
      const prevReset = window.__PERF__.reset;
      window.__PERF__.reset = () => {
        prevReset();
        window.__PERF__!.marks.length = 0;
      };
    }
    return window.__PERF__;
  }
  return createPerfStore();
}

export interface PerfProfilerProps {
  id: string;
  meta?: Record<string, unknown>;
  children: ReactNode;
}

export function PerfProfiler({ id, meta, children }: PerfProfilerProps) {
  if (!import.meta.env.DEV) {
    return <>{children}</>;
  }
  const onRender: ProfilerOnRenderCallback = (profilerId, phase, actualDuration, baseDuration, startTime, commitTime) => {
    getStore().record({
      id: profilerId,
      phase: phase as PerfRenderEntry['phase'],
      actualDuration,
      baseDuration,
      startTime,
      commitTime,
      meta,
    });
  };
  return (
    <Profiler id={id} onRender={onRender}>
      {children}
    </Profiler>
  );
}

export function resetPerfStore() {
  getStore().reset();
}

export function snapshotPerfStore() {
  return getStore().snapshot();
}

export function markPerfEvent(name: string, data?: Record<string, unknown>) {
  if (!import.meta.env.DEV) return;
  getStore().mark(name, data);
}
