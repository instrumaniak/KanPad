import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '@testing-library/react';
import { ColumnCardList } from './column-card-list';
import type { Card } from '../cards/use-cards';

// Capture the SortableContext items prop — it must always be the FULL ordered id
// list, never the virtualized/visible slice (dnd-kit indexes into it; a slice
// would desync drag transforms and the keyboard sensor).
const sortable = vi.hoisted(() => ({ items: null as string[] | null }));

vi.mock('@dnd-kit/sortable', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/sortable')>();
  return {
    ...actual,
    SortableContext: ({ items, children }: { items: string[]; children?: React.ReactNode }) => {
      sortable.items = items;
      return <div data-testid="sortable-context">{children}</div>;
    },
  };
});

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>();
  return { ...actual, useDndMonitor: () => {} };
});

vi.mock('./use-columns', () => ({
  useLoadMoreColumnCards: () => ({ loadMore: vi.fn(), isFetching: false }),
  COLUMN_CARD_PAGE_SIZE: 50,
}));

// Keep the DOM light — the Card component's internals are covered by card.test.tsx.
vi.mock('../cards/card', () => ({
  Card: ({ card, index }: { card: { id: number; title: string }; index: number }) => (
    <div data-testid="card" data-card-index={index}>{card.title}</div>
  ),
}));

const makeCard = (id: number): Card => ({
  id,
  title: `Card ${id}`,
  column_id: 1,
  position: id,
  has_description: false,
  labels: [],
  due_date: null,
  checklist_progress: null,
  created_at: '2026-01-01T00:00:00.000Z',
  updated_at: '2026-01-01T00:00:00.000Z',
});

// jsdom reports clientWidth/clientHeight of 0, which makes the virtualizer
// render nothing at all. Give elements a real viewport size so a bounded
// window renders (mirroring a browser).
const VIEWPORT = { width: 320, height: 600 };

const makeCards = (n: number) => Array.from({ length: n }, (_, i) => makeCard(i + 1));

describe('ColumnCardList (virtualized)', () => {
  beforeEach(() => {
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: VIEWPORT.height });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: VIEWPORT.width });
    // jsdom reports offsetHeight of 0 → measured rows collapse and the virtual
    // window expands to cover everything. Report a realistic card height.
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 140 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: VIEWPORT.width });
  });
  afterEach(() => {
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 0 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 0 });
  });

  it('renders only a bounded window of rows for a 500-card column', () => {
    const { container } = render(<ColumnCardList cards={makeCards(500)} />);
    const rows = container.querySelectorAll('[data-testid="virtual-card-row"]');
    expect(rows.length).toBeGreaterThan(0);
    // ~4 visible rows at 128px estimate in a 600px viewport + overscan 5 ≈ 9-13 rows;
    // allow margin for re-measurement shrinking initial rows to 0 height.
    expect(rows.length).toBeLessThanOrEqual(16);
    expect(container.querySelector('[data-card-count="500"]')).not.toBeNull();
  });

  it('passes the FULL ordered id list to SortableContext (never the virtual slice)', () => {
    render(<ColumnCardList cards={makeCards(500)} />);
    expect(sortable.items).toHaveLength(500);
    expect(sortable.items![0]).toBe('card-1');
    expect(sortable.items![499]).toBe('card-500');
  });

  it('keeps the full-list index in the row data-index attributes', () => {
    const { container } = render(<ColumnCardList cards={makeCards(500)} />);
    const rows = [...container.querySelectorAll('[data-testid="virtual-card-row"]')];
    const indexes = rows.map((row) => Number(row.getAttribute('data-index')));
    expect(indexes).toEqual([...Array(rows.length).keys()]);
  });

  it('renders every row for a small column (no clipping below the window)', () => {
    const { container } = render(<ColumnCardList cards={makeCards(3)} />);
    expect(container.querySelectorAll('[data-testid="virtual-card-row"]')).toHaveLength(3);
  });

  it('renders nothing for an empty column', () => {
    const { container } = render(<ColumnCardList cards={[]} />);
    expect(container.querySelector('[data-card-count]')).toBeNull();
    expect(container.querySelectorAll('[data-testid="virtual-card-row"]')).toHaveLength(0);
  });
});
