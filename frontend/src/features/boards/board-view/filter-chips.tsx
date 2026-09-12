import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { FilterState } from './filter-columns-by-filters';

interface FilterChipsProps {
  filters: FilterState;
  onClearFilter: (type: 'labels' | 'dueDate' | 'checklist') => void;
  onClearAll: () => void;
}

export function FilterChips({ filters, onClearFilter, onClearAll }: FilterChipsProps) {
  const chips: { key: string; label: string; onClear: () => void }[] = [];

  if (filters.labels.length > 0) {
    const labelNames = filters.labels.join(', ');
    chips.push({
      key: 'labels',
      label: `Labels: ${labelNames}`,
      onClear: () => onClearFilter('labels'),
    });
  }

  if (filters.dueDate !== null) {
    chips.push({
      key: 'dueDate',
      label: `Due: ${filters.dueDate}`,
      onClear: () => onClearFilter('dueDate'),
    });
  }

  if (filters.checklist !== 'All') {
    chips.push({
      key: 'checklist',
      label: `Checklist: ${filters.checklist}`,
      onClear: () => onClearFilter('checklist'),
    });
  }

  if (chips.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1.5 px-6 pb-2">
      {chips.map((chip) => (
        <span
          key={chip.key}
          className="inline-flex items-center gap-1 rounded bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground"
        >
          {chip.label}
          <button
            type="button"
            onClick={chip.onClear}
            className="rounded-full p-1 min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 sm:p-0.5 hover:bg-muted-foreground/20 focus:outline-none focus:ring-1 focus:ring-ring"
            aria-label={`Clear ${chip.label}`}
          >
            <X className="h-3 w-3" />
          </button>
        </span>
      ))}
      <Button
        variant="ghost"
        size="sm"
        onClick={onClearAll}
        className="h-6 px-2 text-xs"
      >
        Clear all
      </Button>
    </div>
  );
}
