import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
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

function sanitizeLimit(limit?: number): number | undefined {
  return limit !== undefined && Number.isInteger(limit) && limit >= 1 && limit <= 100
    ? limit
    : undefined;
}

export function useBoards(projectId?: number, limit?: number) {
  const project_id = sanitizeProjectId(projectId);
  const cleanLimit = sanitizeLimit(limit);
  return useInfiniteQuery({
    queryKey: ['boards', project_id, cleanLimit],
    queryFn: ({ pageParam }) =>
      fetchBoards({ project_id, page: pageParam, limit: cleanLimit }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const loaded = lastPage.page * lastPage.limit;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
  });
}

export function flattenPages<T>(pages: { data: T[] }[] | undefined): T[] {
  if (!pages) return [];
  const seen = new Set<unknown>();
  const out: T[] = [];
  for (const page of pages) {
    for (const item of page.data) {
      const id = (item as { id?: unknown }).id;
      if (id !== undefined) {
        if (seen.has(id)) continue;
        seen.add(id);
      }
      out.push(item);
    }
  }
  return out;
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

export function useArchivedBoards(limit?: number) {
  const cleanLimit = sanitizeLimit(limit);
  return useInfiniteQuery({
    queryKey: ['archivedBoards', cleanLimit],
    queryFn: ({ pageParam }) =>
      fetchArchivedBoards({ page: pageParam, limit: cleanLimit }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const loaded = lastPage.page * lastPage.limit;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
  });
}
