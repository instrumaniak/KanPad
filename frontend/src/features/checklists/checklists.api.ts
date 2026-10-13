import type { components } from '@/lib/api/schema';
import { apiFetch, apiFetchVoid } from '@/lib/api/client';

/** Wire checklist with items (T26a `ChecklistResponse`, `position ASC`). */
export type Checklist = components['schemas']['ChecklistResponse'];
export type ChecklistItem = components['schemas']['ChecklistItemResponse'];

export interface CreateChecklistData {
  title: string;
  card_id: number;
}

export type UpdateChecklistData = components['schemas']['UpdateChecklistInput'];
export type CreateChecklistItemData = components['schemas']['CreateChecklistItemInput'];
export type UpdateChecklistItemData = components['schemas']['UpdateChecklistItemInput'];

type ChecklistBody = components['schemas']['ChecklistBody'];
type ChecklistItemBody = components['schemas']['ChecklistItemBody'];

export async function createChecklist(data: CreateChecklistData): Promise<ChecklistBody> {
  return apiFetch<ChecklistBody>(`/api/v1/cards/${data.card_id}/checklists`, {
    method: 'POST',
    body: JSON.stringify({ title: data.title }),
  });
}

export async function fetchChecklist(id: number): Promise<ChecklistBody> {
  return apiFetch<ChecklistBody>(`/api/v1/checklists/${id}`);
}

export async function updateChecklist(
  id: number,
  data: UpdateChecklistData,
): Promise<ChecklistBody> {
  return apiFetch<ChecklistBody>(`/api/v1/checklists/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

/** Deletes are `204` — void via `apiFetchVoid` (signature unchanged). */
export async function deleteChecklist(id: number): Promise<void> {
  return apiFetchVoid(`/api/v1/checklists/${id}`, {
    method: 'DELETE',
  });
}

export async function createChecklistItem(
  checklistId: number,
  data: CreateChecklistItemData,
): Promise<ChecklistItemBody> {
  return apiFetch<ChecklistItemBody>(`/api/v1/checklists/${checklistId}/items`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateChecklistItem(
  itemId: number,
  data: UpdateChecklistItemData,
): Promise<ChecklistItemBody> {
  return apiFetch<ChecklistItemBody>(`/api/v1/checklist-items/${itemId}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

/** Deletes are `204` — void via `apiFetchVoid` (signature unchanged). */
export async function deleteChecklistItem(itemId: number): Promise<void> {
  return apiFetchVoid(`/api/v1/checklist-items/${itemId}`, {
    method: 'DELETE',
  });
}
