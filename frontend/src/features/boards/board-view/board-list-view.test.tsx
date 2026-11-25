import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BoardListView } from './board-list-view';

const mockMutateUpdate = vi.fn();
const mockMutateDelete = vi.fn();
const mockMutateCreate = vi.fn();
const mockToast = vi.fn();
const mockFetchNextPage = vi.fn();

let mockBoardCardsReturn: any;
let mockColumnsReturn: any;

vi.mock('../../cards/use-cards', () => ({
  useUpdateCard: () => ({ mutate: mockMutateUpdate }),
  useDeleteCard: () => ({ mutate: mockMutateDelete }),
  useCreateCard: () => ({ mutate: mockMutateCreate, isPending: false }),
  useBoardCards: (...args: unknown[]) => mockBoardCardsReturn(...args),
  flattenBoardCardPages: (pages: Array<{ data: unknown[] }> | undefined) => pages?.flatMap((p) => p.data) ?? [],
}));

vi.mock('../../columns/use-columns', () => ({
  useColumns: (...args: unknown[]) => mockColumnsReturn(...args),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: mockToast }),
}));

vi.mock('../../cards/card-detail-panel', () => ({
  CardDetailPanel: ({ card, open }: { card: { title: string }; open: boolean }) =>
    open ? <div data-testid="detail-panel">{card.title}</div> : null,
}));

const col1 = { id: 1, name: 'To Do', position: 0, board_id: 1, cards: [], total_cards: 0, matching_cards: 0 };
const col2 = { id: 2, name: 'Done', position: 1, board_id: 1, cards: [], total_cards: 0, matching_cards: 0 };

const item1 = {
  id: 1, title: 'Beta', column_id: 1, column_name: 'To Do', position: 0,
  has_description: true, due_date: '2026-03-20', labels: [{ id: 1, name: 'Bug', color: 'red' }],
  checklist_progress: { completed: 1, total: 2, percent: 50 },
  created_at: '2024-01-02T00:00:00.000Z', updated_at: '2024-01-03T00:00:00.000Z',
};
const item2 = {
  id: 2, title: 'Alpha', column_id: 2, column_name: 'Done', position: 0,
  has_description: false, due_date: null, labels: [],
  checklist_progress: null, created_at: '2024-01-01T00:00:00.000Z', updated_at: '2024-01-01T00:00:00.000Z',
};

function makeBoardCards(items: any[], total = items.length) {
  return {
    data: { pages: [{ data: items, page: 1, limit: 20, total }], pageParams: [1] },
    isLoading: false, isError: false, error: null, refetch: vi.fn(),
    hasNextPage: false, fetchNextPage: mockFetchNextPage, isFetchingNextPage: false,
  };
}

describe('BoardListView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 600 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 320 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 100 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 320 });
    mockColumnsReturn = vi.fn(() => ({ data: [col1, col2] }));
    mockBoardCardsReturn = vi.fn(() => makeBoardCards([item1, item2]));
  });

  afterEach(() => {
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 0 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 0 });
  });

  it('renders flat rows with column badge, labels, due and progress', () => {
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.getByText('To Do')).toBeInTheDocument();
    expect(screen.getByText('Done')).toBeInTheDocument();
    expect(screen.getByText('Bug')).toBeInTheDocument();
    expect(screen.getByText('1/2 (50%)')).toBeInTheDocument();
  });

  it('passes server sort params to useBoardCards', () => {
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    expect(mockBoardCardsReturn).toHaveBeenCalledWith(1, expect.objectContaining({ sort: 'created_at', order: 'asc' }));
    fireEvent.click(screen.getByRole('button', { name: 'Title', exact: true }));
    expect(mockBoardCardsReturn).toHaveBeenLastCalledWith(1, expect.objectContaining({ sort: 'title', order: 'asc' }));
  });

  it('opens detail panel on row click', async () => {
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    fireEvent.click(screen.getByLabelText('Open card details: Beta'));
    await waitFor(() => { expect(screen.getByTestId('detail-panel')).toHaveTextContent('Beta'); });
  });

  it('inline edit commits on Enter', async () => {
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    fireEvent.click(screen.getByText('Beta'));
    const input = screen.getByLabelText('Edit card title');
    fireEvent.change(input, { target: { value: 'Beta Updated' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => {
      expect(mockMutateUpdate).toHaveBeenCalledWith({ id: 1, data: { title: 'Beta Updated' } }, expect.anything());
    });
  });

  it('delete flow calls delete mutation', async () => {
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    fireEvent.click(screen.getByLabelText('Delete card Beta'));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => { expect(mockMutateDelete).toHaveBeenCalledWith(1, expect.anything()); });
  });

  it('shows empty state when no cards', () => {
    mockBoardCardsReturn = vi.fn(() => makeBoardCards([], 0));
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    expect(screen.getByText('No cards yet')).toBeInTheDocument();
  });

  it('shows load-more fallback and calls fetchNextPage', () => {
    mockBoardCardsReturn = vi.fn(() => ({ ...makeBoardCards([item1], 2), hasNextPage: true }));
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    fireEvent.click(screen.getByRole('button', { name: 'Load more cards' }));
    expect(mockFetchNextPage).toHaveBeenCalled();
  });

  it('shows filter-active DnD notice', () => {
    render(<BoardListView boardId={1} filters={{ search: 'x' }} filterActive={true} />);
    expect(screen.getByText(/Drag and drop is disabled/i)).toBeInTheDocument();
  });

  it('creates card with selected column', async () => {
    render(<BoardListView boardId={1} filters={{}} filterActive={false} />);
    fireEvent.click(screen.getByRole('button', { name: /add card/i }));
    fireEvent.change(screen.getByLabelText('Card title'), { target: { value: 'New Card' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => {
      expect(mockMutateCreate).toHaveBeenCalledWith({ title: 'New Card', column_id: 1 }, expect.anything());
    });
  });
});
