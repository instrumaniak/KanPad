import { render, screen } from '@testing-library/react';
import { CardListSkeleton } from './card-list-skeleton';

describe('CardListSkeleton', () => {
  it('renders default 4 skeletons', () => {
    render(<CardListSkeleton />);
    const container = screen.getByRole('status');
    expect(container).toHaveAttribute('aria-label', 'Loading cards');
    expect(container.querySelectorAll('.animate-pulse')).toHaveLength(4);
  });

  it('renders custom count', () => {
    render(<CardListSkeleton count={2} />);
    expect(screen.getByRole('status').querySelectorAll('.animate-pulse')).toHaveLength(2);
  });

  it('has aria-busy', () => {
    render(<CardListSkeleton />);
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
  });
});
