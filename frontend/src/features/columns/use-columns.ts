import { useCallback, useRef, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  fetchColumns,
  createColumn,
  updateColumn,
  deleteColumn,
  sortCards,
  moveAllCards,
  type CreateColumnData,
  type UpdateColumnData,
  type Column,
} from './columns.api';
import { fetchCards } from '../cards/cards.api';

export function useColumns(boardId: number) {
  return useQuery({
    queryKey: ['columns', boardId],
    queryFn: () => fetchColumns(boardId).then((res) => res.data),
    enabled: !!boardId,
  });
}

export function useCreateColumn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ boardId, data }: { boardId: number; data: CreateColumnData }) =>
      createColumn(boardId, data),
    onSuccess: (_, { boardId }) => {
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      queryClient.invalidateQueries({ queryKey: ['board', boardId] });
    },
  });
}

export function useUpdateColumn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateColumnData }) =>
      updateColumn(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

export function useDeleteColumn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteColumn(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

export function useSortCards() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ columnId, order }: { columnId: number; order: 'asc' | 'desc' }) =>
      sortCards(columnId, order),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board', data.data.board_id] });
    },
  });
}

export function useMoveAllCards() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ sourceColumnId, targetColumnId }: { sourceColumnId: number; targetColumnId: number }) =>
      moveAllCards(sourceColumnId, targetColumnId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

export type { Column };

export const COLUMN_CARD_PAGE_SIZE = 50;

export function useLoadMoreColumnCards(boardId: number, columnId: number) {
  const queryClient = useQueryClient();
  const [isFetching, setIsFetching] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const inFlightRef = useRef(false);

  const loadMore = useCallback(async () => {
    if (inFlightRef.current) return;
    const columns = queryClient.getQueryData<Column[]>(['columns', boardId]);
    const column = columns?.find((c) => c.id === columnId);
    if (!column) return;
    const total = (column as Column & { total_cards?: number }).total_cards ?? column.cards.length;
    if (column.cards.length >= total) return;

    const nextPage = Math.floor(column.cards.length / COLUMN_CARD_PAGE_SIZE) + 1;
    inFlightRef.current = true;
    setIsFetching(true);
    setError(null);
    try {
      const res = await fetchCards(columnId, nextPage, COLUMN_CARD_PAGE_SIZE);
      queryClient.setQueryData<Column[]>(['columns', boardId], (old) => {
        if (!old) return old;
        return old.map((col) => {
          if (col.id !== columnId) return col;
          const seen = new Set(col.cards.map((c) => c.id));
          const merged = [...col.cards];
          for (const card of res.data) {
            if (seen.has(card.id)) continue;
            seen.add(card.id);
            merged.push(card as Column['cards'][number]);
          }
          return { ...col, cards: merged };
        });
      });
    } catch (err) {
      setError(err);
    } finally {
      inFlightRef.current = false;
      setIsFetching(false);
    }
  }, [boardId, columnId, queryClient]);

  return { loadMore, isFetching, error };
}
