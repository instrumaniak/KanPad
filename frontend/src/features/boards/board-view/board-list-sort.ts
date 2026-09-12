import type { Card } from '../../columns/columns.api';

export type BoardListSortKey = 'created_at' | 'updated_at' | 'due_date' | 'title';
export type SortDir = 'asc' | 'desc';

export interface FlatCard extends Card {
  columnName: string;
}

export function sortFlatCards(cards: FlatCard[], key: BoardListSortKey, dir: SortDir): FlatCard[] {
  const mul = dir === 'asc' ? 1 : -1;
  const timeOf = (v: string): number => {
    const t = new Date(v).getTime();
    return Number.isNaN(t) ? 0 : t;
  };
  return [...cards].sort((a, b) => {
    if (key === 'title') return a.title.localeCompare(b.title) * mul;
    if (key === 'due_date') {
      if (!a.due_date && !b.due_date) return 0;
      if (!a.due_date) return 1;
      if (!b.due_date) return -1;
      return (timeOf(a.due_date) - timeOf(b.due_date)) * mul;
    }
    return (timeOf(a[key]) - timeOf(b[key])) * mul;
  });
}
