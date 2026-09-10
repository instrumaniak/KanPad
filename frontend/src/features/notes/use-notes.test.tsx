import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactNode } from 'react';
import { useNotes, useNote, useBoardNotes, useCreateNote, useUpdateNote, useDeleteNote } from './use-notes';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

vi.mock('./notes.api', () => ({
  fetchNotes: vi.fn(),
  fetchNote: vi.fn(),
  fetchBoardNotes: vi.fn(),
  createNote: vi.fn(),
  updateNote: vi.fn(),
  deleteNote: vi.fn(),
}));

import { fetchNotes, fetchNote, fetchBoardNotes, createNote, updateNote, deleteNote } from './notes.api';
const mockFetchNotes = fetchNotes as ReturnType<typeof vi.fn>;
const mockFetchNote = fetchNote as ReturnType<typeof vi.fn>;
const mockFetchBoardNotes = fetchBoardNotes as ReturnType<typeof vi.fn>;
const mockCreateNote = createNote as ReturnType<typeof vi.fn>;
const mockUpdateNote = updateNote as ReturnType<typeof vi.fn>;
const mockDeleteNote = deleteNote as ReturnType<typeof vi.fn>;

describe('useNotes', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('fetches notes list', async () => {
    mockFetchNotes.mockResolvedValue({ data: [{ id: 1, title: 'Note 1' }] });

    const { result } = renderHook(() => useNotes(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ data: [{ id: 1, title: 'Note 1' }] });
    expect(mockFetchNotes).toHaveBeenCalledWith(undefined);
  });

  it('fetches notes with filters', async () => {
    mockFetchNotes.mockResolvedValue({ data: [] });

    const { result } = renderHook(() => useNotes({ search: 'hello', type: 'general' }), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(mockFetchNotes).toHaveBeenCalledWith({ search: 'hello', type: 'general' });
  });
});

describe('useNote', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('fetches a single note by id', async () => {
    mockFetchNote.mockResolvedValue({ data: { id: 1, title: 'Note 1' } });

    const { result } = renderHook(() => useNote(1), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ data: { id: 1, title: 'Note 1' } });
    expect(mockFetchNote).toHaveBeenCalledWith(1);
  });

  it('does not fetch when id is falsy', async () => {
    const { result } = renderHook(() => useNote(0), { wrapper });

    expect(result.current.isLoading).toBe(false);
    expect(mockFetchNote).not.toHaveBeenCalled();
  });
});

describe('useBoardNotes', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('fetches notes for a board', async () => {
    mockFetchBoardNotes.mockResolvedValue({ data: [{ id: 1, title: 'Board Note' }] });

    const { result } = renderHook(() => useBoardNotes(1), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ data: [{ id: 1, title: 'Board Note' }] });
    expect(mockFetchBoardNotes).toHaveBeenCalledWith(1);
  });

  it('does not fetch when boardId is falsy', async () => {
    const { result } = renderHook(() => useBoardNotes(0), { wrapper });

    expect(result.current.isLoading).toBe(false);
    expect(mockFetchBoardNotes).not.toHaveBeenCalled();
  });
});

describe('useCreateNote', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('creates a note and invalidates queries', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    mockCreateNote.mockResolvedValue({ data: { id: 1, title: 'New' } });

    const { result } = renderHook(() => useCreateNote(), { wrapper });

    await result.current.mutateAsync({ title: 'New', content: '' });

    expect(mockCreateNote).toHaveBeenCalledWith({ title: 'New', content: '' });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['notes'] });
  });
});

describe('useUpdateNote', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('updates a note and invalidates queries', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    mockUpdateNote.mockResolvedValue({ data: { id: 1, title: 'Updated' } });

    const { result } = renderHook(() => useUpdateNote(), { wrapper });

    await result.current.mutateAsync({ id: 1, data: { title: 'Updated' } });

    expect(mockUpdateNote).toHaveBeenCalledWith(1, { title: 'Updated' });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['notes'] });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['notes', 1] });
  });
});

describe('useDeleteNote', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
  });

  it('deletes a note and invalidates queries', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    mockDeleteNote.mockResolvedValue(undefined);

    const { result } = renderHook(() => useDeleteNote(), { wrapper });

    await result.current.mutateAsync({ id: 1 });

    expect(mockDeleteNote).toHaveBeenCalledWith(1);
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['notes'] });
  });
});
