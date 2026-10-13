import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { components } from '@/lib/api/schema';
import { BASE, ApiError, apiFetch, apiFetchVoid } from '@/lib/api/client';

import {
  createNote,
  fetchNotes,
  fetchNote,
  updateNote,
  deleteNote,
  fetchBoardNotes,
  getNoteType,
} from './notes.api';

vi.mock('@/lib/api/client', () => ({
  BASE: '',
  ApiError: class ApiError extends Error {
    status: number;
    constructor(status: number, message: string) {
      super(message);
      this.status = status;
    }
  },
  apiFetch: vi.fn(),
  apiFetchVoid: vi.fn(),
}));

const mockApiFetch = apiFetch as ReturnType<typeof vi.fn>;
const mockApiFetchVoid = apiFetchVoid as ReturnType<typeof vi.fn>;

type Note = components['schemas']['NoteDetailResponse'];

const mockNote: Note = {
  id: 1,
  title: 'My Note',
  content: '# Hello',
  board_id: null,
  project_id: null,
  card_id: null,
  user_id: 1,
  tags: [{ id: 1, name: 'frontend', color: 'teal' }],
  created_at: '2024-01-01T00:00:00.000000Z',
  updated_at: '2024-01-01T00:00:00.000000Z',
};

describe('notes.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('createNote', () => {
    it('creates a note', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockNote });

      const body: components['schemas']['CreateNoteInput'] = {
        title: 'My Note',
        content: '# Hello',
      };
      const result = await createNote(body);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes`, {
        method: 'POST',
        body: JSON.stringify(body),
      });
      expect(result).toEqual({ data: mockNote });
    });

    it('creates a note with snake_case tag_ids', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockNote });

      await createNote({ title: 'My Note', content: '# Hello', tag_ids: [1] });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes`, {
        method: 'POST',
        body: JSON.stringify({ title: 'My Note', content: '# Hello', tag_ids: [1] }),
      });
    });

    it('propagates validation errors with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(createNote({ title: '', content: '' })).rejects.toMatchObject({
        status: 400,
      });
    });
  });

  describe('fetchNotes', () => {
    it('fetches the note page (paginated envelope, no content key in items)', async () => {
      const { content: _content, ...summary } = mockNote;
      const page = { data: [summary], page: 1, limit: 50, total: 1 };
      mockApiFetch.mockResolvedValueOnce(page);

      const result = await fetchNotes();
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes`);
      expect(result).toEqual(page);
      expect('content' in result.data[0]).toBe(false);
    });

    it('fetches with search filter', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 50, total: 0 });

      await fetchNotes({ search: 'hello' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes?search=hello`);
    });

    it('fetches with type filter', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 50, total: 0 });

      await fetchNotes({ type: 'general' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes?type=general`);
    });

    it('sends snake_case tag_id (legacy tagId retired)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 50, total: 0 });

      await fetchNotes({ tag_id: 1 });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes?tag_id=1`);
    });

    it('sends snake_case board_id', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 50, total: 0 });

      await fetchNotes({ board_id: 2 });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes?board_id=2`);
    });

    it('omits invalid numerics instead of sending garbage (newly-strict audit)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 50, total: 0 });

      await fetchNotes({ tag_id: 0, board_id: Number.NaN, page: 0, limit: 500 });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes`);
    });

    it('propagates errors with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(fetchNotes({ type: 'bogus' as never })).rejects.toMatchObject({
        status: 400,
      });
    });
  });

  describe('fetchNote', () => {
    it('fetches a single note with content', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockNote });

      const result = await fetchNote(1);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes/1`);
      expect(result).toEqual({ data: mockNote });
    });

    it('propagates not-found errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Note not found'));

      await expect(fetchNote(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('updateNote', () => {
    it('updates a note', async () => {
      const updated = { ...mockNote, title: 'Updated' };
      mockApiFetch.mockResolvedValueOnce({ data: updated });

      const result = await updateNote(1, { title: 'Updated' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes/1`, {
        method: 'PATCH',
        body: JSON.stringify({ title: 'Updated' }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('propagates validation errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(updateNote(1, { title: '' })).rejects.toMatchObject({ status: 400 });
    });
  });

  describe('deleteNote', () => {
    it('deletes a note (204 void)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteNote(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/notes/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates delete errors', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Note not found'));

      await expect(deleteNote(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('fetchBoardNotes', () => {
    it('fetches board notes via the ?board_id= filter (deleted nested route retired)', async () => {
      const page = { data: [], page: 1, limit: 50, total: 0 };
      mockApiFetch.mockResolvedValueOnce(page);

      const result = await fetchBoardNotes(1);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/notes?board_id=1`);
      expect(result).toEqual(page);
    });

    it('propagates errors with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Board not found'));

      await expect(fetchBoardNotes(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('getNoteType', () => {
    it('derives type from the set FK', () => {
      expect(getNoteType({ card_id: 1, project_id: null, board_id: null })).toBe('card');
      expect(getNoteType({ card_id: null, project_id: 2, board_id: null })).toBe('project');
      expect(getNoteType({ card_id: null, project_id: null, board_id: 3 })).toBe('board');
      expect(getNoteType({ card_id: null, project_id: null, board_id: null })).toBe('general');
    });
  });
});
