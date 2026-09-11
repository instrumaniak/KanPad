import { LayoutGrid, List } from 'lucide-react';
import { Button } from '@/components/ui/button';

export type BoardViewMode = 'board' | 'list';

interface BoardViewToggleProps {
  value: BoardViewMode;
  onChange: (v: BoardViewMode) => void;
  disabled?: boolean;
}

export function BoardViewToggle({ value, onChange, disabled = false }: BoardViewToggleProps) {
  return (
    <div role="group" aria-label="Board view mode" className="flex items-center gap-1">
      <Button
        variant={value === 'board' ? 'default' : 'ghost'}
        size="sm"
        aria-label="Board view"
        aria-pressed={value === 'board'}
        disabled={disabled}
        onClick={() => onChange('board')}
      >
        <LayoutGrid className="h-4 w-4 sm:mr-1" />
        <span className="hidden sm:inline">Board</span>
      </Button>
      <Button
        variant={value === 'list' ? 'default' : 'ghost'}
        size="sm"
        aria-label="List view"
        aria-pressed={value === 'list'}
        disabled={disabled}
        onClick={() => onChange('list')}
      >
        <List className="h-4 w-4 sm:mr-1" />
        <span className="hidden sm:inline">List</span>
      </Button>
    </div>
  );
}
