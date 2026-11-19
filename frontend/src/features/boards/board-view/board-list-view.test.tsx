import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BoardListView } from './board-list-view';

const mockMutateUpdate = vi.fn();
const mockMutateDelete = vi.fn();
const mockMutateCreate = vi.fn();
const mockToast = vi.fn();

vi.mock('../../cards/use-cards', () => ({
  useUpdateCard: () => ({ mutate: mockMutateUpdate }),
  useDeleteCard: () => ({ mutate: mockMutateDelete }),
  useCreateCard: () => ({ mutate: mockMutateCreate, isPending: false }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: mockToast }),
}));

vi.mock('../../cards/card-detail-panel', () => ({
  CardDetailPanel: ({ card, open }: { card: { title: string }; open: boolean }) =>
    open ? <div data-testid="detail-panel">{card.title}</div> : null,
}));

const col1 = {
  id: 1,
  name: 'To Do',
  position: 0,
  board_id: 1,
  created_at: '2024-01-01T00:00:00.000Z',
  updated_at: '2024-01-01T00:00:00.000Z',
  cards: [
    {
      id: 1,
      title: 'Beta',
      column_id: 1,
      position: 0,
      due_date: '2026-03-20',
      labels: [
        { id: 1, name: 'Bug', color: 'red', created_at: '2024-01-01', updated_at: '2024-01-01' },
        { id: 2, name: 'UI', color: 'blue', created_at: '2024-01-01', updated_at: '2024-01-01' },
      ],
      checklist_progress: { completed: 1, total: 2, percent: 50 },
      created_at: '2024-01-02T00:00:00.000Z',
      updated_at: '2024-01-03T00:00:00.000Z',
    },
  ],
};

const col2 = {
  id: 2,
  name: 'Done',
  position: 1,
  board_id: 1,
  created_at: '2024-01-01T00:00:00.000Z',
  updated_at: '2024-01-01T00:00:00.000Z',
  cards: [
    {
      id: 2,
      title: 'Alpha',
      column_id: 2,
      position: 0,
      due_date: null,
      labels: [],
      created_at: '2024-01-01T00:00:00.000Z',
      updated_at: '2024-01-01T00:00:00.000Z',
    },
  ],
};

describe('BoardListView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // jsdom reports zero-size scroll containers — give the scroll element a
    // real viewport so the virtualizer mounts a bounded window.
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 600 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 320 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 100 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 320 });
  });

  afterEach(() => {
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 0 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 0 });
  });

  it('renders flat rows with column badge, labels, due and progress', () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.getByText('To Do')).toBeInTheDocument();
    expect(screen.getByText('Done')).toBeInTheDocument();
    expect(screen.getByText('Bug')).toBeInTheDocument();
    expect(screen.getByText('1/2 (50%)')).toBeInTheDocument();
  });

  it('sorts by title asc/desc', () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByRole('button', { name: 'Title', exact: true }));
    let rows = screen.getAllByTestId('board-list-row');
    expect(rows[0]).toHaveTextContent('Alpha');
    fireEvent.click(screen.getByRole('button', { name: 'Title', exact: true }));
    rows = screen.getAllByTestId('board-list-row');
    expect(rows[0]).toHaveTextContent('Beta');
  });

  it('sorts null due dates last', () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByRole('button', { name: /^due/i }));
    const rows = screen.getAllByTestId('board-list-row');
    expect(rows[0]).toHaveTextContent('Beta');
    expect(rows[rows.length - 1]).toHaveTextContent('Alpha');
  });

  it('opens detail panel on row click', async () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByLabelText('Open card details: Beta'));
    await waitFor(() => {
      expect(screen.getByTestId('detail-panel')).toHaveTextContent('Beta');
    });
  });

  it('inline edit commits on Enter', async () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByText('Beta'));
    const input = screen.getByLabelText('Edit card title');
    fireEvent.change(input, { target: { value: 'Beta Updated' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => {
      expect(mockMutateUpdate).toHaveBeenCalledWith(
        { id: 1, data: { title: 'Beta Updated' } },
        expect.anything(),
      );
    });
  });

  it('delete flow calls delete mutation', async () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByLabelText('Delete card Beta'));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => {
      expect(mockMutateDelete).toHaveBeenCalledWith(1, expect.anything());
    });
  });

  it('shows empty state when no cards', () => {
    render(
      <BoardListView
        boardId={1}
        columns={
          [
            { ...col1, cards: [] },
            { ...col2, cards: [] },
          ] as never
        }
      />,
    );
    expect(screen.getByText('No cards yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /add card/i })).toBeInTheDocument();
  });

  it('keeps null due dates last in desc order', () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    const dueBtn = screen.getByRole('button', { name: /^due/i });
    fireEvent.click(dueBtn);
    fireEvent.click(dueBtn);
    const rows = screen.getAllByTestId('board-list-row');
    expect(rows[rows.length - 1]).toHaveTextContent('Alpha');
  });

  it('sorts by created date', () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    // default is created asc (Alpha first); clicking toggles to desc (Beta first)
    fireEvent.click(screen.getByRole('button', { name: /created/i }));
    const rows = screen.getAllByTestId('board-list-row');
    expect(rows[0]).toHaveTextContent('Beta');
  });

  it('cancels inline edit on Escape without mutating', async () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByText('Beta'));
    const input = screen.getByLabelText('Edit card title');
    fireEvent.change(input, { target: { value: 'Beta Changed' } });
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(mockMutateUpdate).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(screen.queryByLabelText('Edit card title')).not.toBeInTheDocument();
    });
  });

  it('commits only once on Enter then blur', async () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByText('Beta'));
    const input = screen.getByLabelText('Edit card title');
    fireEvent.change(input, { target: { value: 'Beta Once' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(mockMutateUpdate).toHaveBeenCalledTimes(1);
    });
  });

  it('shows all labels without overflow truncation', () => {
    const manyLabels = [1, 2, 3, 4, 5].map((i) => ({
      id: i,
      name: `L${i}`,
      color: 'red',
      created_at: '2024-01-01',
      updated_at: '2024-01-01',
    }));
    const colMany = { ...col1, cards: [{ ...col1.cards[0], labels: manyLabels }] };
    render(<BoardListView boardId={1} columns={[colMany, col2] as never} />);
    for (let i = 1; i <= 5; i++) {
      expect(screen.getByText(`L${i}`)).toBeInTheDocument();
    }
    expect(screen.queryByText(/\+\d+/)).not.toBeInTheDocument();
  });

  it('shows description icon when card has_description is true', () => {
    const colDesc = { ...col1, cards: [{ ...col1.cards[0], has_description: true }] };
    render(<BoardListView boardId={1} columns={[colDesc, col2] as never} />);
    expect(screen.getByLabelText('Has description')).toBeInTheDocument();
  });

  it('disables sort controls when empty', () => {
    render(
      <BoardListView boardId={1} columns={[{ ...col1, cards: [] }] as never} />,
    );
    expect(screen.getByRole('button', { name: /title/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /^due/i })).toBeDisabled();
  });

  it('creates card with selected column', async () => {
    render(<BoardListView boardId={1} columns={[col1, col2] as never} />);
    fireEvent.click(screen.getByRole('button', { name: /add card/i }));
    fireEvent.change(screen.getByLabelText('Card title'), {
      target: { value: 'New Card' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => {
      expect(mockMutateCreate).toHaveBeenCalledWith(
        { title: 'New Card', column_id: 1 },
        expect.anything(),
      );
    });
  });

  it('shows hint when no columns exist', () => {
    render(<BoardListView boardId={1} columns={[] as never} />);
    fireEvent.click(screen.getByRole('button', { name: /add card/i }));
    expect(screen.getByText(/no columns yet/i)).toBeInTheDocument();
  });

  describe('virtualization', () => {
    const bigColumn = (count: number) => ({
      ...col1,
      id: 1000 + count,
      cards: Array.from({ length: count }, (_, i) => ({
        ...col1.cards[0],
        id: i + 1,
        title: `Bulk ${i + 1}`,
        position: i,
        checklist_progress: null,
        labels: [],
        due_date: null,
      })),
    });

    it('renders only a bounded window of rows for a 300-card list, keeping full-list aria bookkeeping', () => {
      render(<BoardListView boardId={1} columns={[bigColumn(300)] as never} />);
      // aria-rowcount counts the full table: N data rows + 1 (sr-only header row)
      expect(screen.getByRole('table')).toHaveAttribute('aria-rowcount', '301');
      const rows = screen.getAllByTestId('board-list-row');
      expect(rows.length).toBeGreaterThan(0);
      expect(rows.length).toBeLessThanOrEqual(16);
      // aria-rowindex reflects the FULL list position, not the window (1 = header, data starts at 2)
      expect(rows[0]).toHaveAttribute('aria-rowindex', '2');
    });

    it('renders the editing+focused row exactly once when both pins coincide', () => {
      // Regression: startTitleEdit pins the row as both editing and focused —
      // the rangeExtractor union must not duplicate it. Scroll far down so the
      // pinned index is outside the base window (only then would the union
      // duplicate).
      const { container } = render(<BoardListView boardId={1} columns={[bigColumn(300)] as never} />);
      fireEvent.click(screen.getAllByText('Bulk 2')[0]);
      expect(screen.getByLabelText('Edit card title')).toBeInTheDocument();

      const scroller = container.querySelector('[data-testid="board-list-scroll"]') as HTMLElement;
      scroller.scrollTop = 96 * 250;
      fireEvent.scroll(scroller);
      expect(container.querySelectorAll('[data-index="1"]')).toHaveLength(1);
    });

    it('unpins a focused row once focus moves outside it (row virtualizes away)', () => {
      const { container } = render(<BoardListView boardId={1} columns={[bigColumn(300)] as never} />);
      const target = screen.getAllByLabelText(/open card details: Bulk 2/i)[0];
      fireEvent.focusIn(target);

      // Blur to an element OUTSIDE the row — the pin must release.
      const outside = document.createElement('button');
      outside.textContent = 'outside';
      container.appendChild(outside);
      fireEvent.blur(target, { relatedTarget: outside });

      const scroller = container.querySelector('[data-testid="board-list-scroll"]') as HTMLElement;
      scroller.scrollTop = 96 * 250;
      fireEvent.scroll(scroller);
      expect(container.querySelector('[data-index="1"]')).toBeNull();
      container.removeChild(outside);
    });

    it('keeps a focused row mounted when the list scrolls it out of the virtual window', () => {
      const { container } = render(<BoardListView boardId={1} columns={[bigColumn(300)] as never} />);
      // Focus the "Open card details" button of a row near the top
      const focusTarget = screen.getAllByLabelText(/open card details: Bulk 2/i)[0];
      fireEvent.focusIn(focusTarget);
      // Scroll far down so the focused row would leave the window
      const scroller = container.querySelector('[data-testid="board-list-scroll"]') as HTMLElement;
      scroller.scrollTop = 96 * 250;
      fireEvent.scroll(scroller);
      expect(container.querySelector('[data-testid="board-list-row"][data-index="1"]')).not.toBeNull();
    });

    it('keeps the interacted row mounted when the list scrolls it out of the virtual window', () => {
      // jsdom note: relocating a node via insertBefore resets document.activeElement
      // to null (jsdom-only artifact — browsers keep focus across relocations), which
      // ends the inline edit via the input's onBlur→commit path. The pin's guarantee
      // that still holds — and is asserted here — is that the row component tree
      // stays mounted instead of being virtualized away.
      const { container } = render(<BoardListView boardId={1} columns={[bigColumn(300)] as never} />);
      fireEvent.click(screen.getAllByText('Bulk 2')[0]);
      expect(screen.getByLabelText('Edit card title')).toBeInTheDocument();
      const scroller = container.querySelector('[data-testid="board-list-scroll"]') as HTMLElement;
      scroller.scrollTop = 96 * 250;
      fireEvent.scroll(scroller);
      expect(container.querySelector('[data-testid="board-list-row"][data-index="1"]')).not.toBeNull();
    });
  });
});
