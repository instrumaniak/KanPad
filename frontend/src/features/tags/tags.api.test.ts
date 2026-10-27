import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { components } from '@/lib/api/schema';
import { BASE, ApiError, apiFetch, apiFetchVoid } from '@/lib/api/client';

import { fetchTags, createTag, updateTag, deleteTag } from './tags.api';

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

type Tag = components['schemas']['TagResponse'];

const mockTag: Tag = {
  id: 1,
  name: 'frontend',
  color: 'teal',
  created_at: '2024-01-01T00:00:00.000000Z',
};

describe('tags.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchTags', () => {
    it('fetches the tag page (paginated envelope, no updated_at by design)', async () => {
      const page = { data: [mockTag], page: 1, limit: 50, total: 1 };
      mockApiFetch.mockResolvedValueOnce(page);

      const result = await fetchTags();
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/tags`);
      expect(result).toEqual(page);
      expect('updated_at' in result.data[0]).toBe(false);
    });

    it('sends valid page/limit', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 2, limit: 10, total: 0 });

      await fetchTags(2, 10);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/tags?page=2&limit=10`);
    });

    it('omits invalid page/limit instead of sending garbage (newly-strict audit)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [mockTag], page: 1, limit: 50, total: 1 });

      await fetchTags(0, 500);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/tags`);
    });

    it('propagates ApiError with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(401, 'Not authenticated'));

      await expect(fetchTags()).rejects.toMatchObject({ status: 401 });
    });
  });

  describe('createTag', () => {
    it('creates a tag', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockTag });

      const body: components['schemas']['CreateTagInput'] = {
        name: 'frontend',
        color: 'teal',
      };
      const result = await createTag(body);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/tags`, {
        method: 'POST',
        body: JSON.stringify(body),
      });
      expect(result).toEqual({ data: mockTag });
    });

    it('propagates validation errors with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(createTag({ name: '', color: '' })).rejects.toMatchObject({
        status: 400,
      });
    });
  });

  describe('updateTag', () => {
    it('updates a tag', async () => {
      const updated = { ...mockTag, name: 'backend' };
      mockApiFetch.mockResolvedValueOnce({ data: updated });

      const result = await updateTag(1, { name: 'backend' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/tags/1`, {
        method: 'PATCH',
        body: JSON.stringify({ name: 'backend' }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('propagates validation errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(updateTag(1, { name: '' })).rejects.toMatchObject({ status: 400 });
    });
  });

  describe('deleteTag', () => {
    it('deletes a tag (204 void)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteTag(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/tags/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates delete errors', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Tag not found'));

      await expect(deleteTag(999)).rejects.toMatchObject({ status: 404 });
    });
  });
});
