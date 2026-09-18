interface LoadingSkeletonProps {
  count?: number;
  className?: string;
  ariaLabel?: string;
}

export function LoadingSkeleton({ count = 3, className, ariaLabel = 'Loading content' }: LoadingSkeletonProps) {
  const safeCount = Math.max(0, count);
  
  return (
    <div className={`space-y-3 ${className ?? ''}`} aria-busy="true" aria-label={ariaLabel}>
      {safeCount > 0 && Array.from({ length: safeCount }, (_, i) => (
        <div
          key={i}
          className="h-16 animate-pulse rounded-lg border border-border bg-muted/50"
        />
      ))}
    </div>
  );
}

export { BoardListSkeleton } from './board-list-skeleton';
export { CardListSkeleton } from './card-list-skeleton';
export { FormSkeleton } from './form-skeleton';
