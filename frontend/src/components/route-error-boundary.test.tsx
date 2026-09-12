import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouteErrorBoundary } from './route-error-boundary';

function ThrowError({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error('Test error');
  }
  return <div>Child content</div>;
}

function ErrorFallback({ error, reset }: { error: Error; reset: () => void }) {
  return (
    <div>
      <p>Error: {error.message}</p>
      <button onClick={reset}>Try Again</button>
    </div>
  );
}

describe('RouteErrorBoundary', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders children when no error', () => {
    render(
      <RouteErrorBoundary>
        <ThrowError shouldThrow={false} />
      </RouteErrorBoundary>
    );
    expect(screen.getByText('Child content')).toBeInTheDocument();
  });

  it('renders default fallback on error', () => {
    render(
      <RouteErrorBoundary>
        <ThrowError shouldThrow={true} />
      </RouteErrorBoundary>
    );
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
    expect(screen.getByText(/An unexpected error occurred/)).toBeInTheDocument();
  });

  it('renders custom fallback on error', () => {
    render(
      <RouteErrorBoundary fallback={ErrorFallback}>
        <ThrowError shouldThrow={true} />
      </RouteErrorBoundary>
    );
    expect(screen.getByText('Error: Test error')).toBeInTheDocument();
  });

  it('calls console.error on error', () => {
    render(
      <RouteErrorBoundary>
        <ThrowError shouldThrow={true} />
      </RouteErrorBoundary>
    );
    expect(console.error).toHaveBeenCalled();
  });

  it('resets error on try again click', async () => {
    const user = userEvent.setup();
    const { rerender } = render(
      <RouteErrorBoundary fallback={ErrorFallback}>
        <ThrowError shouldThrow={true} />
      </RouteErrorBoundary>
    );
    
    expect(screen.getByText('Error: Test error')).toBeInTheDocument();
    
    rerender(
      <RouteErrorBoundary fallback={ErrorFallback}>
        <ThrowError shouldThrow={false} />
      </RouteErrorBoundary>
    );
    
    await user.click(screen.getByText('Try Again'));
    expect(screen.getByText('Child content')).toBeInTheDocument();
  });

  it('resets when resetKeys change', () => {
    const { rerender } = render(
      <RouteErrorBoundary resetKeys={['key1']} fallback={ErrorFallback}>
        <ThrowError shouldThrow={true} />
      </RouteErrorBoundary>
    );
    
    expect(screen.getByText('Error: Test error')).toBeInTheDocument();
    
    rerender(
      <RouteErrorBoundary resetKeys={['key2']} fallback={ErrorFallback}>
        <ThrowError shouldThrow={false} />
      </RouteErrorBoundary>
    );
    
    expect(screen.getByText('Child content')).toBeInTheDocument();
  });
});
