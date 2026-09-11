import { describe, it, expect } from 'vitest';
import { filterColumnsByFilters, DEFAULT_FILTER_STATE } from './filter-columns-by-filters';
import type { Column } from '../../columns/columns.api';
import type { FilterState } from './filter-columns-by-filters';

function makeColumn(
  id: number,
  cards: Partial<Column['cards'][0]>[],
): Column {
  return {
    id,
    name: `Column ${id}`,
    position: id,
    board_id: 1,
    cards: cards.map((c, i) => ({
      id: c.id ?? id * 100 + i,
      title: c.title ?? `Card ${i}`,
      column_id: id,
      position: i,
      due_date: c.due_date ?? null,
      labels: c.labels ?? [],
      checklist_progress: c.checklist_progress,
      created_at: '2024-01-01',
      updated_at: '2024-01-01',
    })),
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };
}

describe('filterColumnsByFilters', () => {
  it('returns input unchanged for default filter state', () => {
    const columns = [makeColumn(1, [{ title: 'A' }])];
    expect(filterColumnsByFilters(columns, DEFAULT_FILTER_STATE)).toBe(columns);
  });

  it('returns input unchanged when no filters active', () => {
    const columns = [makeColumn(1, [{ title: 'A' }])];
    const filters: FilterState = { labels: [], dueDate: null, checklist: 'All' };
    expect(filterColumnsByFilters(columns, filters)).toBe(columns);
  });

  describe('label filtering (AND logic)', () => {
    it('filters by single label', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', labels: [{ id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' }] },
          { title: 'B', labels: [{ id: 2, name: 'Blue', color: 'blue', created_at: '', updated_at: '' }] },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, labels: ['Red'] });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('filters by multiple labels (AND logic)', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', labels: [
            { id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' },
            { id: 2, name: 'Blue', color: 'blue', created_at: '', updated_at: '' },
          ] },
          { title: 'B', labels: [{ id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' }] },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, labels: ['Red', 'Blue'] });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('returns empty when no cards match all labels', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', labels: [{ id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' }] },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, labels: ['Red', 'Blue'] });
      expect(result[0].cards).toEqual([]);
    });
  });

  describe('due date filtering', () => {
    it('filters No Date (null due_date)', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', due_date: null },
          { title: 'B', due_date: '2024-06-15T00:00:00Z' },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, dueDate: 'No Date' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('filters Overdue (due_date < today)', () => {
      const pastDate = new Date();
      pastDate.setDate(pastDate.getDate() - 2);
      const columns = [
        makeColumn(1, [
          { title: 'A', due_date: pastDate.toISOString() },
          { title: 'B', due_date: null },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, dueDate: 'Overdue' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('filters Today (due_date is today)', () => {
      const today = new Date();
      const columns = [
        makeColumn(1, [
          { title: 'A', due_date: today.toISOString() },
          { title: 'B', due_date: '2020-01-01T00:00:00Z' },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, dueDate: 'Today' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('filters This Week (due_date within 7 days)', () => {
      const soon = new Date();
      soon.setDate(soon.getDate() + 3);
      const far = new Date();
      far.setDate(far.getDate() + 10);
      const columns = [
        makeColumn(1, [
          { title: 'A', due_date: soon.toISOString() },
          { title: 'B', due_date: far.toISOString() },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, dueDate: 'This Week' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });
  });

  describe('checklist filtering', () => {
    it('With Checklist: includes cards with checklist_progress.total > 0', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', checklist_progress: { completed: 1, total: 3, percent: 33 } },
          { title: 'B', checklist_progress: undefined },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, checklist: 'With Checklist' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('Complete: all items completed', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', checklist_progress: { completed: 3, total: 3, percent: 100 } },
          { title: 'B', checklist_progress: { completed: 1, total: 3, percent: 33 } },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, checklist: 'Complete' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('Incomplete: at least one item not completed', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', checklist_progress: { completed: 1, total: 3, percent: 33 } },
          { title: 'B', checklist_progress: { completed: 3, total: 3, percent: 100 } },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, checklist: 'Incomplete' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A']);
    });

    it('All: returns all cards', () => {
      const columns = [
        makeColumn(1, [
          { title: 'A', checklist_progress: { completed: 3, total: 3, percent: 100 } },
          { title: 'B' },
        ]),
      ];
      const result = filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, checklist: 'All' });
      expect(result[0].cards.map((c) => c.title)).toEqual(['A', 'B']);
    });
  });

  it('composes multiple filters sequentially', () => {
    const columns = [
      makeColumn(1, [
        {
          title: 'A',
          labels: [{ id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' }],
          due_date: '2020-01-01T00:00:00Z',
          checklist_progress: { completed: 3, total: 3, percent: 100 },
        },
        {
          title: 'B',
          labels: [{ id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' }],
          due_date: null,
          checklist_progress: { completed: 1, total: 3, percent: 33 },
        },
      ]),
    ];
    const result = filterColumnsByFilters(columns, {
      labels: ['Red'],
      dueDate: 'No Date',
      checklist: 'Incomplete',
    });
    expect(result[0].cards.map((c) => c.title)).toEqual(['B']);
  });

  it('preserves column object identity for untouched columns', () => {
    const untouched = makeColumn(1, [
      { title: 'A', labels: [{ id: 1, name: 'Red', color: 'red', created_at: '', updated_at: '' }] },
    ]);
    const filtered = makeColumn(2, [
      { title: 'B', labels: [{ id: 2, name: 'Blue', color: 'blue', created_at: '', updated_at: '' }] },
    ]);
    const result = filterColumnsByFilters([untouched, filtered], { ...DEFAULT_FILTER_STATE, labels: ['Red'] });
    expect(result[0]).toBe(untouched);
    expect(result[1]).not.toBe(filtered);
  });

  it('does not mutate the original array', () => {
    const columns = [makeColumn(1, [{ title: 'A' }])];
    const snapshot = JSON.stringify(columns);
    filterColumnsByFilters(columns, { ...DEFAULT_FILTER_STATE, labels: ['Red'] });
    expect(JSON.stringify(columns)).toBe(snapshot);
  });
});
