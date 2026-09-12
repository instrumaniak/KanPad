import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ErrorState } from './error-state';

describe('ErrorState', () => {
  it('renders with default title', () => {
    render(<ErrorState message="Something failed" />);
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
  });

  it('renders custom title', () => {
    render(<ErrorState title="Custom Error" message="Failed to load" />);
    expect(screen.getByText('Custom Error')).toBeInTheDocument();
  });

  it('renders message', () => {
    render(<ErrorState message="Network error occurred" />);
    expect(screen.getByText('Network error occurred')).toBeInTheDocument();
  });

  it('has alert role', () => {
    render(<ErrorState message="Error" />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('has aria-live assertive', () => {
    render(<ErrorState message="Error" />);
    const alert = screen.getByRole('alert');
    expect(alert).toHaveAttribute('aria-live', 'assertive');
  });

  it('does not render retry button when onRetry not provided', () => {
    render(<ErrorState message="Error" />);
    expect(screen.queryByRole('button', { name: /retry/i })).not.toBeInTheDocument();
  });

  it('renders retry button when onRetry provided', () => {
    const onRetry = vi.fn();
    render(<ErrorState message="Error" onRetry={onRetry} />);
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });

  it('calls onRetry when retry button clicked', async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    render(<ErrorState message="Error" onRetry={onRetry} />);
    await user.click(screen.getByRole('button', { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('uses custom retry label', () => {
    const onRetry = vi.fn();
    render(<ErrorState message="Error" onRetry={onRetry} retryLabel="Try Again" />);
    expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument();
  });

  it('applies destructive styling', () => {
    render(<ErrorState message="Error" />);
    const alert = screen.getByRole('alert');
    expect(alert).toHaveClass('border-destructive');
  });
});
