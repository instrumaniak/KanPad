import type { components } from '@/lib/api/schema';
import { apiFetch, apiFetchVoid, type Page } from '@/lib/api/client';

/**
 * Wire note detail (T27a `NoteDetailResponse` — the summary shape plus
 * `content`, explicitly selected only by `GET /:id`, create and update).
 * `content` is `string | null` on the wire; list items (see `NoteSummary`)
 * carry no `content` key at all, so every reader must tolerate `undefined`.
 */
export type Note = components['schemas']['NoteDetailResponse'];

/** Wire note list item (TEXT-free — no `content` key, `tags` are `TagRef[]`). */
export type NoteSummary = components['schemas']['Page_NoteSummaryResponse']['data'][number];

/** One tag link inside a note body (`id` order). */
export type TagRef = components['schemas']['TagRef'];

export type NoteType = 'general' | 'board' | 'project' | 'card';

export function getNoteType(note: Pick<Note, 'card_id' | 'project_id' | 'board_id'>): NoteType {
  if (note.card_id) return 'card';
  if (note.project_id) return 'project';
  if (note.board_id) return 'board';
  return 'general';
}

export type CreateNoteData = components['schemas']['CreateNoteInput'];
export type UpdateNoteData = components['schemas']['UpdateNoteInput'];

export interface NoteFilters {
  search?: string;
  type?: NoteType;
  /** Wire name is `tag_id` (snake_case); legacy camelCase `tagId` retired. */
  tag_id?: number;
  board_id?: number;
  page?: number;
  limit?: number;
}

type NoteBody = components['schemas']['NoteBody'];

/**
 * Newly-strict input audit (T41e): `tag_id`/`board_id` (`>= 1`), `page`
 * (`>= 1`) and `limit` (`1..100`) are strict server-side — garbage → 400
 * problem+json. Only defined-and-valid values are sent; anything else is
 * omitted so backend defaults apply instead of 400ing on
 * `undefined`/`NaN` from uninitialized state. `search`/`type` pass through
 * verbatim (`search` is unconstrained; an invalid `type` 400s as a surfaced
 * bug — the only producer is the fixed UI filter list).
 */
function noteQuery(filters?: NoteFilters): string {
  const params = new URLSearchParams();
  if (filters?.search) {
    params.set('search', filters.search);
  }
  if (filters?.type) {
    params.set('type', filters.type);
  }
  const tagId = filters?.tag_id;
  if (typeof tagId === 'number' && Number.isInteger(tagId) && tagId >= 1) {
    params.set('tag_id', String(tagId));
  }
  const boardId = filters?.board_id;
  if (typeof boardId === 'number' && Number.isInteger(boardId) && boardId >= 1) {
    params.set('board_id', String(boardId));
  }
  const page = filters?.page;
  if (typeof page === 'number' && Number.isInteger(page) && page >= 1) {
    params.set('page', String(page));
  }
  const limit = filters?.limit;
  if (typeof limit === 'number' && Number.isInteger(limit) && limit >= 1 && limit <= 100) {
    params.set('limit', String(limit));
  }
  const query = params.toString();
  return query ? `?${query}` : '';
}

export async function createNote(data: CreateNoteData): Promise<NoteBody> {
  return apiFetch<NoteBody>('/api/v1/notes', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function fetchNotes(filters?: NoteFilters): Promise<Page<NoteSummary>> {
  return apiFetch<Page<NoteSummary>>(`/api/v1/notes${noteQuery(filters)}`);
}

export async function fetchNote(id: number): Promise<NoteBody> {
  return apiFetch<NoteBody>(`/api/v1/notes/${id}`);
}

export async function updateNote(id: number, data: UpdateNoteData): Promise<NoteBody> {
  return apiFetch<NoteBody>(`/api/v1/notes/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

/** Deletes are `204` — void via `apiFetchVoid` (signature change expected). */
export async function deleteNote(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/notes/${id}`, {
    method: 'DELETE',
  });
}

/**
 * Board-scoped list via the supported `?board_id=` filter (the old
 * `GET /boards/:boardId/notes` duplicate route is deleted server-side —
 * see T27a — it 404s from the router, so this helper targets the list
 * endpoint instead). `boardId` passes through verbatim: garbage surfaces
 * the backend 400 rather than silently listing another scope's notes.
 */
export async function fetchBoardNotes(boardId: number): Promise<Page<NoteSummary>> {
  return apiFetch<Page<NoteSummary>>(`/api/v1/notes?board_id=${boardId}`);
}
