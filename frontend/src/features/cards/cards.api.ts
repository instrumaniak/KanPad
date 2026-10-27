import type { components } from '@/lib/api/schema';
import { apiFetch, apiFetchVoid, type ApiResponse, type Page } from '@/lib/api/client';

/**
 * Cards feature wire types (T41d).
 *
 * - `Label` resolves the old `LabelColor`-union-vs-`string` drift toward the
 *   generated `LabelResponse` (`color` is a plain `string` per the schema).
 *   The `LabelColor` union is kept as a local domain helper for the labels
 *   feature (`labels.api.ts`, `label-picker.tsx`, `label-colors.ts` import it
 *   from here) — the six enum values the backend validates against.
 * - `CardSummary` vs `CardDetail` is the real wire split and survives as two
 *   schemas: lists nest summaries (`has_description`, no TEXT), detail routes
 *   return the full shape (`description`, `checklists`).
 * - `Card` is the shared component-level type (drag-drop caches, previews,
 *   panels all thread it through): the SUMMARY shape (what the board cache
 *   really holds) plus the detail-only fields as optional — see below.
 */
export type LabelColor = 'red' | 'orange' | 'yellow' | 'green' | 'blue' | 'purple';

export type Label = components['schemas']['LabelResponse'];

export type CardSummary = components['schemas']['CardSummary'];

export type CardDetail = components['schemas']['CardDetailResponse'];

export type ChecklistItemData = components['schemas']['ChecklistItemResponse'];

export type ChecklistData = components['schemas']['ChecklistResponse'];

/**
 * The card the BOARD renders — ONE type, sourced from the generated
 * contract (T69).
 *
 * The wire has TWO card shapes and they are NOT interchangeable:
 * - `CardSummary` — what the card list and the columns embed return:
 *   `has_description` (a boolean flag), no TEXT, no checklists.
 * - `CardDetailResponse` — what the detail route returns: `description`
 *   (TEXT) + `checklists`, and NO `has_description` flag.
 *
 * The board path (columns cache → card tile → drag/drop → detail panel)
 * threads the SUMMARY; the panel hydrates it with the detail route. So the
 * board card is the summary plus the two detail-only fields as OPTIONAL
 * (absent on the wire summary, present once the card has been hydrated).
 *
 * The previous `CardDetail & Pick<CardSummary, 'has_description'>` was a
 * shape the backend NEVER emits — it claimed the summary's flag AND the
 * detail's fields at once. That phantom shape is what made the columns
 * cache (summaries) and the drag/drop payload mutually unassignable.
 */
export type Card = CardSummary & {
  description?: string | null;
  checklists?: ChecklistData[];
};

export type CardBody = components['schemas']['CardBody'];

export type CardListPage = components['schemas']['Page_CardSummary'];

export type { ApiResponse };

export type CreateCardData = components['schemas']['CreateCardInput'];

export type UpdateCardData = components['schemas']['UpdateCardInput'];

export const CARD_LIST_DEFAULT_PAGE = 1;
export const CARD_LIST_DEFAULT_LIMIT = 50;
export const CARD_LIST_MAX_LIMIT = 100;

/**
 * Newly-strict input audit (T41d): the rewrite validates `page`/`limit`
 * strictly (`page >= 1`, `1 <= limit <= 100`, else 400 problem+json). The
 * column card list is the only fetcher in this feature that sends query
 * params, so it pre-sanitizes here — garbage (undefined/NaN/out-of-range
 * from uninitialized state) falls back to valid defaults instead of
 * surfacing a 400.
 */
export function sanitizeCardListParams(
  page?: number,
  limit?: number,
): { page: number; limit: number } {
  return {
    page:
      typeof page === 'number' && Number.isInteger(page) && page >= 1
        ? page
        : CARD_LIST_DEFAULT_PAGE,
    limit:
      typeof limit === 'number' &&
      Number.isInteger(limit) &&
      limit >= 1 &&
      limit <= CARD_LIST_MAX_LIMIT
        ? limit
        : CARD_LIST_DEFAULT_LIMIT,
  };
}

export async function createCard(data: CreateCardData): Promise<CardBody> {
  return apiFetch<CardBody>('/api/v1/cards', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateCard(id: number, data: UpdateCardData): Promise<CardBody> {
  return apiFetch<CardBody>(`/api/v1/cards/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

export async function deleteCard(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/cards/${id}`, {
    method: 'DELETE',
  });
}

export async function fetchCards(
  columnId: number,
  page?: number,
  limit?: number,
): Promise<Page<CardSummary>> {
  const params = sanitizeCardListParams(page, limit);
  return apiFetch<Page<CardSummary>>(
    `/api/v1/columns/${columnId}/cards?page=${params.page}&limit=${params.limit}`,
  );
}

/**
 * Attach a label (canonical `PUT /api/v1/cards/:id/labels/:label_id`, no body;
 * idempotent). Replaces the deprecated alias
 * `POST /api/cards/:id/labels {labelId}` (camelCase body key retired with the
 * Nest envelope). Hook signature (`{cardId, labelId}`) is unchanged.
 */
export async function assignLabelToCard(cardId: number, labelId: number): Promise<CardBody> {
  return apiFetch<CardBody>(`/api/v1/cards/${cardId}/labels/${labelId}`, {
    method: 'PUT',
  });
}

export async function fetchCard(id: number): Promise<CardBody> {
  return apiFetch<CardBody>(`/api/v1/cards/${id}`);
}

export async function removeLabelFromCard(cardId: number, labelId: number): Promise<void> {
  return apiFetchVoid(`/api/v1/cards/${cardId}/labels/${labelId}`, {
    method: 'DELETE',
  });
}
