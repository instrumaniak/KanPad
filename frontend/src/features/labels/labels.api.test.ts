import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { components } from '@/lib/api/schema';
import { BASE, ApiError, apiFetch, apiFetchVoid } from '@/lib/api/client';

import {
  fetchLabels,
  createLabel,
  updateLabel,
  deleteLabel,
} from './labels.api';

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

type Label = components['schemas']['LabelResponse'];

const mockLabel: Label = {
  id: 1,
  name: 'Bug',
  color: 'red',
  created_at: '2024-01-01T00:00:00.000000Z',
  updated_at: '2024-01-01T00:00:00.000000Z',
};

describe('labels.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchLabels', () => {
    it('fetches the label page (paginated envelope)', async () => {
      const page = { data: [mockLabel], page: 1, limit: 50, total: 1 };
      mockApiFetch.mockResolvedValueOnce(page);

      const result = await fetchLabels();
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/labels`);
      expect(result).toEqual(page);
    });

    it('sends valid page/limit', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 2, limit: 10, total: 0 });

      await fetchLabels(2, 10);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/labels?page=2&limit=10`);
    });

    it('omits invalid page/limit instead of sending garbage (newly-strict audit)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [mockLabel], page: 1, limit: 50, total: 1 });

      await fetchLabels(0, 500);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/labels`);

      await fetchLabels(Number.NaN, undefined);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/labels`);
    });

    it('propagates ApiError with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(401, 'Not authenticated'));

      await expect(fetchLabels()).rejects.toMatchObject({ status: 401 });
    });
  });

  describe('createLabel', () => {
    it('creates a new label', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockLabel });

      const data: components['schemas']['CreateLabelInput'] = { name: 'Bug', color: 'red' };
      const result = await createLabel(data);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/labels`, {
        method: 'POST',
        body: JSON.stringify(data),
      });
      expect(result).toEqual({ data: mockLabel });
    });

    it('propagates validation errors with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(createLabel({ name: '', color: 'red' })).rejects.toMatchObject({
        status: 400,
      });
    });
  });

  describe('updateLabel', () => {
    it('updates a label name', async () => {
      const updated = { ...mockLabel, name: 'Critical' };
      mockApiFetch.mockResolvedValueOnce({ data: updated });

      const result = await updateLabel(1, { name: 'Critical' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/labels/1`, {
        method: 'PATCH',
        body: JSON.stringify({ name: 'Critical' }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('propagates not-found errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Label not found'));

      await expect(updateLabel(999, { name: 'X' })).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('deleteLabel', () => {
    it('deletes a label (204 void)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteLabel(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/labels/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates delete errors', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Label not found'));

      await expect(deleteLabel(999)).rejects.toMatchObject({ status: 404 });
    });
  });
});
