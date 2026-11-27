import type { Column } from '../columns/columns.api';

export type DropResolution =
  | {
      kind: 'anchor';
      payload: { column_id?: number; before_card_id?: number; after_card_id?: number };
    }
  | { kind: 'append'; payload: { column_id: number } }
  | { kind: 'blocked'; reason: 'no-visible-anchor' | 'unloaded-tail' | 'hidden-only-target' }
  | { kind: 'noop' };

export interface ResolveCardDropInput {
  activeCardId: number;
  sourceColumnId: number;
  targetColumnId: number;
  /** Card being hovered over, if any. v1 rule: before over card. */
  overCardId?: number;
  /** True when over the column container/droppable area rather than a card. */
  overIsColumnContainer: boolean;
  columns: Array<
    Pick<Column, 'id' | 'cards' | 'total_cards' | 'matching_cards'>
  >;
  filterActive: boolean;
}

function getColumn(
  columns: ResolveCardDropInput['columns'],
  columnId: number,
) {
  return columns.find((c) => c.id === columnId);
}

function visibleLoadedCount(column: { cards: Array<{ id: number }> }) {
  return column.cards.length;
}

function effectiveTotal(
  column: { cards: Array<{ id: number }>; total_cards?: number; matching_cards?: number },
  filterActive: boolean,
): number | undefined {
  const visible = visibleLoadedCount(column);
  if (!filterActive) {
    return typeof column.total_cards === 'number' ? column.total_cards : visible;
  }
  if (typeof column.matching_cards === 'number') return column.matching_cards;
  return undefined;
}

function isTrulyEmpty(column: { total_cards?: number; cards: Array<{ id: number }> }) {
  if (typeof column.total_cards === 'number') return column.total_cards === 0;
  return column.cards.length === 0;
}

/**
 * Pure drop resolver for board card DnD (T053).
 * Container-over maps to after last visible loaded anchor.
 * Top/start only via over-first-card (before over card). No pointer midpoint.
 */
export function resolveCardDrop(input: ResolveCardDropInput): DropResolution {
  const {
    activeCardId,
    sourceColumnId,
    targetColumnId,
    overCardId,
    overIsColumnContainer,
    columns,
    filterActive,
  } = input;

  const target = getColumn(columns, targetColumnId);
  if (!target) {
    return { kind: 'blocked', reason: 'no-visible-anchor' };
  }

  const crossColumn = sourceColumnId !== targetColumnId;
  const columnIdPayload = crossColumn ? { column_id: targetColumnId } : {};

  // Truly empty target column: unambiguous append for cross-column moves.
  if (isTrulyEmpty(target)) {
    if (crossColumn) {
      return { kind: 'append', payload: { column_id: targetColumnId } };
    }
    // Same-column into empty column shouldn't happen, but block safely.
    return { kind: 'blocked', reason: 'hidden-only-target' };
  }

  // Exclude active card from anchor candidates (may be present during live dragOver).
  const visibleAnchors = target.cards.filter((c) => c.id !== activeCardId);
  const visibleCount = visibleAnchors.length;
  const actualVisibleCount = target.cards.length;

  // Over a specific visible card: v1 => before that card.
  if (overCardId !== undefined && !overIsColumnContainer) {
    if (overCardId === activeCardId) return { kind: 'noop' };
    const overIndex = visibleAnchors.findIndex((c) => c.id === overCardId);
    if (overIndex === -1) {
      return { kind: 'blocked', reason: 'no-visible-anchor' };
    }
    return {
      kind: 'anchor',
      payload: { ...columnIdPayload, before_card_id: overCardId },
    };
  }

  // Container/end drop.
  if (visibleCount === 0) {
    return { kind: 'blocked', reason: 'hidden-only-target' };
  }

  if (filterActive) {
    const eff = effectiveTotal(target, filterActive);
    if (eff === undefined) {
      return { kind: 'blocked', reason: 'unloaded-tail' };
    }
    if (actualVisibleCount < eff) {
      return { kind: 'blocked', reason: 'unloaded-tail' };
    }
  } else {
    const eff = effectiveTotal(target, false);
    if (eff !== undefined && actualVisibleCount < eff) {
      return { kind: 'blocked', reason: 'unloaded-tail' };
    }
  }

  const lastVisible = visibleAnchors[visibleCount - 1];
  if (!lastVisible) return { kind: 'blocked', reason: 'no-visible-anchor' };

  return {
    kind: 'anchor',
    payload: { ...columnIdPayload, after_card_id: lastVisible.id },
  };
}
