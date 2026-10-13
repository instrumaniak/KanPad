import { describe, it, expect, vi, beforeEach } from 'vitest';
import { BASE, ApiError } from '@/lib/api/client';
import * as client from '@/lib/api/client';
import type { components } from '@/lib/api/schema';
import * as projectsApiModule from './projects.api';

vi.mock('@/lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof client>();
  return {
    ...actual,
    apiFetch: vi.fn(),
    apiFetchVoid: vi.fn(),
  };
});

const apiFetch = vi.mocked(client.apiFetch);
const apiFetchVoid = vi.mocked(client.apiFetchVoid);

type Project = components['schemas']['ProjectResponse'];

function projectFixture(overrides: Partial<Project> = {}): Project {
  return {
    id: 1,
    name: 'Project 1',
    board_count: 2,
    created_at: '2024-01-01T00:00:00.000000Z',
    updated_at: '2024-01-01T00:00:00.000000Z',
    ...overrides,
  };
}

function pageFixture(projects: Project[] = [projectFixture()]): projectsApiModule.ProjectsPage {
  return {
    data: projects,
    page: 1,
    limit: 50,
    total: projects.length,
  };
}

describe('projects.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchProjects', () => {
    it('fetches the default page with full URL incl BASE', async () => {
      const page = pageFixture();
      apiFetch.mockResolvedValueOnce(page);

      const result = await projectsApiModule.fetchProjects();

      expect(apiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/projects?page=1&limit=50`);
      expect(result).toEqual(page);
    });

    it('forwards explicit pagination and filters', async () => {
      const page = pageFixture();
      apiFetch.mockResolvedValueOnce(page);

      await projectsApiModule.fetchProjects({ page: 2, limit: 10, q: 'alpha', sort: 'name', order: 'asc' });

      expect(apiFetch).toHaveBeenCalledWith(
        `${BASE}/api/v1/projects?page=2&limit=10&q=alpha&sort=name&order=asc`,
      );
    });

    it('pre-sanitizes undefined/garbage query params instead of sending a 400', async () => {
      apiFetch.mockResolvedValueOnce(pageFixture());

      // NaN (uninitialized state), out-of-range page/limit, blank strings.
      await projectsApiModule.fetchProjects({
        page: NaN,
        limit: 500,
        q: '   ',
        sort: '',
        order: undefined,
      });

      expect(apiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/projects?page=1&limit=50`);
    });

    it('drops non-integer and negative page/limit', async () => {
      apiFetch.mockResolvedValueOnce(pageFixture());

      await projectsApiModule.fetchProjects({ page: 1.5, limit: -3 });

      expect(apiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/projects?page=1&limit=50`);
    });

    it('propagates ApiError with status preserved', async () => {
      apiFetch.mockRejectedValueOnce(new ApiError(401, 'Unauthorized'));

      await expect(projectsApiModule.fetchProjects()).rejects.toMatchObject({ status: 401 });
    });
  });

  describe('createProject', () => {
    it('creates a project via POST with full URL incl BASE', async () => {
      const body = { data: projectFixture({ id: 1, name: 'New Project', board_count: 0 }) };
      apiFetch.mockResolvedValueOnce(body);

      const result = await projectsApiModule.createProject('New Project');

      expect(apiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/projects`, {
        method: 'POST',
        body: JSON.stringify({ name: 'New Project' }),
      });
      expect(result).toEqual(body);
    });

    it('propagates validation errors', async () => {
      apiFetch.mockRejectedValueOnce(new ApiError(400, 'Name is required'));

      await expect(projectsApiModule.createProject('')).rejects.toThrow('Name is required');
    });
  });

  describe('updateProject', () => {
    it('renames a project via PATCH with full URL incl BASE', async () => {
      const body = { data: projectFixture({ name: 'Updated' }) };
      apiFetch.mockResolvedValueOnce(body);

      const result = await projectsApiModule.updateProject(1, 'Updated');

      expect(apiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/projects/1`, {
        method: 'PATCH',
        body: JSON.stringify({ name: 'Updated' }),
      });
      expect(result).toEqual(body);
    });

    it('propagates not-found errors', async () => {
      apiFetch.mockRejectedValueOnce(new ApiError(404, 'Project not found'));

      await expect(projectsApiModule.updateProject(999, 'Test')).rejects.toThrow(
        'Project not found',
      );
    });
  });

  describe('deleteProject', () => {
    it('deletes via apiFetchVoid and resolves void (204, no body)', async () => {
      apiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await projectsApiModule.deleteProject(1);

      expect(apiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/projects/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates not-found errors', async () => {
      apiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Project not found'));

      await expect(projectsApiModule.deleteProject(999)).rejects.toThrow('Project not found');
    });
  });

  describe('recreateProject', () => {
    it('delegates to createProject', async () => {
      const body = { data: projectFixture({ name: 'Recreated' }) };
      apiFetch.mockResolvedValueOnce(body);

      const result = await projectsApiModule.recreateProject('Recreated');

      expect(apiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/projects`, {
        method: 'POST',
        body: JSON.stringify({ name: 'Recreated' }),
      });
      expect(result).toEqual(body);
    });
  });

  describe('resolveProjectsQuery', () => {
    it('resolves defaults for empty input', () => {
      expect(projectsApiModule.resolveProjectsQuery()).toEqual({ page: 1, limit: 50 });
      expect(projectsApiModule.resolveProjectsQuery({})).toEqual({ page: 1, limit: 50 });
    });

    it('keeps valid values and trims the audit surface', () => {
      expect(
        projectsApiModule.resolveProjectsQuery({ page: 3, limit: 25, q: 'x', sort: 'name', order: 'desc' }),
      ).toEqual({ page: 3, limit: 25, q: 'x', sort: 'name', order: 'desc' });
    });
  });

  describe('type exports', () => {
    it('aliases generated schemas (no hand-rolled shapes)', () => {
      const project: projectsApiModule.Project = projectFixture();
      const body: projectsApiModule.ApiResponse<projectsApiModule.Project> = { data: project };
      const page: projectsApiModule.Paginated<projectsApiModule.Project> =
        pageFixture([project]);
      expect(project.board_count).toBe(2);
      expect(body.data).toBeDefined();
      expect(page.total).toBe(1);
    });
  });
});
