import { describe, it, expect } from 'vitest';
import { resolveCardDrop, type ResolveCardDropInput } from './drop-resolver';

const col = (id: number, cardIds: number[], total_cards?: number, matching_cards?: number) => ({
  id,
  cards: cardIds.map((cid) => ({ id: cid })),
  total_cards,
  matching_cards,
});

const base = (overrides: Partial<ResolveCardDropInput> = {}): ResolveCardDropInput => ({
  activeCardId: 1,
  sourceColumnId: 10,
  targetColumnId: 20,
  overIsColumnContainer: false,
  columns: [col(10, [1, 2], 2, 2), col(20, [3, 4], 2, 2)],
  filterActive: true,
  ...overrides,
});

describe('resolveCardDrop', () => {
  it('over card => before_card_id anchor (cross-column includes column_id)', () => {
    const r = resolveCardDrop(base({ overCardId: 3 }));
    expect(r).toEqual({ kind: 'anchor', payload: { column_id: 20, before_card_id: 3 } });
  });

  it('same-column over card omits column_id', () => {
    const r = resolveCardDrop(base({ sourceColumnId: 10, targetColumnId: 10, overCardId: 2, columns: [col(10, [1, 2], 2, 2)] }));
    expect(r).toEqual({ kind: 'anchor', payload: { before_card_id: 2 } });
  });

  it('container/end with safe visible anchor => after last visible', () => {
    const r = resolveCardDrop(base({ overIsColumnContainer: true }));
    expect(r).toEqual({ kind: 'anchor', payload: { column_id: 20, after_card_id: 4 } });
  });

  it('truly empty target cross-column => append', () => {
    const r = resolveCardDrop(base({ columns: [col(10, [1], 1, 1), col(20, [], 0, 0)], overIsColumnContainer: true }));
    expect(r).toEqual({ kind: 'append', payload: { column_id: 20 } });
  });

  it('hidden-only target blocked', () => {
    const r = resolveCardDrop(base({ columns: [col(10, [1], 1, 1), col(20, [], 5, 2)], overIsColumnContainer: true }));
    expect(r).toEqual({ kind: 'blocked', reason: 'hidden-only-target' });
  });

  it('unloaded matching tail blocks container/end', () => {
    const r = resolveCardDrop(base({ columns: [col(10, [1], 1, 1), col(20, [3, 4], 5, 5)], overIsColumnContainer: true }));
    expect(r).toEqual({ kind: 'blocked', reason: 'unloaded-tail' });
  });

  it('unknown matching_cards fails closed for ambiguous end/container', () => {
    const r = resolveCardDrop(base({ columns: [col(10, [1], 1, 1), col(20, [3, 4], 5, undefined)], overIsColumnContainer: true }));
    expect(r).toEqual({ kind: 'blocked', reason: 'unloaded-tail' });
  });

  it('over-card allowed even when tail unloaded', () => {
    const r = resolveCardDrop(base({ columns: [col(10, [1], 1, 1), col(20, [3, 4], 5, 5)], overCardId: 3 }));
    expect(r).toEqual({ kind: 'anchor', payload: { column_id: 20, before_card_id: 3 } });
  });

  it('self-over is noop', () => {
    const r = resolveCardDrop(base({ sourceColumnId: 20, targetColumnId: 20, columns: [col(20, [1, 3], 2, 2)], overCardId: 1 }));
    expect(r).toEqual({ kind: 'noop' });
  });
});
