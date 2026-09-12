import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Card } from './card';

vi.mock('./use-cards', () => ({
  useUpdateCard: () => ({ mutate: vi.fn(), mutateAsync: Promise.resolve({}) }),
  useDeleteCard: () => ({ mutate: vi.fn(), mutateAsync: Promise.resolve({}) }),
  useCreateCard: () => ({ mutate: vi.fn(), mutateAsync: Promise.resolve({}) }),
  useCard: () => ({ data: undefined, isLoading: false, isError: false, refetch: vi.fn() }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

vi.mock('./card-draggable', () => ({
  CardDraggable: ({
    children,
  }: {
    children: (props: {
      isDragging: boolean;
      transform: null;
      attributes: Record<string, unknown>;
      listeners: Record<string, unknown>;
      setNodeRef: () => void;
    }) => React.ReactNode;
  }) =>
    children({
      isDragging: false,
      transform: null,
      attributes: {},
      listeners: {},
      setNodeRef: vi.fn(),
    }),
}));

vi.mock('./drag-drop-context', () => ({
  DragDropContext: ({ children }: { children: React.ReactNode }) => children,
}));

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={queryClient}>{component}</QueryClientProvider>);
};

describe('Card with Drag', () => {
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

  const mockCard = { id: 1, title: 'Test Card', column_id: 1, position: 0, created_at: '2024-01-01', updated_at: '2024-01-01' };

  it('renders card title', () => {
    renderWithProviders(<Card card={mockCard} index={0} />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();
  });

  it('renders card with role button', () => {
    renderWithProviders(<Card card={mockCard} index={0} />);
    expect(screen.getAllByRole('button').length).toBeGreaterThan(0);
  });
});

describe('CardDraggable', () => {
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

  it('uses useSortable for within-column reordering', async () => {
    const fs = await import('fs');
    const content = fs.readFileSync('./src/features/cards/card-draggable.tsx', 'utf-8');
    expect(content).toContain('useSortable');
  });
});

describe('DragDropContext', () => {
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

  it('includes TouchSensor for mobile support', async () => {
    const fs = await import('fs');
    const content = fs.readFileSync('./src/features/cards/drag-drop-context.tsx', 'utf-8');
    expect(content).toContain('TouchSensor');
  });
});

describe('ColumnDroppable', () => {
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

  it('file exists', async () => {
    const fs = await import('fs');
    expect(fs.existsSync('./src/features/columns/column-droppable.tsx')).toBe(true);
  });
});
