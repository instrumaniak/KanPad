import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ArchivedBoards } from './archived-boards';
import { ToastProvider } from '@/components/ui/toast-provider';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';

const mockUseArchivedBoards = vi.fn();
const mockUseRestoreBoard = vi.fn();
const mockUsePermanentDeleteBoard = vi.fn();
const mockUseUpdateBoard = vi.fn();
const mockUseArchiveBoard = vi.fn();

vi.mock('./use-boards', () => ({
  useArchivedBoards: () => mockUseArchivedBoards(),
  useRestoreBoard: () => mockUseRestoreBoard(),
  usePermanentDeleteBoard: () => mockUsePermanentDeleteBoard(),
  useUpdateBoard: () => mockUseUpdateBoard(),
  useArchiveBoard: () => mockUseArchiveBoard(),
  flattenPages: (pages: Array<{ data: unknown[] }> | undefined) =>
    pages?.flatMap((p) => p.data) ?? [],
}));

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <MemoryRouter>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>{children}</ToastProvider>
      </QueryClientProvider>
    </MemoryRouter>
  );
};

describe('ArchivedBoards', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseArchivedBoards.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
      error: null,
      refetch: vi.fn(),
      hasNextPage: false,
      fetchNextPage: vi.fn(),
      isFetchingNextPage: false,
    });
    mockUseRestoreBoard.mockReturnValue({
      mutateAsync: vi.fn().mockResolvedValue({}),
      isPending: false,
    });
    mockUsePermanentDeleteBoard.mockReturnValue({
      mutateAsync: vi.fn().mockResolvedValue({}),
      isPending: false,
    });
  });

  it('renders loading state', () => {
    const Wrapper = createWrapper();
    render(<ArchivedBoards />, { wrapper: Wrapper });
    expect(screen.getByText('Archived Boards')).toBeInTheDocument();
  });

  it('renders empty state when no archived boards', () => {
    mockUseArchivedBoards.mockReturnValue({
      data: { pages: [{ data: [], page: 1, limit: 50, total: 0 }], pageParams: [1] },
      isLoading: false,
      isError: false,
      error: null,
      refetch: vi.fn(),
      hasNextPage: false,
      fetchNextPage: vi.fn(),
      isFetchingNextPage: false,
    });

    const Wrapper = createWrapper();
    render(<ArchivedBoards />, { wrapper: Wrapper });
    expect(screen.getByText('No archived boards')).toBeInTheDocument();
  });

  it('renders archived boards when data is available', () => {
    mockUseArchivedBoards.mockReturnValue({
      data: {
        pages: [
          {
            data: [
              { id: 1, name: 'Archived Board 1', background_color: '#0079BF', is_archived: true },
            ],
            page: 1,
            limit: 50,
            total: 1,
          },
        ],
        pageParams: [1],
      },
      isLoading: false,
      isError: false,
      error: null,
      refetch: vi.fn(),
      hasNextPage: false,
      fetchNextPage: vi.fn(),
      isFetchingNextPage: false,
    });

    const Wrapper = createWrapper();
    render(<ArchivedBoards />, { wrapper: Wrapper });
    expect(screen.getByText('Archived Board 1')).toBeInTheDocument();
  });
});
