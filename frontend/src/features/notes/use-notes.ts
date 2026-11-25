import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createNote,
  fetchNotes,
  fetchNote,
  updateNote,
  deleteNote,
  fetchBoardNotes,
  BOARD_NOTES_PAGE_SIZE,
} from './notes.api';
import type {
  CreateNoteData,
  UpdateNoteData,
  NoteFilters,
  NoteSummary,
} from './notes.api';
import type { Page } from '@/lib/api/client';

export const noteKeys = {
  all: () => ['notes'] as const,
  list: (filters?: Omit<NoteFilters, 'page'>) => ['notes', 'list', filters] as const,
  detail: (id: number) => ['notes', id] as const,
  byBoard: (boardId: number) => ['notes', 'board', boardId] as const,
};

export function useNotes(filters?: NoteFilters) {
  const { page: _page, ...rest } = filters ?? {};
  void _page;
  return useInfiniteQuery({
    queryKey: noteKeys.list(rest),
    queryFn: ({ pageParam }) => fetchNotes({ ...rest, page: pageParam }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const loaded = lastPage.page * lastPage.limit;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
  });
}

export function flattenNotePages(pages: Page<NoteSummary>[] | undefined): NoteSummary[] {
  if (!pages) return [];
  const seen = new Set<number>();
  const out: NoteSummary[] = [];
  for (const page of pages) {
    for (const item of page.data) {
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      out.push(item);
    }
  }
  return out;
}

export function useNote(id: number) {
  return useQuery({
    queryKey: noteKeys.detail(id),
    queryFn: () => fetchNote(id),
    enabled: !!id,
  });
}

export function useBoardNotes(boardId: number) {
  return useInfiniteQuery({
    queryKey: noteKeys.byBoard(boardId),
    queryFn: ({ pageParam }) => fetchBoardNotes(boardId, pageParam, BOARD_NOTES_PAGE_SIZE),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const loaded = lastPage.page * lastPage.limit;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
    enabled: !!boardId,
  });
}

export function useCreateNote() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: CreateNoteData) => createNote(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: noteKeys.all() });
      queryClient.invalidateQueries({ queryKey: ['notes', 'board'] });
    },
    onError: (error: Error) => {
      console.error('Failed to create note:', error.message);
    },
  });
}

export function useUpdateNote() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateNoteData }) =>
      updateNote(id, data),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: noteKeys.all() });
      queryClient.invalidateQueries({ queryKey: noteKeys.detail(variables.id) });
    },
    onError: (error: Error) => {
      console.error('Failed to update note:', error.message);
    },
  });
}

export function useDeleteNote() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id }: { id: number }) => deleteNote(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: noteKeys.all() });
    },
    onError: (error: Error) => {
      console.error('Failed to delete note:', error.message);
    },
  });
}
