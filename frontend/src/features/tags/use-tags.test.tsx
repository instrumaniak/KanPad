import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactNode } from 'react';
import { useTags, useCreateTag, useUpdateTag, useDeleteTag } from './use-tags';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

vi.mock('./tags.api', () => ({
  fetchTags: vi.fn(),
  createTag: vi.fn(),
  updateTag: vi.fn(),
  deleteTag: vi.fn(),
}));

import { fetchTags, createTag, updateTag, deleteTag } from './tags.api';
const mockFetchTags = fetchTags as ReturnType<typeof vi.fn>;
const mockCreateTag = createTag as ReturnType<typeof vi.fn>;
const mockUpdateTag = updateTag as ReturnType<typeof vi.fn>;
const mockDeleteTag = deleteTag as ReturnType<typeof vi.fn>;

describe('useTags', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('fetches tags list', async () => {
    mockFetchTags.mockResolvedValue({ data: [{ id: 1, name: 'frontend', color: 'teal' }] });

    const { result } = renderHook(() => useTags(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ data: [{ id: 1, name: 'frontend', color: 'teal' }] });
    expect(mockFetchTags).toHaveBeenCalled();
  });
});

describe('useCreateTag', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('creates a tag and invalidates queries', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    mockCreateTag.mockResolvedValue({ data: { id: 1, name: 'frontend', color: 'teal' } });

    const { result } = renderHook(() => useCreateTag(), { wrapper });

    await result.current.mutateAsync({ name: 'frontend', color: 'teal' });

    expect(mockCreateTag).toHaveBeenCalledWith({ name: 'frontend', color: 'teal' });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['tags'] });
  });
});

describe('useUpdateTag', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('updates a tag and invalidates queries', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    mockUpdateTag.mockResolvedValue({ data: { id: 1, name: 'updated', color: 'blue' } });

    const { result } = renderHook(() => useUpdateTag(), { wrapper });

    await result.current.mutateAsync({ id: 1, data: { name: 'updated' } });

    expect(mockUpdateTag).toHaveBeenCalledWith(1, { name: 'updated' });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['tags'] });
  });
});

describe('useDeleteTag', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('deletes a tag and invalidates queries', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    mockDeleteTag.mockResolvedValue(undefined);

    const { result } = renderHook(() => useDeleteTag(), { wrapper });

    await result.current.mutateAsync({ id: 1 });

    expect(mockDeleteTag).toHaveBeenCalledWith(1);
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['tags'] });
  });
});
