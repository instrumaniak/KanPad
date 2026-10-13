import type { components } from '@/lib/api/schema';
import {
  apiFetch,
  apiFetchVoid,
  type ApiResponse,
  type Page,
} from '@/lib/api/client';

/**
 * Wire tag (T27b `TagResponse` — carries `created_at` ONLY, no `updated_at`
 * by design; color is free-form, defaulting to teal server-side).
 */
export type Tag = components['schemas']['TagResponse'];

export type CreateTagData = components['schemas']['CreateTagInput'];
export type UpdateTagData = components['schemas']['UpdateTagInput'];

/**
 * Newly-strict input audit (T41e): `page`/`limit` are strict server-side
 * (`page >= 1`, `1 <= limit <= 100`, garbage → 400 problem+json). Only
 * defined-and-valid values are sent; anything else is omitted so the
 * backend defaults (`page=1, limit=50`) apply instead of 400ing on
 * `undefined`/`NaN` from uninitialized state.
 */
function pageQuery(page?: number, limit?: number): string {
  const params = new URLSearchParams();
  if (Number.isInteger(page) && (page as number) >= 1) {
    params.set('page', String(page));
  }
  if (Number.isInteger(limit) && (limit as number) >= 1 && (limit as number) <= 100) {
    params.set('limit', String(limit));
  }
  const query = params.toString();
  return query ? `?${query}` : '';
}

export async function fetchTags(page?: number, limit?: number): Promise<Page<Tag>> {
  return apiFetch<Page<Tag>>(`/api/v1/tags${pageQuery(page, limit)}`);
}

export async function createTag(data: CreateTagData): Promise<ApiResponse<Tag>> {
  return apiFetch<ApiResponse<Tag>>('/api/v1/tags', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateTag(id: number, data: UpdateTagData): Promise<ApiResponse<Tag>> {
  return apiFetch<ApiResponse<Tag>>(`/api/v1/tags/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

/** Deletes are `204` — void via `apiFetchVoid` (signature change expected). */
export async function deleteTag(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/tags/${id}`, {
    method: 'DELETE',
  });
}
