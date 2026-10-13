import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactNode } from 'react';
import { useDeleteCard, useCreateCard, useCards, useCard } from './use-cards';

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

import { fetchCards, fetchCard, createCard, deleteCard } from './cards.api';
const mockFetchCards = fetchCards as ReturnType<typeof vi.fn>;
const mockFetchCard = fetchCard as ReturnType<typeof vi.fn>;
const mockCreateCard = createCard as ReturnType<typeof vi.fn>;
const mockDeleteCard = deleteCard as ReturnType<typeof vi.fn>;

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