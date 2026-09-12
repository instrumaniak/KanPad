import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { BoardView } from './board-view';

const mockNavigate = vi.fn();

const mockUseBoard = vi.fn();
const mockUseColumns = vi.fn();
const mockMutateAsync = vi.fn();
const mockToast = vi.fn();
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
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: mockToast }),
}));

vi.mock('../../columns/column', () => ({
  Column: ({
    column,
    allColumns,
  }: {
    column: { id: number; name: string; cards?: { id: number; title: string }[] };
    allColumns?: unknown;
  }) => (
    <div
      data-testid={`column-${column.id}`}
      data-all-columns={Array.isArray(allColumns) ? allColumns.length : 0}
    >
      {column.name}
      {(column.cards ?? []).map((c) => (
        <span key={c.id}>{c.title}</span>
      ))}
    </div>
  ),
}));

vi.mock('../../columns/add-column-button', () => ({
  AddColumnButton: ({ onClick }: { onClick: () => void }) => (
    <button data-testid="add-column-button" onClick={onClick}>
      Add Column
    </button>
  ),
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
      {(columns ?? [])
        .flatMap((c) => c.cards ?? [])
        .map((card, i) => (
          <span key={i}>{card.title}</span>
        ))}
    </div>
  ),
}));

vi.mock('./filter-dropdown', () => ({
  FilterDropdown: ({
    filters,
    onFiltersChange,
  }: {
    filters: { labels: string[]; dueDate: string | null; checklist: string };
    onFiltersChange: (f: { labels: string[]; dueDate: string | null; checklist: string }) => void;
  }) => (
    <div data-testid="filter-dropdown">
      <button
        data-testid="filter-label-red"
        onClick={() =>
          onFiltersChange({
            ...filters,
            labels: filters.labels.includes('Red')
              ? filters.labels.filter((l: string) => l !== 'Red')
              : [...filters.labels, 'Red'],
          })
        }
      >
        Toggle Red
      </button>
      <button
        data-testid="filter-due-overdue"
        onClick={() =>
          onFiltersChange({
            ...filters,
            dueDate: filters.dueDate === 'Overdue' ? null : 'Overdue',
          })
        }
      >
        Set Overdue
      </button>
      <button
        data-testid="filter-checklist-complete"
        onClick={() =>
          onFiltersChange({ ...filters, checklist: 'Complete' })
        }
      >
        Set Complete
      </button>
      <button
        data-testid="filter-checklist-incomplete"
        onClick={() =>
          onFiltersChange({ ...filters, checklist: 'Incomplete' })
        }
      >
        Set Incomplete
      </button>
    </div>
  ),
}));

vi.mock('./filter-chips', () => ({
  FilterChips: ({
    filters,
    onClearFilter,
    onClearAll,
  }: {
    filters: { labels: string[]; dueDate: string | null; checklist: string };
    onClearFilter: (type: 'labels' | 'dueDate' | 'checklist') => void;
    onClearAll: () => void;
  }) => {
    const hasFilters =
      filters.labels.length > 0 || filters.dueDate !== null || filters.checklist !== 'All';
    if (!hasFilters) return null;
    return (
      <div data-testid="filter-chips">
        <span data-testid="filter-chips-text">
          {filters.labels.length > 0 && `Labels: ${filters.labels.join(', ')}`}
          {filters.dueDate && ` Due: ${filters.dueDate}`}
          {filters.checklist !== 'All' && ` Checklist: ${filters.checklist}`}
        </span>
        {filters.labels.length > 0 && (
          <button data-testid="clear-label-chip" onClick={() => onClearFilter('labels')}>
            Clear labels
          </button>
        )}
        {filters.dueDate && (
          <button data-testid="clear-due-chip" onClick={() => onClearFilter('dueDate')}>
            Clear due
          </button>
        )}
        {filters.checklist !== 'All' && (
          <button data-testid="clear-checklist-chip" onClick={() => onClearFilter('checklist')}>
            Clear checklist
          </button>
        )}
        <button data-testid="clear-all-filters" onClick={onClearAll}>
          Clear all
        </button>
      </div>
    );
  },
}));

vi.mock('../../labels/use-labels', () => ({
  useLabels: () => ({ data: [{ name: 'Red', color: 'red' }, { name: 'Blue', color: 'blue' }] }),
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

describe('BoardView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows loading when board is loading', () => {
    mockUseBoard.mockReturnValue({ isLoading: true, data: undefined });
    render(<BoardView />);
    expect(screen.getByText('Loading...')).toBeInTheDocument();
  });

  it('shows loading when columns are loading', () => {
    mockUseColumns.mockReturnValue({ isLoading: true, data: undefined });
    render(<BoardView />);
    expect(screen.getByText('Loading...')).toBeInTheDocument();
  });

  it('shows board not found when board data is missing', () => {
    mockUseBoard.mockReturnValue({ isLoading: false, data: null });
    render(<BoardView />);
    expect(screen.getByText('Board not found')).toBeInTheDocument();
  });

  it('renders board name, columns, and add column button when loaded', () => {
    render(<BoardView />);
    expect(screen.getByText('Test Board')).toBeInTheDocument();
    expect(screen.getByTestId('column-1')).toHaveTextContent('To Do');
    expect(screen.getByTestId('add-column-button')).toBeInTheDocument();
    expect(screen.getByTestId('drag-context')).toBeInTheDocument();
  });

  it('navigates to / on back button click', () => {
    render(<BoardView />);
    const header = screen.getByText('Test Board').closest('div')!;
    const backButton = header.querySelector('button')!;
    fireEvent.click(backButton);
    expect(mockNavigate).toHaveBeenCalledWith('/');
  });

  it('calls createColumn mutation and shows success toast on add column', async () => {
    mockMutateAsync.mockResolvedValueOnce({});
    render(<BoardView />);
    fireEvent.click(screen.getByTestId('add-column-button'));
    await waitFor(() => {
      expect(mockMutateAsync).toHaveBeenCalledWith({ boardId: 1, data: {} });
    });
    expect(mockToast).toHaveBeenCalledWith({ title: 'Column added', type: 'success' });
  });

  it('shows error toast on add column failure with error message', async () => {
    mockMutateAsync.mockRejectedValueOnce(new Error('API Error'));
    render(<BoardView />);
    fireEvent.click(screen.getByTestId('add-column-button'));
    await waitFor(() => {
      expect(mockToast).toHaveBeenCalledWith({
        title: 'Failed to add column',
        description: 'API Error',
        type: 'error',
      });
    });
  });

  it('shows generic error toast on add column failure with non-Error', async () => {
    mockMutateAsync.mockRejectedValueOnce('string error');
    render(<BoardView />);
    fireEvent.click(screen.getByTestId('add-column-button'));
    await waitFor(() => {
      expect(mockToast).toHaveBeenCalledWith({
        title: 'Failed to add column',
        description: 'Something went wrong',
        type: 'error',
      });
    });
  });

  it('renders view toggle with board active by default', () => {
    render(<BoardView />);
    expect(screen.getByRole('group', { name: 'Board view mode' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /list/i })).toHaveAttribute('aria-pressed', 'false');
  });

  it('keeps filtering and view controls available on mobile', () => {
    Object.defineProperty(window, 'innerWidth', { writable: true, value: 375 });

    render(<BoardView />);

    expect(screen.getByTestId('filter-dropdown')).toBeInTheDocument();
    expect(screen.getByRole('group', { name: 'Board view mode' })).toBeInTheDocument();
  });

  it('mounts every mobile column as a drop destination and keeps add column available', () => {
    Object.defineProperty(window, 'innerWidth', { writable: true, value: 375 });
    mockUseColumns.mockReturnValue({
      isLoading: false,
      data: [mockColumn, { ...mockColumn, id: 2, name: 'Done', position: 1 }],
    });

    render(<BoardView />);

    expect(screen.getByTestId('column-1')).toBeInTheDocument();
    expect(screen.getByTestId('column-2')).toBeInTheDocument();
    expect(screen.getByTestId('add-column-button')).toBeInTheDocument();
  });

  it('switching to list calls updateBoard with view_mode and preserves kanban on switch back', async () => {
    mockUseBoard.mockReturnValue({ isLoading: false, data: mockBoardResponse });
    const { unmount } = render(<BoardView />);
    fireEvent.click(screen.getByRole('button', { name: /list/i }));
    await waitFor(() => {
      expect(mockUpdateMutate).toHaveBeenCalledWith(
        { id: 1, data: { view_mode: 'list' } },
        expect.anything(),
      );
    });
    expect(screen.getByRole('button', { name: /list/i })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: /board/i }));
    await waitFor(() => {
      expect(mockUpdateMutate).toHaveBeenCalledWith(
        { id: 1, data: { view_mode: 'board' } },
        expect.anything(),
      );
    });
    expect(screen.getByTestId('drag-context')).toBeInTheDocument();
    unmount();
  });

  it('initializes list view from board.view_mode', () => {
    mockUseBoard.mockReturnValue({
      isLoading: false,
      data: { data: { ...mockBoardResponse.data, view_mode: 'list' } },
    });
    render(<BoardView />);
    expect(screen.getByRole('button', { name: /list/i })).toHaveAttribute('aria-pressed', 'true');
  });

  it('does not PATCH when clicking the active view', () => {
    render(<BoardView />);
    fireEvent.click(screen.getByRole('button', { name: /board/i }));
    expect(mockUpdateMutate).not.toHaveBeenCalled();
  });

  it('rolls back view and toasts on PATCH failure', async () => {
    render(<BoardView />);
    fireEvent.click(screen.getByRole('button', { name: /list/i }));
    await waitFor(() => {
      expect(mockUpdateMutate).toHaveBeenCalled();
    });
    const onError = mockUpdateMutate.mock.calls[0][1]?.onError as
      ((err: Error) => void) | undefined;
    expect(onError).toBeDefined();
    onError?.(new Error('Network fail'));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /list/i })).toHaveAttribute(
        'aria-pressed',
        'false',
      );
    });
    expect(mockToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Failed to save view' }),
    );
  });

  describe('card search', () => {
    const searchColumns = [
      {
        ...mockColumn,
        id: 1,
        name: 'To Do',
        cards: [
          {
            id: 101,
            title: 'Alpha task',
            column_id: 1,
            position: 0,
            due_date: null,
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          },
          {
            id: 102,
            title: 'Beta task',
            column_id: 1,
            position: 1,
            due_date: null,
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          },
        ],
      },
      {
        ...mockColumn,
        id: 2,
        name: 'Done',
        cards: [
          {
            id: 201,
            title: 'Zulu task',
            column_id: 2,
            position: 0,
            due_date: null,
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          },
        ],
      },
    ];

    function renderWithSearchData() {
      mockUseColumns.mockReturnValue({ isLoading: false, data: searchColumns });
      vi.useFakeTimers();
      render(<BoardView />);
    }

    function typeSearch(value: string) {
      fireEvent.change(screen.getByLabelText('Search cards'), { target: { value } });
      act(() => {
        vi.advanceTimersByTime(300);
      });
    }

    it('renders search input in header', () => {
      renderWithSearchData();
      expect(screen.getByRole('search')).toBeInTheDocument();
      expect(screen.getByLabelText('Search cards')).toBeInTheDocument();
    });

    it('filters cards when typing 2+ chars and restores on clear', () => {
      renderWithSearchData();
      typeSearch('Al');
      expect(screen.getByText('Alpha task')).toBeInTheDocument();
      expect(screen.queryByText('Beta task')).not.toBeInTheDocument();
      expect(screen.queryByText('Zulu task')).not.toBeInTheDocument();
      fireEvent.click(screen.getByLabelText('Clear search'));
      act(() => {
        vi.advanceTimersByTime(300);
      });
      expect(screen.getByText('Beta task')).toBeInTheDocument();
      expect(screen.getByText('Zulu task')).toBeInTheDocument();
    });

    it('shows all cards for 1-char query', () => {
      renderWithSearchData();
      typeSearch('A');
      expect(screen.getByText('Alpha task')).toBeInTheDocument();
      expect(screen.getByText('Beta task')).toBeInTheDocument();
      expect(screen.getByText('Zulu task')).toBeInTheDocument();
    });

    it('shows empty state with Clear search on zero matches and resets', () => {
      renderWithSearchData();
      typeSearch('zzz-no-match');
      expect(screen.getByText('No cards found')).toBeInTheDocument();
      fireEvent.click(screen.getByText('Clear search'));
      act(() => {
        vi.advanceTimersByTime(300);
      });
      expect(screen.getByText('Alpha task')).toBeInTheDocument();
      expect(screen.queryByText('No cards found')).not.toBeInTheDocument();
    });

    it('debounces filtering by 300ms', () => {
      renderWithSearchData();
      fireEvent.change(screen.getByLabelText('Search cards'), { target: { value: 'Al' } });
      // Before debounce fires, all cards still visible
      expect(screen.getByText('Beta task')).toBeInTheDocument();
      act(() => {
        vi.advanceTimersByTime(300);
      });
      expect(screen.queryByText('Beta task')).not.toBeInTheDocument();
    });

    it('preserves search when toggling board/list view', () => {
      renderWithSearchData();
      typeSearch('Beta');
      fireEvent.click(screen.getByRole('button', { name: /list/i }));
      expect(screen.getByTestId('board-list-view')).toHaveTextContent('Beta task');
      expect(screen.getByTestId('board-list-view')).not.toHaveTextContent('Alpha task');
      expect(screen.getByLabelText('Search cards')).toHaveValue('Beta');
    });

    it('resets search on boardId change', () => {
      mockUseColumns.mockReturnValue({ isLoading: false, data: searchColumns });
      vi.useFakeTimers();
      const { rerender } = render(<BoardView />);
      typeSearch('Beta');
      expect(screen.queryByText('Alpha task')).not.toBeInTheDocument();
      mockBoardId = '2';
      rerender(<BoardView />);
      expect(screen.getByLabelText('Search cards')).toHaveValue('');
      expect(screen.getByText('Alpha task')).toBeInTheDocument();
    });
  });

  describe('card filtering', () => {
    const filterColumns = [
      {
        ...mockColumn,
        id: 1,
        name: 'To Do',
        cards: [
          {
            id: 101,
            title: 'Red card',
            column_id: 1,
            position: 0,
            due_date: null,
            labels: [{ id: 1, name: 'Red', color: 'red' }],
            checklist_progress: { completed: 1, total: 3, percent: 33 },
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          },
          {
            id: 102,
            title: 'Blue card',
            column_id: 1,
            position: 1,
            due_date: null,
            labels: [{ id: 2, name: 'Blue', color: 'blue' }],
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          },
        ],
      },
    ];

    function renderWithFilterData() {
      mockUseColumns.mockReturnValue({ isLoading: false, data: filterColumns });
      render(<BoardView />);
    }

    it('renders filter button in header', () => {
      renderWithFilterData();
      expect(screen.getByTestId('filter-dropdown')).toBeInTheDocument();
    });

    it('renders filter chips when filters are active', () => {
      renderWithFilterData();
      fireEvent.click(screen.getByTestId('filter-label-red'));
      expect(screen.getByTestId('filter-chips')).toBeInTheDocument();
      expect(screen.getByTestId('filter-chips-text')).toHaveTextContent('Labels: Red');
    });

    it('clears all filters via Clear all button', () => {
      renderWithFilterData();
      fireEvent.click(screen.getByTestId('filter-label-red'));
      expect(screen.getByTestId('filter-chips')).toBeInTheDocument();
      fireEvent.click(screen.getByTestId('clear-all-filters'));
      expect(screen.queryByTestId('filter-chips')).not.toBeInTheDocument();
    });

    it('filters cards by label and shows empty state', () => {
      renderWithFilterData();
      fireEvent.click(screen.getByTestId('filter-label-red'));
      expect(screen.getByText('Red card')).toBeInTheDocument();
      expect(screen.queryByText('Blue card')).not.toBeInTheDocument();
    });

    it('shows empty state with Clear filters when all cards filtered out', () => {
      renderWithFilterData();
      // Set a label filter that no card has
      fireEvent.click(screen.getByTestId('filter-label-red'));
      // Then switch to a filter that eliminates all
      fireEvent.click(screen.getByTestId('filter-checklist-complete'));
      expect(screen.getByText('No cards found')).toBeInTheDocument();
      expect(screen.getByText('Clear filters')).toBeInTheDocument();
    });

    it('composes search and filter independently', () => {
      mockUseColumns.mockReturnValue({ isLoading: false, data: filterColumns });
      vi.useFakeTimers();
      render(<BoardView />);
      // Apply label filter
      fireEvent.click(screen.getByTestId('filter-label-red'));
      // Type search
      fireEvent.change(screen.getByLabelText('Search cards'), { target: { value: 'Red' } });
      act(() => {
        vi.advanceTimersByTime(300);
      });
      expect(screen.getByText('Red card')).toBeInTheDocument();
      expect(screen.queryByText('Blue card')).not.toBeInTheDocument();
    });

    it('resets filters on board change', () => {
      mockUseColumns.mockReturnValue({ isLoading: false, data: filterColumns });
      const { rerender } = render(<BoardView />);
      fireEvent.click(screen.getByTestId('filter-label-red'));
      expect(screen.getByTestId('filter-chips')).toBeInTheDocument();
      mockBoardId = '2';
      rerender(<BoardView />);
      expect(screen.queryByTestId('filter-chips')).not.toBeInTheDocument();
    });

    it('clears individual label chip while keeping other filters', () => {
      renderWithFilterData();
      fireEvent.click(screen.getByTestId('filter-label-red'));
      fireEvent.click(screen.getByTestId('filter-due-overdue'));
      expect(screen.getByTestId('filter-chips-text')).toHaveTextContent('Labels: Red');
      expect(screen.getByTestId('filter-chips-text')).toHaveTextContent('Due: Overdue');
      fireEvent.click(screen.getByTestId('clear-label-chip'));
      expect(screen.queryByTestId('clear-label-chip')).not.toBeInTheDocument();
      expect(screen.getByTestId('clear-due-chip')).toBeInTheDocument();
    });

    it('filters by due date (Overdue) independently', () => {
      const pastDate = new Date();
      pastDate.setDate(pastDate.getDate() - 2);
      const dueDateColumns = [
        {
          ...mockColumn,
          id: 1,
          name: 'To Do',
          cards: [
            {
              id: 101,
              title: 'Overdue task',
              column_id: 1,
              position: 0,
              due_date: pastDate.toISOString(),
              created_at: '2024-01-01',
              updated_at: '2024-01-01',
            },
            {
              id: 102,
              title: 'No date task',
              column_id: 1,
              position: 1,
              due_date: null,
              created_at: '2024-01-01',
              updated_at: '2024-01-01',
            },
          ],
        },
      ];
      mockUseColumns.mockReturnValue({ isLoading: false, data: dueDateColumns });
      render(<BoardView />);
      fireEvent.click(screen.getByTestId('filter-due-overdue'));
      expect(screen.getByText('Overdue task')).toBeInTheDocument();
      expect(screen.queryByText('No date task')).not.toBeInTheDocument();
    });

    it('filters by checklist (Complete) independently', () => {
      const checklistColumns = [
        {
          ...mockColumn,
          id: 1,
          name: 'To Do',
          cards: [
            {
              id: 101,
              title: 'Complete task',
              column_id: 1,
              position: 0,
              due_date: null,
              checklist_progress: { completed: 3, total: 3, percent: 100 },
              created_at: '2024-01-01',
              updated_at: '2024-01-01',
            },
            {
              id: 102,
              title: 'Incomplete task',
              column_id: 1,
              position: 1,
              due_date: null,
              checklist_progress: { completed: 1, total: 3, percent: 33 },
              created_at: '2024-01-01',
              updated_at: '2024-01-01',
            },
          ],
        },
      ];
      mockUseColumns.mockReturnValue({ isLoading: false, data: checklistColumns });
      render(<BoardView />);
      fireEvent.click(screen.getByTestId('filter-checklist-complete'));
      expect(screen.getByText('Complete task')).toBeInTheDocument();
      expect(screen.queryByText('Incomplete task')).not.toBeInTheDocument();
    });

    it('applies multiple filters simultaneously (intersection)', () => {
      renderWithFilterData();
      // Red card: labels=[Red], checklist_progress={completed:1, total:3}
      // Blue card: labels=[Blue], no checklist
      fireEvent.click(screen.getByTestId('filter-label-red'));
      fireEvent.click(screen.getByTestId('filter-checklist-incomplete'));
      expect(screen.getByText('Red card')).toBeInTheDocument();
      expect(screen.queryByText('Blue card')).not.toBeInTheDocument();
    });
  });
});
