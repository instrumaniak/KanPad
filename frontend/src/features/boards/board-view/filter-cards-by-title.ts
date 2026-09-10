import type { Column } from '../../columns/columns.api';

// Story 5.1: Card Search lives in board-view/ (not features/search/ yet).
// 5-2 will promote/merge search+filter together — do not create a second divergent search implementation.

export function matchesTitle(cardTitle: string, query: string): boolean {
  const normalizedQuery = query.trim().toLowerCase();
  if (normalizedQuery.length === 0) return true;
  return cardTitle.toLowerCase().includes(normalizedQuery);
}

export function filterColumnsByTitle(columns: Column[], query: string): Column[] {
  // Defensive single-source-of-truth for threshold: caller gates with isSearching,
  // util never throws on short query — returns input unchanged.
  if (query.trim().length < 2) return columns;
  return columns.map((column) => {
    const cards = column.cards ?? [];
    const filtered = cards.filter((card) => matchesTitle(card.title, query));
    // Preserve column object identity for untouched columns where possible.
    if (filtered.length === cards.length) return column;
    return { ...column, cards: filtered };
  });
}
