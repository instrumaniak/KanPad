import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  fetchLabels,
  createLabel,
  updateLabel,
  deleteLabel,
  type CreateLabelData,
  type UpdateLabelData,
} from './labels.api';

const labelKeys = {
  all: () => ['labels'] as const,
  /** Paginated list key — page is part of the key (T41e `Paginated<T>`). */
  list: (page?: number, limit?: number) =>
    [...labelKeys.all(), page ?? 1, limit ?? 50] as const,
};

export function useLabels(page?: number, limit?: number) {
  return useQuery({
    queryKey: labelKeys.list(page, limit),
    queryFn: () => fetchLabels(page, limit),
  });
}

export function useCreateLabel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: CreateLabelData) => createLabel(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: labelKeys.all() });
    },
  });
}

export function useUpdateLabel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateLabelData }) =>
      updateLabel(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: labelKeys.all() });
      queryClient.invalidateQueries({ queryKey: ['cards'] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

export function useDeleteLabel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteLabel(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: labelKeys.all() });
      queryClient.invalidateQueries({ queryKey: ['cards'] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
    onError: (error: Error) => {
      console.error('Failed to delete label:', error.message);
    },
  });
}
