import type { components } from '@/lib/api/schema';
import {
  apiFetch,
  apiFetchVoid,
  type ApiResponse,
  type Page,
} from '@/lib/api/client';

/** Wire label (T25 `LabelResponse` — timestamps are UTC ISO-8601 strings). */
export type Label = components['schemas']['LabelResponse'];

/**
 * Label color. Resolves the old `LabelColor` union-vs-`string` drift to the
 * generated schema: the wire type is `string` (the 6-value enum is enforced
 * server-side with a 400 on violation — see `20-api-contract.md` labels).
 */
export type LabelColor = Label['color'];

export type CreateLabelData = components['schemas']['CreateLabelInput'];
export type UpdateLabelData = components['schemas']['UpdateLabelInput'];

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

export async function fetchLabels(page?: number, limit?: number): Promise<Page<Label>> {
  return apiFetch<Page<Label>>(`/api/v1/labels${pageQuery(page, limit)}`);
}

export async function createLabel(data: CreateLabelData): Promise<ApiResponse<Label>> {
  return apiFetch<ApiResponse<Label>>('/api/v1/labels', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateLabel(id: number, data: UpdateLabelData): Promise<ApiResponse<Label>> {
  return apiFetch<ApiResponse<Label>>(`/api/v1/labels/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

/** Deletes are `204` — void via `apiFetchVoid` (signature change expected). */
export async function deleteLabel(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/labels/${id}`, {
    method: 'DELETE',
  });
}
