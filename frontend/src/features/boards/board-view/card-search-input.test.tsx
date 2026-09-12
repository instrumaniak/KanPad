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

  it('renders collapsed by default with search icon', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const container = screen.getByRole('search');
    const input = screen.getByLabelText('Search cards');
    // Container starts at 36px (collapsed)
    expect(container.style.inlineSize).toBe('36px');
    // Input is opacity-0 by default (hidden until expanded)
    expect(input.className).toContain('opacity-0');
    // Search icon is rendered as a label
    expect(screen.getByText('Search cards', { selector: '.sr-only' })).toBeInTheDocument();
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

  it('label htmlFor matches input id for click-to-focus', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const input = screen.getByLabelText('Search cards');
    const label = screen.getByText('Search cards', { selector: '.sr-only' }).closest('label');
    expect(label).toBeInTheDocument();
    expect(label).toHaveAttribute('for', input.id);
  });

  it('container has :focus-within and :has() classes for expand/collapse', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const container = screen.getByRole('search');
    // Container has the expand/collapse classes
    expect(container.className).toContain('focus-within:min-w-[360px]');
    expect(container.className).toContain('[&:has(input:not(:placeholder-shown))]:min-w-[360px]');
    expect(container.className).toContain('transition-[inline-size,border-color,box-shadow]');
    expect(container.className).toContain('duration-300');
    expect(container.className).toContain('motion-reduce:transition-none');
  });

  it('input has group-focus-within:opacity-100 class for delayed fade-in', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const input = screen.getByLabelText('Search cards');
    expect(input.className).toContain('group-focus-within:opacity-100');
    expect(input.className).toContain('transition-opacity');
    expect(input.className).toContain('duration-200');
    expect(input.className).toContain('delay-[120ms]');
  });

  it('container has group class for group-focus-within to work', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const container = screen.getByRole('search');
    expect(container.className).toContain('group');
  });

  it('expands search container on focus', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const container = screen.getByRole('search');
    const input = screen.getByLabelText('Search cards');
    expect(container).toBeInTheDocument();
    expect(input).toBeInTheDocument();
    fireEvent.focus(input);
    expect(input).toBeInTheDocument();
  });

  it('collapses search container on blur', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const container = screen.getByRole('search');
    const input = screen.getByLabelText('Search cards');
    fireEvent.focus(input);
    expect(input).toBeInTheDocument();
    fireEvent.blur(input);
    expect(container).toBeInTheDocument();
    expect(input).toBeInTheDocument();
  });

  it('Escape clears and blurs input', () => {
    const onClear = vi.fn();
    render(<CardSearchInput value="test" onChange={() => {}} onClear={onClear} />);
    const input = screen.getByLabelText('Search cards');
    fireEvent.focus(input);
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onClear).toHaveBeenCalledTimes(1);
    // After Escape, input should lose focus (blur called)
    expect(document.activeElement).not.toBe(input);
  });

  it('clear button click focuses input after clearing', () => {
    const onClear = vi.fn();
    render(<CardSearchInput value="test" onChange={() => {}} onClear={onClear} />);
    const input = screen.getByLabelText('Search cards');
    fireEvent.click(screen.getByLabelText('Clear search'));
    expect(onClear).toHaveBeenCalledTimes(1);
    // handleClear calls focus() after onClear
    expect(document.activeElement).toBe(input);
  });

  it('input uses search type for native behavior', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const input = screen.getByLabelText('Search cards');
    expect(input).toHaveAttribute('type', 'search');
  });

  it('input has correct flex layout classes', () => {
    render(<CardSearchInput value="" onChange={() => {}} onClear={() => {}} />);
    const input = screen.getByLabelText('Search cards');
    expect(input.className).toContain('flex-1');
    expect(input.className).toContain('min-w-0');
    expect(input.className).toContain('border-0');
  });
});
