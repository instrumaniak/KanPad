import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { TagPicker } from './tag-picker';

const renderWithProviders = (ui: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>{ui}</ToastProvider>
    </QueryClientProvider>,
  );
};

const mockTagsData = {
  data: [
    { id: 1, name: 'frontend', color: 'teal', user_id: 1, created_at: '2024-01-01' },
    { id: 2, name: 'backend', color: 'blue', user_id: 1, created_at: '2024-01-01' },
    { id: 3, name: 'bug', color: 'rose', user_id: 1, created_at: '2024-01-01' },
  ],
};

const mockUseTags = vi.fn();
const mockCreateMutate = vi.fn();

vi.mock('./use-tags', () => ({
  useTags: () => mockUseTags(),
  useCreateTag: () => ({ mutate: mockCreateMutate, isPending: false }),
}));

vi.mock('./tag-badge', () => ({
  TagBadge: ({ name, color, onRemove }: { name: string; color: string; onRemove?: () => void }) => (
    <span data-testid={`tag-badge-${name}`}>
      {name}
      {onRemove && <button data-testid={`remove-${name}`} onClick={onRemove}>x</button>}
    </span>
  ),
}));

vi.mock('@/components/ui/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}));

describe('TagPicker', () => {
  const onTagsChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseTags.mockReturnValue({ data: mockTagsData });
  });

  it('renders add tags button', () => {
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    expect(screen.getByText('Add tags')).toBeInTheDocument();
  });

  it('opens dropdown when add tags is clicked', () => {
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    expect(screen.getByPlaceholderText('Search tags...')).toBeInTheDocument();
  });

  it('renders available tags in dropdown', () => {
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    expect(screen.getByText('frontend')).toBeInTheDocument();
    expect(screen.getByText('backend')).toBeInTheDocument();
    expect(screen.getByText('bug')).toBeInTheDocument();
  });

  it('filters tags by search input', () => {
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    const searchInput = screen.getByPlaceholderText('Search tags...');
    fireEvent.change(searchInput, { target: { value: 'front' } });
    expect(screen.getByText('frontend')).toBeInTheDocument();
    expect(screen.queryByText('backend')).not.toBeInTheDocument();
  });

  it('calls onTagsChange when tag is selected', () => {
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    fireEvent.click(screen.getByText('frontend'));
    expect(onTagsChange).toHaveBeenCalledWith([1]);
  });

  it('shows selected tags as badges', () => {
    renderWithProviders(<TagPicker selectedTagIds={[1]} onTagsChange={onTagsChange} />);
    expect(screen.getByTestId('tag-badge-frontend')).toBeInTheDocument();
  });

  it('removes tag when remove is clicked', () => {
    renderWithProviders(<TagPicker selectedTagIds={[1]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByTestId('remove-frontend'));
    expect(onTagsChange).toHaveBeenCalledWith([]);
  });

  it('filters out selected tags from dropdown', () => {
    renderWithProviders(<TagPicker selectedTagIds={[1]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    const dropdownButtons = screen.getAllByRole('button');
    const frontendBtn = dropdownButtons.find(b => b.textContent === 'frontend');
    expect(frontendBtn).toBeUndefined();
    expect(screen.getByText('backend')).toBeInTheDocument();
  });

  it('shows create tag form when search yields no results', () => {
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    const searchInput = screen.getByPlaceholderText('Search tags...');
    fireEvent.change(searchInput, { target: { value: 'newtag' } });
    expect(screen.getByPlaceholderText('New tag name...')).toBeInTheDocument();
  });

  it('creates a new tag', () => {
    mockCreateMutate.mockImplementation((_data: unknown, { onSuccess }: { onSuccess: (result: { data: { id: number } }) => void }) => {
      onSuccess({ data: { id: 4 } });
    });
    renderWithProviders(<TagPicker selectedTagIds={[]} onTagsChange={onTagsChange} />);
    fireEvent.click(screen.getByText('Add tags'));
    const searchInput = screen.getByPlaceholderText('Search tags...');
    fireEvent.change(searchInput, { target: { value: 'newtag' } });
    const nameInput = screen.getByPlaceholderText('New tag name...');
    fireEvent.change(nameInput, { target: { value: 'newtag' } });
    fireEvent.click(screen.getByText(/Create/));
    expect(mockCreateMutate).toHaveBeenCalled();
  });
});
