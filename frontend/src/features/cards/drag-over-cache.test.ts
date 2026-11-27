import { describe, it, expect } from 'vitest';
import { moveCardToColumn } from './drag-over-cache';
import type { Card } from './use-cards';
import type { Column } from '../columns/columns.api';

const makeCard = (id: number, columnId: number, position: number): Card => ({
  id,
  title: `Card ${id}`,
  column_id: columnId,
  position,
  has_description: false,
  labels: [],
  due_date: null,
  checklist_progress: null,
  created_at: '2026-01-01T00:00:00.000Z',
  updated_at: '2026-01-01T00:00:00.000Z',
});

const makeColumn = (id: number, position: number, cards: Card[]): Column => ({
  id,
  name: `Col ${id}`,
  position,
  board_id: 1,
  created_at: '2026-01-01T00:00:00.000Z',
  updated_at: '2026-01-01T00:00:00.000Z',
  cards,
});

const seed = () => {
  const cardA = makeCard(10, 1, 0);
  const cardB = makeCard(11, 1, 1);
  const cardC = makeCard(12, 2, 0);
  const columns = [
    makeColumn(1, 0, [cardA, cardB]),
    makeColumn(2, 1, [cardC]),
    makeColumn(3, 2, []),
  ];
  return { columns, cardA, cardB, cardC };
};

describe('moveCardToColumn', () => {
  it('returns the same array reference when there is nothing to move (target missing)', () => {
    const { columns, cardA } = seed();
    expect(moveCardToColumn(columns, cardA.id, 777, 0, cardA)).toBe(columns);
  });

  it('returns the same array reference when the card is not found (corrupt/unknown drag payload)', () => {
    const { columns, cardA } = seed();
    expect(moveCardToColumn(columns, 999, 2, 0, cardA)).toBe(columns);
  });

  it('returns the same array reference when the card is already at the requested position (echo)', () => {
    const { columns, cardA } = seed();
    expect(moveCardToColumn(columns, cardA.id, 1, 0, cardA)).toBe(columns);
  });

  it('moves the card across columns and keeps every card object whose position did not change', () => {
    const { columns, cardA, cardB, cardC } = seed();
    const next = moveCardToColumn(columns, cardA.id, 2, 1, cardA);

    expect(next).not.toBe(columns);
    // Source loses the dragged card; the remaining card keeps its reference (filter does not clone).
    expect(next[0].cards).toEqual([cardB]);
    expect(next[0].cards[0]).toBe(cardB);
    // Card C stays at position 0 → same object reference.
    expect(next[1].cards[0]).toBe(cardC);
    expect(next[1].cards[0].position).toBe(0);
    // The moved card is inserted at index 1 with a fresh reference (new column + position).
    expect(next[1].cards[1]).not.toBe(cardA);
    expect(next[1].cards[1].id).toBe(10);
    expect(next[1].cards[1].column_id).toBe(2);
    expect(next[1].cards[1].position).toBe(1);
    // Uninvolved column keeps identity.
    expect(next[2]).toBe(columns[2]);
  });

  it('appends to the target without re-creating unchanged cards when positions stay sequential', () => {
    const { columns, cardA, cardC } = seed();
    // cardC is alone at position 0; appending the dragged card at the end leaves every
    // pre-existing position untouched → all target cards keep identity.
    const next = moveCardToColumn(columns, cardA.id, 2, 1, cardA);
    expect(next[1].cards[0]).toBe(cardC);
  });

  it('inserts into an empty target column', () => {
    const { columns, cardA } = seed();
    const next = moveCardToColumn(columns, cardA.id, 3, 0, cardA);
    expect(next[2].cards).toHaveLength(1);
    expect(next[2].cards[0].id).toBe(10);
    expect(next[2].cards[0].position).toBe(0);
    expect(next[0].cards.map((c) => c.id)).toEqual([11]);
  });

  it('reindexes only the shifted cards in the target and keeps the moved card position consistent', () => {
    const { columns, cardA } = seed();
    const cardX = makeCard(20, 2, 0);
    const cardY = makeCard(21, 2, 1);
    const cardZ = makeCard(22, 2, 2);
    const withFullTarget = columns.map((c) =>
      c.id === 2 ? { ...c, cards: [cardX, cardY, cardZ] } : c,
    );
    const next = moveCardToColumn(withFullTarget, cardA.id, 2, 1, cardA);
    // Insert at 1: X stays at 0 (identity), cardA takes 1, Y moves 1→2, Z moves 2→3.
    expect(next[1].cards[0]).toBe(cardX);
    expect(next[1].cards[1].id).toBe(10);
    expect(next[1].cards[1].position).toBe(1);
    expect(next[1].cards[2].id).toBe(21);
    expect(next[1].cards[2].position).toBe(2);
    expect(next[1].cards[2]).not.toBe(cardY); // position changed → new object
    expect(next[1].cards[3].id).toBe(22);
    expect(next[1].cards[3].position).toBe(3);
    expect(next[1].cards[3]).not.toBe(cardZ); // position changed → new object
  });

  it('clamps an out-of-range or non-finite insertIndex instead of corrupting the target', () => {
    const { columns, cardA } = seed();
    const high = moveCardToColumn(columns, cardA.id, 3, 99, cardA);
    expect(high[2].cards.map((c) => c.id)).toEqual([10]);
    const nan = moveCardToColumn(columns, cardA.id, 2, Number.NaN, cardA);
    expect(nan[1].cards.map((c) => c.id)).toEqual([12, 10]);
    const negative = moveCardToColumn(columns, cardA.id, 2, -5, cardA);
    expect(negative[1].cards.map((c) => c.id)).toEqual([10, 12]);
  });

  it('repositions a card within its own column (post-live-move drop), keeping identity of unmoved cards', () => {
    const { columns, cardA, cardC } = seed();
    // Simulate the cache after a live drag moved A into col 2 at index 0.
    const cardAInTarget: Card = { ...cardA, column_id: 2, position: 0 };
    const cardCAtOne: Card = { ...cardC, position: 1 };
    const moved: Column[] = [
      { ...columns[0], cards: [columns[0].cards[1]] },
      { ...columns[1], cards: [cardAInTarget, cardCAtOne] },
      columns[2],
    ];
    // Drop "on the second card" — insert at 1 into the remaining list: [C, A].
    const same = moveCardToColumn(moved, cardA.id, 2, 1, cardAInTarget);
    expect(same).not.toBe(moved);
    expect(same[1].cards.map((c) => c.id)).toEqual([12, 10]);
    // Drop "on the first card" — insert at 0: order stays [A, C] → echo, same reference.
    const sameFirst = moveCardToColumn(moved, cardA.id, 2, 0, cardAInTarget);
    expect(sameFirst).toBe(moved);
    // Reorder mid-list: [A, C, X], drop X at 0 → X first.
    const cardX = makeCard(30, 2, 2);
    const full: Column[] = [
      { ...columns[0], cards: [columns[0].cards[1]] },
      { ...columns[1], cards: [cardAInTarget, cardCAtOne, cardX] },
      columns[2],
    ];
    const next = moveCardToColumn(full, 30, 2, 0, cardX);
    expect(next[1].cards.map((c) => c.id)).toEqual([30, 10, 12]);
    // A and C positions both changed (0 and 1 → 1 and 2? No: [X,A,C] → A: 0→1, C: 1→2) — all re-created, X re-created.
    expect(next[1].cards[0].id).toBe(30);
  });

  it('returns a fresh array (not identity) when a same-column reorder actually changes order', () => {
    const { columns, cardA, cardB } = seed();
    const next = moveCardToColumn(columns, cardA.id, 1, 1, cardA);
    expect(next).not.toBe(columns);
    expect(next[0].cards.map((c) => c.id)).toEqual([11, 10]);
    // B moved from index 1 → 0, so its position changed → new object identity.
    expect(next[0].cards[0]).not.toBe(cardB);
    expect(next[0].cards[0].position).toBe(0);
    expect(next[0].cards[1].position).toBe(1);
  });

  it('filtered mode: cross-column adjusts matching_cards/total_cards and does not reindex positions', () => {
    const a = { ...makeCard(10, 1, 5), title: 'A' };
    const b = { ...makeCard(11, 2, 7), title: 'B' };
    const cols = [
      { ...makeColumn(1, 0, [a]), total_cards: 3, matching_cards: 1 },
      { ...makeColumn(2, 1, [b]), total_cards: 4, matching_cards: 1 },
    ] as unknown as Column[];
    const next = moveCardToColumn(cols, 10, 2, 0, a, { filterActive: true });
    expect(next[0].cards.map((c) => c.id)).toEqual([]);
    expect(next[1].cards.map((c) => c.id)).toEqual([10, 11]);
    expect((next[0] as Column & { total_cards?: number; matching_cards?: number }).total_cards).toBe(2);
    expect((next[0] as Column & { total_cards?: number; matching_cards?: number }).matching_cards).toBe(0);
    expect((next[1] as Column & { total_cards?: number; matching_cards?: number }).total_cards).toBe(5);
    expect((next[1] as Column & { total_cards?: number; matching_cards?: number }).matching_cards).toBe(2);
    expect(next[1].cards[0].position).toBe(5);
    expect(next[1].cards[1].position).toBe(7);
  });

  it('filtered mode: same-column reorder keeps counts unchanged', () => {
    const a = { ...makeCard(10, 1, 5) };
    const b = { ...makeCard(11, 1, 8) };
    const cols = [{ ...makeColumn(1, 0, [a, b]), total_cards: 5, matching_cards: 2 }] as unknown as Column[];
    const next = moveCardToColumn(cols, 10, 1, 1, a, { filterActive: true });
    expect(next[0].cards.map((c) => c.id)).toEqual([11, 10]);
    expect((next[0] as Column & { total_cards?: number; matching_cards?: number }).total_cards).toBe(5);
    expect((next[0] as Column & { total_cards?: number; matching_cards?: number }).matching_cards).toBe(2);
  });
});
