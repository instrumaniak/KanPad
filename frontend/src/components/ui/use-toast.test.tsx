import { describe, it, expect, vi } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useToast } from './use-toast';
import { ToastProvider } from './toast-provider';

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>{children}</ToastProvider>
    </QueryClientProvider>
  );
};

describe('ToastProvider', () => {
  it('renders children', () => {
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button onClick={() => toast({ title: 'Test' })}>Show Toast</button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    expect(screen.getByRole('button', { name: 'Show Toast' })).toBeInTheDocument();
  });

  it('shows toast with title', async () => {
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button onClick={() => toast({ title: 'Hello World' })}>Show Toast</button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toast' }).click();
    });

    expect(screen.getByText('Hello World')).toBeInTheDocument();
  });

  it('shows toast with description', async () => {
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button onClick={() => toast({ title: 'Title', description: 'Description text' })}>
            Show Toast
          </button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toast' }).click();
    });

    expect(screen.getByText('Description text')).toBeInTheDocument();
  });

  it('shows success toast', async () => {
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button onClick={() => toast({ title: 'Success', type: 'success' })}>Show Toast</button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toast' }).click();
    });

    expect(screen.getByText('Success')).toBeInTheDocument();
  });

  it('shows error toast', async () => {
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button onClick={() => toast({ title: 'Error', type: 'error' })}>Show Toast</button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toast' }).click();
    });

    expect(screen.getByText('Error')).toBeInTheDocument();
  });

  it('shows destructive toast with action button', async () => {
    const onAction = vi.fn();
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button
            onClick={() =>
              toast({
                title: 'Item deleted',
                type: 'destructive',
                action: { label: 'Undo', onClick: onAction },
              })
            }
          >
            Show Toast
          </button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toast' }).click();
    });

    expect(screen.getByText('Undo')).toBeInTheDocument();
  });

  it('dismisses toast when dismiss button is clicked', async () => {
    vi.useFakeTimers();
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button onClick={() => toast({ title: 'Dismiss me' })}>Show Toast</button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toast' }).click();
    });

    expect(screen.getByText('Dismiss me')).toBeInTheDocument();

    const dismissButton = screen.getByText('×');
    act(() => {
      dismissButton.click();
    });

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(screen.queryByText('Dismiss me')).not.toBeInTheDocument();
    vi.useRealTimers();
  });

  it('allows multiple toasts', async () => {
    const TestComponent = () => {
      const { toast } = useToast();
      return (
        <div>
          <button
            onClick={() => {
              toast({ title: 'First' });
              toast({ title: 'Second' });
              toast({ title: 'Third' });
            }}
          >
            Show Toasts
          </button>
        </div>
      );
    };

    render(<TestComponent />, { wrapper: createWrapper() });
    act(() => {
      screen.getByRole('button', { name: 'Show Toasts' }).click();
    });

    expect(screen.getByText('First')).toBeInTheDocument();
    expect(screen.getByText('Second')).toBeInTheDocument();
    expect(screen.getByText('Third')).toBeInTheDocument();
  });
});

describe('useToast', () => {
  it('throws error when used outside ToastProvider', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const TestComponent = () => {
      useToast();
      return <div>Test</div>;
    };

    expect(() => render(<TestComponent />)).toThrow(
      'useToast must be used within a ToastProvider',
    );
    consoleError.mockRestore();
  });
});
