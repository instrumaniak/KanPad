import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { NoteEditor } from './note-editor';
import type { Note } from './notes.api';

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

const mockCreateMutate = vi.fn();
const mockUpdateMutate = vi.fn();
const mockTagsData = { data: [{ id: 1, name: 'frontend', color: 'teal' }] };

vi.mock('./use-notes', () => ({
  useCreateNote: () => ({ mutate: mockCreateMutate, isPending: false }),
  useUpdateNote: () => ({ mutate: mockUpdateMutate, isPending: false }),
}));

vi.mock('@/features/tags/use-tags', () => ({
  useTags: () => ({ data: mockTagsData }),
  useCreateTag: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

vi.mock('react-router-dom', () => ({
  useBlocker: () => ({ state: 'idle' as const, proceed: vi.fn(), reset: vi.fn() }),
}));

vi.mock('./markdown-renderer', () => ({
  MarkdownRenderer: ({ content }: { content: string }) => <div data-testid="markdown-renderer">{content}</div>,
}));

vi.mock('@/features/boards/use-boards', () => ({
  useBoards: () => ({ data: undefined }),
}));

vi.mock('@/features/projects/use-projects', () => ({
  useProjects: () => ({ data: undefined }),
}));

vi.mock('@/features/columns/use-columns', () => ({
  useColumns: () => ({ data: undefined }),
}));

vi.mock('@/features/cards/use-cards', () => ({
  useCards: () => ({ data: undefined }),
}));

vi.mock('@/features/tags/tag-picker', () => ({
  TagPicker: ({ selectedTagIds, onTagsChange }: { selectedTagIds: number[]; onTagsChange: (ids: number[]) => void }) => (
    <div data-testid="tag-picker">
      <span>Selected: {selectedTagIds.join(',')}</span>
      <button data-testid="add-tag" onClick={() => onTagsChange([...selectedTagIds, 1])}>Add Tag</button>
    </div>
  ),
}));

describe('NoteEditor', () => {
  const onSave = vi.fn();
  const onCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('create mode', () => {
    it('renders "New Note" title', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByText('New Note')).toBeInTheDocument();
    });

    it('renders title input', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByPlaceholderText('Note title...')).toBeInTheDocument();
    });

    it('renders content textarea', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByPlaceholderText(/Write your note content/)).toBeInTheDocument();
    });

    it('renders formatting toolbar buttons', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByTitle('Bold')).toBeInTheDocument();
      expect(screen.getByTitle('Italic')).toBeInTheDocument();
      expect(screen.getByTitle('Heading')).toBeInTheDocument();
      expect(screen.getByTitle('Code')).toBeInTheDocument();
      expect(screen.getByTitle('List')).toBeInTheDocument();
    });

    it('renders preview toggle button', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByText('Preview')).toBeInTheDocument();
    });

    it('renders TagPicker', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByTestId('tag-picker')).toBeInTheDocument();
    });

    it('renders cancel and save buttons', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByText('Cancel')).toBeInTheDocument();
      expect(screen.getByText('Save')).toBeInTheDocument();
    });

    it('toggles preview mode', async () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      fireEvent.click(screen.getByText('Preview'));
      await waitFor(() => {
        expect(screen.getByTestId('markdown-renderer')).toBeInTheDocument();
      });
      expect(screen.getByText('Edit')).toBeInTheDocument();
    });

    it('calls onCancel when cancel clicked', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      fireEvent.click(screen.getByText('Cancel'));
      expect(onCancel).toHaveBeenCalled();
    });

    it('does not call create mutation when title is empty', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      fireEvent.click(screen.getByText('Save'));
      expect(mockCreateMutate).not.toHaveBeenCalled();
    });

    it('does not render delete button in create mode', () => {
      renderWithProviders(<NoteEditor onSave={onSave} onCancel={onCancel} />);
      expect(screen.queryByText('Delete')).not.toBeInTheDocument();
    });
  });

  describe('edit mode', () => {
    const mockNote: Note = {
      id: 1,
      title: 'Existing Note',
      content: 'Existing content',
      user_id: 1,
      tags: [{ id: 1, name: 'frontend', color: 'teal' }],
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-06-15T00:00:00Z',
    };

    it('renders "Edit Note" title', () => {
      renderWithProviders(<NoteEditor note={mockNote} onSave={onSave} onCancel={onCancel} />);
      expect(screen.getByText('Edit Note')).toBeInTheDocument();
    });

    it('pre-fills title and content', () => {
      renderWithProviders(<NoteEditor note={mockNote} onSave={onSave} onCancel={onCancel} />);
      const titleInput = screen.getByPlaceholderText('Note title...') as HTMLInputElement;
      expect(titleInput.value).toBe('Existing Note');
    });

    it('calls update mutation on save', () => {
      renderWithProviders(<NoteEditor note={mockNote} onSave={onSave} onCancel={onCancel} />);
      fireEvent.click(screen.getByText('Save'));
      expect(mockUpdateMutate).toHaveBeenCalled();
    });

    it('renders delete button in edit mode when onDelete provided', () => {
      const onDelete = vi.fn();
      renderWithProviders(<NoteEditor note={mockNote} onSave={onSave} onCancel={onCancel} onDelete={onDelete} />);
      expect(screen.getByText('Delete')).toBeInTheDocument();
    });
  });
});
