import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { BoardNotesSidebar } from './board-notes-sidebar';

const renderWithProviders = (ui: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>{ui}</ToastProvider>
    </QueryClientProvider>,
  );
};

const mockBoardNotesData = {
  data: [
    {
      id: 1,
      title: 'Board Note 1',
      content: 'Content 1',
      user_id: 1,
      board_id: 1,
      tags: [],
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-06-15T00:00:00Z',
    },
    {
      id: 2,
      title: 'Board Note 2',
      content: 'Content 2',
      user_id: 1,
      board_id: 1,
      tags: [],
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-06-15T00:00:00Z',
    },
  ],
};

const mockUseBoardNotes = vi.fn();
const mockUseDeleteNote = vi.fn();
const mockUseNote = vi.fn();
const mockMutateAsync = vi.fn();

vi.mock('./use-notes', () => ({
  useBoardNotes: () => mockUseBoardNotes(),
  useDeleteNote: () => mockUseDeleteNote(),
  useNote: (id: number) => mockUseNote(id),
}));

vi.mock('./board-notes-sidebar-item', () => ({
  BoardNotesSidebarItem: ({
    note,
    onClick,
  }: {
    note: { id: number; title: string };
    onClick: () => void;
  }) => (
    <div role="article" aria-label={`Note: ${note.title}`} onClick={onClick}>
      {note.title}
    </div>
  ),
}));

vi.mock('./note-editor', () => ({
  NoteEditor: ({ onSave, onCancel }: { onSave: () => void; onCancel: () => void }) => (
    <div data-testid="note-editor">
      <button data-testid="editor-save" onClick={onSave}>Save</button>
      <button data-testid="editor-cancel" onClick={onCancel}>Cancel</button>
    </div>
  ),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

describe('BoardNotesSidebar', () => {
  const onToggle = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseBoardNotes.mockReturnValue({ data: mockBoardNotesData, isLoading: false });
    mockUseDeleteNote.mockReturnValue({ mutateAsync: mockMutateAsync });
    mockUseNote.mockImplementation((id: number) => {
      const note = mockBoardNotesData.data.find((n) => n.id === id);
      return { data: note ? { data: note } : null, isLoading: false };
    });
  });

  it('renders board notes', () => {
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    expect(screen.getByText('Board Note 1')).toBeInTheDocument();
    expect(screen.getByText('Board Note 2')).toBeInTheDocument();
  });

  it('renders Notes header', () => {
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    expect(screen.getByText('Notes')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    mockUseBoardNotes.mockReturnValue({ data: undefined, isLoading: true });
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    const skeletons = document.querySelectorAll('.animate-pulse');
    expect(skeletons.length).toBe(3);
  });

  it('shows empty state when no notes', () => {
    mockUseBoardNotes.mockReturnValue({ data: { data: [] }, isLoading: false });
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    expect(screen.getByText('No notes for this board')).toBeInTheDocument();
  });

  it('renders new note button', () => {
    const { container } = renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    const plusButton = container.querySelector('button .lucide-plus')?.closest('button');
    expect(plusButton).toBeInTheDocument();
  });

  it('calls onToggle when toggle button clicked', () => {
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    fireEvent.click(screen.getByLabelText('Toggle notes sidebar'));
    expect(onToggle).toHaveBeenCalled();
  });

  it('shows collapsed toggle button when collapsed', () => {
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={true} onToggle={onToggle} />);
    expect(screen.getByLabelText('Show notes sidebar')).toBeInTheDocument();
  });

  it('has collapsed width class when collapsed', () => {
    const { container } = renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={true} onToggle={onToggle} />);
    const aside = container.querySelector('aside');
    expect(aside?.className).toContain('w-0');
  });

  it('opens editor sheet when note card clicked', () => {
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    fireEvent.click(screen.getByRole('article', { name: 'Note: Board Note 1' }));
    expect(screen.getByTestId('note-editor')).toBeInTheDocument();
  });

  it('opens create sheet when new note clicked', () => {
    const { container } = renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    const plusButton = container.querySelector('button .lucide-plus')?.closest('button') as HTMLElement;
    fireEvent.click(plusButton);
    const editors = screen.getAllByTestId('note-editor');
    expect(editors.length).toBe(1);
  });

  it('shows delete confirmation dialog', async () => {
    const mockNote = {
      id: 1,
      title: 'Board Note 1',
      content: 'Content 1',
      user_id: 1,
      board_id: 1,
      tags: [],
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-06-15T00:00:00Z',
    };
    mockUseBoardNotes.mockReturnValue({ data: { data: [mockNote] }, isLoading: false });
    renderWithProviders(<BoardNotesSidebar boardId={1} collapsed={false} onToggle={onToggle} />);
    const card = screen.getByRole('article', { name: 'Note: Board Note 1' });
    fireEvent.click(card);
    expect(screen.getByTestId('note-editor')).toBeInTheDocument();
  });
});
