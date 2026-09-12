import type { Label } from '../cards/cards.api';
import { X } from 'lucide-react';
import { getLabelColorClass } from './label-colors';

interface LabelBadgeProps {
  label: Pick<Label, 'id' | 'name'> & { color: string };
  className?: string;
  onRemove?: () => void;
}

export function LabelBadge({ label, className = '', onRemove }: LabelBadgeProps) {
  const colorClass = getLabelColorClass(label.color);

  return (
    <span
      className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs font-medium ${colorClass} ${className}`}
      title={label.name}
    >
      {label.name}
      {onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="ml-0.5 rounded-full p-0.5 hover:bg-white/20 focus:outline-none focus:ring-1 focus:ring-white"
          aria-label={`Remove ${label.name}`}
        >
          <X className="h-3 w-3" />
        </button>
      )}
    </span>
  );
}
