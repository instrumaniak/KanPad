import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CardDetailPanel } from './card-detail-panel';
import type { Card } from './cards.api';

const mockMutate = vi.fn();
const mockMutateObj = {
  mutate: mockMutate,
  mutateAsync: vi.fn(),
  isPending: false,
};

const mockToast = vi.fn();

const mockUseCardState = vi.hoisted(() => ({
  data: undefined as Card | undefined,
  isLoading: false,
  isError: false,
  refetch: vi.fn(),
}));

vi.mock('./use-cards', () => ({
  useUpdateCard: () => mockMutateObj,
  useAssignCardLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useRemoveCardLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useCard: () => mockUseCardState,
}));

vi.mock('../labels/use-labels', () => ({
  useLabels: () => ({ data: [], isLoading: false }),
  useCreateLabel: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: mockToast }),
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

describe('CardDetailPanel', () => {
  const mockCard = {
    id: 1,
    title: 'Test Card',
    column_id: 1,
    position: 0,
    description: 'A description',
    due_date: '2026-07-20T00:00:00.000Z',
    checklists: [
      {
        id: 10,
        title: 'API Checklist',
        card_id: 1,
        created_at: '2024-01-01',
        updated_at: '2024-01-01',
        items: [
          {
            id: 100,
            text: 'Fetched from API',
            is_completed: true,
            checklist_id: 10,
            position: 0,
            created_at: '2024-01-01',
            updated_at: '2024-01-01',
          },
        ],
      },
    ],
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };

  const mockCardNoExtras = {
    id: 2,
    title: 'Simple Card',
    column_id: 1,
    position: 1,
    description: null,
    due_date: null,
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };

  beforeEach(() => {
    vi.clearAllMocks();
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
    mockUseCardState.data = { ...mockCard, description: 'A description' };
    mockUseCardState.isLoading = false;
    mockUseCardState.isError = false;
    mockUseCardState.refetch = vi.fn();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('loading state', () => {
    it('shows skeleton placeholders when loading and hides card content', () => {
      mockUseCardState.isLoading = true;
      mockUseCardState.data = undefined;
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
      expect(screen.queryByDisplayValue('Test Card')).not.toBeInTheDocument();
      expect(screen.getByLabelText('Card details')).toBeInTheDocument();
    });
  });

  describe('error state', () => {
    it('shows error message and retry button when fetch fails', () => {
      mockUseCardState.isError = true;
      mockUseCardState.data = undefined;
      mockUseCardState.isLoading = false;
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
      expect(screen.getByText('Failed to load card details.')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
      expect(screen.queryByDisplayValue('Test Card')).not.toBeInTheDocument();
    });

    it('calls refetch when retry button is clicked', () => {
      const refetch = vi.fn();
      mockUseCardState.isError = true;
      mockUseCardState.data = undefined;
      mockUseCardState.isLoading = false;
      mockUseCardState.refetch = refetch;
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
      fireEvent.click(screen.getByRole('button', { name: /retry/i }));
      expect(refetch).toHaveBeenCalled();
    });
  });

  it('renders card title in input', () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
    expect(screen.getByDisplayValue('Test Card')).toBeInTheDocument();
  });

  it('renders description textarea with card description', async () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByDisplayValue('A description')).toBeInTheDocument();
    });
  });

  it('shows formatted due date when set', () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
    expect(screen.getByText(/Jul 20, 2026/)).toBeInTheDocument();
  });

  it('shows no due date placeholder when due_date is null', () => {
    mockUseCardState.data = { ...mockCardNoExtras };
    renderWithProviders(<CardDetailPanel card={mockCardNoExtras} open={true} onOpenChange={vi.fn()} />);
    expect(screen.getByText('No due date')).toBeInTheDocument();
  });

  it('shows labels section', () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);
    expect(screen.getByText('Add labels')).toBeInTheDocument();
  });

  it('shows the add checklist button and opens the form', async () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByRole('button', { name: /\+ Add Checklist/i })).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /\+ Add Checklist/i }));

    expect(screen.getByPlaceholderText('Checklist title...')).toBeInTheDocument();
  });

  it('renders checklist data from the api response', () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByText('API Checklist')).toBeInTheDocument();
    expect(screen.getByText('Fetched from API')).toBeInTheDocument();
    expect(screen.getByText('1/1 (100%)')).toBeInTheDocument();
  });

  it('saves title on blur when changed', async () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    const titleInput = screen.getByLabelText('Card title');
    await userEvent.clear(titleInput);
    await userEvent.type(titleInput, 'Updated Title');
    fireEvent.blur(titleInput);

    await waitFor(() => {
      expect(mockMutate).toHaveBeenCalledWith(
        { id: 1, data: { title: 'Updated Title' } },
        expect.objectContaining({ onSettled: expect.any(Function) }),
      );
    });
  });

  it('does not save title on blur when unchanged', () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    const titleInput = screen.getByLabelText('Card title');
    fireEvent.blur(titleInput);

    expect(mockMutate).not.toHaveBeenCalled();
  });

  it('reverts title to original when emptied and blurred', () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    const titleInput = screen.getByLabelText('Card title') as HTMLInputElement;
    fireEvent.change(titleInput, { target: { value: '' } });
    fireEvent.blur(titleInput);

    expect(titleInput.value).toBe('Test Card');
    expect(mockMutate).not.toHaveBeenCalled();
  });

  it('saves description on blur when changed', async () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    const textarea = screen.getByLabelText('Card description');
    await userEvent.clear(textarea);
    await userEvent.type(textarea, 'New description');
    fireEvent.blur(textarea);

    await waitFor(() => {
      expect(mockMutate).toHaveBeenCalledWith(
        { id: 1, data: { description: 'New description' } },
        expect.objectContaining({ onSettled: expect.any(Function) }),
      );
    });
  });

  it('saves null description when cleared and blurred', async () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    const textarea = screen.getByLabelText('Card description');
    await userEvent.clear(textarea);
    fireEvent.blur(textarea);

    await waitFor(() => {
      expect(mockMutate).toHaveBeenCalledWith(
        { id: 1, data: { description: undefined } },
        expect.objectContaining({ onSettled: expect.any(Function) }),
      );
    });
  });

  it('shows saving indicator during title mutation', async () => {
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

    const titleInput = screen.getByLabelText('Card title');
    await userEvent.type(titleInput, 'Updated Title');
    fireEvent.blur(titleInput);

    expect(screen.getByText('Saving...')).toBeInTheDocument();
  });

  it('updates local state when card prop changes', async () => {
    const { rerender } = renderWithProviders(
      <CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />,
    );

    expect(screen.getByDisplayValue('Test Card')).toBeInTheDocument();

    rerender(
      <QueryClientProvider client={new QueryClient()}>
        <CardDetailPanel card={{ ...mockCard, title: 'New Title' }} open={true} onOpenChange={vi.fn()} />
      </QueryClientProvider>,
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue('New Title')).toBeInTheDocument();
    });
  });

  it('calls onOpenChange when dialog is dismissed', async () => {
    const onOpenChange = vi.fn();
    renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={onOpenChange} />);

    const closeButton = screen.getByRole('button', { name: /close/i });
    await userEvent.click(closeButton);

    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  describe('Markdown description editor', () => {
    beforeAll(async () => {
      await import('./markdown-preview');
    });

    it('defaults to Edit tab with textarea visible', async () => {
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

      expect(screen.getByRole('tab', { name: /edit/i })).toHaveAttribute('data-state', 'active');
      expect(screen.getByRole('tab', { name: /preview/i })).toHaveAttribute('data-state', 'inactive');
      await waitFor(() => {
        expect(screen.getByLabelText('Card description')).toBeInTheDocument();
        expect(screen.getByDisplayValue('A description')).toBeInTheDocument();
      });
    });

    it('switches to Preview tab and renders markdown', async () => {
      mockUseCardState.data = { ...mockCard, description: '# Hello\n\n- item 1\n- item 2' };
      const markdownCard = { ...mockCard, description: '# Hello\n\n- item 1\n- item 2' };
      renderWithProviders(<CardDetailPanel card={markdownCard} open={true} onOpenChange={vi.fn()} />);

      const previewTab = screen.getByRole('tab', { name: /preview/i });
      await userEvent.click(previewTab);

      await waitFor(() => {
        expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Hello');
      });
      expect(screen.getByRole('list')).toBeInTheDocument();
    });

    it('switches back to Edit tab and preserves textarea content', async () => {
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

      const previewTab = screen.getByRole('tab', { name: /preview/i });
      await userEvent.click(previewTab);
      await waitFor(() => {
        expect(screen.queryByLabelText('Card description')).not.toBeInTheDocument();
      });

      const editTab = screen.getByRole('tab', { name: /edit/i });
      await userEvent.click(editTab);

      await waitFor(() => {
        expect(screen.getByLabelText('Card description')).toHaveValue('A description');
      });
    });

    it('auto-saves raw markdown on blur in Edit mode', async () => {
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

      const textarea = screen.getByLabelText('Card description');
      await userEvent.clear(textarea);
      await userEvent.type(textarea, '# Updated');
      fireEvent.blur(textarea);

      await waitFor(() => {
        expect(mockMutate).toHaveBeenCalledWith(
          { id: 1, data: { description: '# Updated' } },
          expect.objectContaining({ onSettled: expect.any(Function) }),
        );
      });
    });

    it('shows "Nothing to preview" when description is empty in Preview mode', async () => {
      mockUseCardState.data = { ...mockCard, description: '' };
      const emptyCard = { ...mockCard, description: '' };
      renderWithProviders(<CardDetailPanel card={emptyCard} open={true} onOpenChange={vi.fn()} />);

      const previewTab = screen.getByRole('tab', { name: /preview/i });
      await userEvent.click(previewTab);

      await waitFor(() => {
        expect(screen.getByText('Nothing to preview')).toBeInTheDocument();
      });
    });

    it('sanitizes XSS payload in Preview mode', async () => {
      const xssDescription = '<script>alert("xss")</script>\n\n[link](javascript:alert(1))';
      mockUseCardState.data = { ...mockCard, description: xssDescription };
      const xssCard = { ...mockCard, description: xssDescription };
      renderWithProviders(<CardDetailPanel card={xssCard} open={true} onOpenChange={vi.fn()} />);

      const previewTab = screen.getByRole('tab', { name: /preview/i });
      await userEvent.click(previewTab);

      await waitFor(() => {
        expect(screen.queryByText('alert("xss")')).not.toBeInTheDocument();
      });

      const previewPanel = screen.getByRole('tabpanel');
      const html = previewPanel.innerHTML;
      expect(html).not.toContain('<script>');
      expect(html).not.toContain('javascript:');
    });

    it('sanitizes dangerous HTML tags in Preview mode', async () => {
      const payload = '<img src=x onerror=alert(1)>\n\n<iframe src="javascript:alert(1)">\n\n<svg onload=alert(1)>';
      mockUseCardState.data = { ...mockCard, description: payload };
      const xssCard = { ...mockCard, description: payload };
      renderWithProviders(<CardDetailPanel card={xssCard} open={true} onOpenChange={vi.fn()} />);

      const previewTab = screen.getByRole('tab', { name: /preview/i });
      await userEvent.click(previewTab);

      const previewPanel = screen.getByRole('tabpanel');
      const html = previewPanel.innerHTML;
      expect(html).not.toContain('<iframe');
      expect(html).not.toContain('onerror');
      expect(html).not.toContain('<svg');
    });

    it('has accessible tab list with aria-label', () => {
      renderWithProviders(<CardDetailPanel card={mockCard} open={true} onOpenChange={vi.fn()} />);

      expect(screen.getByLabelText('Description mode')).toBeInTheDocument();
    });
  });
});
