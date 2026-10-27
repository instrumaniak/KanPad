import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { components } from '@/lib/api/schema';
import { BASE, ApiError, apiFetch, apiFetchVoid } from '@/lib/api/client';

import {
  createChecklist,
  fetchChecklist,
  updateChecklist,
  deleteChecklist,
  createChecklistItem,
  updateChecklistItem,
  deleteChecklistItem,
} from './checklists.api';

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

type Checklist = components['schemas']['ChecklistResponse'];
type ChecklistItem = components['schemas']['ChecklistItemResponse'];

const mockChecklist: Checklist = {
  id: 1,
  title: 'My Checklist',
  card_id: 10,
  items: [],
  created_at: '2024-01-01T00:00:00.000000Z',
  updated_at: '2024-01-01T00:00:00.000000Z',
};

const mockChecklistItem: ChecklistItem = {
  id: 1,
  text: 'Task 1',
  is_completed: false,
  checklist_id: 1,
  position: 0,
  created_at: '2024-01-01T00:00:00.000000Z',
  updated_at: '2024-01-01T00:00:00.000000Z',
};

describe('checklists.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('createChecklist', () => {
    it('creates a checklist for a card', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockChecklist });

      const result = await createChecklist({ title: 'My Checklist', card_id: 10 });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/cards/10/checklists`, {
        method: 'POST',
        body: JSON.stringify({ title: 'My Checklist' }),
      });
      expect(result).toEqual({ data: mockChecklist });
    });

    it('propagates errors with status', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Card not found'));

      await expect(createChecklist({ title: 'X', card_id: 999 })).rejects.toMatchObject({
        status: 404,
      });
    });
  });

  describe('fetchChecklist', () => {
    it('fetches a checklist by id', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockChecklist });

      const result = await fetchChecklist(1);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/checklists/1`);
      expect(result).toEqual({ data: mockChecklist });
    });

    it('propagates not-found errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Checklist not found'));

      await expect(fetchChecklist(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('updateChecklist', () => {
    it('updates a checklist title', async () => {
      const updated = { ...mockChecklist, title: 'Updated Checklist' };
      mockApiFetch.mockResolvedValueOnce({ data: updated });

      const result = await updateChecklist(1, { title: 'Updated Checklist' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/checklists/1`, {
        method: 'PATCH',
        body: JSON.stringify({ title: 'Updated Checklist' }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('propagates validation errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(400, 'Validation failed'));

      await expect(updateChecklist(1, { title: '' })).rejects.toMatchObject({ status: 400 });
    });
  });

  describe('deleteChecklist', () => {
    it('deletes a checklist (204 void)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteChecklist(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/checklists/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates delete errors', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Checklist not found'));

      await expect(deleteChecklist(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('createChecklistItem', () => {
    it('creates a checklist item', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: mockChecklistItem });

      const result = await createChecklistItem(1, { text: 'Task 1', position: 0 });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/checklists/1/items`, {
        method: 'POST',
        body: JSON.stringify({ text: 'Task 1', position: 0 }),
      });
      expect(result).toEqual({ data: mockChecklistItem });
    });

    it('propagates create errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Checklist not found'));

      await expect(createChecklistItem(999, { text: 'X' })).rejects.toMatchObject({
        status: 404,
      });
    });
  });

  describe('updateChecklistItem', () => {
    it('updates a checklist item', async () => {
      const updated = { ...mockChecklistItem, is_completed: true };
      mockApiFetch.mockResolvedValueOnce({ data: updated });

      const result = await updateChecklistItem(1, { is_completed: true });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/checklist-items/1`, {
        method: 'PATCH',
        body: JSON.stringify({ is_completed: true }),
      });
      expect(result).toEqual({ data: updated });
    });

    it('propagates update errors', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Item not found'));

      await expect(updateChecklistItem(999, { text: 'X' })).rejects.toMatchObject({
        status: 404,
      });
    });
  });

  describe('deleteChecklistItem', () => {
    it('deletes a checklist item (204 void)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteChecklistItem(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/checklist-items/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates delete errors', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Item not found'));

      await expect(deleteChecklistItem(999)).rejects.toMatchObject({ status: 404 });
    });
  });
});
