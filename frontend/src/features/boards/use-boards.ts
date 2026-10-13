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
  type BoardsQuery,
  type CreateBoardData,
  type UpdateBoardData,
} from './boards.api';

export type { Board, BoardDetail } from './boards.api';
export type { BoardsQuery, CreateBoardData, UpdateBoardData };

export type BoardViewMode = 'board' | 'list';

function sanitizeProjectId(projectId?: number): number | undefined {
  return projectId !== undefined && Number.isInteger(projectId) && projectId >= 1
    ? projectId
    : undefined;
}

function sanitizePage(page?: number): number | undefined {
  return page !== undefined && Number.isInteger(page) && page >= 1 ? page : undefined;
}

function sanitizeLimit(limit?: number): number | undefined {
  return limit !== undefined && Number.isInteger(limit) && limit >= 1 && limit <= 100
    ? limit
    : undefined;
}

export function useBoards(projectId?: number, page?: number, limit?: number) {
  const project_id = sanitizeProjectId(projectId);
  const cleanPage = sanitizePage(page);
  const cleanLimit = sanitizeLimit(limit);
  return useQuery({
    queryKey: ['boards', project_id, cleanPage, cleanLimit],
    queryFn: () => fetchBoards({ project_id, page: cleanPage, limit: cleanLimit }),
  });
}

export function useBoard(id: number) {
  return useQuery({
    queryKey: ['board', id],
    queryFn: () => fetchBoard(id),
    enabled: Number.isInteger(id) && id >= 1,
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

export function useArchivedBoards(page?: number, limit?: number) {
  const cleanPage = sanitizePage(page);
  const cleanLimit = sanitizeLimit(limit);
  return useQuery({
    queryKey: ['archivedBoards', cleanPage, cleanLimit],
    queryFn: () => fetchArchivedBoards({ page: cleanPage, limit: cleanLimit }),
  });
}
