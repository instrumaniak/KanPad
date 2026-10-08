import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { BoardFormModal } from './board-form-modal';
import { ToastProvider } from '@/components/ui/toast-provider';

const mockCreateMutate = vi.fn();
const mockUpdateMutate = vi.fn();
const mockCreateProjectMutate = vi.fn();

const { mockNavigate, mockState } = vi.hoisted(() => ({
  mockNavigate: vi.fn(),
  mockState: { boardsPending: false, projectsPending: false },
}));

vi.mock('react-router-dom', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock('./use-boards', () => ({
  useCreateBoard: () => ({
    mutateAsync: mockCreateMutate,
    isPending: mockState.boardsPending,
  }),
  useUpdateBoard: () => ({
    mutateAsync: mockUpdateMutate,
    isPending: mockState.boardsPending,
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
  useCreateProject: () => ({
    mutateAsync: mockCreateProjectMutate,
    isPending: mockState.projectsPending,
  }),
}));

beforeEach(() => {
  mockState.boardsPending = false;
  mockState.projectsPending = false;
});

const renderWithRouter = (ui: React.ReactElement) => {
  return render(
    <MemoryRouter>
      <ToastProvider>{ui}</ToastProvider>
    </MemoryRouter>
  );
};

async function selectProjectOption(user: UserEvent, optionName: string) {
  await user.click(screen.getByRole('combobox', { name: /project \(optional\)/i }));
  const option = await screen.findByRole('option', { name: optionName });
  await user.click(option);
}

describe('BoardFormModal create mode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateMutate.mockResolvedValue({ data: { id: 1, name: 'Test Board' } });
    mockUpdateMutate.mockResolvedValue({});
    mockCreateProjectMutate.mockResolvedValue({ data: { id: 99, name: 'New Project' } });
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

  it('calls onOpenChange when Cancel is clicked', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('disables submit when name is empty', () => {
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByRole('button', { name: /Create Board/i })).toBeDisabled();
  });

  it('enables submit when name has content', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    await user.type(screen.getByLabelText(/Board name/i), 'My Board');
    expect(screen.getByRole('button', { name: /Create Board/i })).toBeEnabled();
  });

  it('renders project select with options', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    const trigger = screen.getByRole('combobox', { name: /project \(optional\)/i });
    expect(trigger).toHaveTextContent('No project');

    await user.click(trigger);
    expect(await screen.findByRole('option', { name: 'No project' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Project 1' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Project 2' })).toBeInTheDocument();
  });

  it('submits trimmed name with project', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const onSuccess = vi.fn();
    renderWithRouter(
      <BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} onSuccess={onSuccess} />
    );

    await user.type(screen.getByLabelText(/Board name/i), '  My Board  ');
    await selectProjectOption(user, 'Project 1');
    await user.click(screen.getByRole('button', { name: /Create Board/i }));

    await waitFor(() =>
      expect(mockCreateMutate).toHaveBeenCalledWith({
        name: 'My Board',
        project_id: 1,
      })
    );
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
      expect(mockNavigate).toHaveBeenCalledWith('/board/1');
    });
  });

  it('submits with null project when No project is selected', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    await user.type(screen.getByLabelText(/Board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /Create Board/i }));

    await waitFor(() =>
      expect(mockCreateMutate).toHaveBeenCalledWith({
        name: 'My Board',
        project_id: null,
      })
    );
    await waitFor(() => {
      expect(mockCreateProjectMutate).not.toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
      expect(mockNavigate).toHaveBeenCalledWith('/board/1');
    });
  });
});

describe('BoardFormModal inline new project', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateMutate.mockResolvedValue({ data: { id: 1, name: 'Test Board' } });
    mockUpdateMutate.mockResolvedValue({});
    mockCreateProjectMutate.mockResolvedValue({ data: { id: 99, name: 'New Project' } });
  });

  it('toggles between select and new project modes', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    expect(screen.getByRole('combobox', { name: /project \(optional\)/i })).toBeInTheDocument();
    const toggle = screen.getByRole('button', { name: /new project/i });
    expect(toggle).toHaveAttribute('aria-pressed', 'false');

    await user.click(toggle);
    expect(screen.getByText('New Project Name')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Enter project name...')).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: /project \(optional\)/i })
    ).not.toBeInTheDocument();
    const toggleBack = screen.getByRole('button', { name: /select existing/i });
    expect(toggleBack).toHaveAttribute('aria-pressed', 'true');

    await user.click(toggleBack);
    expect(screen.getByText('Project (optional)')).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /project \(optional\)/i })).toBeInTheDocument();
  });

  it('preserves the selected project when toggling modes', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    await selectProjectOption(user, 'Project 2');
    expect(screen.getByRole('combobox', { name: /project \(optional\)/i })).toHaveTextContent(
      'Project 2'
    );

    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.click(screen.getByRole('button', { name: /select existing/i }));

    expect(screen.getByRole('combobox', { name: /project \(optional\)/i })).toHaveTextContent(
      'Project 2'
    );
  });

  it('preserves the board name and new project draft when toggling modes', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Draft Project');
    await user.click(screen.getByRole('button', { name: /select existing/i }));

    expect(screen.queryByLabelText(/new project name/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /new project/i }));
    expect(screen.getByLabelText(/new project name/i)).toHaveValue('Draft Project');
    expect(screen.getByLabelText(/board name/i)).toHaveValue('My Board');
  });

  it('disables submit when new project name is empty or whitespace only', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    await user.type(screen.getByLabelText(/Board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));

    expect(screen.getByRole('button', { name: /Create Board/i })).toBeDisabled();

    await user.type(screen.getByLabelText(/new project name/i), 'Fresh Project');
    expect(screen.getByRole('button', { name: /Create Board/i })).toBeEnabled();

    await user.clear(screen.getByLabelText(/new project name/i));
    await user.type(screen.getByLabelText(/new project name/i), '   ');
    expect(screen.getByRole('button', { name: /Create Board/i })).toBeDisabled();
  });

  it('creates the project then the board on submit', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const onSuccess = vi.fn();
    renderWithRouter(
      <BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} onSuccess={onSuccess} />
    );

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), '  Fresh Project  ');
    await user.click(screen.getByRole('button', { name: /create board/i }));

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledWith('Fresh Project'));
    await waitFor(() =>
      expect(mockCreateMutate).toHaveBeenCalledWith({
        name: 'My Board',
        project_id: 99,
      })
    );
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
      expect(mockNavigate).toHaveBeenCalledWith('/board/1');
    });
  });

  it('creates the project only once on rapid double submit', async () => {
    const user = userEvent.setup();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Fresh');

    // Intentionally synchronous: both dispatches must run before microtasks
    // flush so the second hits the submittingRef guard.
    const submit = screen.getByRole('button', { name: /create board/i });
    fireEvent.click(submit);
    fireEvent.click(submit);

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mockCreateMutate).toHaveBeenCalledTimes(1));
  });

  it('keeps the modal open and skips board creation when project creation fails', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    mockCreateProjectMutate.mockRejectedValueOnce(new Error('Project name taken'));
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Duplicate');
    await user.click(screen.getByRole('button', { name: /create board/i }));

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledWith('Duplicate'));
    expect(mockCreateMutate).not.toHaveBeenCalled();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
    expect(await screen.findByText('Failed to create project')).toBeInTheDocument();
  });

  it('reuses the created project when retrying after a board failure', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const onSuccess = vi.fn();
    mockCreateMutate.mockRejectedValueOnce(new Error('Board exploded'));
    renderWithRouter(
      <BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} onSuccess={onSuccess} />
    );

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Fresh');

    const submit = screen.getByRole('button', { name: /create board/i });
    await user.click(submit);

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mockCreateMutate).toHaveBeenCalledTimes(1));
    expect(await screen.findByText('Failed to create board')).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);

    await user.click(submit);

    await waitFor(() => expect(mockCreateMutate).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  });

  it('creates a new project when the name changes after a board failure', async () => {
    const user = userEvent.setup();
    mockCreateMutate.mockRejectedValueOnce(new Error('Board exploded'));
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={vi.fn()} />);

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Fresh');
    await user.click(screen.getByRole('button', { name: /create board/i }));

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mockCreateMutate).toHaveBeenCalledTimes(1));

    await user.clear(screen.getByLabelText(/new project name/i));
    await user.type(screen.getByLabelText(/new project name/i), 'Other');
    await user.click(screen.getByRole('button', { name: /create board/i }));

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(2));
    expect(mockCreateProjectMutate).toHaveBeenLastCalledWith('Other');
    await waitFor(() => expect(mockCreateMutate).toHaveBeenCalledTimes(2));
  });

  it('submits null project when switching back to No project', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await selectProjectOption(user, 'Project 1');
    expect(screen.getByRole('combobox', { name: /project \(optional\)/i })).toHaveTextContent(
      'Project 1'
    );

    await selectProjectOption(user, 'No project');
    await user.click(screen.getByRole('button', { name: /create board/i }));

    await waitFor(() =>
      expect(mockCreateMutate).toHaveBeenCalledWith({
        name: 'My Board',
        project_id: null,
      })
    );
    expect(mockCreateProjectMutate).not.toHaveBeenCalled();
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it('creates nothing when cancelled from new project mode', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    await user.type(screen.getByLabelText(/board name/i), 'My Board');
    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Abandoned');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(mockCreateProjectMutate).not.toHaveBeenCalled();
    expect(mockCreateMutate).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

describe('BoardFormModal edit mode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateMutate.mockResolvedValue({ data: { id: 1, name: 'Test Board' } });
    mockUpdateMutate.mockResolvedValue({});
    mockCreateProjectMutate.mockResolvedValue({ data: { id: 99, name: 'New Project' } });
  });

  const initialBoard = { id: 7, name: 'Old Name', project_id: 1 as number | null };

  it('renders edit title and prefills values', () => {
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    expect(screen.getByText('Edit board')).toBeInTheDocument();
    expect(screen.getByLabelText(/Board name/i)).toHaveValue('Old Name');
    expect(screen.getByRole('combobox', { name: /project \(optional\)/i })).toHaveTextContent(
      'Project 1'
    );
    expect(screen.getByRole('button', { name: /Save changes/i })).toBeInTheDocument();
  });

  it('disables save when pristine', () => {
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    expect(screen.getByRole('button', { name: /Save changes/i })).toBeDisabled();
  });

  it('blocks submit on the disabled pristine button', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithRouter(
      <BoardFormModal
        mode="edit"
        open={true}
        onOpenChange={onOpenChange}
        initialBoard={initialBoard}
      />
    );

    const saveButton = screen.getByRole('button', { name: /Save changes/i });
    expect(saveButton).toBeDisabled();
    await user.click(saveButton);

    expect(mockUpdateMutate).not.toHaveBeenCalled();
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it('closes without mutating on pristine form submit', () => {
    // The Save button is disabled when pristine, so submit the form directly
    // to exercise the pristine early-return branch in handleSubmit.
    const onOpenChange = vi.fn();
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={onOpenChange} initialBoard={initialBoard} />
    );

    const form = screen.getByRole('dialog').querySelector('form');
    expect(form).not.toBeNull();
    if (form) fireEvent.submit(form);

    expect(mockUpdateMutate).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('enables save when name changes and sends diff-only payload', async () => {
    const user = userEvent.setup();
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

    await user.clear(screen.getByLabelText(/Board name/i));
    await user.type(screen.getByLabelText(/Board name/i), 'New Name');
    const saveButton = screen.getByRole('button', { name: /Save changes/i });
    expect(saveButton).toBeEnabled();
    await user.click(saveButton);

    await waitFor(() =>
      expect(mockUpdateMutate).toHaveBeenCalledWith({
        id: 7,
        data: { name: 'New Name', project_id: undefined },
      })
    );
    await waitFor(() => {
      expect(onOpenChange).toHaveBeenCalledWith(false);
      expect(onSuccess).toHaveBeenCalled();
      expect(mockNavigate).not.toHaveBeenCalled();
    });
  });

  it('sends project-only diff when only project changes', async () => {
    const user = userEvent.setup();
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    await selectProjectOption(user, 'Project 2');
    await user.click(screen.getByRole('button', { name: /Save changes/i }));

    await waitFor(() =>
      expect(mockUpdateMutate).toHaveBeenCalledWith({
        id: 7,
        data: { name: undefined, project_id: 2 },
      })
    );
  });

  it('disables save when new project name is whitespace only', async () => {
    const user = userEvent.setup();
    renderWithRouter(
      <BoardFormModal mode="edit" open={true} onOpenChange={vi.fn()} initialBoard={initialBoard} />
    );

    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), '   ');

    expect(screen.getByRole('button', { name: /Save changes/i })).toBeDisabled();
  });

  it('creates a new project then updates the board in edit mode', async () => {
    const user = userEvent.setup();
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

    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Brand New');
    await user.click(screen.getByRole('button', { name: /Save changes/i }));

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledWith('Brand New'));
    await waitFor(() =>
      expect(mockUpdateMutate).toHaveBeenCalledWith({
        id: 7,
        data: { name: undefined, project_id: 99 },
      })
    );
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  });

  it('reuses the created project when retrying an edit after update failure', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const onSuccess = vi.fn();
    mockUpdateMutate.mockRejectedValueOnce(new Error('Update exploded'));
    renderWithRouter(
      <BoardFormModal
        mode="edit"
        open={true}
        onOpenChange={onOpenChange}
        onSuccess={onSuccess}
        initialBoard={initialBoard}
      />
    );

    await user.click(screen.getByRole('button', { name: /new project/i }));
    await user.type(screen.getByLabelText(/new project name/i), 'Brand New');

    const saveButton = screen.getByRole('button', { name: /Save changes/i });
    await user.click(saveButton);

    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mockUpdateMutate).toHaveBeenCalledTimes(1));
    expect(await screen.findByText('Failed to update board')).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);

    await user.click(saveButton);

    await waitFor(() => expect(mockUpdateMutate).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(mockCreateProjectMutate).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  });
});

describe('BoardFormModal pending dismissal guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateMutate.mockResolvedValue({ data: { id: 1, name: 'Test Board' } });
    mockUpdateMutate.mockResolvedValue({});
    mockCreateProjectMutate.mockResolvedValue({ data: { id: 99, name: 'New Project' } });
    mockState.boardsPending = true;
  });

  it('disables Cancel and blocks X close while pending', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();

    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it('blocks Escape dismissal while pending', () => {
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it('allows Escape dismissal when idle', () => {
    mockState.boardsPending = false;
    const onOpenChange = vi.fn();
    renderWithRouter(<BoardFormModal mode="create" open={true} onOpenChange={onOpenChange} />);

    fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('blocks dismissal when project creation is pending in edit mode', async () => {
    const user = userEvent.setup();
    mockState.boardsPending = false;
    mockState.projectsPending = true;
    const onOpenChange = vi.fn();
    renderWithRouter(
      <BoardFormModal
        mode="edit"
        open={true}
        onOpenChange={onOpenChange}
        initialBoard={{ id: 7, name: 'Old Name', project_id: 1 }}
      />
    );

    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();

    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(onOpenChange).not.toHaveBeenCalled();

    fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
    expect(onOpenChange).not.toHaveBeenCalled();
  });
});
