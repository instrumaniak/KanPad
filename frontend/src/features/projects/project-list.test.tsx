import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { ProjectList } from './project-list';

const mockFetchProjects = vi.fn();
const mockCreateProject = vi.fn();
const mockUpdateProject = vi.fn();
const mockDeleteProject = vi.fn();

const mockRecreateProject = vi.fn();

vi.mock('./projects.api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./projects.api')>();
  return {
    ...actual,
    fetchProjects: (...args: unknown[]) => mockFetchProjects(...args),
    createProject: (...args: unknown[]) => mockCreateProject(...args),
    updateProject: (...args: unknown[]) => mockUpdateProject(...args),
    deleteProject: (...args: unknown[]) => mockDeleteProject(...args),
    recreateProject: (...args: unknown[]) => mockRecreateProject(...args),
  };
});

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <ToastProvider>{children}</ToastProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
};

describe('ProjectList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the page title', async () => {
    mockFetchProjects.mockResolvedValue({ data: [], page: 1, limit: 50, total: 0 });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('My Projects')).toBeInTheDocument();
    });
  });

  it('shows empty state when no projects', async () => {
    mockFetchProjects.mockResolvedValue({ data: [], page: 1, limit: 50, total: 0 });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Start organizing')).toBeInTheDocument();
      expect(screen.getByText('Create your first project')).toBeInTheDocument();
    });
  });

  it('renders project list with projects', async () => {
    mockFetchProjects.mockResolvedValue({
      data: [
        { id: 1, name: 'Project Alpha', board_count: 2, created_at: '2026-01-01', updated_at: '2026-01-01' },
        { id: 2, name: 'Project Beta', board_count: 0, created_at: '2026-01-02', updated_at: '2026-01-02' },
      ],
      page: 1,
      limit: 50,
      total: 2,
    });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Project Alpha')).toBeInTheDocument();
      expect(screen.getByText('Project Beta')).toBeInTheDocument();
      expect(screen.getByText('2 boards')).toBeInTheDocument();
      expect(screen.getByText('0 boards')).toBeInTheDocument();
    });
  });

  it('shows Create Project button when projects exist', async () => {
    mockFetchProjects.mockResolvedValue({
      data: [{ id: 1, name: 'Project', board_count: 0, created_at: '2026-01-01', updated_at: '2026-01-01' }],
      page: 1,
      limit: 50,
      total: 1,
    });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Create Project')).toBeInTheDocument();
    });
  });

  it('shows inline create form when clicking Create Project', async () => {
    mockFetchProjects.mockResolvedValue({
      data: [{ id: 1, name: 'Project', board_count: 0, created_at: '2026-01-01', updated_at: '2026-01-01' }],
      page: 1,
      limit: 50,
      total: 1,
    });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      fireEvent.click(screen.getByText('Create Project'));
    });

    expect(screen.getByPlaceholderText('Project name')).toBeInTheDocument();
    expect(screen.getByText('Save')).toBeInTheDocument();
    expect(screen.getByText('Cancel')).toBeInTheDocument();
  });

  it('creates a project via inline form', async () => {
    mockFetchProjects.mockResolvedValue({ data: [], page: 1, limit: 50, total: 0 });
    mockCreateProject.mockResolvedValue({
      data: { id: 1, name: 'New Project', board_count: 0 },
      message: 'Project created',
    });

    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      fireEvent.click(screen.getByText('Create your first project'));
    });

    const input = screen.getByPlaceholderText('Project name');
    fireEvent.change(input, { target: { value: 'New Project' } });

    await waitFor(() => {
      fireEvent.click(screen.getByText('Save'));
    });

    await waitFor(() => {
      expect(mockCreateProject).toHaveBeenCalledWith('New Project');
    });
  });

  it('cancels inline create form on Escape', async () => {
    mockFetchProjects.mockResolvedValue({
      data: [{ id: 1, name: 'Project', board_count: 0, created_at: '2026-01-01', updated_at: '2026-01-01' }],
      page: 1,
      limit: 50,
      total: 1,
    });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      fireEvent.click(screen.getByText('Create Project'));
    });

    const input = screen.getByPlaceholderText('Project name');
    fireEvent.keyDown(input, { key: 'Escape' });

    await waitFor(() => {
      expect(screen.queryByPlaceholderText('Project name')).not.toBeInTheDocument();
    });
  });

  it('singular board count for 1 board', async () => {
    mockFetchProjects.mockResolvedValue({
      data: [{ id: 1, name: 'Solo Project', board_count: 1, created_at: '2026-01-01', updated_at: '2026-01-01' }],
      page: 1,
      limit: 50,
      total: 1,
    });
    render(<ProjectList />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('1 board')).toBeInTheDocument();
    });
  });
});
