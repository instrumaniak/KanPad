/* eslint-disable @typescript-eslint/no-explicit-any */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { resetPerfStore, snapshotPerfStore } from '@/lib/perf-profiler';
import { ColumnCardList } from '../../columns/column-card-list';
import { BoardView } from './board-view';

// F4: render-count budgets (vitest/jsdom). Durations are print-only because
// jsdom wall-clock is not deterministic. Counts are integer-deterministic.
// Note: vitest does not run the React Compiler preset (vite.config.ts:71-73
// applies it, vitest.config.ts:14 does not), so these counts guard
// identity-stability of uncompiled React, not compiled prod renders.

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: () => {} }),
  ToastProvider: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock('../../cards/card', () => ({
  Card: ({ card }: { card: { id: number; title: string } }) => (
    <div data-testid="card">{card.title}</div>
  ),
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getVirtualItems: () =>
      Array.from({ length: Math.min(count, 10) }, (_, i) => ({
        index: i,
        key: i,
        start: i * 128,
      })),
    getTotalSize: () => count * 128,
    measureElement: () => {},
  }),
  defaultRangeExtractor: (range: unknown) => range,
}));

vi.mock('@dnd-kit/core', async () => {
  const actual = await vi.importActual<typeof import('@dnd-kit/core')>('@dnd-kit/core');
  return {
    ...actual,
    useDndMonitor: () => {},
  };
});

// -- BoardView mocks (copied from board-view.test.tsx — same directory,
//    so the relative module paths are identical) ------------------------------
const mockNavigate = vi.fn();
const mockUseBoard = vi.fn();
const mockUseColumns = vi.fn();
const mockMutateAsync = vi.fn();
const mockUpdateMutate = vi.fn();
let mockBoardId = '1';

vi.mock('react-router-dom', () => ({
  useParams: () => ({ boardId: mockBoardId }),
  useNavigate: () => mockNavigate,
}));

vi.mock('../use-boards', () => ({
  useBoard: () => mockUseBoard(),
  useUpdateBoard: () => ({ mutate: mockUpdateMutate, isPending: false }),
}));

vi.mock('../../columns/use-columns', () => ({
  useColumns: () => mockUseColumns(),
  useCreateColumn: () => ({ mutateAsync: mockMutateAsync }),
  // ColumnHeader (rendered inside BoardView) needs these too — board-view.test.tsx
  // doesn't exercise the ColumnHeader path so it doesn't mock them.
  useUpdateColumn: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteColumn: () => ({ mutate: vi.fn(), isPending: false }),
  useSortCards: () => ({ mutate: vi.fn(), isPending: false }),
  useMoveAllCards: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('../../cards/drag-drop-context', () => ({
  DragDropContext: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="drag-context">{children}</div>
  ),
}));

vi.mock('../../notes/board-notes-sidebar', () => ({
  BoardNotesSidebar: () => <div data-testid="board-notes-sidebar" />,
}));

vi.mock('../../cards/use-cards', () => ({
  useUpdateCard: () => ({ mutate: vi.fn() }),
  useDeleteCard: () => ({ mutate: vi.fn() }),
  useCreateCard: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('./board-list-view', () => ({
  BoardListView: ({ columns }: { columns: { cards?: { title: string }[] }[] }) => (
    <div data-testid="board-list-view">
      {(columns ?? []).flatMap((c) => c.cards ?? []).map((card, i) => (
        <span key={i}>{card.title}</span>
      ))}
    </div>
  ),
}));

vi.mock('./filter-dropdown', () => ({
  FilterDropdown: () => null,
}));

vi.mock('./filter-chips', () => ({
  FilterChips: () => null,
}));

vi.mock('../../labels/use-labels', () => ({
  useLabels: () => ({ data: [{ name: 'Red', color: 'red' }] }),
}));

const mockColumn = {
  id: 1,
  name: 'To Do',
  position: 0,
  board_id: 1,
  cards: [],
  created_at: '2024-01-01',
  updated_at: '2024-01-01',
};

const mockBoardResponse = {
  data: {
    id: 1,
    name: 'Test Board',
    background_color: '#fff',
    project_id: null,
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  },
};

function mountCounts(id: string) {
  const entries = snapshotPerfStore().filter((e) => e.id === id);
  return {
    mounts: entries.filter((e) => e.phase === 'mount').length,
    updates: entries.filter((e) => e.phase === 'update').length,
    nested: entries.filter((e) => e.phase === 'nested-update').length,
    entries,
  };
}

describe('ColumnCardList render-count budget', () => {
  beforeEach(() => {
    resetPerfStore();
    mockUseBoard.mockReturnValue({ isLoading: false, data: mockBoardResponse });
    mockUseColumns.mockReturnValue({ isLoading: false, data: [mockColumn] });
  });

  it('mounts once on initial render', () => {
    const cards = Array.from({ length: 5 }, (_, i) => ({ id: i + 1, title: `Card ${i + 1}` }));
    const qc = new QueryClient();
    render(
      <QueryClientProvider client={qc}>
        <ColumnCardList cards={cards as any} />
      </QueryClientProvider>,
    );
    const counts = mountCounts('ColumnCardList');
    // print-only duration info
    const durations = counts.entries.map((e) => e.actualDuration);
    console.log(`[F4] ColumnCardList mount renders=${counts.mounts} durations=${durations.join(',')}`);
    expect(counts.mounts).toBeLessThanOrEqual(2);
  });

  it('render counts stay bounded under synthetic prop churn', () => {
    // "Churn": new card objects with the same ids on every rerender — a
    // realistic React Query cache refresh without remounting.
    const churn = () => Array.from({ length: 5 }, (_, i) => ({ id: i + 1, title: `Card ${i + 1}` }));
    const qc = new QueryClient();
    const tree = () => (
      <QueryClientProvider client={qc}>
        <ColumnCardList cards={churn() as any} />
      </QueryClientProvider>
    );
    const { rerender } = render(tree());
    for (let i = 0; i < 3; i++) rerender(tree());
    const counts = mountCounts('ColumnCardList');
    console.log(
      `[F4] ColumnCardList after churn: mounts=${counts.mounts} updates=${counts.updates} nested=${counts.nested}`,
    );
    expect(counts.mounts).toBeLessThanOrEqual(2);
  });
});

describe('BoardView render-count budget (F4)', () => {
  beforeEach(() => {
    resetPerfStore();
    Object.defineProperty(window, 'innerWidth', { writable: true, value: 1024 });
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
    mockBoardId = '1';
    mockUseBoard.mockReturnValue({ isLoading: false, data: mockBoardResponse });
    mockUseColumns.mockReturnValue({ isLoading: false, data: [mockColumn] });
  });

  it('BoardView mounts once on initial render', () => {
    const qc = new QueryClient();
    render(
      <QueryClientProvider client={qc}>
        <BoardView />
      </QueryClientProvider>,
    );
    const counts = mountCounts('BoardView');
    const durations = counts.entries.map((e) => e.actualDuration);
    console.log(`[F4] BoardView mount renders=${counts.mounts} durations=${durations.join(',')}`);
    expect(counts.mounts).toBeLessThanOrEqual(2);
  });
});
