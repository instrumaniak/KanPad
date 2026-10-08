import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { BoardFormModal } from './board-form-modal';
import { ToastProvider } from '@/components/ui/toast-provider';

const mockCreateMutate = vi.fn();
const mockUpdateMutate = vi.fn();

vi.mock('./use-boards', () => ({
  useCreateBoard: () => ({
    mutateAsync: mockCreateMutate,
    isPending: false,
  }),
  useUpdateBoard: () => ({
    mutateAsync: mockUpdateMutate,
    isPending: false,
  }),
}));

vi.mock('../projects/use-projects', () => ({
  useProjects: () => ({
    data: {
      data: [
        { id: 1, name: 'Project 1' },
        { id: 2, name: 'Project 2' },
      ],
    },
  }),
}));

const renderWithRouter = (ui: React.ReactElement) => {
  return render(
    <MemoryRouter>
      <ToastProvider>{ui}</ToastProvider>
    </MemoryRouter>
  );
};

describe('BoardFormModal create mode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateMutate.mockResolvedValue({ data: { id: 1, name: 'Test Board' } });
    mockUpdateMutate.mockResolvedValue({});
  });

  it('renders modal with form fields', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('Create new board')).toBeInTheDocument();
    expect(screen.getByLabelText(/Board name/i)).toBeInTheDocument();
    expect(screen.getByText('Project (optional)')).toBeInTheDocument();
  });

  it('shows Cancel and Create Board buttons', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Create Board/i })).toBeInTheDocument();
  });

  it('starts with empty name', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByLabelText(/Board name/i)).toHaveValue('');
  });

  it('calls onOpenChange when Cancel is clicked', () => {
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('disables submit when name is empty', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByRole('button', { name: /Create Board/i })).toBeDisabled();
  });

  it('enables submit when name has content', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    fireEvent.change(screen.getByLabelText(/Board name/i), { target: { value: 'My Board' } });
    expect(screen.getByRole('button', { name: /Create Board/i })).toBeEnabled();
  });

  it('renders project dropdown with options', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByText('No project')).toBeInTheDocument();
    expect(screen.getByText('Project 1')).toBeInTheDocument();
    expect(screen.getByText('Project 2')).toBeInTheDocument();
  });

  it('submits trimmed name with project', async () => {
    const onOpenChange = vi.fn();
    const onSuccess = vi.fn();
    renderWithRouter(
      <BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} onSuccess={onSuccess} />
    );

    fireEvent.change(screen.getByLabelText(/Board name/i), { target: { value: '  My Board  ' } });
    fireEvent.click(screen.getByRole('button', { name: /Create Board/i }));

    await waitFor(() => expect(mockCreateMutate).toHaveBeenCalledWith({
      name: 'My Board',
      project_id: null,
    }));
    expect(onSuccess).toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

describe('BoardFormModal edit mode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateMutate.mockResolvedValue({ data: { id: 1, name: 'Test Board' } });
    mockUpdateMutate.mockResolvedValue({});
  });

  const initialBoard = { id: 7, name: 'Old Name', project_id: 1 as number | null };

  it('renders edit title and prefills values', () => {
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    expect(screen.getByText('Edit board')).toBeInTheDocument();
    expect(screen.getByLabelText(/Board name/i)).toHaveValue('Old Name');
    expect(screen.getByRole('button', { name: /Save changes/i })).toBeInTheDocument();
  });

  it('disables save when pristine', () => {
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    expect(screen.getByRole('button', { name: /Save changes/i })).toBeDisabled();
  });

  it('enables save when name changes and sends diff-only payload', async () => {
    const onOpenChange = vi.fn();
    const onSuccess = vi.fn();
    renderWithRouter(
      <BoardFormModal
        mode="edit"
        open={true}
        onOpenChange={onOpenChange}
        onSuccess={onSuccess}
        initialBoard={initialBoard}
      />
    );

    fireEvent.change(screen.getByLabelText(/Board name/i), { target: { value: 'New Name' } });
    const saveButton = screen.getByRole('button', { name: /Save changes/i });
    expect(saveButton).toBeEnabled();
    fireEvent.click(saveButton);

    await waitFor(() =>
      expect(mockUpdateMutate).toHaveBeenCalledWith({
        id: 7,
        data: { name: 'New Name', project_id: undefined },
      })
    );
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(onSuccess).toHaveBeenCalled();
  });

  it('sends project-only diff when only project changes', async () => {
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    const projectSelect = screen.getByLabelText(/Project \(optional\)/i);
    fireEvent.change(projectSelect, { target: { value: '2' } });
    fireEvent.click(screen.getByRole('button', { name: /Save changes/i }));

    await waitFor(() =>
      expect(mockUpdateMutate).toHaveBeenCalledWith({
        id: 7,
        data: { name: undefined, project_id: 2 },
      })
    );
  });

  it('closes without mutating when pristine submit is blocked', () => {
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    expect(mockUpdateMutate).not.toHaveBeenCalled();
  });
});
