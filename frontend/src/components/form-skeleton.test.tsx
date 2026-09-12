import { render, screen } from '@testing-library/react';
import { FormSkeleton } from './form-skeleton';

describe('FormSkeleton', () => {
  it('renders default 3 fields', () => {
    render(<FormSkeleton />);
    const container = screen.getByRole('status');
    expect(container).toHaveAttribute('aria-label', 'Loading form');
    expect(container.querySelectorAll('.animate-pulse')).toHaveLength(7); // 3 fields (2 each) + 1 button
  });

  it('renders custom field count', () => {
    render(<FormSkeleton fields={2} />);
    expect(screen.getByRole('status').querySelectorAll('.animate-pulse')).toHaveLength(5); // 2 fields (2 each) + 1 button
  });

  it('has aria-busy', () => {
    render(<FormSkeleton />);
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
  });
});
