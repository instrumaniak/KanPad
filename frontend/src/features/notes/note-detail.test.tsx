import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { NoteDetail } from './note-detail';
import type { Note } from './notes.api';

vi.mock('./markdown-renderer', () => ({
  MarkdownRenderer: ({ content }: { content: string }) => <div data-testid="markdown-renderer">{content}</div>,
}));

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

describe('NoteDetail', () => {
  const mockNote: Note = {
    id: 1,
    title: 'Test Note',
    content: '# Hello\nThis is **markdown** content.',
    user_id: 1,
    tags: [{ id: 1, name: 'frontend', color: 'teal' }],
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-06-15T00:00:00Z',
  };

  const onBack = vi.fn();
  const onEdit = vi.fn();
  const onDelete = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders note title', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Test Note')).toBeInTheDocument();
  });

  it('renders type badge', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('general')).toBeInTheDocument();
  });

  it('renders tags', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('frontend')).toBeInTheDocument();
  });

  it('renders back button', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Back')).toBeInTheDocument();
  });

  it('calls onBack when back button clicked', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    fireEvent.click(screen.getByText('Back'));
    expect(onBack).toHaveBeenCalled();
  });

  it('renders edit button', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Edit')).toBeInTheDocument();
  });

  it('calls onEdit when edit button clicked', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    fireEvent.click(screen.getByText('Edit'));
    expect(onEdit).toHaveBeenCalled();
  });

  it('renders delete button', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Delete')).toBeInTheDocument();
  });

  it('shows delete confirmation dialog then calls onDelete', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    fireEvent.click(screen.getByText('Delete'));

    expect(screen.getByText('Delete note?')).toBeInTheDocument();
    const confirmDelete = screen.getByText('Delete', { selector: '[data-slot="alert-dialog-action"]' });
    fireEvent.click(confirmDelete);
    expect(onDelete).toHaveBeenCalled();
  });

  it('renders created and updated dates', () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText(/Created/)).toBeInTheDocument();
    expect(screen.getByText(/Updated/)).toBeInTheDocument();
  });

  it('handles note without tags', () => {
    const noteWithoutTags: Note = { ...mockNote, tags: undefined };
    renderWithProviders(<NoteDetail note={noteWithoutTags} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Test Note')).toBeInTheDocument();
    expect(screen.queryByText('frontend')).not.toBeInTheDocument();
  });

  it('renders markdown content', async () => {
    renderWithProviders(<NoteDetail note={mockNote} onBack={onBack} onEdit={onEdit} onDelete={onDelete} />);
    expect(await screen.findByTestId('markdown-renderer')).toBeInTheDocument();
  });
});
