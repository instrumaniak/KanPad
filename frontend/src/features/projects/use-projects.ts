import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
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
  const resolved = resolveProjectsQuery(query);
  return useQuery({
    queryKey: ['projects', resolved],
    queryFn: () => fetchProjects(resolved),
    staleTime: 5 * 60 * 1000,
  });
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
      const previous = queryClient.getQueriesData<ProjectsPage>({ queryKey: ['projects'] });

      queryClient.setQueriesData<ProjectsPage>({ queryKey: ['projects'] }, (old) => {
        if (!old) return old;
        return {
          ...old,
          data: old.data.map((p) => (p.id === id ? { ...p, name } : p)),
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
