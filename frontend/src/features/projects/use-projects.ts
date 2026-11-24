import { useInfiniteQuery, useMutation, useQueryClient, type InfiniteData } from '@tanstack/react-query';
import {
  fetchProjects,
  createProject as createProjectApi,
  updateProject as updateProjectApi,
  deleteProject as deleteProjectApi,
  resolveProjectsQuery,
  type ProjectsPage,
  type ProjectsQuery,
} from './projects.api';
import { currentSessionEpoch } from '@/lib/query-client-reset';

export function useProjects(query: ProjectsQuery = {}) {
  const { page: _page, ...rest } = query;
  const resolved = resolveProjectsQuery(rest);
  const { page: _resolvedPage, ...base } = resolved;
  return useInfiniteQuery({
    queryKey: ['projects', base],
    queryFn: ({ pageParam }) => fetchProjects({ ...base, page: pageParam }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const loaded = lastPage.page * lastPage.limit;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
    staleTime: 5 * 60 * 1000,
  });
}

export function flattenProjectPages(pages: ProjectsPage[] | undefined) {
  if (!pages) return [];
  const seen = new Set<number>();
  const out: ProjectsPage['data'] = [];
  for (const page of pages) {
    for (const item of page.data) {
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      out.push(item);
    }
  }
  return out;
}

export function useCreateProject() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (name: string) => createProjectApi(name),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['projects'] });
    },
  });
}

export function useUpdateProject() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) => updateProjectApi(id, name),
    onMutate: async ({ id, name }) => {
      await queryClient.cancelQueries({ queryKey: ['projects'] });
      const previous = queryClient.getQueriesData<InfiniteData<ProjectsPage>>({ queryKey: ['projects'] });

      queryClient.setQueriesData<InfiniteData<ProjectsPage>>({ queryKey: ['projects'] }, (old) => {
        if (!old || !('pages' in old)) return old;
        return {
          ...old,
          pages: old.pages.map((page) => ({
            ...page,
            data: page.data.map((p) => (p.id === id ? { ...p, name } : p)),
          })),
        };
      });

      return { previous, sessionEpoch: currentSessionEpoch() };
    },
    onError: (_err, _vars, context) => {
      // A session boundary crossed while the mutation was in flight — the
      // rollback would recreate ['projects', …] with the previous user's
      // snapshot. Skip it.
      if (context?.sessionEpoch !== undefined && context.sessionEpoch !== currentSessionEpoch()) {
        return;
      }
      if (context?.previous) {
        for (const [key, data] of context.previous) {
          queryClient.setQueryData(key, data);
        }
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: ['projects'] });
    },
  });
}

export function useDeleteProject() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: number) => deleteProjectApi(id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['projects'] });
    },
  });
}
