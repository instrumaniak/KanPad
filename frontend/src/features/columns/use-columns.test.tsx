import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  useColumns,
  useCreateColumn,
  useUpdateColumn,
  useDeleteColumn,
  useSortCards,
  useMoveAllCards,
} from './use-columns';

vi.mock('./columns.api', () => ({
  fetchColumns: vi.fn(),
  createColumn: vi.fn(),
  updateColumn: vi.fn(),
  deleteColumn: vi.fn(),
  sortCards: vi.fn(),
  moveAllCards: vi.fn(),
}));

import {
  fetchColumns,
  createColumn,
  updateColumn,
  deleteColumn,
  sortCards,
  moveAllCards,
} from './columns.api';

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
};

describe('useColumns', () => {
  beforeEach(() => vi.clearAllMocks());

  it('fetches columns for a board', async () => {
    (fetchColumns as ReturnType<typeof vi.fn>).mockResolvedValueOnce({ data: [] });
    const { result } = renderHook(() => useColumns(1), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(fetchColumns).toHaveBeenCalledWith(1);
  });

  it('does not fetch when boardId is falsy', async () => {
    const { result } = renderHook(() => useColumns(0), { wrapper: createWrapper() });
    expect(result.current.isFetching).toBe(false);
    expect(fetchColumns).not.toHaveBeenCalled();
  });
});

describe('useCreateColumn', () => {
  beforeEach(() => vi.clearAllMocks());

  it('creates a column and invalidates queries', async () => {
    (createColumn as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'New', position: 0, board_id: 1, cards: [], created_at: '', updated_at: '' },
    });
    const { result } = renderHook(() => useCreateColumn(), { wrapper: createWrapper() });
    result.current.mutate({ boardId: 1, data: { name: 'New' } });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(createColumn).toHaveBeenCalledWith(1, { name: 'New' });
  });
});

describe('useUpdateColumn', () => {
  beforeEach(() => vi.clearAllMocks());

  it('updates a column and invalidates queries', async () => {
    (updateColumn as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'Updated', position: 0, board_id: 1, cards: [], created_at: '', updated_at: '' },
    });
    const { result } = renderHook(() => useUpdateColumn(), { wrapper: createWrapper() });
    result.current.mutate({ id: 1, data: { name: 'Updated' } });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(updateColumn).toHaveBeenCalledWith(1, { name: 'Updated' });
  });
});

describe('useDeleteColumn', () => {
  beforeEach(() => vi.clearAllMocks());

  it('deletes a column and invalidates queries', async () => {
    (deleteColumn as ReturnType<typeof vi.fn>).mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => useDeleteColumn(), { wrapper: createWrapper() });
    result.current.mutate(1);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(deleteColumn).toHaveBeenCalledWith(1);
  });
});

describe('useSortCards', () => {
  beforeEach(() => vi.clearAllMocks());

  it('sorts cards in a column and invalidates queries', async () => {
    (sortCards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'Col', position: 0, board_id: 5, cards: [], created_at: '', updated_at: '' },
    });
    const { result } = renderHook(() => useSortCards(), { wrapper: createWrapper() });
    result.current.mutate({ columnId: 1, order: 'asc' });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(sortCards).toHaveBeenCalledWith(1, 'asc');
  });

  it('sorts cards in desc order', async () => {
    (sortCards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { id: 1, name: 'Col', position: 0, board_id: 5, cards: [], created_at: '', updated_at: '' },
    });
    const { result } = renderHook(() => useSortCards(), { wrapper: createWrapper() });
    result.current.mutate({ columnId: 1, order: 'desc' });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(sortCards).toHaveBeenCalledWith(1, 'desc');
  });
});

describe('useMoveAllCards', () => {
  beforeEach(() => vi.clearAllMocks());

  it('moves all cards and invalidates queries', async () => {
    (moveAllCards as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      data: { moved_count: 3 },
    });
    const { result } = renderHook(() => useMoveAllCards(), { wrapper: createWrapper() });
    result.current.mutate({ sourceColumnId: 1, targetColumnId: 2 });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(moveAllCards).toHaveBeenCalledWith(1, 2);
  });
});
