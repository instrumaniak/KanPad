import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ToastProvider } from '@/components/ui/toast-provider';

const mockUseNote = vi.fn();
const mockNavigate = vi.fn();

vi.mock('./use-notes', () => ({
  useNote: (id: number) => mockUseNote(id),
}));

vi.mock('./note-editor', () => ({
  NoteEditor: ({ note, onSave, onCancel }: { note: { id: number; title: string }; onSave: () => void; onCancel: () => void }) => (
    <div data-testid="note-editor">
      <span>Edit: {note.title}</span>
      <button data-testid="editor-save" onClick={onSave}>Save</button>
      <button data-testid="editor-cancel" onClick={onCancel}>Cancel</button>
    </div>
  ),
}));

vi.mock('@/components/loading-skeleton', () => ({
  LoadingSkeleton: ({ count }: { count: number }) => (
    <div data-testid="loading-skeleton" data-count={count}>Loading...</div>
  ),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

const mockNoteData = {
  data: {
    id: 1,
    title: 'Test Note',
    content: 'Test content',
    user_id: 1,
    tags: [],
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-06-15T00:00:00Z',
  },
};

const renderWithProviders = (noteId = '1') => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, refetchOnWindowFocus: false },
      mutations: { retry: false },
    },
  });
  return render(
    <MemoryRouter initialEntries={[`/notes/${noteId}/edit`]}>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <Routes>
            <Route path="/notes/:id/edit" element={<NoteEditPage />} />
            <Route path="/notes/:id" element={<div data-testid="note-detail-page">Note Detail</div>} />
            <Route path="/notes" element={<div data-testid="notes-list-page">Notes List</div>} />
          </Routes>
        </ToastProvider>
      </QueryClientProvider>
    </MemoryRouter>,
  );
};

import { NoteEditPage } from './note-edit-page';

describe('NoteEditPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseNote.mockReturnValue({ data: mockNoteData, isLoading: false, error: null });
  });

  it('renders loading skeleton while loading', () => {
    mockUseNote.mockReturnValue({ data: undefined, isLoading: true, error: null });
    renderWithProviders();
    expect(screen.getByTestId('loading-skeleton')).toBeInTheDocument();
  });

  it('renders note not found when data is null', () => {
    mockUseNote.mockReturnValue({ data: null, isLoading: false, error: null });
    renderWithProviders();
    expect(screen.getByText('Note not found')).toBeInTheDocument();
  });

  it('renders note editor with note data', () => {
    renderWithProviders();
    expect(screen.getByTestId('note-editor')).toBeInTheDocument();
    expect(screen.getByText('Edit: Test Note')).toBeInTheDocument();
  });

  it('navigates to note detail on save', () => {
    renderWithProviders();
    fireEvent.click(screen.getByTestId('editor-save'));
    expect(screen.getByTestId('note-detail-page')).toBeInTheDocument();
  });

  it('navigates to note detail on cancel', () => {
    renderWithProviders();
    fireEvent.click(screen.getByTestId('editor-cancel'));
    expect(screen.getByTestId('note-detail-page')).toBeInTheDocument();
  });

  it('navigates to notes list when back is clicked on not found', () => {
    mockUseNote.mockReturnValue({ data: null, isLoading: false, error: null });
    renderWithProviders('999');
    fireEvent.click(screen.getByText('Back to notes'));
    expect(screen.getByTestId('notes-list-page')).toBeInTheDocument();
  });
});
