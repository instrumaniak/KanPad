import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  createCard,
  updateCard,
  deleteCard,
  fetchCards,
  fetchCard,
  assignLabelToCard,
  removeLabelFromCard,
  sanitizeCardListParams,
  type CreateCardData,
  type UpdateCardData,
  type Card,
  type CardDetail,
  type CardSummary,
} from './cards.api';

const cardKeys = {
  byColumn: (columnId: number, page?: number, limit?: number) => {
    const params = sanitizeCardListParams(page, limit);
    return ['cards', columnId, params.page, params.limit] as const;
  },
  byId: (cardId: number) => ['card', cardId] as const,
  all: () => ['cards'] as const,
};

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
    },
  });
}

export function useUpdateCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateCardData }) =>
      updateCard(id, data),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.id) });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
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

      if (targetColumnId === undefined) return { previousCards };

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

      return { previousCards };
    },
    onError: (_err, _variables, context) => {
      if (context?.previousCards) {
        Object.entries(context.previousCards).forEach(([columnId, cards]) => {
          queryClient.setQueryData(['cards', Number(columnId)], cards);
        });
      }
    },
    onSettled: (_data, _err, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.id) });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
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

      return { previousCards, sourceColumnId };
    },
    onError: (_err, _variables, context) => {
      if (context?.sourceColumnId !== undefined && context?.previousCards) {
        queryClient.setQueryData(['cards', context.sourceColumnId], context.previousCards);
      }
    },
    onSettled: (_data, _err, variables) => {
      queryClient.invalidateQueries({ queryKey: cardKeys.byId(variables.id) });
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

      return { previousCards };
    },
    onError: (_err, _id, context) => {
      if (context?.previousCards) {
        context.previousCards.forEach(({ columnId, cards }) => {
          queryClient.setQueryData(['cards', columnId], cards);
        });
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['columns'] });
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