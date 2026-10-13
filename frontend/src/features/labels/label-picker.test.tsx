import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { LabelPicker } from './label-picker';
import { LABEL_COLOR_CLASS_MAP } from './label-colors';

const mockAssignMutate = vi.fn();
const mockRemoveMutate = vi.fn();
const mockCreateMutate = vi.fn();

const mockLabels = [
  { id: 1, name: 'Bug', color: 'red', created_at: '2024-01-01', updated_at: '2024-01-01' },
  { id: 2, name: 'Feature', color: 'green', created_at: '2024-01-01', updated_at: '2024-01-01' },
  { id: 3, name: 'Enhancement', color: 'blue', created_at: '2024-01-01', updated_at: '2024-01-01' },
];

const manyMockLabels = Array.from({ length: 24 }, (_, index) => ({
  id: index + 1,
  name: `Label ${index + 1}`,
  color: ['red', 'green', 'blue', 'yellow', 'orange', 'purple'][index % 6],
  created_at: '2024-01-01',
  updated_at: '2024-01-01',
}));

let labelsData = mockLabels;

vi.mock('../labels/use-labels', () => ({
  useLabels: () => ({
    data: { data: labelsData, page: 1, limit: 50, total: labelsData.length },
    isLoading: false,
  }),
  useCreateLabel: () => ({ mutate: mockCreateMutate, isPending: false }),
}));

vi.mock('../cards/use-cards', () => ({
  useAssignCardLabel: () => ({ mutate: mockAssignMutate, isPending: false }),
  useRemoveCardLabel: () => ({ mutate: mockRemoveMutate, isPending: false }),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
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

describe('LabelPicker', () => {
  const mockCard = {
    id: 1,
    title: 'Test Card',
    column_id: 1,
    position: 0,
    description: null,
    due_date: null,
    labels: [mockLabels[0]],
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };

  const mockCardNoLabels = {
    ...mockCard,
    labels: [],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    labelsData = mockLabels;
  });

  it('renders a trigger button showing label count', () => {
    renderWithProviders(<LabelPicker card={mockCard} />);
    expect(screen.getByText('1 label')).toBeInTheDocument();
  });

  it('renders trigger button with "Add labels" when no labels', () => {
    renderWithProviders(<LabelPicker card={mockCardNoLabels} />);
    expect(screen.getByText('Add labels')).toBeInTheDocument();
  });

  it('shows assigned label with higher opacity when popover is open', async () => {
    const user = userEvent.setup();
    renderWithProviders(<LabelPicker card={mockCard} />);
    await user.click(screen.getByText('1 label'));

    await waitFor(() => {
      const bugButton = screen.getByRole('button', { name: /remove bug/i });
      expect(bugButton.className).toContain('opacity-100');
    });
  });

  it('shows unassigned labels with lower opacity when popover is open', async () => {
    const user = userEvent.setup();
    renderWithProviders(<LabelPicker card={mockCard} />);
    await user.click(screen.getByText('1 label'));

    await waitFor(() => {
      const featureButton = screen.getByRole('button', { name: /add feature/i });
      expect(featureButton.className).toContain('opacity-50');
    });
  });

  it('calls assign mutation when clicking unassigned label', async () => {
    const user = userEvent.setup();
    renderWithProviders(<LabelPicker card={mockCardNoLabels} />);
    await user.click(screen.getByText('Add labels'));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /add feature/i })).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: /add feature/i }));

    await waitFor(() => {
      expect(mockAssignMutate).toHaveBeenCalledWith(
        { cardId: 1, labelId: 2 },
        expect.any(Object),
      );
    });
  });

  it('calls remove mutation when clicking assigned label', async () => {
    const user = userEvent.setup();
    renderWithProviders(<LabelPicker card={mockCard} />);
    await user.click(screen.getByText('1 label'));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /remove bug/i })).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: /remove bug/i }));

    await waitFor(() => {
      expect(mockRemoveMutate).toHaveBeenCalledWith(
        { cardId: 1, labelId: 1 },
        expect.any(Object),
      );
    });
  });

  it('applies correct Tailwind color class', async () => {
    const user = userEvent.setup();
    renderWithProviders(<LabelPicker card={mockCard} />);
    await user.click(screen.getByText('1 label'));

    await waitFor(() => {
      const bugButton = screen.getByRole('button', { name: /remove bug/i });
      expect(bugButton.className).toContain(LABEL_COLOR_CLASS_MAP.red);
    });
  });

  it('has accessibility attributes on label buttons', async () => {
    const user = userEvent.setup();
    renderWithProviders(<LabelPicker card={mockCard} />);
    await user.click(screen.getByText('1 label'));

    await waitFor(() => {
      const bugButton = screen.getByRole('button', { name: /remove bug/i });
      expect(bugButton).toHaveAttribute('aria-pressed', 'true');
      expect(bugButton).toHaveAttribute('aria-label', 'Remove Bug');
    });
  });

  it('bounds and scrolls the popover content when many labels are available', async () => {
    const user = userEvent.setup();
    labelsData = manyMockLabels;

    renderWithProviders(<LabelPicker card={mockCardNoLabels} />);
    await user.click(screen.getByText('Add labels'));

    await waitFor(() => {
      const popoverContent = document.querySelector('[data-state="open"][data-side]');
      expect(popoverContent).toBeTruthy();
      expect(popoverContent).toHaveClass('max-h-[var(--radix-popper-available-height)]');
      expect(popoverContent).toHaveClass('overflow-y-auto');
      expect(screen.getByRole('button', { name: /create new label/i })).toBeInTheDocument();
    });
  });
});
