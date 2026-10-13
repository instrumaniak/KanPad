import { apiFetch, apiFetchVoid, type ApiResponse, type Page } from '@/lib/api/client';
import type { components } from '@/lib/api/schema';

/**
 * Boards feature types are aliases over the generated Rust-openapi schemas
 * (`schema.d.ts` — never hand-edited). Wire keys stay snake_case verbatim.
 */
export type Board = components['schemas']['BoardListResponse'];
export type BoardDetail = components['schemas']['BoardDetailResponse'];
export type CreateBoardData = components['schemas']['CreateBoardInput'];
export type UpdateBoardData = components['schemas']['UpdateBoardInput'];

// Shared envelopes live in `@/lib/api/client`; re-exported so existing
// imports keep working. The Nest-only `ListResponse` is retired.
export type { ApiResponse, Page };

const BOARDS_BASE = '/api/v1/boards';

export interface BoardsQuery {
  project_id?: number;
  archived?: boolean;
  page?: number;
  limit?: number;
}

/**
 * Newly-strict input audit (T41c): the Rust rewrite answers `?project_id=abc`,
 * `?page=0`, `?limit=500` with `400 problem+json`. Omit garbage instead of
 * sending it — `undefined` simply means "no filter".
 */
export function buildBoardsQuery(query: BoardsQuery = {}): string {
  const params = new URLSearchParams();
  if (
    query.project_id !== undefined &&
    Number.isInteger(query.project_id) &&
    query.project_id >= 1
  ) {
    params.set('project_id', String(query.project_id));
  }
  if (query.archived === true) {
    params.set('archived', 'true');
  }
  if (query.page !== undefined && Number.isInteger(query.page) && query.page >= 1) {
    params.set('page', String(query.page));
  }
  if (
    query.limit !== undefined &&
    Number.isInteger(query.limit) &&
    query.limit >= 1 &&
    query.limit <= 100
  ) {
    params.set('limit', String(query.limit));
  }
  const qs = params.toString();
  return qs ? `${BOARDS_BASE}?${qs}` : BOARDS_BASE;
}

export async function fetchBoards(query: BoardsQuery = {}): Promise<Page<Board>> {
  return apiFetch<Page<Board>>(buildBoardsQuery(query));
}

export async function fetchBoard(id: number): Promise<ApiResponse<BoardDetail>> {
  return apiFetch<ApiResponse<BoardDetail>>(`${BOARDS_BASE}/${id}`);
}

export async function createBoard(data: CreateBoardData): Promise<ApiResponse<BoardDetail>> {
  return apiFetch<ApiResponse<BoardDetail>>(BOARDS_BASE, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateBoard(
  id: number,
  data: UpdateBoardData,
): Promise<ApiResponse<BoardDetail>> {
  return apiFetch<ApiResponse<BoardDetail>>(`${BOARDS_BASE}/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

/** `DELETE` is `204` (archived boards only; active → `409`). Void by contract. */
export async function deleteBoard(id: number): Promise<void> {
  return apiFetchVoid(`${BOARDS_BASE}/${id}`, { method: 'DELETE' });
}

/** Verb route `PATCH /:id/archive` deleted — archive is an `is_archived` flip. */
export async function archiveBoard(id: number): Promise<ApiResponse<BoardDetail>> {
  return updateBoard(id, { is_archived: true });
}

/** Verb route `PATCH /:id/restore` deleted — restore is an `is_archived` flip. */
export async function restoreBoard(id: number): Promise<ApiResponse<BoardDetail>> {
  return updateBoard(id, { is_archived: false });
}

/** Verb route `DELETE /:id/permanent` deleted — plain `DELETE` (archived only). */
export async function permanentDeleteBoard(id: number): Promise<void> {
  return deleteBoard(id);
}

/** Duplicate route `GET /boards/archived` deleted — `?archived=true` instead. */
export async function fetchArchivedBoards(
  query: Omit<BoardsQuery, 'archived'> = {},
): Promise<Page<Board>> {
  return fetchBoards({ ...query, archived: true });
}

export async function recreateBoard(name: string): Promise<ApiResponse<BoardDetail>> {
  return createBoard({ name });
}
