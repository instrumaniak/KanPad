import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import type { Card } from './use-cards';
import type { Column } from '../columns/columns.api';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DragDropContext } from './drag-drop-context';
import React from 'react';
import type { DropAnimation } from '@dnd-kit/core';

const mockMutations = vi.hoisted(() => ({
  moveCardMutateAsync: vi.fn().mockResolvedValue({}),
  reorderCardMutateAsync: vi.fn().mockResolvedValue({}),
}));

vi.mock('./use-cards', () => ({
  useMoveCard: () => ({ mutateAsync: mockMutations.moveCardMutateAsync }),
  useReorderCard: () => ({ mutateAsync: mockMutations.reorderCardMutateAsync }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

const useSensorSpy = vi.hoisted(() =>
  vi.fn().mockImplementation((_sensor: unknown, options?: unknown) => ({
    sensor: _sensor,
    options,
  })),
);

// Captures the props passed to DndContext (onDragStart/onDragOver/onDragEnd/onDragCancel)
// so tests can drive synthetic dnd-kit events without simulating pointer input.
const dndHandlers = vi.hoisted(() => ({
  current: null as null | {
    onDragStart?: (event: unknown) => void;
    onDragOver?: (event: unknown) => void;
    onDragEnd?: (event: unknown) => void;
    onDragCancel?: () => void;
  },
}));

const rafQueue = vi.hoisted(() => ({ queue: [] as Array<() => void> }));

const dragOverlayProps = vi.hoisted(() => ({ dropAnimation: null as DropAnimation | null }));

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>();
  return {
    ...actual,
    useSensors: (...args: unknown[]) => args,
    useSensor: useSensorSpy,
    DndContext: ({ children, ...props }: { children?: React.ReactNode } & Record<string, unknown>) => {
      dndHandlers.current = props as typeof dndHandlers.current;
      return <div data-testid="dnd-context">{children}</div>;
    },
    DragOverlay: vi.fn(({ dropAnimation, children }) => {
      dragOverlayProps.dropAnimation = dropAnimation;
      return <div data-testid="drag-overlay">{children}</div>;
    }),
  };
});

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient();
  return render(<QueryClientProvider client={queryClient}>{component}</QueryClientProvider>);
};

describe('DragDropContext', () => {
  let originalMatchMedia: typeof window.matchMedia;
  beforeEach(() => {
    originalMatchMedia = window.matchMedia;
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
    dragOverlayProps.dropAnimation = null;
  });
  afterEach(() => {
    if (originalMatchMedia) {
      Object.defineProperty(window, 'matchMedia', {
        writable: true,
        value: originalMatchMedia,
      });
    }
  });

  it('renders children', () => {
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div data-testid="children">Drag content</div>
      </DragDropContext>
    );
    expect(screen.getByTestId('children')).toBeInTheDocument();
  });

  it('configures TouchSensor so tap-and-hold opens context menu instead of drag', () => {
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div>Content</div>
      </DragDropContext>
    );

    const touchCall = useSensorSpy.mock.calls.find(
      (call: [{ name: string }]) => call[0]?.name === 'TouchSensor',
    );
    expect(touchCall).toBeDefined();
    // Radix ContextMenu long-press is 700ms — drag delay must exceed it.
    expect(touchCall[1].activationConstraint.delay).toBeGreaterThan(700);
    expect(touchCall[1].activationConstraint.delay).toBe(750);
    expect(touchCall[1].activationConstraint.tolerance).toBe(8);
  });

  it('provides bounce easing drop animation to DragOverlay', () => {
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div>Content</div>
      </DragDropContext>
    );
    expect(dragOverlayProps.dropAnimation).toBeDefined();
    expect(dragOverlayProps.dropAnimation.duration).toBe(300);
    expect(dragOverlayProps.dropAnimation.easing).toBe('cubic-bezier(0.34, 1.56, 0.64, 1)');
  });

  // P4 — dragOver cache-write throttling (T021)
  describe('dragOver cache writes', () => {
    beforeEach(() => {
      rafQueue.queue = [];
      vi.stubGlobal('requestAnimationFrame', (cb: () => void) => {
        rafQueue.queue.push(cb);
        return rafQueue.queue.length;
      });
      vi.stubGlobal('cancelAnimationFrame', () => {
        rafQueue.queue = [];
      });
      dndHandlers.current = null;
    });

    afterEach(() => {
      // NB: handleDragEnd schedules a real 300 ms setTimeout (clears activeCard);
      // it fires after the test finishes and React treats the setState on the
      // unmounted tree as a no-op — the settled-animation path is untested here.
      vi.unstubAllGlobals();
    });

    const makeCard = (id: number, columnId: number, position: number): Card => ({
      id,
      title: `Card ${id}`,
      column_id: columnId,
      position,
      has_description: false,
      labels: [],
      due_date: null,
      checklist_progress: null,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
    });

    const makeColumn = (id: number, position: number, cards: Card[]): Column => ({
      id,
      name: `Col ${id}`,
      position,
      board_id: 1,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
      cards,
    });

    const seedColumns = (queryClient: QueryClient) => {
      const cardA = makeCard(10, 1, 0);
      const cardB = makeCard(11, 1, 1);
      const cardC = makeCard(12, 2, 0);
      queryClient.setQueryData<Column[]>(['columns', 1], [
        makeColumn(1, 0, [cardA, cardB]),
        makeColumn(2, 1, [cardC]),
        makeColumn(3, 2, []),
      ]);
      return { cardA, cardB, cardC };
    };

    const renderHarness = () => {
      const queryClient = new QueryClient();
      seedColumns(queryClient);
      const setQueryDataSpy = vi.spyOn(queryClient, 'setQueryData');
      render(
        <QueryClientProvider client={queryClient}>
          <DragDropContext boardId={1}>
            <div>Board</div>
          </DragDropContext>
        </QueryClientProvider>,
      );
      return { queryClient, setQueryDataSpy };
    };

    const startDrag = (card: Card, index = 0) => {
      act(() => {
        dndHandlers.current?.onDragStart?.({
          active: { data: { current: { cardId: card.id, sourceColumnId: card.column_id, card, index } } },
        });
      });
    };

    const dragOverToColumn = (columnId: number) => {
      act(() => {
        dndHandlers.current?.onDragOver?.({
          active: { data: { current: { cardId: 10, sourceColumnId: 1, card: makeCard(10, 1, 0) } } },
          over: { id: `column-${columnId}`, data: { current: { columnId } } },
        });
      });
    };

    const flushRaf = () => {
      act(() => {
        const cb = rafQueue.queue.shift();
        if (cb) cb();
      });
    };

    const flushAllRaf = () => {
      // Tripwire against an infinite rAF re-scheduling regression
      let ticks = 0;
      while (rafQueue.queue.length > 0) {
        ticks += 1;
        if (ticks > 50) throw new Error('flushAllRaf: rAF callback kept re-scheduling itself');
        flushRaf();
      }
    };

    it('coalesces a synthetic >=30-event cross-column dragOver burst into <= 1 write per rAF tick', () => {
      const { queryClient, setQueryDataSpy } = renderHarness();
      const seeded = queryClient.getQueryData<Column[]>(['columns', 1]);
      startDrag(seeded![0].cards[0]);
      setQueryDataSpy.mockClear();

      // 30 cross-column dragOver events fired faster than rAF ticks (as during a real drag),
      // alternating the hovered column. No frame ever fires mid-burst, so the single
      // pending slot just keeps the LAST event (column 3).
      for (let i = 0; i < 30; i += 1) {
        dragOverToColumn(i % 2 === 0 ? 2 : 3);
      }
      // No cache write may happen synchronously during the burst.
      expect(setQueryDataSpy).toHaveBeenCalledTimes(0);

      flushAllRaf();
      // The burst coalesced into a single pending frame: exactly one write,
      // at most rAF-ticks + 1, and the live rearrangement did still happen.
      expect(rafQueue.queue).toHaveLength(0);
      expect(setQueryDataSpy.mock.calls).toHaveLength(1);
      const after = queryClient.getQueryData<Column[]>(['columns', 1]);
      // Card ended up in the last hovered column (column 3)
      expect(after![2].cards.map((c) => c.id)).toContain(10);
    });

    it('skips the cache write when no card order changed (same-column steady hover)', () => {
      const { queryClient, setQueryDataSpy } = renderHarness();
      const seeded = queryClient.getQueryData<Column[]>(['columns', 1]);
      startDrag(seeded![0].cards[0]);

      // Move card into column 2 once (live rearrangement)
      dragOverToColumn(2);
      flushAllRaf();
      const cacheAfterMove = queryClient.getQueryData<Column[]>(['columns', 1]);
      setQueryDataSpy.mockClear();

      // Repeated identical events while hovering the same target: no order changes,
      // so no cache write may be issued at all.
      for (let i = 0; i < 10; i += 1) {
        dragOverToColumn(2);
      }
      flushAllRaf();
      expect(setQueryDataSpy).toHaveBeenCalledTimes(0);
      expect(queryClient.getQueryData(['columns', 1])).toBe(cacheAfterMove);
    });

    it('keeps unchanged card object references across cross-column dragOver writes', () => {
      // NOTE: these identity assertions also pass through React Query's
      // replaceEqualDeep structural share, which returns the prev reference
      // anyway — the strict oracle for the helper's contract is
      // drag-over-cache.test.ts. Here we assert the end-to-end cache state.
      const { queryClient, setQueryDataSpy } = renderHarness();
      const seeded = queryClient.getQueryData<Column[]>(['columns', 1]);
      startDrag(seeded![0].cards[0]);

      dragOverToColumn(2);
      flushAllRaf();
      const after = queryClient.getQueryData<Column[]>(['columns', 1]);
      // Uninvolved column keeps identity
      expect(after![2]).toBe(seeded![2]);
      // Target column (2): pre-existing card did not change position (insert after it? card lands at index 1) — identity kept when position unchanged
      expect(after![1].cards.map((c) => c.id)).toEqual([12, 10]);
      expect(after![1].cards[0]).toBe(seeded![1].cards[0]); // cardC position 0 unchanged => same object
      expect(setQueryDataSpy.mock.calls.length).toBeGreaterThanOrEqual(1);
    });

    it('cancels a pending dragOver frame on dragEnd so no stale write lands after the drop', async () => {
      const { queryClient, setQueryDataSpy } = renderHarness();
      const seeded = queryClient.getQueryData<Column[]>(['columns', 1]);
      startDrag(seeded![0].cards[0]);
      setQueryDataSpy.mockClear();

      dragOverToColumn(3); // pending frame targets column 3 — DIFFERENT from the drop's column 2,
      // so a leaked stale frame is detectable as a wrong-column write.
      expect(rafQueue.queue).toHaveLength(1);
      await act(async () => {
        dndHandlers.current?.onDragEnd?.({
          active: { data: { current: { cardId: 10, sourceColumnId: 1, card: makeCard(10, 1, 0), index: 0 } } },
          over: { id: 'column-2', data: { current: { columnId: 2 } } },
        });
      });
      // The pending frame must have been canceled — nothing left queued.
      expect(rafQueue.queue).toHaveLength(0);
      // Every post-drop write comes from dragEnd's setColumns updater function;
      // a flushed stale dragOver frame would instead pass a concrete Column[] array.
      for (const call of setQueryDataSpy.mock.calls) {
        expect(typeof call[1]).toBe('function');
      }
      const after = queryClient.getQueryData<Column[]>(['columns', 1]);
      // Card landed in column 2 (the drop target), never in column 3 (the stale frame's target).
      expect(after![1].cards.map((c) => c.id)).toContain(10);
      expect(after![2].cards.map((c) => c.id)).not.toContain(10);
    });

    it('a cross-column round-trip (A→B→A) then same-column drop reorders the right card and persists', async () => {
      const { queryClient } = renderHarness();
      const seeded = queryClient.getQueryData<Column[]>(['columns', 1]);
      const cardA = seeded![0].cards[0];
      startDrag(cardA);

      // Round trip: col1 → col2 → col1 (card ends up at index 1, after B)
      dragOverToColumn(2);
      flushAllRaf();
      dragOverToColumn(1);
      flushAllRaf();
      expect(queryClient.getQueryData<Column[]>(['columns', 1])![0].cards.map((c) => c.id)).toEqual([11, 10]);

      mockMutations.reorderCardMutateAsync.mockClear();
      await act(async () => {
        dndHandlers.current?.onDragEnd?.({
          active: { data: { current: { cardId: 10, sourceColumnId: 1, card: cardA, index: 0 } } },
          over: { id: 'card-11', data: { current: { card: seeded![0].cards[1] } } },
        });
      });
      // Dropped on card 11 → live index lookup puts the dragged card at index 0
      // (dragStart's stale index 0 would no-op and silently skip the reorder).
      expect(mockMutations.reorderCardMutateAsync).toHaveBeenCalledWith({ id: 10, position: 0 });
      const after = queryClient.getQueryData<Column[]>(['columns', 1]);
      expect(after![0].cards.map((c) => c.id)).toEqual([10, 11]);
    });
  });

  it('renders with reduced motion preference', () => {
    // Mock matchMedia to return true for prefers-reduced-motion
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: query === '(prefers-reduced-motion: reduce)',
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div>Content</div>
      </DragDropContext>
    );
    // Ensure component renders without error
    expect(screen.getByText('Content')).toBeInTheDocument();
    // dropAnimation duration should be 1 (or minimal) for reduced motion
    expect(dragOverlayProps.dropAnimation).toBeDefined();
    expect(dragOverlayProps.dropAnimation.duration).toBe(1);
  });
});
