import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BoardViewToggle } from './board-view-toggle';

describe('BoardViewToggle', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
  });

  it('renders Board and List options', () => {
    render(<BoardViewToggle value="board" onChange={() => {}} />);
    expect(screen.getByRole('group', { name: 'Board view mode' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /board/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /list/i })).toBeInTheDocument();
  });

  it('indicates active view with aria-pressed', () => {
    render(<BoardViewToggle value="list" onChange={() => {}} />);
    expect(screen.getByRole('button', { name: /board/i })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByRole('button', { name: /list/i })).toHaveAttribute('aria-pressed', 'true');
  });

  it('calls onChange with list when List clicked', () => {
    const onChange = vi.fn();
    render(<BoardViewToggle value="board" onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: /list/i }));
    expect(onChange).toHaveBeenCalledWith('list');
  });

  it('calls onChange with board when Board clicked', () => {
    const onChange = vi.fn();
    render(<BoardViewToggle value="list" onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: /board/i }));
    expect(onChange).toHaveBeenCalledWith('board');
  });

  it('disables buttons when disabled', () => {
    render(<BoardViewToggle value="board" onChange={() => {}} disabled />);
    expect(screen.getByRole('button', { name: /board/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /list/i })).toBeDisabled();
  });
});
