import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import { PerfProfiler, resetPerfStore, snapshotPerfStore } from './perf-profiler';

describe('perf-profiler store', () => {
  beforeEach(() => {
    resetPerfStore();
  });

  it('records render entries with id and phase', () => {
    function Child() {
      return <div>child</div>;
    }
    render(
      <PerfProfiler id="TestComponent">
        <Child />
      </PerfProfiler>,
    );
    const snap = snapshotPerfStore();
    expect(snap.length).toBeGreaterThan(0);
    expect(snap[0].id).toBe('TestComponent');
    expect(snap[0].phase).toBe('mount');
    expect(Object.keys(snap[0])).not.toContain('props');
  });

  it('reset clears entries', () => {
    function Child() {
      return <div>child</div>;
    }
    render(
      <PerfProfiler id="TestComponent2">
        <Child />
      </PerfProfiler>,
    );
    resetPerfStore();
    expect(snapshotPerfStore()).toHaveLength(0);
  });
});
