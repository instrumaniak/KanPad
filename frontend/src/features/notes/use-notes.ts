import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createNote,
  fetchNotes,
  fetchNote,
  updateNote,
  deleteNote,
  fetchBoardNotes,
} from './notes.api';
import type {
  CreateNoteData,
  UpdateNoteData,
  NoteFilters,
} from './notes.api';

export const noteKeys = {
  all: () => ['notes'] as const,
  list: (filters?: NoteFilters) => ['notes', 'list', filters] as const,
  detail: (id: number) => ['notes', id] as const,
  byBoard: (boardId: number) => ['notes', 'board', boardId] as const,
};

export function useNotes(filters?: NoteFilters) {
  return useQuery({
    queryKey: noteKeys.list(filters),
    queryFn: () => fetchNotes(filters),
  });
}

export function useNote(id: number) {
  return useQuery({
    queryKey: noteKeys.detail(id),
    queryFn: () => fetchNote(id),
    enabled: !!id,
  });
}

export function useBoardNotes(boardId: number) {
  return useQuery({
    queryKey: noteKeys.byBoard(boardId),
    queryFn: () => fetchBoardNotes(boardId),
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
