import type { components } from '@/lib/api/schema';
import type { Card as BoardCard } from '../cards/cards.api';
import { apiFetch, apiFetchVoid, type ApiResponse } from '@/lib/api/client';

/**
 * Columns feature wire types (T41d).
 *
 * Local `Label`/`Card`/`Column` names are kept (out-of-scope importers in
 * `boards/board-view/*` and the hook layer depend on them) but every shape is
 * sourced from the generated `components["schemas"]`:
 * - `Label` resolves the old `LabelColor`-union-vs-`string` drift toward the
 *   generated `CardLabelRef` (`{id, name, color}` — the shape actually nested
 *   on card summaries; `color` is a plain `string` per the schema).
 * - `Column` is the generated `ColumnResponse` with `cards` narrowed to the
 *   board-cache `Card` below.
 *
 * `Card` is NOT narrowed here (T69): the columns embed carries card
 * SUMMARIES — exactly the generated `CardSummary` — so the type is the
 * generated one, byte for byte. The old
 * `Omit<CardSummary, 'due_date' | 'checklist_progress'> & {…}` narrowed
 * `due_date` to a always-present `string | null` and `checklist_progress` to a
 * never-`null` optional, but the fetch path returns the GENERATED type
 * (`ColumnListBody`), so the narrowed type never described the data that
 * actually flowed through it — that mismatch was 7 of the 15 `tsc` errors.
 * The board card now lives in ONE place: `cards/cards.api.ts`.
 */
export type Label = components['schemas']['CardLabelRef'];

export type CardSummary = components['schemas']['CardSummary'];

/**
 * The board card (owned by the cards feature — the columns embed carries
 * card summaries, the same wire shape the board renders).
 */
export type Card = BoardCard;

export type Column = Omit<components['schemas']['ColumnResponse'], 'cards'> & {
  cards: Card[];
};

export type ColumnListResponse = components['schemas']['ColumnListBody'];
export type ColumnResponse = components['schemas']['ColumnBody'];
export type MoveCardsResponse = components['schemas']['MoveCardsBody'];
export type MoveCardsData = components['schemas']['MoveCardsData'];

export type { ApiResponse };

export type CreateColumnData = components['schemas']['CreateColumnInput'];

export type UpdateColumnData = components['schemas']['UpdateColumnInput'];

export interface ColumnListFilters {
  search?: string;
  labels?: number[];
  due_date?: 'overdue' | 'today' | 'this_week' | 'no_date';
  checklist?: 'with' | 'complete' | 'incomplete';
}

export function buildColumnFilterParams(filters?: ColumnListFilters): URLSearchParams {
  const params = new URLSearchParams();
  if (!filters) return params;
  const search = filters.search?.trim();
  if (search) params.set('search', search);
  if (filters.labels?.length) {
    for (const id of [...new Set(filters.labels)].filter((n) => Number.isInteger(n) && n > 0)) {
      params.append('labels', String(id));
    }
  }
  if (filters.due_date) params.set('due_date', filters.due_date);
  if (filters.checklist) params.set('checklist', filters.checklist);
  return params;
}

export async function fetchColumns(
  boardId: number,
  options?: { embedLimit?: number; filters?: ColumnListFilters },
): Promise<ColumnListResponse> {
  const params = buildColumnFilterParams(options?.filters);
  if (typeof options?.embedLimit === 'number') {
    params.set('embed_limit', String(options.embedLimit));
  }
  const query = params.toString();
  return apiFetch<ColumnListResponse>(
    `/api/v1/boards/${boardId}/columns${query ? `?${query}` : ''}`,
  );
}

export async function createColumn(
  boardId: number,
  data: CreateColumnData,
): Promise<ColumnResponse> {
  return apiFetch<ColumnResponse>(`/api/v1/boards/${boardId}/columns`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateColumn(id: number, data: UpdateColumnData): Promise<ColumnResponse> {
  return apiFetch<ColumnResponse>(`/api/v1/columns/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

export async function deleteColumn(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/columns/${id}`, {
    method: 'DELETE',
  });
}

/**
 * Sort a column's cards by creation date. The rewrite folds the old verb
 * route `PATCH /api/columns/:id/sort {order}` into
 * `PATCH /api/v1/columns/:id {sort_order}` — same hook signature, new wire
 * key (`order` → `sort_order`, snake_case per `21 §columns`).
 */
export async function sortCards(columnId: number, order: 'asc' | 'desc'): Promise<ColumnResponse> {
  return apiFetch<ColumnResponse>(`/api/v1/columns/${columnId}`, {
    method: 'PATCH',
    body: JSON.stringify({ sort_order: order }),
  });
}

/**
 * Move every card out of the source column into the target column. The
 * rewrite replaces the verb route `POST /api/columns/:id/move-all
 * {targetColumnId}` with `POST /api/v1/columns/:id/move-cards
 * {target_column_id}` returning `200 {data: {moved_count}}` (snake_case per
 * `21 §columns`). Hook signature (`sourceColumnId`, `targetColumnId`) is
 * unchanged; only the wire key and the response field changed.
 */
export async function moveAllCards(
  sourceColumnId: number,
  targetColumnId: number,
): Promise<MoveCardsResponse> {
  return apiFetch<MoveCardsResponse>(`/api/v1/columns/${sourceColumnId}/move-cards`, {
    method: 'POST',
    body: JSON.stringify({ target_column_id: targetColumnId }),
  });
}
