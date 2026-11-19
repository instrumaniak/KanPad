import type { Card } from './use-cards';
import type { Column } from '../columns/columns.api';

/**
 * Pure structural-sharing computation for moving a card between columns (or
 * repositioning it inside its current column) during drag operations (T021 / P4).
 *
 * Contract:
 * - Removes the card from the column it is currently in (may be `targetColumnId`
 *   itself after an optimistic live move) and inserts it into `targetColumnId`
 *   at `insertIndex` — filter-then-splice semantics, matching dnd-kit's drop index.
 * - Returns the SAME `columns` array reference when the resulting order is
 *   identical to the input — the caller skips the cache write entirely.
 * - Otherwise returns a new top-level array in which ONLY the columns whose
 *   arrays actually changed get new object identities; every other column keeps
 *   its reference.
 * - Inside every column, `position` is reindexed ONLY for cards whose position
 *   actually changed — every other card object keeps its reference, so React
 *   sees referentially stable cards and memoized consumers skip them.
 * - Cards that leave the target's old source column are NOT reindexed (position
 *   is array-order bookkeeping the server recomputes at read time; re-creating
 *   trailing cards would defeat per-card memoization during drags).
 *
 * `insertIndex` is clamped into `[0, targetCards.length]` (after the dragged
 * card is removed from the target) — callers may pass the raw index computed
 * from the `over` item (NaN / out-of-range becomes "append to the end").
 */
export function moveCardToColumn(
  columns: Column[],
  cardId: number,
  targetColumnId: number,
  insertIndex: number,
  card: Card,
): Column[] {
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
    // Columns-cache cards always carry the flag (backend source of truth);
    // keep the defensive default for a card that arrives without it.
    has_description: card.has_description ?? false,
    column_id: targetColumnId,
    // Placeholder — the reindex below assigns the clamped insert position
    // (always ≠ -1, so the moved card is re-created with the right index).
    position: -1,
  };

  const targetWithoutCard = target.cards.filter((c) => c.id !== cardId);
  const requested = Number.isFinite(insertIndex) ? Math.trunc(insertIndex) : targetWithoutCard.length;
  const clamped = Math.max(0, Math.min(requested, targetWithoutCard.length));

  // Echo fast path: the card is already at `clamped` and positions are
  // sequential → the drop lands exactly where the live move put the card.
  // Return the input unchanged and skip the write entirely.
  const currentIndex = target.cards.findIndex((c) => c.id === cardId);
  if (currentIndex === clamped && target.cards.every((c, i) => c.position === i)) {
    return columns;
  }

  // Insert movedCard, then reindex ONLY the cards whose position actually changed.
  // Cards whose index position is unchanged keep their object identity — this is
  // what keeps per-card memoization (React Compiler / React.memo children) intact.
  const inserted: Card[] = [
    ...targetWithoutCard.slice(0, clamped),
    movedCard,
    ...targetWithoutCard.slice(clamped),
  ];
  const nextTargetCards = inserted.map((c, i) => (c.position === i ? c : { ...c, position: i }));

  const next = columns.slice();
  if (sameColumn) {
    // The echo case was already handled by the short-circuit above — reaching
    // here means the order actually changed; write it.
    next[targetIndex] = { ...target, cards: nextTargetCards };
    return next;
  }
  // Note: the source column is NOT reindexed (positions are array-order
  // bookkeeping; trailing cards keep their identity).
  next[fromIndex] = { ...from, cards: from.cards.filter((c) => c.id !== cardId) };
  next[targetIndex] = { ...target, cards: nextTargetCards };
  return next;
}
