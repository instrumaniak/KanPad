import { describe, it, expect, vi, beforeEach } from 'vitest';
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
    fireEvent.click(screen.getByRole('button', { name: /title/i }));
    let rows = screen.getAllByTestId('board-list-row');
    expect(rows[0]).toHaveTextContent('Alpha');
    fireEvent.click(screen.getByRole('button', { name: /title/i }));
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

  it('shows +N overflow when more than 3 labels', () => {
    const manyLabels = [1, 2, 3, 4, 5].map((i) => ({
      id: i,
      name: `L${i}`,
      color: 'red',
      created_at: '2024-01-01',
      updated_at: '2024-01-01',
    }));
    const colMany = { ...col1, cards: [{ ...col1.cards[0], labels: manyLabels }] };
    render(<BoardListView boardId={1} columns={[colMany, col2] as never} />);
    expect(screen.getByText('+2')).toBeInTheDocument();
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
});
