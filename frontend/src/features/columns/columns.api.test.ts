import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { components } from '@/lib/api/schema';
import { BASE, ApiError } from '@/lib/api/client';
import {
  fetchColumns,
  createColumn,
  updateColumn,
  deleteColumn,
  sortCards,
  moveAllCards,
} from './columns.api';

vi.mock('@/lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/api/client')>();
  return {
    ...actual,
    apiFetch: vi.fn(),
    apiFetchVoid: vi.fn(),
  };
});

import { apiFetch, apiFetchVoid } from '@/lib/api/client';

const mockApiFetch = apiFetch as ReturnType<typeof vi.fn>;
const mockApiFetchVoid = apiFetchVoid as ReturnType<typeof vi.fn>;

type Column = components['schemas']['ColumnResponse'];

function columnFixture(overrides?: Partial<Column>): Column {
  return {
    id: 1,
    name: 'To Do',
    position: 0,
    board_id: 1,
    cards: [],
    created_at: '2024-01-01T00:00:00.000000Z',
    updated_at: '2024-01-01T00:00:00.000000Z',
    ...overrides,
  };
}

describe('columns.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchColumns', () => {
    it('fetches columns for a board', async () => {
      const body: components['schemas']['ColumnListBody'] = {
        data: [
          columnFixture(),
          columnFixture({ id: 2, name: 'In Progress', position: 1 }),
        ],
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await fetchColumns(1);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/boards/1/columns`);
      expect(result).toEqual(body);
      expect(result.data[0].position).toBe(0);
    });

    it('propagates ApiError on failure', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Board not found'));
      await expect(fetchColumns(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('createColumn', () => {
    it('creates a new column', async () => {
      const body: components['schemas']['ColumnBody'] = { data: columnFixture({ name: 'New Column' }) };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await createColumn(1, { name: 'New Column' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/boards/1/columns`, {
        method: 'POST',
        body: JSON.stringify({ name: 'New Column' }),
      });
      expect(result).toEqual(body);
    });

    it('creates column with default name when not provided', async () => {
      const body: components['schemas']['ColumnBody'] = { data: columnFixture() };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await createColumn(1, {});
      expect(mockApiFetch).toHaveBeenCalledWith(
        `${BASE}/api/v1/boards/1/columns`,
        expect.objectContaining({ method: 'POST', body: JSON.stringify({}) }),
      );
      expect(result.data.name).toBe('To Do');
    });

    it('propagates ApiError on unauthorized', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(401, 'Unauthorized'));
      await expect(createColumn(1, { name: 'Test' })).rejects.toMatchObject({ status: 401 });
    });
  });

  describe('updateColumn', () => {
    it('updates a column name', async () => {
      const body: components['schemas']['ColumnBody'] = {
        data: columnFixture({ name: 'Updated Column' }),
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await updateColumn(1, { name: 'Updated Column' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/columns/1`, {
        method: 'PATCH',
        body: JSON.stringify({ name: 'Updated Column' }),
      });
      expect(result).toEqual(body);
    });

    it('propagates ApiError on failed update', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Column not found'));
      await expect(updateColumn(999, { name: 'Test' })).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('deleteColumn', () => {
    it('deletes a column via the void transport (204)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteColumn(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/columns/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates ApiError on failed delete', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Column not found'));
      await expect(deleteColumn(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('sortCards', () => {
    it('sorts via PATCH {sort_order} on the column (verb route retired)', async () => {
      const body: components['schemas']['ColumnBody'] = { data: columnFixture() };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await sortCards(1, 'asc');
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/columns/1`, {
        method: 'PATCH',
        body: JSON.stringify({ sort_order: 'asc' }),
      });
      expect(result).toEqual(body);
    });
  });

  describe('moveAllCards', () => {
    it('moves via POST move-cards with snake_case keys', async () => {
      const body: components['schemas']['MoveCardsBody'] = { data: { moved_count: 3 } };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await moveAllCards(1, 2);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/columns/1/move-cards`, {
        method: 'POST',
        body: JSON.stringify({ target_column_id: 2 }),
      });
      expect(result.data.moved_count).toBe(3);
    });
  });
});
