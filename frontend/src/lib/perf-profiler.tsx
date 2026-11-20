/* eslint-disable react-refresh/only-export-components */
import { Profiler, type ProfilerOnRenderCallback, type ReactNode } from 'react';

export interface PerfRenderEntry {
  id: string;
  phase: 'mount' | 'update' | 'nested-update';
  actualDuration: number;
  baseDuration: number;
  startTime: number;
  commitTime: number;
}

export interface PerfStore {
  entries: PerfRenderEntry[];
  record: (entry: PerfRenderEntry) => void;
  reset: () => void;
  snapshot: () => PerfRenderEntry[];
}

declare global {
  interface Window {
    __PERF__?: PerfStore;
  }
}

function createPerfStore(): PerfStore {
  const entries: PerfRenderEntry[] = [];
  return {
    entries,
    record: (entry) => {
      entries.push(entry);
    },
    reset: () => {
      entries.length = 0;
    },
    snapshot: () => [...entries],
  };
}

function getStore(): PerfStore {
  if (typeof window !== 'undefined') {
    if (!window.__PERF__) {
      window.__PERF__ = createPerfStore();
    }
    return window.__PERF__;
  }
  return createPerfStore();
}

const onRender: ProfilerOnRenderCallback = (id, phase, actualDuration, baseDuration, startTime, commitTime) => {
  getStore().record({
    id,
    phase: phase as PerfRenderEntry['phase'],
    actualDuration,
    baseDuration,
    startTime,
    commitTime,
  });
};

export interface PerfProfilerProps {
  id: string;
  children: ReactNode;
}

export function PerfProfiler({ id, children }: PerfProfilerProps) {
  if (!import.meta.env.DEV) {
    return <>{children}</>;
  }
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
