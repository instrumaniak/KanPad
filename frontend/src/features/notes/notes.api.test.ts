import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  createNote,
  fetchNotes,
  fetchNote,
  updateNote,
  deleteNote,
  fetchBoardNotes,
  type Note,
  type Tag,
} from './notes.api';

global.fetch = vi.fn();

const mockFetch = global.fetch as ReturnType<typeof vi.fn>;

const mockTag: Tag = { id: 1, name: 'frontend', color: 'teal' };
const mockNote: Note = {
  id: 1,
  title: 'My Note',
  content: '# Hello',
  user_id: 1,
  tags: [mockTag],
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
};

describe('notes.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('createNote', () => {
    it('creates a note', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: mockNote, message: 'Note created' }),
      });

      const result = await createNote({ title: 'My Note', content: '# Hello' });
      expect(mockFetch).toHaveBeenCalledWith('/api/notes', {
        credentials: 'include',
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: 'My Note', content: '# Hello' }),
      });
      expect(result).toEqual({ data: mockNote, message: 'Note created' });
    });

    it('throws on failure', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 422,
        json: () => Promise.resolve({ message: 'Validation failed' }),
      });

      await expect(createNote({ title: '', content: '' })).rejects.toThrow('Validation failed');
    });

    it('throws on network error', async () => {
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      await expect(createNote({ title: 'X', content: '' })).rejects.toThrow('Network error — please check your connection');
    });

    it('throws fallback message when no error body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        statusText: 'Server Error',
        json: () => Promise.reject(new Error('parse failed')),
      });

      await expect(createNote({ title: 'X', content: '' })).rejects.toThrow('Server Error');
    });
  });

  describe('fetchNotes', () => {
    it('fetches all notes', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: [mockNote] }),
      });

      const result = await fetchNotes();
      expect(mockFetch).toHaveBeenCalledWith('/api/notes', { credentials: 'include' });
      expect(result).toEqual({ data: [mockNote] });
    });

    it('fetches with search filter', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: [mockNote] }),
      });

      await fetchNotes({ search: 'hello' });
      expect(mockFetch).toHaveBeenCalledWith('/api/notes?search=hello', { credentials: 'include' });
    });

    it('fetches with type filter', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: [mockNote] }),
      });

      await fetchNotes({ type: 'general' });
      expect(mockFetch).toHaveBeenCalledWith('/api/notes?type=general', { credentials: 'include' });
    });

    it('fetches with tag filter', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: [mockNote] }),
      });

      await fetchNotes({ tagId: 1 });
      expect(mockFetch).toHaveBeenCalledWith('/api/notes?tagId=1', { credentials: 'include' });
    });

    it('throws on error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        json: () => Promise.resolve({ message: 'Server error' }),
      });

      await expect(fetchNotes()).rejects.toThrow('Server error');
    });
  });

  describe('fetchNote', () => {
    it('fetches a single note', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: mockNote }),
      });

      const result = await fetchNote(1);
      expect(mockFetch).toHaveBeenCalledWith('/api/notes/1', { credentials: 'include' });
      expect(result).toEqual({ data: mockNote });
    });

    it('throws on not found', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Note not found' }),
      });

      await expect(fetchNote(999)).rejects.toThrow('Note not found');
    });
  });

  describe('updateNote', () => {
    it('updates a note', async () => {
      const updated = { ...mockNote, title: 'Updated' };
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: updated }),
      });

      const result = await updateNote(1, { title: 'Updated' });
      expect(mockFetch).toHaveBeenCalledWith('/api/notes/1', {
        credentials: 'include',
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: 'Updated' }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('throws on validation error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 422,
        json: () => Promise.resolve({ message: 'Validation failed' }),
      });

      await expect(updateNote(1, { title: '' })).rejects.toThrow('Validation failed');
    });
  });

  describe('deleteNote', () => {
    it('deletes a note (204 no content)', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
        json: () => Promise.resolve(undefined),
      });

      const result = await deleteNote(1);
      expect(mockFetch).toHaveBeenCalledWith('/api/notes/1', {
        credentials: 'include',
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('throws on failed delete', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Note not found' }),
      });

      await expect(deleteNote(999)).rejects.toThrow('Note not found');
    });
  });

  describe('fetchBoardNotes', () => {
    it('fetches notes for a board', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: [mockNote] }),
      });

      const result = await fetchBoardNotes(1);
      expect(mockFetch).toHaveBeenCalledWith('/api/boards/1/notes', { credentials: 'include' });
      expect(result).toEqual({ data: [mockNote] });
    });

    it('throws on error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Board not found' }),
      });

      await expect(fetchBoardNotes(999)).rejects.toThrow('Board not found');
    });
  });
});
