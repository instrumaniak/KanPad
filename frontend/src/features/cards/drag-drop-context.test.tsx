import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DragDropContext } from './drag-drop-context';
import React from 'react';
import type { DropAnimation } from '@dnd-kit/core';

vi.mock('./use-cards', () => ({
  useMoveCard: () => ({ mutateAsync: vi.fn().mockResolvedValue({}) }),
  useReorderCard: () => ({ mutateAsync: vi.fn().mockResolvedValue({}) }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

const useSensorSpy = vi.hoisted(() =>
  vi.fn().mockImplementation((_sensor: unknown, options?: unknown) => ({
    sensor: _sensor,
    options,
  })),
);

const dragOverlayProps = vi.hoisted(() => ({ dropAnimation: null as DropAnimation | null }));

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>();
  return {
    ...actual,
    useSensors: (...args: unknown[]) => args,
    useSensor: useSensorSpy,
    DragOverlay: vi.fn(({ dropAnimation, children }) => {
      dragOverlayProps.dropAnimation = dropAnimation;
      return <div data-testid="drag-overlay">{children}</div>;
    }),
  };
});

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient();
  return render(<QueryClientProvider client={queryClient}>{component}</QueryClientProvider>);
};

describe('DragDropContext', () => {
  let originalMatchMedia: typeof window.matchMedia;
  beforeEach(() => {
    originalMatchMedia = window.matchMedia;
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
    dragOverlayProps.dropAnimation = null;
  });
  afterEach(() => {
    if (originalMatchMedia) {
      Object.defineProperty(window, 'matchMedia', {
        writable: true,
        value: originalMatchMedia,
      });
    }
  });

  it('renders children', () => {
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div data-testid="children">Drag content</div>
      </DragDropContext>
    );
    expect(screen.getByTestId('children')).toBeInTheDocument();
  });

  it('configures TouchSensor with 500ms activation delay', () => {
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div>Content</div>
      </DragDropContext>
    );

    const touchCall = useSensorSpy.mock.calls.find(
      (call: [{ name: string }]) => call[0]?.name === 'TouchSensor',
    );
    expect(touchCall).toBeDefined();
    expect(touchCall[1].activationConstraint.delay).toBe(500);
    expect(touchCall[1].activationConstraint.tolerance).toBe(5);
  });

  it('provides bounce easing drop animation to DragOverlay', () => {
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div>Content</div>
      </DragDropContext>
    );
    expect(dragOverlayProps.dropAnimation).toBeDefined();
    expect(dragOverlayProps.dropAnimation.duration).toBe(300);
    expect(dragOverlayProps.dropAnimation.easing).toBe('cubic-bezier(0.34, 1.56, 0.64, 1)');
  });

  it('renders with reduced motion preference', () => {
    // Mock matchMedia to return true for prefers-reduced-motion
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: query === '(prefers-reduced-motion: reduce)',
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
    renderWithProviders(
      <DragDropContext boardId={1}>
        <div>Content</div>
      </DragDropContext>
    );
    // Ensure component renders without error
    expect(screen.getByText('Content')).toBeInTheDocument();
    // dropAnimation duration should be 1 (or minimal) for reduced motion
    expect(dragOverlayProps.dropAnimation).toBeDefined();
    expect(dragOverlayProps.dropAnimation.duration).toBe(1);
  });
});
