import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BoardView } from './board-view';

const mockNavigate = vi.fn();

const mockUseBoard = vi.fn();
const mockUseColumns = vi.fn();
const mockMutateAsync = vi.fn();
const mockToast = vi.fn();
const mockUpdateMutate = vi.fn();

vi.mock('react-router-dom', () => ({
  useParams: () => ({ boardId: '1' }),
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
  Column: ({ column }: { column: { id: number; name: string } }) => (
    <div data-testid={`column-${column.id}`}>{column.name}</div>
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
  BoardListView: () => <div data-testid="board-list-view" />,
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
    mockUseBoard.mockReturnValue({ isLoading: false, data: mockBoardResponse });
    mockUseColumns.mockReturnValue({ isLoading: false, data: [mockColumn] });
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
      | ((err: Error) => void)
      | undefined;
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
});
