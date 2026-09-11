import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MobileBottomSheet } from './mobile-bottom-sheet';

vi.mock('@/hooks/use-prefers-reduced-motion', () => ({
  usePrefersReducedMotion: () => false,
}));

describe('MobileBottomSheet', () => {
  it('renders children when open', () => {
    render(
      <MobileBottomSheet open onOpenChange={() => undefined}>
        <div data-testid="sheet-content">Sheet body</div>
      </MobileBottomSheet>,
    );

    expect(screen.getByTestId('sheet-content')).toBeInTheDocument();
  });

  it('does not render children when closed', () => {
    render(
      <MobileBottomSheet open={false} onOpenChange={() => undefined}>
        <div data-testid="sheet-content">Sheet body</div>
      </MobileBottomSheet>,
    );

    expect(screen.queryByTestId('sheet-content')).not.toBeInTheDocument();
  });

  it('shows title when provided', () => {
    render(
      <MobileBottomSheet open onOpenChange={() => undefined} title="Sheet Title">
        <div>Body</div>
      </MobileBottomSheet>,
    );

    expect(screen.getByText('Sheet Title')).toBeInTheDocument();
  });

  it('calls onOpenChange when the close button is clicked', () => {
    const onOpenChange = vi.fn();
    render(
      <MobileBottomSheet open onOpenChange={onOpenChange}>
        <div>Body</div>
      </MobileBottomSheet>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});
