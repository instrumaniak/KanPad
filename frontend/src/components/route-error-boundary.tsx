import { Component, type ReactNode } from 'react';
import { Button } from '@/components/ui/button';

interface RouteErrorBoundaryProps {
  children: ReactNode;
  fallback?: React.ComponentType<{ error: Error; reset: () => void; retryCount: number }>;
  onReset?: () => void;
  resetKeys?: unknown[];
}

interface RouteErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  retryCount: number;
}

const MAX_RETRIES = 3;

// DefaultFallback receives error prop but doesn't use it — required by FallbackComponent type
function DefaultFallback(props: { error: Error; reset: () => void; retryCount: number }) {
  const { reset, retryCount } = props;
  return (
    <div
      className="rounded-lg border border-destructive bg-destructive/10 p-6 text-center"
      role="alert"
      aria-live="assertive"
    >
      <h3 className="text-lg font-semibold text-destructive">Something went wrong</h3>
      <p className="mt-2 text-sm text-muted-foreground">
        An unexpected error occurred. Please try again.
      </p>
      {retryCount < MAX_RETRIES ? (
        <Button variant="outline" className="mt-4" onClick={reset}>
          Try Again
        </Button>
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">
          If this persists, please contact support.
        </p>
      )}
    </div>
  );
}

export class RouteErrorBoundary extends Component<RouteErrorBoundaryProps, RouteErrorBoundaryState> {
  state: RouteErrorBoundaryState = {
    hasError: false,
    error: null,
    retryCount: 0,
  };

  static getDerivedStateFromError(error: Error): Partial<RouteErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('RouteErrorBoundary caught:', error, errorInfo);
  }

  componentDidUpdate(prevProps: RouteErrorBoundaryProps) {
    if (this.state.hasError && prevProps.resetKeys !== this.props.resetKeys) {
      const prevKeys = prevProps.resetKeys ?? [];
      const nextKeys = this.props.resetKeys ?? [];
      const hasChanged =
        prevKeys.length !== nextKeys.length ||
        prevKeys.some((key, i) => key !== nextKeys[i]);
      if (hasChanged) {
        this.setState({ hasError: false, error: null, retryCount: 0 });
        this.props.onReset?.();
      }
    }
  }

  handleReset = () => {
    this.setState((prev) => ({
      hasError: false,
      error: null,
      retryCount: prev.retryCount + 1,
    }));
    this.props.onReset?.();
  };

  render() {
    if (this.state.hasError && this.state.error) {
      const FallbackComponent = this.props.fallback ?? DefaultFallback;
      return (
        <FallbackComponent
          error={this.state.error}
          reset={this.handleReset}
          retryCount={this.state.retryCount}
        />
      );
    }

    return this.props.children;
  }
}
