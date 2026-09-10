import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TagBadge } from './tag-badge';

describe('TagBadge', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders tag name', () => {
    render(<TagBadge name="frontend" color="teal" />);
    expect(screen.getByText('frontend')).toBeInTheDocument();
  });

  it('renders with correct color style', () => {
    const { container } = render(<TagBadge name="frontend" color="teal" />);
    const badge = container.firstChild as HTMLElement;
    expect(badge).toHaveStyle({ color: 'rgb(0, 128, 128)' });
  });

  it('does not render remove button when onRemove is not provided', () => {
    render(<TagBadge name="frontend" color="teal" />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders remove button when onRemove is provided', () => {
    const onRemove = vi.fn();
    render(<TagBadge name="frontend" color="teal" onRemove={onRemove} />);
    expect(screen.getByRole('button')).toBeInTheDocument();
  });

  it('calls onRemove when remove button is clicked', () => {
    const onRemove = vi.fn();
    render(<TagBadge name="frontend" color="teal" onRemove={onRemove} />);
    fireEvent.click(screen.getByRole('button'));
    expect(onRemove).toHaveBeenCalled();
  });

  it('stops propagation when remove is clicked', () => {
    const onRemove = vi.fn();
    const onParentClick = vi.fn();
    render(
      <div onClick={onParentClick}>
        <TagBadge name="frontend" color="teal" onRemove={onRemove} />
      </div>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(onParentClick).not.toHaveBeenCalled();
  });

  it('renders sr-only remove label', () => {
    const onRemove = vi.fn();
    render(<TagBadge name="frontend" color="teal" onRemove={onRemove} />);
    expect(screen.getByText('Remove frontend')).toBeInTheDocument();
  });
});
