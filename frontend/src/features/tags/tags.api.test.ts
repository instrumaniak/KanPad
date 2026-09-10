import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchTags, createTag, updateTag, deleteTag, type Tag } from './tags.api';

global.fetch = vi.fn();

const mockFetch = global.fetch as ReturnType<typeof vi.fn>;

const mockTag: Tag = {
  id: 1,
  name: 'frontend',
  color: 'teal',
  user_id: 1,
  created_at: '2024-01-01T00:00:00Z',
};

describe('tags.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchTags', () => {
    it('fetches all tags', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: [mockTag] }),
      });

      const result = await fetchTags();
      expect(mockFetch).toHaveBeenCalledWith('/api/tags', { credentials: 'include' });
      expect(result).toEqual({ data: [mockTag] });
    });

    it('throws on error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        json: () => Promise.resolve({ message: 'Server error' }),
      });

      await expect(fetchTags()).rejects.toThrow('Server error');
    });

    it('throws fallback message when no error body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
        json: () => Promise.reject(new Error('parse failed')),
      });

      await expect(fetchTags()).rejects.toThrow('Internal Server Error');
    });
  });

  describe('createTag', () => {
    it('creates a tag', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: mockTag, message: 'Tag created' }),
      });

      const result = await createTag({ name: 'frontend', color: 'teal' });
      expect(mockFetch).toHaveBeenCalledWith('/api/tags', {
        credentials: 'include',
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: 'frontend', color: 'teal' }),
      });
      expect(result).toEqual({ data: mockTag, message: 'Tag created' });
    });

    it('throws on failure', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 422,
        json: () => Promise.resolve({ message: 'Validation failed' }),
      });

      await expect(createTag({ name: '', color: '' })).rejects.toThrow('Validation failed');
    });
  });

  describe('updateTag', () => {
    it('updates a tag', async () => {
      const updated = { ...mockTag, name: 'backend' };
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: updated }),
      });

      const result = await updateTag(1, { name: 'backend' });
      expect(mockFetch).toHaveBeenCalledWith('/api/tags/1', {
        credentials: 'include',
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: 'backend' }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('throws on validation error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 422,
        json: () => Promise.resolve({ message: 'Validation failed' }),
      });

      await expect(updateTag(1, { name: '' })).rejects.toThrow('Validation failed');
    });
  });

  describe('deleteTag', () => {
    it('deletes a tag (204 no content)', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        status: 204,
        json: () => Promise.resolve(undefined),
      });

      const result = await deleteTag(1);
      expect(mockFetch).toHaveBeenCalledWith('/api/tags/1', {
        credentials: 'include',
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('throws on failed delete', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Tag not found' }),
      });

      await expect(deleteTag(999)).rejects.toThrow('Tag not found');
    });
  });
});
