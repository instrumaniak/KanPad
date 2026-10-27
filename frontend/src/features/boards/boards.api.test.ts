import { describe, it, expect, vi, beforeEach } from 'vitest';
import { apiFetch, apiFetchVoid, BASE, ApiError } from '@/lib/api/client';
import type { components } from '@/lib/api/schema';
import {
  fetchBoards,
  fetchBoard,
  createBoard,
  updateBoard,
  deleteBoard,
  archiveBoard,
  restoreBoard,
  permanentDeleteBoard,
  fetchArchivedBoards,
  recreateBoard,
  buildBoardsQuery,
} from './boards.api';

vi.mock('@/lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/api/client')>();
  return { ...actual, apiFetch: vi.fn(), apiFetchVoid: vi.fn() };
});

const mockApiFetch = apiFetch as unknown as ReturnType<typeof vi.fn>;
const mockApiFetchVoid = apiFetchVoid as unknown as ReturnType<typeof vi.fn>;

type BoardList = components['schemas']['BoardListResponse'];
type BoardDetail = components['schemas']['BoardDetailResponse'];

const boardFixture: BoardList = {
  id: 1,
  name: 'Board 1',
  background_color: '#0079BF',
  project_id: null,
  created_at: '2024-01-01T00:00:00.000000Z',
  updated_at: '2024-01-01T00:00:00.000000Z',
  is_archived: false,
  view_mode: 'board',
};

const detailFixture: BoardDetail = {
  ...boardFixture,
  columns: [
    { id: 10, name: 'To Do', position: 0 },
    { id: 11, name: 'In Progress', position: 1 },
    { id: 12, name: 'Done', position: 2 },
  ],
};

describe('boards.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('targets the versioned path under BASE', () => {
    expect(`${BASE}${buildBoardsQuery({ project_id: 5 })}`).toBe(`${BASE}/api/v1/boards?project_id=5`);
  });

  describe('fetchBoards', () => {
    it('fetches the first page without params', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [boardFixture], page: 1, limit: 20, total: 1 });
      const result = await fetchBoards();
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards');
      expect(result.data).toEqual([boardFixture]);
      expect(result.total).toBe(1);
    });

    it('filters by project_id with snake_case key', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 20, total: 0 });
      await fetchBoards({ project_id: 5 });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards?project_id=5');
    });

    it('paginates with page/limit', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 2, limit: 10, total: 0 });
      await fetchBoards({ page: 2, limit: 10 });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards?page=2&limit=10');
    });

    it('sanitizes garbage that would 400 (NaN ids, page=0, limit=500)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 20, total: 0 });
      await fetchBoards({ project_id: NaN, page: 0, limit: 500 });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards');
    });

    it('sanitizes non-integer and negative ids', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 20, total: 0 });
      await fetchBoards({ project_id: 1.5 });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards');
      await fetchBoards({ project_id: -3 });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards');
    });

    it('propagates ApiError with status preserved', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(401, 'Unauthorized'));
      await expect(fetchBoards()).rejects.toMatchObject({ status: 401 });
    });
  });

  describe('fetchBoard', () => {
    it('fetches single board detail by id', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: detailFixture });
      const result = await fetchBoard(1);
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards/1');
      expect(result.data.columns).toHaveLength(3);
    });
  });

  describe('createBoard', () => {
    it('creates a new board via POST', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: detailFixture });
      const result = await createBoard({ name: 'New Board' });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards', {
        method: 'POST',
        body: JSON.stringify({ name: 'New Board' }),
      });
      expect(result.data.name).toBe('Board 1');
    });

    it('passes snake_case project_id through verbatim', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: detailFixture });
      await createBoard({ name: 'New Board', project_id: 7 });
      expect(mockApiFetch).toHaveBeenCalledWith(
        '/api/v1/boards',
        expect.objectContaining({ body: JSON.stringify({ name: 'New Board', project_id: 7 }) }),
      );
    });
  });

  describe('updateBoard', () => {
    it('patches a board', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: detailFixture });
      const result = await updateBoard(1, { name: 'Updated Board' });
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards/1', {
        method: 'PATCH',
        body: JSON.stringify({ name: 'Updated Board' }),
      });
      expect(result.data.id).toBe(1);
    });
  });

  describe('deleteBoard', () => {
    it('deletes via apiFetchVoid (204, void by contract)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);
      const result = await deleteBoard(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith('/api/v1/boards/1', { method: 'DELETE' });
      expect(result).toBeUndefined();
    });
  });

  describe('archiveBoard / restoreBoard', () => {
    it('archives via is_archived flip (no verb route)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: { ...detailFixture, is_archived: true } });
      const result = await archiveBoard(1);
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards/1', {
        method: 'PATCH',
        body: JSON.stringify({ is_archived: true }),
      });
      expect(result.data.is_archived).toBe(true);
    });

    it('restores via is_archived flip (no verb route)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: detailFixture });
      const result = await restoreBoard(1);
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards/1', {
        method: 'PATCH',
        body: JSON.stringify({ is_archived: false }),
      });
      expect(result.data.is_archived).toBe(false);
    });

    it('propagates 404 when the board is missing', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Board not found'));
      await expect(archiveBoard(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('permanentDeleteBoard', () => {
    it('deletes via plain DELETE (no /permanent verb route)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);
      const result = await permanentDeleteBoard(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith('/api/v1/boards/1', { method: 'DELETE' });
      expect(result).toBeUndefined();
    });

    it('surfaces the 409 wrong-state conflict for active boards', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(409, 'Board is not archived'));
      await expect(permanentDeleteBoard(1)).rejects.toMatchObject({ status: 409 });
    });
  });

  describe('fetchArchivedBoards', () => {
    it('fetches with archived=true (no /archived duplicate route)', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [boardFixture], page: 1, limit: 20, total: 1 });
      const result = await fetchArchivedBoards();
      expect(mockApiFetch).toHaveBeenCalledWith('/api/v1/boards?archived=true');
      expect(result.total).toBe(1);
    });

    it('returns an empty page when nothing is archived', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 20, total: 0 });
      const result = await fetchArchivedBoards();
      expect(result.data).toHaveLength(0);
      expect(result.total).toBe(0);
    });
  });

  describe('recreateBoard', () => {
    it('delegates to createBoard with a bare name', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: detailFixture });
      await recreateBoard('Fresh');
      expect(mockApiFetch).toHaveBeenCalledWith(
        '/api/v1/boards',
        expect.objectContaining({ method: 'POST' }),
      );
    });
  });
});
