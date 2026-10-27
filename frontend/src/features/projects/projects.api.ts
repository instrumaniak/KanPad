import type { components } from '@/lib/api/schema';
import {
  apiFetch,
  apiFetchVoid,
  type ApiResponse,
  type Page,
} from '@/lib/api/client';

/**
 * Wire project item. `board_count` is real (single `LEFT JOIN ... GROUP BY`
 * aggregation) — the old Nest `boardCount: 0` stub is dropped.
 */
export type Project = components['schemas']['ProjectResponse'];
/** Paginated projects envelope (`Page<Project>`). */
export type ProjectsPage = components['schemas']['Page_ProjectResponse'];
/** Generic paginated envelope (top-level lists only). */
export type Paginated<T> = Page<T>;
export type { ApiResponse };

export interface ProjectsQuery {
  page?: number;
  limit?: number;
  q?: string;
  sort?: string;
  order?: string;
}

export interface ResolvedProjectsQuery {
  page: number;
  limit: number;
  q?: string;
  sort?: string;
  order?: string;
}

export const PROJECTS_DEFAULT_PAGE = 1;
export const PROJECTS_DEFAULT_LIMIT = 50;
export const PROJECTS_MAX_LIMIT = 100;

/**
 * Newly-strict input audit (T41b): the Rust rewrite rejects garbage query
 * params with `400 problem+json` + `errors[]` (out-of-range `page`/`limit`,
 * non-integer `?page=abc`, `NaN` from uninitialized state — all silently
 * ignored by Nest). Resolve everything to valid values here so the hook
 * never sends undefined/garbage; invalid values fall back to the server
 * defaults (`page=1`, `limit=50`) instead of surfacing a 400.
 */
export function resolveProjectsQuery(query: ProjectsQuery = {}): ResolvedProjectsQuery {
  const page =
    typeof query.page === 'number' && Number.isInteger(query.page) && query.page >= 1
      ? query.page
      : PROJECTS_DEFAULT_PAGE;
  const limit =
    typeof query.limit === 'number' &&
    Number.isInteger(query.limit) &&
    query.limit >= 1 &&
    query.limit <= PROJECTS_MAX_LIMIT
      ? query.limit
      : PROJECTS_DEFAULT_LIMIT;
  const resolved: ResolvedProjectsQuery = { page, limit };
  for (const key of ['q', 'sort', 'order'] as const) {
    const value = query[key];
    if (typeof value === 'string' && value.trim().length > 0) {
      resolved[key] = value;
    }
  }
  return resolved;
}

function toQueryString(query: ResolvedProjectsQuery): string {
  const params = new URLSearchParams();
  params.set('page', String(query.page));
  params.set('limit', String(query.limit));
  if (query.q !== undefined) params.set('q', query.q);
  if (query.sort !== undefined) params.set('sort', query.sort);
  if (query.order !== undefined) params.set('order', query.order);
  return params.toString();
}

export async function fetchProjects(query: ProjectsQuery = {}): Promise<ProjectsPage> {
  const resolved = resolveProjectsQuery(query);
  return apiFetch<ProjectsPage>(`/api/v1/projects?${toQueryString(resolved)}`);
}

export async function createProject(name: string): Promise<ApiResponse<Project>> {
  return apiFetch<ApiResponse<Project>>('/api/v1/projects', {
    method: 'POST',
    body: JSON.stringify({ name }),
  });
}

export async function updateProject(id: number, name: string): Promise<ApiResponse<Project>> {
  return apiFetch<ApiResponse<Project>>(`/api/v1/projects/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ name }),
  });
}

export async function deleteProject(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/projects/${id}`, {
    method: 'DELETE',
  });
}

export async function recreateProject(name: string): Promise<ApiResponse<Project>> {
  return createProject(name);
}
