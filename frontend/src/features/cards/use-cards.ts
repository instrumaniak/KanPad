import { useInfiniteQuery, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  createCard,
  updateCard,
  deleteCard,
  fetchCards,
  fetchBoardCards,
  fetchCard,
  assignLabelToCard,
  removeLabelFromCard,
  sanitizeCardListParams,
  type BoardCardListItem,
  type BoardCardListParams,
  type CardListFilters,
  type CreateCardData,
  type UpdateCardData,
  type Card,
  type CardDetail,
  type CardSummary,
} from './cards.api';
import type { Column } from '../columns/columns.api';
import { currentSessionEpoch } from '@/lib/query-client-reset';

const cardKeys = {
  byColumn: (columnId: number, page?: number, limit?: number) => {
    const params = sanitizeCardListParams(page, limit);
    return ['cards', columnId, params.page, params.limit] as const;
  },
  byId: (cardId: number) => ['card', cardId] as const,
  all: () => ['cards'] as const,
};

export const BOARD_CARDS_PAGE_SIZE = 20;

function normalizeBoardCardFilters(filters?: CardListFilters) {
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

export function boardCardsQueryKey(
  boardId: number,
  params?: Omit<BoardCardListParams, 'page' | 'limit'>,
) {
  const normalized = normalizeBoardCardFilters(params);
  const sort = params?.sort ?? 'created_at';
  const order = params?.order ?? 'asc';
  return ['board-cards', boardId, { ...normalized, sort, order }] as const;
}

export function useBoardCards(boardId: number, params?: Omit<BoardCardListParams, 'page' | 'limit'>) {
  const normalized = normalizeBoardCardFilters(params);
  const sort = params?.sort ?? 'created_at';
  const order = params?.order ?? 'asc';
  return useInfiniteQuery({
    queryKey: boardCardsQueryKey(boardId, { ...normalized, sort, order }),
    queryFn: ({ pageParam }) =>
      fetchBoardCards(boardId, {
        ...normalized,
        sort,
        order,
        page: pageParam,
        limit: BOARD_CARDS_PAGE_SIZE,
      }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const loaded = lastPage.page * lastPage.limit;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
    enabled: !!boardId,
  });
}

export function flattenBoardCardPages(pages: Array<{ data: BoardCardListItem[] }> | undefined): BoardCardListItem[] {
  if (!pages) return [];
  const seen = new Set<number>();
  const out: BoardCardListItem[] = [];
  for (const page of pages) {
    for (const item of page.data) {
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      out.push(item);
    }
  }
  return out;
}

export function useCards(columnId: number, page?: number, limit?: number) {
  const params = sanitizeCardListParams(page, limit);
  return useQuery({
    queryKey: cardKeys.byColumn(columnId, params.page, params.limit),
    queryFn: () => fetchCards(columnId, params.page, params.limit).then((res) => res.data),
    enabled: !!columnId,
  });
}

export function useCard(id: number) {
  return useQuery({
    queryKey: cardKeys.byId(id),
    queryFn: () => fetchCard(id).then((res) => res.data),
    enabled: !!id,
  });
}

export function useCreateCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: CreateCardData) => createCard(data),
    onSuccess: (_, { column_id }) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byColumn(column_id) });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export function useUpdateCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateCardData }) =>
      updateCard(id, data),
    onSuccess: (response, variables) => {
      // Seed detail cache immediately so open panels (e.g. due-date picker)
      // reflect the saved value without waiting for an invalidation refetch.
      if (response?.data) {
        queryClient.setQueryData(cardKeys.byId(variables.id), response.data);
      } else {
        queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.id) });
      }
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export function useMoveCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateCardData }) => updateCard(id, data),
    onMutate: async ({ id, data }) => {
      await queryClient.cancelQueries({ queryKey: ['cards'] });

      const previousCards: Record<number, Card[] | undefined> = {};
      const targetColumnId = data.column_id;

      // The generated `UpdateCardInput.column_id` is `number | null | undefined`
      // (an explicit `null` clears it server-side). A move needs a real
      // target column, so both "absent" and "null" mean "no move" here.
      if (targetColumnId === undefined || targetColumnId === null) {
        return { previousCards, sessionEpoch: currentSessionEpoch() };
      }

      // Find source column and card
      const allCardsQueries = queryClient.getQueriesData<Card[]>({ queryKey: ['cards'] });
      let sourceColumnId: number | undefined;
      let cardToMove: Card | undefined;

      for (const [queryKey, cards] of allCardsQueries) {
        const columnId = queryKey[1] as number;
        const card = cards?.find((c) => c.id === id);
        if (card) {
          sourceColumnId = columnId;
          cardToMove = card;
          previousCards[columnId] = cards;
          break;
        }
      }

      if (sourceColumnId !== undefined && cardToMove && sourceColumnId !== targetColumnId) {
        // Remove from source
        queryClient.setQueryData<Card[]>(['cards', sourceColumnId], (old) =>
          old?.filter((c) => c.id !== id),
        );

        // Add to target
        const targetCards = queryClient.getQueryData<Card[]>(['cards', targetColumnId]);
        previousCards[targetColumnId] = targetCards;
        queryClient.setQueryData<Card[]>(['cards', targetColumnId], (old) => [
          ...(old || []),
          { ...cardToMove!, column_id: targetColumnId },
        ]);
      }

      return { previousCards, sessionEpoch: currentSessionEpoch() };
    },
    onError: (_err, _variables, context) => {
      // Session boundary crossed while the mutation was in flight?
      // A concrete-value rollback would recreate the query with the
      // PREVIOUS user's snapshot — skip it.
      if (context?.sessionEpoch !== undefined && context.sessionEpoch !== currentSessionEpoch()) {
        return;
      }
      if (context?.previousCards) {
        Object.entries(context.previousCards).forEach(([columnId, cards]) => {
          queryClient.setQueryData(['cards', Number(columnId)], cards);
        });
      }
    },
    onSettled: (_data, err, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.id) });
      if (err) {
        queryClient.invalidateQueries({ queryKey: ['columns'] });
        queryClient.invalidateQueries({ queryKey: ['board-cards'] });
      }
    },
  });
}

export function useReorderCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, position }: { id: number; position: number }) =>
      updateCard(id, { position }),
    onMutate: async ({ id, position }) => {
      await queryClient.cancelQueries({ queryKey: ['cards'] });

      const allCardsQueries = queryClient.getQueriesData<Card[]>({ queryKey: ['cards'] });
      let sourceColumnId: number | undefined;
      let previousCards: Card[] | undefined;

      for (const [queryKey, cards] of allCardsQueries) {
        const columnId = queryKey[1] as number;
        const card = cards?.find((c) => c.id === id);
        if (card) {
          sourceColumnId = columnId;
          previousCards = cards;
          break;
        }
      }

      if (sourceColumnId !== undefined && previousCards) {
        queryClient.setQueryData<Card[]>(['cards', sourceColumnId], (old) => {
          if (!old) return [];
          const newCards = [...old];
          const index = newCards.findIndex((c) => c.id === id);
          if (index !== -1) {
            const [removed] = newCards.splice(index, 1);
            newCards.splice(position, 0, { ...removed, position });
            // Update positions of all cards in the list
            return newCards.map((c, i) => ({ ...c, position: i }));
          }
          return old;
        });
      }

      return { previousCards, sourceColumnId, sessionEpoch: currentSessionEpoch() };
    },
    onError: (_err, _variables, context) => {
      if (context?.sessionEpoch !== undefined && context.sessionEpoch !== currentSessionEpoch()) {
        return;
      }
      if (context?.sourceColumnId !== undefined && context?.previousCards) {
        queryClient.setQueryData(['cards', context.sourceColumnId], context.previousCards);
      }
    },
    onSettled: (_data, _err, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.id) });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export function useDeleteCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteCard(id),
    onMutate: async (id) => {
      await queryClient.cancelQueries({ queryKey: ['cards'] });
      const allCardsQueries = queryClient.getQueriesData<Card[]>({ queryKey: ['cards'] });
      const previousCards: { columnId: number; cards: Card[] | undefined }[] = [];

      for (const [queryKey, cards] of allCardsQueries) {
        const columnId = queryKey[1] as number;
        const card = cards?.find((c) => c.id === id);
        if (card) {
          previousCards.push({ columnId, cards });
          queryClient.setQueryData<Card[]>(['cards', columnId], (old) =>
            old?.filter((c) => c.id !== id),
          );
        }
      }

      return { previousCards, sessionEpoch: currentSessionEpoch() };
    },
    onError: (_err, _id, context) => {
      if (context?.sessionEpoch !== undefined && context.sessionEpoch !== currentSessionEpoch()) {
        return;
      }
      if (context?.previousCards) {
        context.previousCards.forEach(({ columnId, cards }) => {
          queryClient.setQueryData(['cards', columnId], cards);
        });
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export function useAssignCardLabel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ cardId, labelId }: { cardId: number; labelId: number }) =>
      assignLabelToCard(cardId, labelId),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.cardId) });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export function useRemoveCardLabel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ cardId, labelId }: { cardId: number; labelId: number }) =>
      removeLabelFromCard(cardId, labelId),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.cardId) });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
      queryClient.invalidateQueries({ queryKey: ['board-cards'] });
    },
  });
}

export interface AnchorMoveCardData {
  column_id?: number;
  before_card_id?: number;
  after_card_id?: number;
}

interface AnchorMoveVariables {
  id: number;
  data: AnchorMoveCardData;
  activeKey: readonly unknown[];
  optimistic?: (columns: Column[] | undefined) => Column[] | undefined;
}

interface AnchorMoveContext {
  previous: Column[] | undefined;
  activeKey: readonly unknown[];
  sessionEpoch: number;
}

/**
 * Board-scoped anchor DnD mutation (T053). No ['cards'] optimism, no global
 * ['columns'] invalidation. Optimistic update only on captured active filtered
 * query key. Settled/error/cancel invalidates ['columns', boardId] prefix and
 * ['board-cards', boardId].
 */
export function useAnchorMoveCard(boardId: number) {
  const queryClient = useQueryClient();
  return useMutation<unknown, unknown, AnchorMoveVariables, AnchorMoveContext>({
    mutationFn: ({ id, data }) => updateCard(id, data as UpdateCardData),
    onMutate: async ({ activeKey, optimistic }) => {
      await queryClient.cancelQueries({ queryKey: activeKey as unknown[] });
      const previous = queryClient.getQueryData<Column[]>(activeKey as unknown[]);
      if (optimistic) {
        queryClient.setQueryData<Column[] | undefined>(activeKey as unknown[], optimistic);
      }
      return { previous, activeKey, sessionEpoch: currentSessionEpoch() };
    },
    onError: (_err, _vars, context) => {
      if (context?.sessionEpoch !== undefined && context.sessionEpoch !== currentSessionEpoch()) {
        return;
      }
      if (context) {
        queryClient.setQueryData(context.activeKey as unknown[], context.previous);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      queryClient.invalidateQueries({ queryKey: ['board-cards', boardId] });
    },
  });
}

export type { Card, CardDetail, CardSummary };

export interface DragData {
  cardId: number;
  sourceColumnId: number;
  card: Card;
  index?: number;
}