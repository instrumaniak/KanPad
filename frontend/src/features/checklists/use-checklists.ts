import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  createChecklist,
  updateChecklist,
  deleteChecklist,
  createChecklistItem,
  updateChecklistItem,
  deleteChecklistItem,
} from './checklists.api';
import type {
  CreateChecklistData,
  UpdateChecklistData,
  CreateChecklistItemData,
  UpdateChecklistItemData,
  ChecklistItem,
  Checklist,
} from './checklists.api';
import { currentSessionEpoch } from '@/lib/query-client-reset';

export const checklistKeys = {
  all: ['checklists'] as const,
  byCard: (cardId: number) => ['checklists', cardId] as const,
};

export function useCreateChecklist() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: CreateChecklistData) => createChecklist(data),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.card_id] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
    onError: (error: Error) => {
      console.error('Failed to create checklist:', error.message);
    },
  });
}

export function useUpdateChecklist() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateChecklistData; cardId: number }) =>
      updateChecklist(id, data),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.cardId] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
    onError: (error: Error) => {
      console.error('Failed to update checklist:', error.message);
    },
  });
}

export function useDeleteChecklist() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id }: { id: number; cardId: number }) => deleteChecklist(id),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.cardId] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
    onError: (error: Error) => {
      console.error('Failed to delete checklist:', error.message);
    },
  });
}

export function useCreateChecklistItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      checklistId,
      data,
    }: {
      checklistId: number;
      data: CreateChecklistItemData;
      cardId: number;
    }) => createChecklistItem(checklistId, data),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.cardId] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
    onError: (error: Error) => {
      console.error('Failed to create checklist item:', error.message);
    },
  });
}

export function useUpdateChecklistItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      itemId,
      data,
    }: {
      itemId: number;
      data: UpdateChecklistItemData;
      cardId: number;
    }) => updateChecklistItem(itemId, data),
    onMutate: async ({ itemId, data, cardId }) => {
      await queryClient.cancelQueries({ queryKey: ['card', cardId] });

      const previousCardData = queryClient.getQueryData(['card', cardId]);

      queryClient.setQueryData(['card', cardId], (old: unknown) => {
        if (!old) return old;
        const card = (old as { checklists?: Array<Checklist & { items: ChecklistItem[] }> });
        return {
          ...card,
          checklists: card.checklists?.map((cl) => ({
            ...cl,
            items: cl.items.map((item) =>
              item.id === itemId ? { ...item, ...data } : item
            ),
          })),
        };
      });

      return { previousCardData, sessionEpoch: currentSessionEpoch() };
    },
    onError: (_error: Error, _variables, context) => {
      // A session boundary crossed while the mutation was in flight — the
      // rollback would recreate ['card', cardId] with the previous user's
      // snapshot. Skip it.
      if (context?.sessionEpoch !== undefined && context.sessionEpoch !== currentSessionEpoch()) {
        return;
      }
      if (context?.previousCardData) {
        queryClient.setQueryData(['card', _variables.cardId], context.previousCardData);
      }
    },
    onSettled: (_data, _err, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.cardId] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

export function useDeleteChecklistItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ itemId }: { itemId: number; cardId: number }) => deleteChecklistItem(itemId),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.cardId] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
    onError: (error: Error) => {
      console.error('Failed to delete checklist item:', error.message);
    },
  });
}
