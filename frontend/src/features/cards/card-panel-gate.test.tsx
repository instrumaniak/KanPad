import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import { Card } from './card';

// T021: CardDetailPanel must NOT mount (its hooks/queries included) until the
// card's panel is opened for the first time — then it stays mounted so the
// exit animation is preserved.
const panelRenders = vi.hoisted(() => [] as boolean[]);

vi.mock('./card-detail-panel', () => ({
  CardDetailPanel: ({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) => {
    panelRenders.push(open);
    return (
      <div data-testid="detail-panel-mock" data-open={open}>
        <button data-testid="mock-close-panel" onClick={() => onOpenChange(false)}>close</button>
      </div>
    );
  },
}));

vi.mock('./use-cards', () => ({
  useDeleteCard: () => ({ mutate: vi.fn(), isPending: false }),
  useCreateCard: () => ({ mutate: vi.fn(), mutateAsync: vi.fn(), isPending: false }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

const mockCard = {
  id: 1,
  title: 'Gate Card',
  column_id: 1,
  position: 0,
  due_date: null,
  has_description: false,
  labels: [],
  checklist_progress: null,
  created_at: '2026-01-01T00:00:00.000Z',
  updated_at: '2026-01-01T00:00:00.000Z',
};

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(<QueryClientProvider client={queryClient}>{component}</QueryClientProvider>);
};

describe('Card — CardDetailPanel mount gate (T021)', () => {
  beforeEach(() => {
    panelRenders.length = 0;
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

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('does not mount CardDetailPanel before the first open', () => {
    renderWithProviders(<Card card={mockCard} index={0} />);
    expect(panelRenders).toEqual([]);
    expect(screen.queryByTestId('detail-panel-mock')).not.toBeInTheDocument();
  });

  it('mounts on first open, keeps it mounted after the panel closes', () => {
    renderWithProviders(<Card card={mockCard} index={0} />);
    fireEvent.click(screen.getByText('Gate Card'));
    expect(screen.getByTestId('detail-panel-mock')).toBeInTheDocument();
    expect(screen.getByTestId('detail-panel-mock')).toHaveAttribute('data-open', 'true');

    // close the panel — the component stays mounted (exit animation preserved)
    fireEvent.click(screen.getByTestId('mock-close-panel'));
    expect(screen.getByTestId('detail-panel-mock')).toBeInTheDocument();
    expect(screen.getByTestId('detail-panel-mock')).toHaveAttribute('data-open', 'false');
  });
});
