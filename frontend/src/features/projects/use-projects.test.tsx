import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useProjects, useCreateProject, useUpdateProject, useDeleteProject } from './use-projects';
import * as projectsApi from './projects.api';
import type { components } from '@/lib/api/schema';

vi.mock('./projects.api', async (importOriginal) => {
  const actual = await importOriginal<typeof projectsApi>();
  return {
    ...actual,
    fetchProjects: vi.fn(),
    createProject: vi.fn(),
    updateProject: vi.fn(),
    deleteProject: vi.fn(),
    recreateProject: vi.fn(),
  };
});

type Project = components['schemas']['ProjectResponse'];

function projectFixture(overrides: Partial<Project> = {}): Project {
  return {
    id: 1,
    name: 'Project 1',
    board_count: 2,
    created_at: '',
    updated_at: '',
    ...overrides,
  };
}

function pageFixture(projects: Project[] = [projectFixture()]) {
  return { data: projects, page: 1, limit: 50, total: projects.length };
}

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
};

describe('useProjects', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('fetches projects successfully', async () => {
    const mockProjects = pageFixture([
      projectFixture({ id: 1, name: 'Project 1', board_count: 2 }),
      projectFixture({ id: 2, name: 'Project 2', board_count: 0 }),
    ]);

    (projectsApi.fetchProjects as ReturnType<typeof vi.fn>).mockResolvedValue(mockProjects);

    const { result } = renderHook(() => useProjects(), { wrapper: createWrapper() });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual(mockProjects);
    expect(projectsApi.fetchProjects).toHaveBeenCalledTimes(1);
  });

  it('includes the resolved page in the query key and fetch params', async () => {
    const mockProjects = pageFixture([]);
    const fetchMock = projectsApi.fetchProjects as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(mockProjects);

    const { result } = renderHook(() => useProjects({ page: 2, limit: 10 }), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(fetchMock).toHaveBeenCalledWith({ page: 2, limit: 10 });
  });

  it('handles fetch error', async () => {
    (projectsApi.fetchProjects as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Failed to fetch'),
    );

    const { result } = renderHook(() => useProjects(), { wrapper: createWrapper() });

    await waitFor(() => expect(result.current.isError).toBe(true));

    expect(result.current.error).toBeDefined();
  });

  it('fetches projects and returns data', async () => {
    const mockProjects = pageFixture([]);
    (projectsApi.fetchProjects as ReturnType<typeof vi.fn>).mockResolvedValue(mockProjects);

    const { result } = renderHook(() => useProjects(), { wrapper: createWrapper() });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(mockProjects);
  });
});

describe('useCreateProject', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('creates a project and invalidates query cache', async () => {
    const mockResponse = {
      data: projectFixture({ id: 1, name: 'New Project', board_count: 0 }),
    };
    (projectsApi.createProject as ReturnType<typeof vi.fn>).mockResolvedValue(mockResponse);

    const { result } = renderHook(() => useCreateProject(), { wrapper: createWrapper() });

    let mutationResult: { data: unknown } | undefined;
    await act(async () => {
      const response = await result.current.mutateAsync('New Project');
      mutationResult = { data: response };
    });

    expect(projectsApi.createProject).toHaveBeenCalledWith('New Project');
    expect(mutationResult?.data).toEqual(mockResponse);
  });

  it('handles create error', async () => {
    (projectsApi.createProject as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Creation failed'),
    );

    const { result } = renderHook(() => useCreateProject(), { wrapper: createWrapper() });

    await act(async () => {
      try {
        await result.current.mutateAsync('New Project');
      } catch {
        expect(result.current.error).toBeDefined();
      }
    });
  });

  it('returns mutation state correctly', async () => {
    (projectsApi.createProject as ReturnType<typeof vi.fn>).mockResolvedValue({
      data: projectFixture({ id: 1, name: 'Test', board_count: 0 }),
    });

    const { result } = renderHook(() => useCreateProject(), { wrapper: createWrapper() });

    expect(result.current.isIdle).toBe(true);

    await act(async () => {
      result.current.mutate('Test');
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});

describe('useUpdateProject', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('updates a project with optimistic update', async () => {
    const initialData = pageFixture([projectFixture({ id: 1, name: 'Original', board_count: 0 })]);

    (projectsApi.fetchProjects as ReturnType<typeof vi.fn>).mockResolvedValue(initialData);
    (projectsApi.updateProject as ReturnType<typeof vi.fn>).mockResolvedValue({
      data: projectFixture({ id: 1, name: 'Updated', board_count: 0 }),
    });

    const { result: queryResult } = renderHook(() => useProjects(), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(queryResult.current.isSuccess).toBe(true));

    const { result: mutationResult } = renderHook(() => useUpdateProject(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await mutationResult.current.mutateAsync({ id: 1, name: 'Updated' });
    });

    expect(projectsApi.updateProject).toHaveBeenCalledWith(1, 'Updated');
  });

  it('handles update error and rolls back optimistic update', async () => {
    const initialData = pageFixture([projectFixture({ id: 1, name: 'Original', board_count: 0 })]);

    (projectsApi.fetchProjects as ReturnType<typeof vi.fn>).mockResolvedValue(initialData);
    (projectsApi.updateProject as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Update failed'),
    );

    const { result: queryResult } = renderHook(() => useProjects(), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(queryResult.current.isSuccess).toBe(true));

    const { result: mutationResult } = renderHook(() => useUpdateProject(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      try {
        await mutationResult.current.mutateAsync({ id: 1, name: 'Updated' });
      } catch {
        // Expected error
      }
    });

    await waitFor(() => expect(mutationResult.current.isError).toBe(true));
  });

  it('returns correct mutation states', async () => {
    (projectsApi.updateProject as ReturnType<typeof vi.fn>).mockResolvedValue({
      data: projectFixture({ id: 1, name: 'Test', board_count: 0 }),
    });

    const { result } = renderHook(() => useUpdateProject(), { wrapper: createWrapper() });

    expect(result.current.isIdle).toBe(true);

    await act(async () => {
      result.current.mutate({ id: 1, name: 'Test' });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});

describe('useDeleteProject', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('deletes a project and invalidates query cache', async () => {
    (projectsApi.deleteProject as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    const { result } = renderHook(() => useDeleteProject(), { wrapper: createWrapper() });

    await act(async () => {
      await result.current.mutateAsync(1);
    });

    expect(projectsApi.deleteProject).toHaveBeenCalledWith(1);
  });

  it('handles delete error', async () => {
    (projectsApi.deleteProject as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Delete failed'),
    );

    const { result } = renderHook(() => useDeleteProject(), { wrapper: createWrapper() });

    await act(async () => {
      try {
        await result.current.mutateAsync(1);
      } catch {
        // Expected error
      }
    });

    await waitFor(() => expect(result.current.isError).toBe(true));
  });

  it('returns correct mutation states', async () => {
    (projectsApi.deleteProject as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    const { result } = renderHook(() => useDeleteProject(), { wrapper: createWrapper() });

    expect(result.current.isIdle).toBe(true);

    await act(async () => {
      result.current.mutate(1);
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
