interface CardListSkeletonProps {
  count?: number;
}

export function CardListSkeleton({ count = 4 }: CardListSkeletonProps) {
  return (
    <div
      className="space-y-2"
      role="status"
      aria-label="Loading cards"
      aria-busy="true"
    >
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="h-16 animate-pulse rounded-lg bg-primary/10" />
      ))}
    </div>
  );
}
