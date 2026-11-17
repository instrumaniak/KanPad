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

describe('same-commit crash + key change', () => {
  // Deterministic metric: React re-invokes the child function during its error
  // recovery passes, so counting child invocations says nothing about boundary
  // behavior. componentDidCatch fires once per caught crash — exactly what the
  // upstream prevState.didCatch guard controls.
  const caughtCount = () =>
    (console.error as unknown as ReturnType<typeof vi.fn>).mock.calls.filter(
      (args: unknown[]) => args[0] === 'RouteErrorBoundary caught:',
    ).length;

  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('does not reset on the commit that catches the error (upstream prevState.didCatch guard)', () => {
    function CountingThrow(): never {
      throw new Error('Test error');
    }
    function Ok() {
      return <div>Child content</div>;
    }

    const { rerender } = render(
      <RouteErrorBoundary resetKeys={['a']} fallback={ErrorFallback}>
        <Ok />
      </RouteErrorBoundary>
    );

    // Crash and key change in the SAME commit — upstream react-error-boundary
    // does not reset here (the key that triggered the error must not reset it).
    rerender(
      <RouteErrorBoundary resetKeys={['b']} fallback={ErrorFallback}>
        <CountingThrow />
      </RouteErrorBoundary>
    );

    expect(screen.getByText('Error: Test error')).toBeInTheDocument();
    // Guard holds: no immediate reset → no re-render of the crashing subtree →
    // exactly one catch, not two.
    expect(caughtCount()).toBe(1);

    // A key change on a LATER commit still resets (navigation semantics kept).
    rerender(
      <RouteErrorBoundary resetKeys={['c']} fallback={ErrorFallback}>
        <Ok />
      </RouteErrorBoundary>
    );
    expect(screen.getByText('Child content')).toBeInTheDocument();
    expect(caughtCount()).toBe(1);
  });
});
