import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CardSearchInput } from './card-search-input';

describe('CardSearchInput', () => {
  it('renders search input with placeholder and a11y roles', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    expect(screen.getByRole('search')).toBeInTheDocument();
    expect(screen.getByLabelText('Search cards')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Search cards...')).toBeInTheDocument();
  });

  it('shows clear button only when value is non-empty', () => {
    const { rerender } = render(
      <CardSearchInput value="" onChange={() => {}} onClear={() => {}} />,
    );
    expect(screen.queryByLabelText('Clear search')).not.toBeInTheDocument();
    rerender(<CardSearchInput value="Al" onChange={() => {}} onClear={() => {}} />);
    expect(screen.getByLabelText('Clear search')).toBeInTheDocument();
  });

  it('fires onChange on type', () => {
    const onChange = vi.fn();
    render(<CardSearchInput value="" onChange={onChange} onClear={() => {}} />);
    fireEvent.change(screen.getByLabelText('Search cards'), { target: { value: 'A' } });
    expect(onChange).toHaveBeenCalledWith('A');
  });

  it('fires onClear on X click', () => {
    const onClear = vi.fn();
    render(<CardSearchInput value="Al" onChange={() => {}} onClear={onClear} />);
    fireEvent.click(screen.getByLabelText('Clear search'));
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it('fires onClear on Escape', () => {
    const onClear = vi.fn();
    render(<CardSearchInput value="Al" onChange={() => {}} onClear={onClear} />);
    fireEvent.keyDown(screen.getByLabelText('Search cards'), { key: 'Escape' });
    expect(onClear).toHaveBeenCalledTimes(1);
  });
});
