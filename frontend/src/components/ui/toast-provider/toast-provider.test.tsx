import { render, screen, act } from '@testing-library/react';
import { ToastProvider } from './index';
import { useToast } from '../use-toast';

function ToastTrigger() {
  const { toast } = useToast();
  return (
    <button onClick={() => toast({ title: 'Test toast', description: 'Test description' })}>
      Show Toast
    </button>
  );
}

describe('ToastProvider animations', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders toast with animation class', () => {
    render(
      <ToastProvider>
        <ToastTrigger />
      </ToastProvider>
    );

    act(() => {
      screen.getByText('Show Toast').click();
    });

    const toast = screen.getByTestId('toast-default');
    expect(toast).toHaveClass('toast-enter');
  });

  it('adds exit class on dismiss', () => {
    render(
      <ToastProvider>
        <ToastTrigger />
      </ToastProvider>
    );

    act(() => {
      screen.getByText('Show Toast').click();
    });

    const toast = screen.getByTestId('toast-default');
    const dismissButton = toast.querySelector('button:last-child')!;
    
    act(() => {
      dismissButton.click();
    });

    expect(toast).toHaveClass('toast-exit');
  });

  it('removes toast after exit animation', () => {
    render(
      <ToastProvider>
        <ToastTrigger />
      </ToastProvider>
    );

    act(() => {
      screen.getByText('Show Toast').click();
    });

    const toast = screen.getByTestId('toast-default');
    const dismissButton = toast.querySelector('button:last-child')!;
    
    act(() => {
      dismissButton.click();
    });

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(screen.queryByTestId('toast-default')).not.toBeInTheDocument();
  });

  it('auto-dismisses after duration', () => {
    render(
      <ToastProvider>
        <ToastTrigger />
      </ToastProvider>
    );

    act(() => {
      screen.getByText('Show Toast').click();
    });

    expect(screen.getByTestId('toast-default')).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(screen.queryByTestId('toast-default')).not.toBeInTheDocument();
  });
});
