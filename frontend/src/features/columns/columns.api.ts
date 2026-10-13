import type { components } from '@/lib/api/schema';
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
 * `Card` narrows two generated markers back to what the wire always emits
 * (and what `boards/board-view/filter-columns-by-filters.ts` — owned by T41c,
 * out of scope here — reads as non-optional): `due_date` is always rendered
 * as an ISO-8601 string or `null`, and `checklist_progress` is omitted (never
 * `null`) when the card owns no checklists. The narrowing is runtime-safe and
 * keeps `tsc` green without touching T41c's files.
 */
export type Label = components['schemas']['CardLabelRef'];

export type CardSummary = components['schemas']['CardSummary'];

export type Card = Omit<CardSummary, 'due_date' | 'checklist_progress'> & {
  due_date: string | null;
  checklist_progress?: components['schemas']['ChecklistProgress'];
};

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

export async function fetchColumns(boardId: number): Promise<ColumnListResponse> {
  return apiFetch<ColumnListResponse>(`/api/v1/boards/${boardId}/columns`);
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
