import { render, screen } from '@testing-library/react';
import { BoardListSkeleton } from './board-list-skeleton';

describe('BoardListSkeleton', () => {
  it('renders default 6 skeletons', () => {
    render(<BoardListSkeleton />);
    const container = screen.getByRole('status');
    expect(container).toHaveAttribute('aria-label', 'Loading boards');
    expect(container.querySelectorAll('.animate-pulse')).toHaveLength(6);
  });

  it('renders custom count', () => {
    render(<BoardListSkeleton count={3} />);
    expect(screen.getByRole('status').querySelectorAll('.animate-pulse')).toHaveLength(3);
  });

  it('has grid layout', () => {
    render(<BoardListSkeleton />);
    const container = screen.getByRole('status');
    expect(container).toHaveClass('grid');
  });

  it('has aria-busy', () => {
    render(<BoardListSkeleton />);
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
  });
});
