import { Button } from '@/components/ui/button';

interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
}

export function ErrorState({
  title = 'Something went wrong',
  message,
  onRetry,
  retryLabel = 'Retry',
}: ErrorStateProps) {
  return (
    <div
      className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive"
      role="alert"
      aria-live="assertive"
    >
      <p className="text-sm font-medium">{title}</p>
      <p className="text-sm opacity-80">{message}</p>
      {onRetry && (
        <Button
          variant="outline"
          size="sm"
          className="mt-3"
          onClick={onRetry}
        >
          {retryLabel}
        </Button>
      )}
    </div>
  );
}
