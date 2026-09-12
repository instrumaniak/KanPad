interface FormSkeletonProps {
  fields?: number;
}

export function FormSkeleton({ fields = 3 }: FormSkeletonProps) {
  return (
    <div
      className="space-y-4"
      role="status"
      aria-label="Loading form"
      aria-busy="true"
    >
      {Array.from({ length: fields }).map((_, i) => (
        <div key={i} className="space-y-2">
          <div className="h-4 w-24 animate-pulse rounded bg-primary/10" />
          <div className="h-10 w-full animate-pulse rounded-md bg-primary/10" />
        </div>
      ))}
      <div className="h-10 w-24 animate-pulse rounded-md bg-primary/10" />
    </div>
  );
}
