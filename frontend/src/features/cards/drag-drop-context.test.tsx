import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DragDropContext } from './drag-drop-context';

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

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>();
  return {
    ...actual,
    useSensors: (...args: unknown[]) => args,
    useSensor: useSensorSpy,
  };
});

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient();
  return render(<QueryClientProvider client={queryClient}>{component}</QueryClientProvider>);
};

describe('DragDropContext', () => {
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
});