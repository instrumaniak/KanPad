import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { Column } from './column';
import type { Column as ColumnType } from './use-columns';

vi.mock('./column-card-list', () => ({
  ColumnCardList: ({ cards, newCardId }: { cards: unknown[]; newCardId?: number }) => (
    <div data-testid="card-list" data-new-card-id={newCardId} data-card-count={cards.length}>
      {cards.length > 0 ? `${cards.length} card(s)` : 'No cards'}
    </div>
  ),
}));

vi.mock('./column-header', () => ({
  ColumnHeader: ({ column: col }: { column: ColumnType }) => (
    <div data-testid="column-header">{col.name}</div>
  ),
}));

vi.mock('./column-droppable', () => ({
  ColumnDroppable: ({
    columnId,
    children,
  }: {
    columnId: number;
    children: (props: { isOver: boolean; setNodeRef: (node: HTMLElement | null) => void }) => React.ReactNode;
  }) => (
    <div data-testid="droppable-zone" data-column-id={columnId}>
      {children({ isOver: false, setNodeRef: vi.fn() })}
    </div>
  ),
}));

vi.mock('../cards/add-card-input', () => ({
  AddCardInput: ({
    columnId,
    nextColumnId,
    onCardCreated,
  }: {
    columnId: number;
    nextColumnId?: number;
    onCardCreated?: (card: { id: number; title: string; column_id: number; position: number; description: null; due_date: null; created_at: string; updated_at: string }) => void;
  }) => (
    <div data-testid="add-card-input" data-column-id={columnId} data-next-column-id={nextColumnId}>
      <button
        onClick={() =>
          onCardCreated?.({
            id: 999,
            title: 'New Card',
            column_id: columnId,
            position: 0,
            description: null,
            due_date: null,
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          })
        }
      >
        Simulate Create
      </button>
    </div>
  ),
}));

const baseCard = {
  id: 1,
  title: 'Task 1',
  column_id: 1,
  position: 0,
  description: null,
  due_date: null,
  created_at: '2024-01-01',
  updated_at: '2024-01-01',
};

const baseColumn: ColumnType = {
  id: 1,
  name: 'To Do',
  position: 0,
  board_id: 1,
  cards: [baseCard],
  created_at: '2024-01-01',
  updated_at: '2024-01-01',
};

describe('Column', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
  });

  it('renders column header with name', () => {
    render(<Column column={baseColumn} />);
    expect(screen.getByTestId('column-header')).toHaveTextContent('To Do');
  });

  it('renders card list with cards', () => {
    render(<Column column={baseColumn} />);
    const cardList = screen.getByTestId('card-list');
    expect(cardList).toHaveAttribute('data-card-count', '1');
    expect(screen.getByText('1 card(s)')).toBeInTheDocument();
  });

  it('renders add card input', () => {
    render(<Column column={baseColumn} />);
    const input = screen.getByTestId('add-card-input');
    expect(input).toHaveAttribute('data-column-id', '1');
  });

  it('passes nextColumnId when next column exists', () => {
    const nextColumn: ColumnType = {
      id: 2,
      name: 'In Progress',
      position: 1,
      board_id: 1,
      cards: [],
      created_at: '2024-01-01',
      updated_at: '2024-01-01',
    };
    render(<Column column={baseColumn} allColumns={[baseColumn, nextColumn]} />);
    expect(screen.getByTestId('add-card-input')).toHaveAttribute('data-next-column-id', '2');
  });

  it('does not pass nextColumnId when no next column exists', () => {
    render(<Column column={baseColumn} allColumns={[baseColumn]} />);
    expect(screen.getByTestId('add-card-input')).not.toHaveAttribute('data-next-column-id');
  });

  it('handles card created callback and sets newCardId state', async () => {
    render(<Column column={baseColumn} />);
    const cardList = screen.getByTestId('card-list');
    expect(cardList).not.toHaveAttribute('data-new-card-id');

    fireEvent.click(screen.getByText('Simulate Create'));
    await waitFor(() => {
      expect(screen.getByTestId('card-list')).toHaveAttribute('data-new-card-id', '999');
    });
  });
});
