import { Card } from '../entities/card.entity';

export interface CardResponse {
  id: number;
  title: string;
  column_id: number;
  position: number;
  description: string | null;
  due_date: string | null;
  created_at: string;
  updated_at: string;
  labels: { id: number; name: string; color: string }[];
  checklist_progress?: { completed: number; total: number; percent: number };
  checklists: {
    id: number;
    title: string;
    card_id: number;
    created_at: string;
    updated_at: string;
    items: {
      id: number;
      text: string;
      is_completed: boolean;
      checklist_id: number;
      position: number;
      created_at: string;
      updated_at: string;
    }[];
  }[];
}

export type CardDetailResponse = CardResponse;

export interface CardSummaryResponse {
  id: number;
  title: string;
  has_description: boolean;
  column_id: number;
  position: number;
  due_date: string | null;
  created_at: string;
  updated_at: string;
  labels: { id: number; name: string; color: string }[];
  checklist_progress?: { completed: number; total: number; percent: number };
}

export function toCardSummaryResponse(card: Card): CardSummaryResponse {
  const progress: { completed: number; total: number; percent: number } | undefined =
    card.checklist_progress;

  // Prefer pre-computed SQL flag (list queries never SELECT description TEXT).
  // Fallback trims whitespace so '   ' counts as no description.
  // Coerce number/string raw flags (MySQL getRawMany returns '0'/'1').
  const rawFlag: unknown = card.has_description;
  const hasDescription =
    typeof rawFlag === 'boolean'
      ? rawFlag
      : typeof rawFlag === 'number' || typeof rawFlag === 'string'
        ? Number(rawFlag) === 1
        : !!card.description?.trim();

  return {
    id: card.id,
    title: card.title,
    has_description: hasDescription,
    column_id: card.column_id,
    position: card.position,
    due_date: card.due_date ? card.due_date.toISOString() : null,
    created_at: card.created_at.toISOString(),
    updated_at: card.updated_at.toISOString(),
    labels:
      card.cardLabels?.map((cl) => ({
        id: cl.label.id,
        name: cl.label.name,
        color: cl.label.color,
      })) || [],
    ...(progress ? { checklist_progress: progress } : {}),
  };
}

export function toCardResponse(card: Card): CardResponse {
  const allItems = card.checklists?.flatMap((cl) => cl.items ?? []) ?? [];
  const total = allItems.length;
  const completed = allItems.filter((item) => item.is_completed).length;
  const percent = total === 0 ? 0 : Math.round((completed / total) * 100);

  const response: CardResponse = {
    id: card.id,
    title: card.title,
    column_id: card.column_id,
    position: card.position,
    description: card.description,
    due_date: card.due_date ? card.due_date.toISOString() : null,
    created_at: card.created_at.toISOString(),
    updated_at: card.updated_at.toISOString(),
    labels:
      card.cardLabels?.map((cl) => ({
        id: cl.label.id,
        name: cl.label.name,
        color: cl.label.color,
      })) || [],
    checklists:
      card.checklists?.map((cl) => ({
        id: cl.id,
        title: cl.title,
        card_id: cl.card_id,
        created_at: cl.created_at.toISOString(),
        updated_at: cl.updated_at.toISOString(),
        items:
          cl.items?.map((item) => ({
            id: item.id,
            text: item.text,
            is_completed: item.is_completed,
            checklist_id: item.checklist_id,
            position: item.position,
            created_at: item.created_at.toISOString(),
            updated_at: item.updated_at.toISOString(),
          })) || [],
      })) || [],
  };

  if (total > 0 || (card.checklists && card.checklists.length > 0)) {
    response.checklist_progress = { completed, total, percent };
  }

  return response;
}

export function toCardDetailResponse(card: Card): CardDetailResponse {
  const base = toCardResponse(card);
  return {
    ...base,
    checklists:
      card.checklists?.map((cl) => ({
        id: cl.id,
        title: cl.title,
        card_id: cl.card_id,
        created_at: cl.created_at.toISOString(),
        updated_at: cl.updated_at.toISOString(),
        items:
          cl.items?.map((item) => ({
            id: item.id,
            text: item.text,
            is_completed: item.is_completed,
            checklist_id: item.checklist_id,
            position: item.position,
            created_at: item.created_at.toISOString(),
            updated_at: item.updated_at.toISOString(),
          })) || [],
      })) || [],
  };
}
