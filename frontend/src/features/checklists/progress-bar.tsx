import { Check } from 'lucide-react';

interface ProgressBarProps {
  completed: number;
  total: number;
  className?: string;
}

export function ProgressBar({ completed, total, className = '' }: ProgressBarProps) {
  const percent = total <= 0 ? 0 : Math.round((completed / total) * 100);
  const isComplete = total > 0 && percent === 100;
  const fillClass =
    percent === 0 ? 'bg-muted-foreground/30' : isComplete ? 'bg-emerald-500' : 'bg-teal-500';

  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      aria-label={`Checklist progress: ${completed}/${total} (${percent}%)`}
      data-progress-state={isComplete ? 'complete' : percent === 0 ? 'empty' : 'partial'}
      className={`relative h-2 overflow-hidden rounded-full bg-secondary ${className}`}
    >
      <div
        className={`h-full rounded-full transition-all ${fillClass}`}
        style={{ width: `${percent}%` }}
      />
      {isComplete && (
        <span className="absolute inset-0 flex items-center justify-end pr-0.5">
          <Check className="h-3 w-3 text-white" />
        </span>
      )}
    </div>
  );
}
