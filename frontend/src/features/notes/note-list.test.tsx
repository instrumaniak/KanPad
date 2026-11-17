import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { ToastProvider } from '@/components/ui/toast-provider';
import { NoteList } from './note-list';

const mockNavigate = vi.fn();

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom');
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

const renderWithProviders = (ui: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>{ui}</ToastProvider>
      </QueryClientProvider>
    </MemoryRouter>,
  );
};

const mockNotesData = {
  data: [
    {
      id: 1,
      title: 'First Note',
      content: 'Content 1',
      user_id: 1,
      tags: [],
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-06-15T00:00:00Z',
    },
    {
      id: 2,
      title: 'Second Note',
      content: 'Content 2',
      user_id: 1,
      tags: [],
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-06-15T00:00:00Z',
    },
  ],
};

const mockUseNotes = vi.fn();
const mockUseDeleteNote = vi.fn();
const mockMutate = vi.fn();

vi.mock('./use-notes', () => ({
  useNotes: () => mockUseNotes(),
  useDeleteNote: () => mockUseDeleteNote(),
}));

vi.mock('./note-card', () => ({
  NoteCard: ({ note, onClick, onEdit, onDelete }: { note: { id: number; title: string }; onClick: () => void; onEdit: () => void; onDelete: () => void }) => (
    <div data-testid={`note-card-${note.id}`} onClick={onClick}>
      {note.title}
      <button aria-label="Actions" data-testid={`actions-${note.id}`}>Actions</button>
      <button data-testid={`edit-${note.id}`} onClick={(e: React.MouseEvent) => { e.stopPropagation(); onEdit(); }}>Edit</button>
      <button data-testid={`delete-${note.id}`} onClick={(e: React.MouseEvent) => { e.stopPropagation(); onDelete(); }}>Delete</button>
    </div>
  ),
}));

vi.mock('./note-editor', () => ({
  NoteEditor: ({ onSave, onCancel }: { onSave: () => void; onCancel: () => void }) => (
    <div data-testid="note-editor">
      Note Editor
      <button data-testid="editor-save" onClick={onSave}>Save</button>
      <button data-testid="editor-cancel" onClick={onCancel}>Cancel</button>
    </div>
  ),
}));

vi.mock('./create-note-dialog', () => ({
  CreateNoteDialog: ({ children }: { children: React.ReactNode }) => <div data-testid="create-note-dialog">{children}</div>,
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

describe('NoteList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseNotes.mockReturnValue({ data: mockNotesData, isLoading: false, isError: false });
    mockUseDeleteNote.mockReturnValue({ mutate: mockMutate });
  });

  it('renders note list with notes', () => {
    renderWithProviders(<NoteList />);
    expect(screen.getByText('First Note')).toBeInTheDocument();
    expect(screen.getByText('Second Note')).toBeInTheDocument();
  });

  it('renders search input', () => {
    renderWithProviders(<NoteList />);
    expect(screen.getByPlaceholderText('Search notes...')).toBeInTheDocument();
  });

  it('renders type filter chips', () => {
    renderWithProviders(<NoteList />);
    expect(screen.getByText('All')).toBeInTheDocument();
    expect(screen.getByText('General')).toBeInTheDocument();
    expect(screen.getByText('Board')).toBeInTheDocument();
    expect(screen.getByText('Project')).toBeInTheDocument();
    expect(screen.getByText('Card')).toBeInTheDocument();
  });

  it('renders create dialog button', () => {
    renderWithProviders(<NoteList />);
    expect(screen.getByText('New Note')).toBeInTheDocument();
  });

  it('shows loading skeleton when loading', () => {
    mockUseNotes.mockReturnValue({ data: undefined, isLoading: true });
    renderWithProviders(<NoteList />);
    const skeletons = document.querySelectorAll('.rounded-lg.border');
    expect(skeletons.length).toBe(4);
  });

  it('shows empty state when no notes', () => {
    mockUseNotes.mockReturnValue({ data: { data: [] }, isLoading: false });
    renderWithProviders(<NoteList />);
    expect(screen.getByText(/No notes yet/)).toBeInTheDocument();
  });

  it('shows delete confirmation dialog', () => {
    renderWithProviders(<NoteList />);
    const deleteBtn = screen.getByTestId('delete-1');
    fireEvent.click(deleteBtn);
    expect(screen.getByText(/Delete note\?/)).toBeInTheDocument();
  });

  it('calls delete mutation on confirm after delay', () => {
    vi.useFakeTimers();
    renderWithProviders(<NoteList />);
    fireEvent.click(screen.getByTestId('delete-1'));

    const confirmDelete = screen.getByText('Delete', { selector: '[data-slot="alert-dialog-action"]' });
    fireEvent.click(confirmDelete);

    vi.advanceTimersByTime(5000);
    expect(mockMutate).toHaveBeenCalled();
    vi.useRealTimers();
  });

  it('closes delete dialog on cancel', () => {
    renderWithProviders(<NoteList />);
    fireEvent.click(screen.getByTestId('delete-1'));

    const cancelButton = screen.getByText('Cancel');
    fireEvent.click(cancelButton);
    expect(screen.queryByText(/Delete note\?/)).not.toBeInTheDocument();
  });

  it('navigates to note detail when a note is clicked', () => {
    renderWithProviders(<NoteList />);
    fireEvent.click(screen.getByText('First Note'));
    expect(mockNavigate).toHaveBeenCalledWith('/notes/1');
  });

  it('navigates to note detail for second note when clicked', () => {
    renderWithProviders(<NoteList />);
    fireEvent.click(screen.getByText('Second Note'));
    expect(mockNavigate).toHaveBeenCalledWith('/notes/2');
  });
});

describe('NoteList error states', () => {
  const mockRefetch = vi.fn();

  beforeEach(() => {
    mockRefetch.mockClear();
    mockUseNotes.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('boom'),
      refetch: mockRefetch,
    });
  });

  it('shows the error state with retry when the notes fetch fails (never the empty state)', () => {
    renderWithProviders(<NoteList />);
    expect(screen.getByRole('alert')).toHaveTextContent('Failed to load notes');
    expect(screen.getByRole('alert')).toHaveTextContent('boom');
    expect(screen.queryByText('No notes yet')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(mockRefetch).toHaveBeenCalledTimes(1);
  });
});

describe('NoteList error states — header stays interactive, 401 suppressed, cache kept', () => {
  const mockRefetch = vi.fn();

  beforeEach(() => {
    mockRefetch.mockClear();
    mockUseNotes.mockReturnValue({
      data: mockNotesData,
      isLoading: false,
      isError: true,
      error: new Error('stale refetch failed'),
      refetch: mockRefetch,
    });
  });

  it('renders notes from cache when a background refetch fails (M1)', () => {
    renderWithProviders(<NoteList />);
    expect(screen.getByText('First Note')).toBeInTheDocument();
    expect(screen.getByText('Second Note')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('keeps the header and create action available while the list errors (D5)', () => {
    mockUseNotes.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('boom'),
      refetch: mockRefetch,
    });
    renderWithProviders(<NoteList />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('Notes')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /new note/i })).toBeInTheDocument();
  });

  it('renders the skeleton (not an error card) for a 401 — the T014 global handler owns the redirect (m3)', () => {
    mockUseNotes.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: Object.assign(new Error('Session expired'), { status: 401 }),
      refetch: mockRefetch,
    });
    renderWithProviders(<NoteList />);
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.queryByText('No notes yet')).not.toBeInTheDocument();
    expect(screen.queryByText('First Note')).not.toBeInTheDocument();
  });
});
