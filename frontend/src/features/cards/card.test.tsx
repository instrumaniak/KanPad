import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Card } from './card';
import { LABEL_COLOR_CLASS_MAP } from '../labels/label-colors';

const mockMutate = vi.fn();
const mockMutateAsync = vi.fn();
const mockMutateObj = { mutate: mockMutate, mutateAsync: mockMutateAsync, isPending: false };
const mockDeleteMutate = vi.fn();
const mockDeleteMutateAsync = vi.fn();
const mockDeleteMutateObj = { mutate: mockDeleteMutate, mutateAsync: mockDeleteMutateAsync, isPending: false };
const mockCreateMutateAsync = vi.fn();
const mockCreateMutateObj = { mutate: vi.fn(), mutateAsync: mockCreateMutateAsync, isPending: false };

const mockToast = vi.fn();

vi.mock('./use-cards', () => ({
  useUpdateCard: () => mockMutateObj,
  useDeleteCard: () => mockDeleteMutateObj,
  useCreateCard: () => mockCreateMutateObj,
  useAssignCardLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useRemoveCardLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useCard: () => ({ data: undefined, isLoading: false, isError: false, refetch: vi.fn() }),
}));

vi.mock('../labels/use-labels', () => ({
  useLabels: () => ({ data: [], isLoading: false }),
  useCreateLabel: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({
    toast: mockToast,
  }),
}));

const renderWithProviders = (component: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(<QueryClientProvider client={queryClient}>{component}</QueryClientProvider>);
};

describe('Card', () => {
  const mockCard = { id: 1, title: 'Test Card', column_id: 1, position: 0, description: null, due_date: null, created_at: '2024-01-01', updated_at: '2024-01-01' };

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
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders card title', () => {
    renderWithProviders(<Card card={mockCard} index={0} />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();
  });

  it('opens detail panel when clicked', async () => {
    renderWithProviders(<Card card={mockCard} index={0} />);

    fireEvent.click(screen.getByText('Test Card'));

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('opens detail panel via keyboard Enter', async () => {
    renderWithProviders(<Card card={mockCard} index={0} />);

    const cardDiv = screen.getByRole('button', { name: /open card details/i });
    fireEvent.keyDown(cardDiv, { key: 'Enter', code: 'Enter' });

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('opens detail panel via keyboard Space', async () => {
    renderWithProviders(<Card card={mockCard} index={0} />);

    const cardDiv = screen.getByRole('button', { name: /open card details/i });
    fireEvent.keyDown(cardDiv, { key: ' ', code: 'Space' });

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('does not open detail panel on drag', () => {
    renderWithProviders(<Card card={mockCard} index={0} />);

    const cardDiv = screen.getByRole('button', { name: /open card details/i });
    fireEvent.pointerDown(cardDiv, { clientX: 0, clientY: 0 });
    fireEvent.click(cardDiv, { clientX: 10, clientY: 10 });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  describe('Card description preview', () => {
    it('does not show description preview when description exists (removed from tile for performance)', () => {
      const cardWithDesc = { ...mockCard, description: 'A task description' };
      renderWithProviders(<Card card={cardWithDesc} index={0} />);

      expect(screen.queryByText('A task description')).not.toBeInTheDocument();
    });

    it('does not show description preview when description is null', () => {
      renderWithProviders(<Card card={mockCard} index={0} />);

      expect(screen.queryByText(/A task description/i)).not.toBeInTheDocument();
    });

    it('does not show description preview when description is whitespace-only', () => {
      const cardWithSpaces = { ...mockCard, description: '   ' };
      renderWithProviders(<Card card={cardWithSpaces} index={0} />);

      expect(screen.queryByText(/\s{3}/)).not.toBeInTheDocument();
    });
  });

  describe('Card labels display', () => {
    it('shows label badges when card has labels', () => {
      const cardWithLabels = {
        ...mockCard,
        labels: [
          { id: 1, name: 'Bug', color: 'red', created_at: '2024-01-01', updated_at: '2024-01-01' },
          { id: 2, name: 'Feature', color: 'green', created_at: '2024-01-01', updated_at: '2024-01-01' },
        ],
      };
      renderWithProviders(<Card card={cardWithLabels} index={0} />);

      expect(screen.getByText('Bug')).toBeInTheDocument();
      expect(screen.getByText('Feature')).toBeInTheDocument();
    });

    it('does not show label section when card has no labels', () => {
      renderWithProviders(<Card card={mockCard} index={0} />);

      expect(screen.queryByText('Bug')).not.toBeInTheDocument();
    });

    it('applies correct background color to label badges', () => {
      const cardWithLabels = {
        ...mockCard,
        labels: [
          { id: 1, name: 'Bug', color: 'red', created_at: '2024-01-01', updated_at: '2024-01-01' },
        ],
      };
      renderWithProviders(<Card card={cardWithLabels} index={0} />);

      const badge = screen.getByText('Bug');
      expect(badge.className).toContain(LABEL_COLOR_CLASS_MAP.red);
    });
  });

  describe('Card menu and deletion', () => {
    it('shows card menu trigger button', () => {
      renderWithProviders(<Card card={mockCard} index={0} />);
      const menuButton = screen.getByRole('button', { name: /card menu/i });
      expect(menuButton).toBeInTheDocument();
    });

    it('calls delete mutation when confirming delete', async () => {
      const user = userEvent.setup();
      mockDeleteMutate.mockImplementation((id, opts) => {
        opts?.onSuccess?.();
      });

      renderWithProviders(<Card card={mockCard} index={0} />);

      const card = screen.getByText('Test Card').closest('div')!;
      await user.hover(card);

      const menuButton = screen.getByRole('button', { name: /card menu/i });
      await user.click(menuButton);

      const deleteItem = await screen.findByRole('menuitem', { name: /delete/i });
      await user.click(deleteItem);

      const confirmBtn = await screen.findByRole('button', { name: /^delete$/i });
      await user.click(confirmBtn);

      await waitFor(() => {
        expect(mockDeleteMutate).toHaveBeenCalledWith(
          mockCard.id,
          expect.objectContaining({
            onSuccess: expect.any(Function),
            onError: expect.any(Function),
          })
        );
      });
    });

    describe('Card deletion undo', () => {
      it('shows undo action in toast after successful delete', async () => {
        const user = userEvent.setup();
        mockDeleteMutate.mockImplementation((id, opts) => {
          opts?.onSuccess?.();
        });

        renderWithProviders(<Card card={mockCard} index={0} />);

        const card = screen.getByText('Test Card').closest('div')!;
        await user.hover(card);

        const menuButton = screen.getByRole('button', { name: /card menu/i });
        await user.click(menuButton);

        const deleteItem = await screen.findByRole('menuitem', { name: /delete/i });
        await user.click(deleteItem);

        const confirmBtn = await screen.findByRole('button', { name: /^delete$/i });
        await user.click(confirmBtn);

        await waitFor(() => {
          expect(mockToast).toHaveBeenCalledWith(
            expect.objectContaining({
              title: 'Card deleted',
              type: 'destructive',
              action: expect.objectContaining({
                label: 'Undo',
              }),
            })
          );
        });
      });

      it('clicking Undo in toast calls createCardMutation with card data', async () => {
        const user = userEvent.setup();
        let undoAction: (() => void) | undefined;

        mockDeleteMutate.mockImplementation((id, opts) => {
          opts?.onSuccess?.();
        });

        mockToast.mockImplementation((args) => {
          if (args.action?.onClick) {
            undoAction = args.action.onClick;
          }
        });

        mockCreateMutateAsync.mockResolvedValue({ data: { ...mockCard, id: 2 } });

        renderWithProviders(<Card card={mockCard} index={0} />);

        const card = screen.getByText('Test Card').closest('div')!;
        await user.hover(card);

        const menuButton = screen.getByRole('button', { name: /card menu/i });
        await user.click(menuButton);

        const deleteItem = await screen.findByRole('menuitem', { name: /delete/i });
        await user.click(deleteItem);

        const confirmBtn = await screen.findByRole('button', { name: /^delete$/i });
        await user.click(confirmBtn);

        await waitFor(() => {
          expect(undoAction).toBeDefined();
        });

        await undoAction!();

        await waitFor(() => {
          expect(mockCreateMutateAsync).toHaveBeenCalledWith({
            title: mockCard.title,
            column_id: mockCard.column_id,
            position: mockCard.position,
          });
        });

        expect(mockToast).toHaveBeenCalledWith(
          expect.objectContaining({ title: 'Card restored to original position', type: 'success' })
        );
      });

      it('shows error toast when delete fails', async () => {
        const user = userEvent.setup();
        mockDeleteMutate.mockImplementation((id, opts) => {
          opts?.onError?.(new Error('Delete failed'));
        });

        renderWithProviders(<Card card={mockCard} index={0} />);

        const card = screen.getByText('Test Card').closest('div')!;
        await user.hover(card);

        const menuButton = screen.getByRole('button', { name: /card menu/i });
        await user.click(menuButton);

        const deleteItem = await screen.findByRole('menuitem', { name: /delete/i });
        await user.click(deleteItem);

        const confirmBtn = await screen.findByRole('button', { name: /^delete$/i });
        await user.click(confirmBtn);

        await waitFor(() => {
          expect(mockToast).toHaveBeenCalledWith(
            expect.objectContaining({ title: 'Failed to delete card', type: 'error' })
          );
        });
      });

      it('shows error toast when undo fails', async () => {
        const user = userEvent.setup();
        let undoAction: (() => void) | undefined;

        mockDeleteMutate.mockImplementation((id, opts) => {
          opts?.onSuccess?.();
        });

        mockToast.mockImplementation((args) => {
          if (args.action?.onClick) {
            undoAction = args.action.onClick;
          }
        });

        mockCreateMutateAsync.mockRejectedValue(new Error('Create failed'));

        renderWithProviders(<Card card={mockCard} index={0} />);

        const card = screen.getByText('Test Card').closest('div')!;
        await user.hover(card);

        const menuButton = screen.getByRole('button', { name: /card menu/i });
        await user.click(menuButton);

        const deleteItem = await screen.findByRole('menuitem', { name: /delete/i });
        await user.click(deleteItem);

        const confirmBtn = await screen.findByRole('button', { name: /^delete$/i });
        await user.click(confirmBtn);

        await waitFor(() => {
          expect(undoAction).toBeDefined();
        });

        await undoAction!();

        await waitFor(() => {
          expect(mockToast).toHaveBeenCalledWith(
            expect.objectContaining({ title: 'Failed to restore card', type: 'error' })
          );
        });
      });
    });
  });

  it('applies animate-slide-up class when isNew is true', () => {
    renderWithProviders(<Card card={mockCard} index={0} isNew={true} />);
    const cardDiv = screen.getByRole('button', { name: /open card details/i });
    expect(cardDiv.className).toContain('animate-slide-up');
  });
});
