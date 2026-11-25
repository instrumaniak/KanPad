import { describe, it, expect } from 'vitest';
import { DEFAULT_FILTER_STATE, hasActiveFilters, toServerCardFilters, cardFiltersEqual } from './filter-columns-by-filters';

describe('filter helpers', () => {
  it('detects active filters', () => {
    expect(hasActiveFilters(DEFAULT_FILTER_STATE)).toBe(false);
    expect(hasActiveFilters({ ...DEFAULT_FILTER_STATE, labels: [1] })).toBe(true);
    expect(hasActiveFilters({ ...DEFAULT_FILTER_STATE, dueDate: 'Today' })).toBe(true);
    expect(hasActiveFilters({ ...DEFAULT_FILTER_STATE, checklist: 'Complete' })).toBe(true);
  });

  it('maps UI filters to server params', () => {
    expect(toServerCardFilters(DEFAULT_FILTER_STATE, '')).toEqual({});
    expect(
      toServerCardFilters(
        { labels: [2, 1], dueDate: 'This Week', checklist: 'With Checklist' },
        '  hello  ',
      ),
    ).toEqual({
      search: 'hello',
      labels: [2, 1],
      due_date: 'this_week',
      checklist: 'with',
    });
  });

  it('ignores search shorter than trimmed empty and maps due/checklist enums', () => {
    expect(toServerCardFilters({ ...DEFAULT_FILTER_STATE, dueDate: 'No Date' }, '  ')).toEqual({
      due_date: 'no_date',
    });
    expect(toServerCardFilters({ ...DEFAULT_FILTER_STATE, checklist: 'Incomplete' })).toEqual({
      checklist: 'incomplete',
    });
  });

  it('compares card filters', () => {
    expect(cardFiltersEqual({}, {})).toBe(true);
    expect(cardFiltersEqual({ search: 'a' }, { search: 'a' })).toBe(true);
    expect(cardFiltersEqual({ labels: [1, 2] }, { labels: [1, 2] })).toBe(true);
    expect(cardFiltersEqual({ labels: [1, 2] }, { labels: [2, 1] })).toBe(false);
    expect(cardFiltersEqual({ due_date: 'today' }, {})).toBe(false);
  });
});
