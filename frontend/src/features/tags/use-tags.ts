import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchTags, createTag, updateTag, deleteTag } from './tags.api';
import type { CreateTagData, UpdateTagData } from './tags.api';

export const tagKeys = {
  all: ['tags'] as const,
  /** Paginated list key — page is part of the key (T41e `Paginated<T>`). */
  list: (page?: number, limit?: number) =>
    [...tagKeys.all, page ?? 1, limit ?? 50] as const,
};

export function useTags(page?: number, limit?: number) {
  return useQuery({
    queryKey: tagKeys.list(page, limit),
    queryFn: () => fetchTags(page, limit),
  });
}

export function useCreateTag() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: CreateTagData) => createTag(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: tagKeys.all });
    },
    onError: (error: Error) => {
      console.error('Failed to create tag:', error.message);
    },
  });
}

export function useUpdateTag() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateTagData }) =>
      updateTag(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: tagKeys.all });
    },
    onError: (error: Error) => {
      console.error('Failed to update tag:', error.message);
    },
  });
}

export function useDeleteTag() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id }: { id: number }) => deleteTag(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: tagKeys.all });
    },
    onError: (error: Error) => {
      console.error('Failed to delete tag:', error.message);
    },
  });
}
