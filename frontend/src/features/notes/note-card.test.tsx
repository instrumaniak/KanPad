import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { NoteCard } from './note-card';
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

describe('NoteCard', () => {
  const mockNote: Note = {
    id: 1,
    title: 'Test Note',
    content: 'Some content here',
    user_id: 1,
    tags: [{ id: 1, name: 'frontend', color: 'teal' }],
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-06-15T00:00:00Z',
  };

  const onClick = vi.fn();
  const onEdit = vi.fn();
  const onDelete = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders note title', () => {
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Test Note')).toBeInTheDocument();
  });

  it('renders type badge', () => {
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('general')).toBeInTheDocument();
  });

  it('renders tags', () => {
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('frontend')).toBeInTheDocument();
  });

  it('renders date', () => {
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Jun 15, 2024')).toBeInTheDocument();
  });

  it('renders content preview without markdown symbols', () => {
    const noteWithMarkdown: Note = {
      ...mockNote,
      content: '# Hello **world**',
    };
    renderWithProviders(<NoteCard note={noteWithMarkdown} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText(/Hello world/)).toBeInTheDocument();
  });

  it('calls onClick when card is clicked', () => {
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    fireEvent.click(screen.getByRole('article'));
    expect(onClick).toHaveBeenCalled();
  });

  it('calls onEdit when edit is clicked in dropdown', async () => {
    const user = userEvent.setup();
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    const actionsButton = screen.getByText('Actions').closest('button')!;
    await user.click(actionsButton);
    const editButton = await screen.findByText('Edit');
    await user.click(editButton);
    expect(onEdit).toHaveBeenCalled();
  });

  it('calls onDelete when delete is clicked in dropdown', async () => {
    const user = userEvent.setup();
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    const actionsButton = screen.getByText('Actions').closest('button')!;
    await user.click(actionsButton);
    const deleteButton = await screen.findByText('Delete');
    await user.click(deleteButton);
    expect(onDelete).toHaveBeenCalled();
  });

  it('renders article role with accessible label', () => {
    renderWithProviders(<NoteCard note={mockNote} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByRole('article', { name: 'Note: Test Note' })).toBeInTheDocument();
  });

  it('handles note without tags', () => {
    const noteWithoutTags: Note = { ...mockNote, tags: undefined };
    renderWithProviders(<NoteCard note={noteWithoutTags} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Test Note')).toBeInTheDocument();
    expect(screen.queryByText('frontend')).not.toBeInTheDocument();
  });

  it('handles note without content', () => {
    const noteWithoutContent: Note = { ...mockNote, content: '' };
    renderWithProviders(<NoteCard note={noteWithoutContent} onClick={onClick} onEdit={onEdit} onDelete={onDelete} />);
    expect(screen.getByText('Test Note')).toBeInTheDocument();
  });
});
