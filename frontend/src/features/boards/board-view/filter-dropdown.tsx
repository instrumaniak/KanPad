import { useCallback } from 'react';
import { SlidersHorizontal } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui/popover';
import { getLabelColorClass } from '../../labels/label-colors';
import type { FilterState, DueDateFilter, ChecklistFilter } from './filter-columns-by-filters';

const DUE_DATE_OPTIONS: DueDateFilter[] = ['Overdue', 'Today', 'This Week', 'No Date'];
const CHECKLIST_OPTIONS: ChecklistFilter[] = ['All', 'With Checklist', 'Complete', 'Incomplete'];

interface FilterDropdownProps {
  filters: FilterState;
  onFiltersChange: (filters: FilterState) => void;
  availableLabels: { name: string; color: string }[];
}

export function FilterDropdown({ filters, onFiltersChange, availableLabels }: FilterDropdownProps) {
  const hasActiveFilters =
    filters.labels.length > 0 || filters.dueDate !== null || filters.checklist !== 'All';

  const toggleLabel = useCallback(
    (labelName: string) => {
      const next = filters.labels.includes(labelName)
        ? filters.labels.filter((l) => l !== labelName)
        : [...filters.labels, labelName];
      onFiltersChange({ ...filters, labels: next });
    },
    [filters, onFiltersChange],
  );

  const setDueDate = useCallback(
    (value: DueDateFilter) => {
      onFiltersChange({
        ...filters,
        dueDate: filters.dueDate === value ? null : value,
      });
    },
    [filters, onFiltersChange],
  );

  const setChecklist = useCallback(
    (value: ChecklistFilter) => {
      onFiltersChange({ ...filters, checklist: value });
    },
    [filters, onFiltersChange],
  );

  const handleClearAll = useCallback(() => {
    onFiltersChange({ labels: [], dueDate: null, checklist: 'All' });
  }, [onFiltersChange]);

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant={hasActiveFilters ? 'secondary' : 'ghost'}
          size="icon"
          aria-label="Filter cards"
          className={hasActiveFilters ? 'bg-primary/10 text-primary' : ''}
        >
          <SlidersHorizontal className="h-5 w-5" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        sideOffset={4}
        className="w-[calc(100vw-2rem)] max-w-72 p-0"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <div className="p-3 space-y-4">
          {/* Labels section */}
          {availableLabels.length > 0 && (
            <div role="group" aria-labelledby="filter-labels-heading">
              <h4 id="filter-labels-heading" className="text-xs font-medium text-muted-foreground mb-2">Labels</h4>
              <div className="flex flex-wrap gap-1.5">
                {availableLabels.map((label) => {
                  const isSelected = filters.labels.includes(label.name);
                  return (
                    <button
                      key={label.name}
                      type="button"
                      onClick={() => toggleLabel(label.name)}
                      className={cn(
                        'inline-flex items-center rounded px-2 py-0.5 text-xs font-medium transition-opacity',
                        getLabelColorClass(label.color),
                        isSelected ? 'opacity-100 ring-2 ring-primary ring-offset-1' : 'opacity-60 hover:opacity-100',
                      )}
                      aria-pressed={isSelected}
                    >
                      {label.name}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Due Date section */}
          <div role="group" aria-labelledby="filter-due-heading">
            <h4 id="filter-due-heading" className="text-xs font-medium text-muted-foreground mb-2">Due Date</h4>
            <div className="flex flex-wrap gap-1.5">
              {DUE_DATE_OPTIONS.map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => setDueDate(option)}
                  className={cn(
                    'inline-flex items-center rounded border px-2 py-1 text-xs font-medium transition-colors',
                    filters.dueDate === option
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-background text-foreground border-border hover:bg-muted',
                  )}
                  aria-pressed={filters.dueDate === option}
                >
                  {option}
                </button>
              ))}
            </div>
          </div>

          {/* Checklist section */}
          <div role="group" aria-labelledby="filter-checklist-heading">
            <h4 id="filter-checklist-heading" className="text-xs font-medium text-muted-foreground mb-2">Checklist</h4>
            <div className="flex flex-wrap gap-1.5">
              {CHECKLIST_OPTIONS.map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => setChecklist(option)}
                  className={cn(
                    'inline-flex items-center rounded border px-2 py-1 text-xs font-medium transition-colors',
                    filters.checklist === option
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-background text-foreground border-border hover:bg-muted',
                  )}
                  aria-pressed={filters.checklist === option}
                >
                  {option}
                </button>
              ))}
            </div>
          </div>

          {/* Clear all */}
          {hasActiveFilters && (
            <div className="pt-1 border-t">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleClearAll}
                className="w-full text-xs"
              >
                Clear all filters
              </Button>
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
