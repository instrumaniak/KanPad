import type { CardListFilters } from '../../cards/cards.api';

export type DueDateFilter = 'Overdue' | 'Today' | 'This Week' | 'No Date';
export type ChecklistFilter = 'All' | 'With Checklist' | 'Complete' | 'Incomplete';

export interface FilterState {
  /** Selected label IDs (server expects label IDs). */
  labels: number[];
  dueDate: DueDateFilter | null;
  checklist: ChecklistFilter;
}

export const DEFAULT_FILTER_STATE: FilterState = {
  labels: [],
  dueDate: null,
  checklist: 'All',
};

export function hasActiveFilters(filters: FilterState): boolean {
  return filters.labels.length > 0 || filters.dueDate !== null || filters.checklist !== 'All';
}

const DUE_DATE_TO_SERVER: Record<DueDateFilter, NonNullable<CardListFilters['due_date']>> = {
  Overdue: 'overdue',
  Today: 'today',
  'This Week': 'this_week',
  'No Date': 'no_date',
};

const CHECKLIST_TO_SERVER: Record<Exclude<ChecklistFilter, 'All'>, NonNullable<CardListFilters['checklist']>> = {
  'With Checklist': 'with',
  Complete: 'complete',
  Incomplete: 'incomplete',
};

export function toServerCardFilters(filters: FilterState, search?: string): CardListFilters {
  const out: CardListFilters = {};
  const trimmed = search?.trim();
  if (trimmed) out.search = trimmed;
  if (filters.labels.length > 0) out.labels = [...filters.labels];
  if (filters.dueDate) out.due_date = DUE_DATE_TO_SERVER[filters.dueDate];
  if (filters.checklist !== 'All') out.checklist = CHECKLIST_TO_SERVER[filters.checklist];
  return out;
}

export function cardFiltersEqual(a: CardListFilters, b: CardListFilters): boolean {
  if (a.search !== b.search) return false;
  if (a.due_date !== b.due_date) return false;
  if (a.checklist !== b.checklist) return false;
  const al = a.labels ?? [];
  const bl = b.labels ?? [];
  if (al.length !== bl.length) return false;
  return al.every((v, i) => v === bl[i]);
}
