import { Badge } from '@/components/ui/badge';
import { X } from 'lucide-react';

interface TagBadgeProps {
  name: string;
  color: string;
  onRemove?: () => void;
  onClick?: () => void;
}

export function TagBadge({ name, color, onRemove, onClick }: TagBadgeProps) {
  return (
    <Badge
      className={`text-[11px] px-1.5 py-0 gap-1 font-normal ${onClick ? 'cursor-pointer hover:opacity-80' : 'cursor-default'}`}
      style={{
        backgroundColor: `${color}20`,
        color,
        borderColor: `${color}40`,
      }}
      variant="outline"
      onClick={(e) => {
        e.stopPropagation();
        onClick?.();
      }}
    >
      {name}
      {onRemove && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="ml-0.5 hover:opacity-70"
        >
          <X className="h-2.5 w-2.5" />
          <span className="sr-only">Remove {name}</span>
        </button>
      )}
    </Badge>
  );
}
