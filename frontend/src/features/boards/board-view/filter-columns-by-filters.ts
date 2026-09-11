import type { Column } from '../../columns/columns.api';

export type DueDateFilter = 'Overdue' | 'Today' | 'This Week' | 'No Date';
export type ChecklistFilter = 'All' | 'With Checklist' | 'Complete' | 'Incomplete';

export interface FilterState {
  labels: string[];
  dueDate: DueDateFilter | null;
  checklist: ChecklistFilter;
}

export const DEFAULT_FILTER_STATE: FilterState = {
  labels: [],
  dueDate: null,
  checklist: 'All',
};

function matchesLabels(cardLabels: { name: string }[] | undefined, selectedLabels: string[]): boolean {
  if (selectedLabels.length === 0) return true;
  const names = new Set((cardLabels ?? []).map((l) => l.name));
  return selectedLabels.every((label) => names.has(label));
}

function matchesDueDate(dueDate: string | null, filter: DueDateFilter | null): boolean {
  if (!filter) return true;
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  switch (filter) {
    case 'No Date':
      return dueDate === null;
    case 'Overdue': {
      if (!dueDate) return false;
      return new Date(dueDate) < today;
    }
    case 'Today': {
      if (!dueDate) return false;
      const d = new Date(dueDate);
      return (
        d.getFullYear() === today.getFullYear() &&
        d.getMonth() === today.getMonth() &&
        d.getDate() === today.getDate()
      );
    }
    case 'This Week': {
      if (!dueDate) return false;
      const d = new Date(dueDate);
      const windowEnd = new Date(today);
      windowEnd.setDate(today.getDate() + 7);
      return d >= today && d < windowEnd;
    }
    default:
      return true;
  }
}

function matchesChecklist(
  checklistProgress: { completed: number; total: number } | undefined,
  filter: ChecklistFilter,
): boolean {
  if (filter === 'All') return true;

  const total = checklistProgress?.total ?? 0;
  const completed = checklistProgress?.completed ?? 0;

  switch (filter) {
    case 'With Checklist':
      return total > 0;
    case 'Complete':
      return total > 0 && completed === total;
    case 'Incomplete':
      return total > 0 && completed < total;
    default:
      return true;
  }
}

export function filterColumnsByFilters(columns: Column[], filters: FilterState): Column[] {
  const { labels, dueDate, checklist } = filters;
  const hasFilters = labels.length > 0 || dueDate !== null || checklist !== 'All';
  if (!hasFilters) return columns;

  return columns.map((column) => {
    const cards = column.cards ?? [];
    const filtered = cards.filter((card) => {
      if (!matchesLabels(card.labels, labels)) return false;
      if (!matchesDueDate(card.due_date, dueDate)) return false;
      if (!matchesChecklist(card.checklist_progress, checklist)) return false;
      return true;
    });
    if (filtered.length === cards.length) return column;
    return { ...column, cards: filtered };
  });
}
