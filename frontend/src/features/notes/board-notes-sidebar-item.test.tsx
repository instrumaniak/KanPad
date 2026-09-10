import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BoardNotesSidebarItem } from './board-notes-sidebar-item';
import type { Note } from './notes.api';

const mockNote: Note = {
  id: 1,
  title: 'Test Note',
  content: 'Some content',
  user_id: 1,
  tags: [],
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-06-15T00:00:00Z',
};

const onClick = vi.fn();
const onEdit = vi.fn();
const onDelete = vi.fn();

describe('BoardNotesSidebarItem', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders note title', () => {
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    expect(screen.getByText('Test Note')).toBeInTheDocument();
  });

  it('renders updated date', () => {
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    expect(screen.getByText('Jun 15, 2024')).toBeInTheDocument();
  });

  it('has accessible article role with label', () => {
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    expect(
      screen.getByRole('article', { name: 'Note: Test Note' }),
    ).toBeInTheDocument();
  });

  it('calls onClick when card is clicked', () => {
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    fireEvent.click(screen.getByRole('article'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('calls onEdit when Edit clicked in dropdown', async () => {
    const user = userEvent.setup();
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    const actionsButton = screen.getByText('Actions').closest('button')!;
    await user.click(actionsButton);
    const editButton = await screen.findByText('Edit');
    await user.click(editButton);
    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  it('calls onDelete when Delete clicked in dropdown', async () => {
    const user = userEvent.setup();
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    const actionsButton = screen.getByText('Actions').closest('button')!;
    await user.click(actionsButton);
    const deleteButton = await screen.findByText('Delete');
    await user.click(deleteButton);
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it('does not call onClick when actions button is clicked', async () => {
    const user = userEvent.setup();
    render(
      <BoardNotesSidebarItem
        note={mockNote}
        onClick={onClick}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );
    const actionsButton = screen.getByText('Actions').closest('button')!;
    await user.click(actionsButton);
    expect(onClick).not.toHaveBeenCalled();
  });
});
