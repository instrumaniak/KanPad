import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { MemoryRouter, useNavigate } from 'react-router-dom';
import { BoardCard, DeleteDialog } from './board-card';
import { ToastProvider } from '@/components/ui/toast-provider';

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom');
  return {
    ...actual,
    useNavigate: vi.fn(),
  };
});

vi.mock('./use-boards', () => ({
  useUpdateBoard: () => ({
    mutateAsync: vi.fn().mockResolvedValue({}),
    isPending: false,
  }),
  useDeleteBoard: () => ({
    mutateAsync: vi.fn().mockResolvedValue({}),
    isPending: false,
  }),
  useArchiveBoard: () => ({
    mutateAsync: vi.fn().mockResolvedValue({}),
    isPending: false,
  }),
  usePermanentDeleteBoard: () => ({
    mutateAsync: vi.fn().mockResolvedValue({}),
    isPending: false,
  }),
}));

const mockBoard = {
  id: 1,
  name: 'Test Board',
  background_color: '#0079BF',
  project_id: null,
  created_at: '2024-01-01',
  updated_at: '2024-01-01',
};

const renderWithToastAndRouter = (ui: React.ReactElement) => {
  return render(
    <MemoryRouter>
      <ToastProvider>{ui}</ToastProvider>
    </MemoryRouter>
  );
};

describe('BoardCard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('renders board name', () => {
    renderWithToastAndRouter(<BoardCard board={mockBoard} onEdit={vi.fn()} onDelete={vi.fn()} />);
    expect(screen.getByText('Test Board')).toBeInTheDocument();
  });

  it('renders the board card container', () => {
    const { container } = renderWithToastAndRouter(<BoardCard board={mockBoard} onEdit={vi.fn()} onDelete={vi.fn()} />);
    const card = container.firstChild as HTMLElement;
    expect(card).toBeInTheDocument();
    expect(card).toHaveTextContent('Test Board');
  });

  it('shows edit button on hover', async () => {
    const onEdit = vi.fn();
    renderWithToastAndRouter(<BoardCard board={mockBoard} onEdit={onEdit} onDelete={vi.fn()} />);
    
    const card = screen.getByText('Test Board').closest('div');
    fireEvent.mouseEnter(card!);
    
    expect(screen.getByRole('button', { name: /Edit board/i })).toBeInTheDocument();
  });

  it('shows delete button on hover', async () => {
    const onDelete = vi.fn();
    renderWithToastAndRouter(<BoardCard board={mockBoard} onEdit={vi.fn()} onDelete={onDelete} />);
    
    const card = screen.getByText('Test Board').closest('div');
    fireEvent.mouseEnter(card!);
    
    expect(screen.getByRole('button', { name: /Delete board/i })).toBeInTheDocument();
  });

  it('navigates to board view when card is clicked', () => {
    const navigate = vi.fn();
    vi.mocked(useNavigate).mockReturnValue(navigate);
    
    renderWithToastAndRouter(<BoardCard board={mockBoard} onEdit={vi.fn()} onDelete={vi.fn()} />);
    
    fireEvent.click(screen.getByText('Test Board'));
    expect(navigate).toHaveBeenCalledWith('/board/1');
  });

  it('renders project name when present', () => {
    const boardWithProject = { ...mockBoard, project: { id: 1, name: 'My Project' } };
    renderWithToastAndRouter(<BoardCard board={boardWithProject} onEdit={vi.fn()} onDelete={vi.fn()} />);
    
    expect(screen.getByText('My Project')).toBeInTheDocument();
  });
});

describe('DeleteDialog', () => {
  beforeEach(() => vi.clearAllMocks());

  it('renders dialog when open', async () => {
    await act(async () => {
      renderWithToastAndRouter(
        <DeleteDialog
          boardName="Test Board"
          boardId={1}
          open={true}
          onOpenChange={vi.fn()}
          onDeleted={vi.fn()}
        />
      );
    });

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('Archive board')).toBeInTheDocument();
  });

  it('does not render dialog when closed', async () => {
    await act(async () => {
      renderWithToastAndRouter(
        <DeleteDialog
          boardName="Test Board"
          boardId={1}
          open={false}
          onOpenChange={vi.fn()}
          onDeleted={vi.fn()}
        />
      );
    });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
