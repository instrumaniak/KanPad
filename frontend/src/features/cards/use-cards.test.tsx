import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactNode } from 'react';
import { useDeleteCard, useCreateCard, useCards, useCard, useMoveCard, useReorderCard } from './use-cards';
import { resetSessionCaches } from '@/lib/query-client-reset';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

vi.mock('./cards.api', () => ({
  fetchCards: vi.fn(),
  fetchCard: vi.fn(),
  createCard: vi.fn(),
  updateCard: vi.fn(),
  deleteCard: vi.fn(),
  sanitizeCardListParams: vi.fn((page?: number, limit?: number) => {
    const p = typeof page === 'number' && Number.isInteger(page) && page >= 1 ? page : 1;
    const l = typeof limit === 'number' && Number.isInteger(limit) && limit >= 1 && limit <= 100 ? limit : 50;
    return { page: p, limit: l };
  }),
}));

import { fetchCards, fetchCard, createCard, updateCard, deleteCard } from './cards.api';
const mockFetchCards = fetchCards as ReturnType<typeof vi.fn>;
const mockFetchCard = fetchCard as ReturnType<typeof vi.fn>;
const mockCreateCard = createCard as ReturnType<typeof vi.fn>;
const mockUpdateCard = updateCard as ReturnType<typeof vi.fn>;
const mockDeleteCard = deleteCard as ReturnType<typeof vi.fn>;

describe('useDeleteCard post-boundary guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('does NOT recreate the cards cache after a session boundary mid-flight', async () => {
    const mockCards = [
      { id: 1, title: 'Card 1', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' },
    ];

    queryClient.setQueryData(['cards', 1], mockCards);

    let rejectDelete!: (err: unknown) => void;
    mockDeleteCard.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectDelete = reject;
      }),
    );

    const { result } = renderHook(() => useDeleteCard(), { wrapper });

    result.current.mutate(1);

    // Optimistic removal applied (source column kept for rollback).
    await waitFor(() => {
      expect(queryClient.getQueryData(['cards', 1])).toEqual([]);
    });

    // Session boundary lands while the mutation is in flight (e.g. a global
    // 401 from another query clears the cache).
    await resetSessionCaches(queryClient);

    rejectDelete(new Error('Delete failed'));

    await waitFor(() => {
      expect(result.current.isError).toBe(true);
    });

    // The rollback MUST NOT recreate ['cards', 1] with the previous user's
    // snapshot — a session boundary invalidates the optimistic context.
    expect(queryClient.getQueryData(['cards', 1])).toBeUndefined();
  });
});

describe('useMoveCard post-boundary guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('does NOT recreate the cards cache after a session boundary mid-flight', async () => {
    const sourceCards = [
      { id: 1, title: 'Card 1', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' },
    ];
    const targetCards: typeof sourceCards = [];
    queryClient.setQueryData(['cards', 1], sourceCards);
    queryClient.setQueryData(['cards', 2], targetCards);

    let rejectUpdate!: (err: unknown) => void;
    mockUpdateCard.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectUpdate = reject;
      }),
    );

    const { result } = renderHook(() => useMoveCard(), { wrapper });

    result.current.mutate({ id: 1, data: { column_id: 2 } });

    // Optimistic move applied: source empty, card appended to target.
    await waitFor(() => {
      expect(queryClient.getQueryData(['cards', 1])).toEqual([]);
      expect(queryClient.getQueryData(['cards', 2])).toHaveLength(1);
    });

    // Session boundary lands while the mutation is in flight.
    await resetSessionCaches(queryClient);

    rejectUpdate(new Error('Move failed'));

    await waitFor(() => expect(result.current.isError).toBe(true));

    // The rollback MUST NOT recreate the cards caches with the previous
    // user's snapshot — neither for the source nor the target column.
    expect(queryClient.getQueryData(['cards', 1])).toBeUndefined();
    expect(queryClient.getQueryData(['cards', 2])).toBeUndefined();
  });
});

describe('useReorderCard post-boundary guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('does NOT recreate the cards cache after a session boundary mid-flight', async () => {
    const cards = [
      { id: 1, title: 'Card 1', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' },
      { id: 2, title: 'Card 2', column_id: 1, position: 1, created_at: '2024-01-01', updated_at: '2024-01-01' },
    ];
    queryClient.setQueryData(['cards', 1], cards);

    let rejectUpdate!: (err: unknown) => void;
    mockUpdateCard.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectUpdate = reject;
      }),
    );

    const { result } = renderHook(() => useReorderCard(), { wrapper });

    result.current.mutate({ id: 1, position: 1 });

    // Optimistic reorder applied (Card 1 moved after Card 2).
    await waitFor(() => {
      const cached = queryClient.getQueryData<{ id: number; position: number }[]>(['cards', 1]);
      expect(cached?.map((c) => c.id)).toEqual([2, 1]);
    });

    // Session boundary lands while the mutation is in flight.
    await resetSessionCaches(queryClient);

    rejectUpdate(new Error('Reorder failed'));

    await waitFor(() => expect(result.current.isError).toBe(true));

    // The rollback MUST NOT recreate ['cards', 1] with the previous user's
    // snapshot.
    expect(queryClient.getQueryData(['cards', 1])).toBeUndefined();
  });
});

describe('useDeleteCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('optimistically removes card from cache on mutate', async () => {
    const mockCards = [
      { id: 1, title: 'Card 1', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' },
      { id: 2, title: 'Card 2', column_id: 1, position: 1, created_at: '2024-01-01', updated_at: '2024-01-01' },
    ];

    queryClient.setQueryData(['cards', 1], mockCards);
    mockDeleteCard.mockResolvedValue(undefined);

    const { result } = renderHook(() => useDeleteCard(), { wrapper });

    result.current.mutate(1);

    await waitFor(() => {
      const cachedCards = queryClient.getQueryData<typeof mockCards>(['cards', 1]);
      expect(cachedCards).toHaveLength(1);
      expect(cachedCards?.[0].id).toBe(2);
    });
  });

  it('rolls back cache on error', async () => {
    const mockCards = [
      { id: 1, title: 'Card 1', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' },
    ];

    queryClient.setQueryData(['cards', 1], mockCards);
    mockDeleteCard.mockRejectedValue(new Error('Delete failed'));

    const { result } = renderHook(() => useDeleteCard(), { wrapper });

    result.current.mutate(1);

    await waitFor(() => {
      expect(result.current.isError).toBe(true);
    });

    const cachedCards = queryClient.getQueryData<typeof mockCards>(['cards', 1]);
    expect(cachedCards).toEqual(mockCards);
  });

  it('invalidates queries on settled', async () => {
    mockDeleteCard.mockResolvedValue(undefined);

    const { result } = renderHook(() => useDeleteCard(), { wrapper });

    result.current.mutate(1);

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockDeleteCard).toHaveBeenCalledWith(1);
  });
});

describe('useCreateCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('creates card with position and invalidates queries', async () => {
    mockCreateCard.mockResolvedValue({
      data: { id: 3, title: 'New Card', column_id: 1, position: 2, created_at: '2024-01-01', updated_at: '2024-01-01' },
    });

    const { result } = renderHook(() => useCreateCard(), { wrapper });

    await result.current.mutateAsync({
      title: 'New Card',
      column_id: 1,
      position: 2,
    });

    expect(mockCreateCard).toHaveBeenCalledWith({
      title: 'New Card',
      column_id: 1,
      position: 2,
    });
  });
});

describe('useCards', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('fetches cards for a column', async () => {
    const mockCards = [
      { id: 1, title: 'Card 1', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' },
    ];

    mockFetchCards.mockResolvedValue({ data: mockCards, page: 1, limit: 50, total: 1 });

    const { result } = renderHook(() => useCards(1), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual(mockCards);
    expect(mockFetchCards).toHaveBeenCalledWith(1, 1, 50);
  });

  it('pages via queryKey', async () => {
    mockFetchCards.mockResolvedValue({ data: [], page: 2, limit: 10, total: 0 });

    const { result } = renderHook(() => useCards(1, 2, 10), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(mockFetchCards).toHaveBeenCalledWith(1, 2, 10);
  });
});

describe('useCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('fetches a single card by id', async () => {
    const mockCard = { id: 1, title: 'Card 1', column_id: 1, position: 0, description: 'Detail', created_at: '2024-01-01', updated_at: '2024-01-01' };

    mockFetchCard.mockResolvedValue({ data: mockCard });

    const { result } = renderHook(() => useCard(1), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual(mockCard);
    expect(mockFetchCard).toHaveBeenCalledWith(1);
  });

  it('does not fetch when id is 0', async () => {
    const { result } = renderHook(() => useCard(0), { wrapper });

    expect(result.current.isLoading).toBe(false);
    expect(mockFetchCard).not.toHaveBeenCalled();
  });
});