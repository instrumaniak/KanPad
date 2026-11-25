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
  type ColumnListFilters,
} from './columns.api';
import { fetchCards, type CardListFilters } from '../cards/cards.api';

export const BOARD_COLUMNS_EMBED_LIMIT = 10;
export const COLUMN_CARD_PAGE_SIZE = 20;

export type BoardColumnFilters = ColumnListFilters;

function normalizeFilters(filters?: BoardColumnFilters): BoardColumnFilters | undefined {
  if (!filters) return undefined;
  const search = filters.search?.trim();
  const labels = filters.labels?.length ? [...new Set(filters.labels)].sort((a, b) => a - b) : undefined;
  const due_date = filters.due_date;
  const checklist = filters.checklist;
  if (!search && !labels?.length && !due_date && !checklist) return undefined;
  return {
    ...(search ? { search } : {}),
    ...(labels?.length ? { labels } : {}),
    ...(due_date ? { due_date } : {}),
    ...(checklist ? { checklist } : {}),
  };
}

export function hasBoardColumnFilters(filters?: BoardColumnFilters): boolean {
  return !!normalizeFilters(filters);
}

export function columnQueryKey(boardId: number, filters?: BoardColumnFilters) {
  const normalized = normalizeFilters(filters);
  return normalized ? (['columns', boardId, normalized] as const) : (['columns', boardId] as const);
}

export function useColumns(boardId: number, filters?: BoardColumnFilters) {
  const normalized = normalizeFilters(filters);
  return useQuery({
    queryKey: columnQueryKey(boardId, normalized),
    queryFn: () =>
      fetchColumns(boardId, { embedLimit: BOARD_COLUMNS_EMBED_LIMIT, filters: normalized }).then(
        (res) => res.data,
      ),
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
      queryClient.invalidateQueries({ queryKey: ['board-cards', boardId] });
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
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export function useDeleteColumn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteColumn(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
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
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
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
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export type { Column };

export function useLoadMoreColumnCards(boardId: number, columnId: number, filters?: BoardColumnFilters) {
  const queryClient = useQueryClient();
  const [isFetching, setIsFetching] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const inFlightRef = useRef(false);
  const normalized = normalizeFilters(filters);
  const filterKey = JSON.stringify(normalized ?? null);

  const loadMore = useCallback(async () => {
    if (inFlightRef.current) return;
    const activeFilters = normalizeFilters(filters);
    const queryKey = columnQueryKey(boardId, activeFilters);
    const columns = queryClient.getQueryData<Column[]>(queryKey);
    const column = columns?.find((c) => c.id === columnId);
    if (!column) return;
    const total = activeFilters
      ? ((column as Column & { matching_cards?: number }).matching_cards ?? column.cards.length)
      : ((column as Column & { total_cards?: number }).total_cards ?? column.cards.length);
    if (column.cards.length >= total) return;

    const cardFilters: CardListFilters | undefined = activeFilters
      ? {
          search: activeFilters.search,
          labels: activeFilters.labels,
          due_date: activeFilters.due_date,
          checklist: activeFilters.checklist,
        }
      : undefined;

    const nextPage = Math.floor(column.cards.length / COLUMN_CARD_PAGE_SIZE) + 1;
    inFlightRef.current = true;
    setIsFetching(true);
    setError(null);
    try {
      const res = await fetchCards(columnId, nextPage, COLUMN_CARD_PAGE_SIZE, cardFilters);
      queryClient.setQueryData<Column[]>(queryKey, (old) => {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [boardId, columnId, queryClient, filterKey]);

  return { loadMore, isFetching, error };
}
