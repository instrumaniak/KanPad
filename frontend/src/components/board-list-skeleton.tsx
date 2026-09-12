interface BoardListSkeletonProps {
  count?: number;
}

export function BoardListSkeleton({ count = 6 }: BoardListSkeletonProps) {
  return (
    <div
      className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4"
      role="status"
      aria-label="Loading boards"
      aria-busy="true"
    >
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="h-32 animate-pulse rounded-lg bg-primary/10" />
      ))}
    </div>
  );
}
