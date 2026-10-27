import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  useBoards,
  useBoard,
  useCreateBoard,
  useUpdateBoard,
  useDeleteBoard,
  useArchiveBoard,
  useRestoreBoard,
  usePermanentDeleteBoard,
  useArchivedBoards,
} from './use-boards';

vi.mock('./boards.api', () => ({
  fetchBoards: vi.fn(),
  fetchBoard: vi.fn(),
  createBoard: vi.fn(),
  updateBoard: vi.fn(),
  deleteBoard: vi.fn(),
  archiveBoard: vi.fn(),
  restoreBoard: vi.fn(),
  permanentDeleteBoard: vi.fn(),
  fetchArchivedBoards: vi.fn(),
}));

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
} from './boards.api';

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
};

describe('useBoards', () => {
  beforeEach(() => vi.clearAllMocks());

  it('fetches boards without filters', async () => {
    (fetchBoards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: [],
      page: 1,
      limit: 20,
      total: 0,
    });
    const { result } = renderHook(() => useBoards(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(fetchBoards).toHaveBeenCalledWith({
      project_id: undefined,
      page: undefined,
      limit: undefined,
    });
  });

  it('fetches boards with projectId and pagination', async () => {
    (fetchBoards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: [],
      page: 2,
      limit: 10,
      total: 0,
    });
    const { result } = renderHook(() => useBoards(5, 2, 10), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(fetchBoards).toHaveBeenCalledWith({ project_id: 5, page: 2, limit: 10 });
  });

  it('sanitizes garbage so the strict backend never sees it', async () => {
    (fetchBoards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: [],
      page: 1,
      limit: 20,
      total: 0,
    });
    renderHook(() => useBoards(NaN, 0, 500), { wrapper: createWrapper() });
    await waitFor(() =>
      expect(fetchBoards).toHaveBeenCalledWith({
        project_id: undefined,
        page: undefined,
        limit: undefined,
      }),
    );
  });
});

describe('useBoard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('fetches board by id', async () => {
    (fetchBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'Test', background_color: '#fff', project_id: null, created_at: '', updated_at: '' }
    });
    const { result } = renderHook(() => useBoard(1), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(fetchBoard).toHaveBeenCalledWith(1);
  });

  it('does not fetch when id is falsy', async () => {
    const { result } = renderHook(() => useBoard(0), { wrapper: createWrapper() });
    expect(result.current.isFetching).toBe(false);
    expect(fetchBoard).not.toHaveBeenCalled();
  });

  it('does not fetch non-integer ids', async () => {
    const { result } = renderHook(() => useBoard(1.5), { wrapper: createWrapper() });
    expect(result.current.isFetching).toBe(false);
    expect(fetchBoard).not.toHaveBeenCalled();
  });
});

describe('useCreateBoard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('creates a board and invalidates queries', async () => {
    (createBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'New', background_color: '#fff', project_id: null, created_at: '', updated_at: '' }
    });
    const { result } = renderHook(() => useCreateBoard(), { wrapper: createWrapper() });
    result.current.mutate({ name: 'New' });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(createBoard).toHaveBeenCalledWith({ name: 'New' });
  });
});

describe('useUpdateBoard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('updates a board and invalidates queries', async () => {
    (updateBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'Updated', background_color: '#fff', project_id: null, created_at: '', updated_at: '' }
    });
    const { result } = renderHook(() => useUpdateBoard(), { wrapper: createWrapper() });
    result.current.mutate({ id: 1, data: { name: 'Updated' } });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(updateBoard).toHaveBeenCalledWith(1, { name: 'Updated' });
  });
});

describe('useDeleteBoard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('deletes a board and invalidates queries (void by contract)', async () => {
    (deleteBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => useDeleteBoard(), { wrapper: createWrapper() });
    result.current.mutate(1);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(deleteBoard).toHaveBeenCalledWith(1);
    expect(result.current.data).toBeUndefined();
  });
});

describe('useArchiveBoard / useRestoreBoard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('archives via the is_archived flip', async () => {
    (archiveBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce({ data: { id: 1 } });
    const { result } = renderHook(() => useArchiveBoard(), { wrapper: createWrapper() });
    result.current.mutate(1);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(archiveBoard).toHaveBeenCalledWith(1);
  });

  it('restores via the is_archived flip', async () => {
    (restoreBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce({ data: { id: 1 } });
    const { result } = renderHook(() => useRestoreBoard(), { wrapper: createWrapper() });
    result.current.mutate(1);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(restoreBoard).toHaveBeenCalledWith(1);
  });
});

describe('usePermanentDeleteBoard', () => {
  beforeEach(() => vi.clearAllMocks());

  it('permanently deletes (void by contract)', async () => {
    (permanentDeleteBoard as ReturnType<typeof vi.fn>).mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => usePermanentDeleteBoard(), { wrapper: createWrapper() });
    result.current.mutate(1);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(permanentDeleteBoard).toHaveBeenCalledWith(1);
  });
});

describe('useArchivedBoards', () => {
  beforeEach(() => vi.clearAllMocks());

  it('fetches archived boards via the archived filter', async () => {
    (fetchArchivedBoards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: [],
      page: 1,
      limit: 20,
      total: 0,
    });
    const { result } = renderHook(() => useArchivedBoards(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(fetchArchivedBoards).toHaveBeenCalledWith({ page: undefined, limit: undefined });
  });
});
