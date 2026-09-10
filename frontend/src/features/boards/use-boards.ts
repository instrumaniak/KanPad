import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
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
  type CreateBoardData,
  type UpdateBoardData,
} from './boards.api';

export type { Board } from './boards.api';

export type BoardViewMode = 'board' | 'list';

export function useBoards(projectId?: number) {
  return useQuery({
    queryKey: ['boards', projectId],
    queryFn: () => fetchBoards(projectId),
  });
}

export function useBoard(id: number) {
  return useQuery({
    queryKey: ['board', id],
    queryFn: () => fetchBoard(id),
    enabled: !!id,
  });
}

export function useCreateBoard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: CreateBoardData) => createBoard(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['boards'] });
    },
  });
}

export function useUpdateBoard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateBoardData }) =>
      updateBoard(id, data),
    onSuccess: (_, { id }) => {
      queryClient.invalidateQueries({ queryKey: ['boards'] });
      queryClient.invalidateQueries({ queryKey: ['board', id] });
    },
  });
}

export function useDeleteBoard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteBoard(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['boards'] });
    },
  });
}

export function useArchiveBoard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => archiveBoard(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['boards'] });
      queryClient.invalidateQueries({ queryKey: ['archivedBoards'] });
    },
  });
}

export function useRestoreBoard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => restoreBoard(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['boards'] });
      queryClient.invalidateQueries({ queryKey: ['archivedBoards'] });
    },
  });
}

export function usePermanentDeleteBoard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => permanentDeleteBoard(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['archivedBoards'] });
    },
  });
}

export function useArchivedBoards() {
  return useQuery({
    queryKey: ['archivedBoards'],
    queryFn: () => fetchArchivedBoards(),
  });
}