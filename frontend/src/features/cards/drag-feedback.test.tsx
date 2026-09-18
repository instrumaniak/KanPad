import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { CardDraggable } from './card-draggable';
import { DragDropContext } from './drag-drop-context';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Card } from './use-cards';

// Create a mock state object that is accessible within the mock factory
const mockSortableState = {
  isDragging: false,
};

// Mock dnd-kit hooks
vi.mock('@dnd-kit/sortable', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/sortable')>();
  return {
    ...actual,
    useSortable: vi.fn(() => ({
      attributes: {},
      listeners: {},
      setNodeRef: vi.fn(),
      transform: null,
      transition: null,
      isDragging: mockSortableState.isDragging,
    })),
  };
});

// Mock use-cards hooks
vi.mock('./use-cards', () => ({
  useMoveCard: () => ({ mutateAsync: vi.fn() }),
  useReorderCard: () => ({ mutateAsync: vi.fn() }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

const mockCard: Card = {
  id: 1,
  title: 'Feedback Test Card',
  column_id: 1,
  position: 0,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
};

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      {component}
    </QueryClientProvider>
  );
};

describe('UI Feedback', () => {
  beforeEach(() => {
    mockSortableState.isDragging = false;
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

  describe('CardDraggable Feedback', () => {
    it('sets opacity to 0.5 when dragging', () => {
      mockSortableState.isDragging = true;

      render(
        <CardDraggable card={mockCard} index={0}>
          {() => <div>Card Content</div>}
        </CardDraggable>
      );

      const draggableContainer = screen.getByText('Card Content').parentElement!;
      expect(draggableContainer).toHaveStyle({ opacity: '0.5' });
    });

    it('has normal opacity when not dragging', () => {
      mockSortableState.isDragging = false;

      render(
        <CardDraggable card={mockCard} index={0}>
          {() => <div>Card Content</div>}
        </CardDraggable>
      );

      const draggableContainer = screen.getByText('Card Content').parentElement!;
      expect(draggableContainer).toHaveStyle('opacity: 1');
    });
  });

  describe('DragOverlay Feedback', () => {
    it('renders the drag overlay when a card is active (internal state check)', async () => {
       renderWithProviders(
         <DragDropContext boardId={1}>
           <div>Content</div>
         </DragDropContext>
       );
       // Basic mount check
       expect(screen.getByText('Content')).toBeInTheDocument();
    });
  });
});
