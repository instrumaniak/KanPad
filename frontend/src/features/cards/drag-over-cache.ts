import type { Card } from './use-cards';
import type { Column } from '../columns/columns.api';

export interface MoveCardToColumnOptions {
  /** When true (T053 filtered DnD), preserve positions and adjust
   *  matching_cards/total_cards instead of reindexing visible positions. */
  filterActive?: boolean;
}

/**
 * Pure structural-sharing computation for moving a card between columns (or
 * repositioning it inside its current column) during drag operations.
 *
 * Filtered mode (T053): no fake full-column position reindexing. Cross-column
 * moves adjust matching_cards +/-1 and total_cards +/-1; same-column reorders
 * leave counts unchanged. Returns SAME reference when no visible change.
 */
export function moveCardToColumn(
  columns: Column[],
  cardId: number,
  targetColumnId: number,
  insertIndex: number,
  card: Card,
  options?: MoveCardToColumnOptions,
): Column[] {
  const filterActive = options?.filterActive === true;
  const targetIndex = columns.findIndex((col) => col.id === targetColumnId);
  if (targetIndex === -1) return columns;
  const target = columns[targetIndex];

  // The column the card is currently in (exactly one, by construction).
  const fromIndex = columns.findIndex((col) => col.cards.some((c) => c.id === cardId));
  if (fromIndex === -1) return columns;
  const from = columns[fromIndex];
  const sameColumn = from.id === target.id;

  const movedCard: Card = {
    ...card,
    has_description: card.has_description ?? false,
    column_id: targetColumnId,
    position: filterActive ? (card.position ?? -1) : -1,
  };

  const targetWithoutCard = target.cards.filter((c) => c.id !== cardId);
  const requested = Number.isFinite(insertIndex) ? Math.trunc(insertIndex) : targetWithoutCard.length;
  const clamped = Math.max(0, Math.min(requested, targetWithoutCard.length));

  const currentIndex = target.cards.findIndex((c) => c.id === cardId);
  if (currentIndex === clamped) {
    if (filterActive) return columns;
    if (target.cards.every((c, i) => c.position === i)) return columns;
  }

  const inserted: Card[] = [
    ...targetWithoutCard.slice(0, clamped),
    movedCard,
    ...targetWithoutCard.slice(clamped),
  ];

  const nextTargetCards = filterActive
    ? inserted
    : inserted.map((c, i) => (c.position === i ? c : { ...c, position: i }));

  const next = columns.slice();
  if (sameColumn) {
    next[targetIndex] = { ...target, cards: nextTargetCards };
    return next;
  }

  const fromTotal = (from as Column & { total_cards?: number }).total_cards ?? from.cards.length;
  const targetTotal = (target as Column & { total_cards?: number }).total_cards ?? target.cards.length;
  const fromMatching = (from as Column & { matching_cards?: number }).matching_cards;
  const targetMatching = (target as Column & { matching_cards?: number }).matching_cards;

  next[fromIndex] = {
    ...from,
    cards: from.cards.filter((c) => c.id !== cardId),
    ...(typeof (from as Column & { total_cards?: number }).total_cards === 'number'
      ? { total_cards: Math.max(0, fromTotal - 1) }
      : {}),
    ...(filterActive && typeof fromMatching === 'number'
      ? { matching_cards: Math.max(0, fromMatching - 1) }
      : {}),
  } as Column;
  next[targetIndex] = {
    ...target,
    cards: nextTargetCards,
    ...(typeof (target as Column & { total_cards?: number }).total_cards === 'number'
      ? { total_cards: targetTotal + 1 }
      : {}),
    ...(filterActive && typeof targetMatching === 'number'
      ? { matching_cards: targetMatching + 1 }
      : {}),
  } as Column;
  return next;
}