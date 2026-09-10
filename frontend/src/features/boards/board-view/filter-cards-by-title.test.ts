import { describe, it, expect } from 'vitest';
import { matchesTitle, filterColumnsByTitle } from './filter-cards-by-title';
import type { Column } from '../../columns/columns.api';

function makeColumn(id: number, titles: string[]): Column {
  return {
    id,
    name: `Column ${id}`,
    position: id,
    board_id: 1,
    cards: titles.map((title, i) => ({
      id: id * 100 + i,
      title,
      column_id: id,
      position: i,
      due_date: null,
      created_at: '2024-01-01',
      updated_at: '2024-01-01',
    })),
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };
}

describe('matchesTitle', () => {
  it('returns true for empty query (pass-through)', () => {
    expect(matchesTitle('Alpha task', '')).toBe(true);
    expect(matchesTitle('Alpha task', '   ')).toBe(true);
  });

  it('matches case-insensitively', () => {
    expect(matchesTitle('Alpha Task', 'alpha')).toBe(true);
    expect(matchesTitle('ALPHA', 'alpha')).toBe(true);
  });

  it('trims query before matching', () => {
    expect(matchesTitle('Alpha task', '  alpha  ')).toBe(true);
  });

  it('returns false for non-match', () => {
    expect(matchesTitle('Alpha task', 'zzz')).toBe(false);
  });
});

describe('filterColumnsByTitle', () => {
  it('returns input unchanged for empty query', () => {
    const columns = [makeColumn(1, ['Alpha'])];
    expect(filterColumnsByTitle(columns, '')).toBe(columns);
  });

  it('returns input unchanged for 1-char query (caller rule)', () => {
    const columns = [makeColumn(1, ['Alpha'])];
    expect(filterColumnsByTitle(columns, 'A')).toBe(columns);
  });

  it('filters cards case-insensitively and trims query', () => {
    const columns = [makeColumn(1, ['Alpha task', 'Beta task'])];
    const result = filterColumnsByTitle(columns, '  ALPHA  ');
    expect(result[0]?.cards.map((c) => c.title)).toEqual(['Alpha task']);
  });

  it('hides non-matching cards', () => {
    const columns = [makeColumn(1, ['Alpha task', 'Beta task']), makeColumn(2, ['Zulu task'])];
    const result = filterColumnsByTitle(columns, 'Al');
    expect(result[0]?.cards.map((c) => c.title)).toEqual(['Alpha task']);
    expect(result[1]?.cards).toEqual([]);
  });

  it('does not mutate the original array', () => {
    const columns = [makeColumn(1, ['Alpha task', 'Beta task'])];
    const snapshot = JSON.stringify(columns);
    filterColumnsByTitle(columns, 'Alpha');
    expect(JSON.stringify(columns)).toBe(snapshot);
  });

  it('preserves column with zero cards', () => {
    const empty = makeColumn(1, []);
    const columns = [empty, makeColumn(2, ['Alpha'])];
    const result = filterColumnsByTitle(columns, 'Alpha');
    expect(result).toHaveLength(2);
    expect(result[0]?.cards).toEqual([]);
    expect(result[0]?.id).toBe(1);
  });

  it('preserves column object identity for untouched columns', () => {
    const untouched = makeColumn(1, ['Beta task']);
    const touched = makeColumn(2, ['Alpha task', 'Beta task']);
    const result = filterColumnsByTitle([untouched, touched], 'Beta');
    expect(result[0]).toBe(untouched);
    expect(result[1]).not.toBe(touched);
  });
});
