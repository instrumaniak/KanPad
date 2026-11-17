import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, renderHook, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { ToastProvider } from '@/components/ui/toast-provider';
import { SessionExpiryHandler } from '@/components/session-expiry-handler';
import { AuthProvider } from './auth-provider';
import { useAuth } from './use-auth';

vi.mock('./auth.api', () => ({
  registerApi: vi.fn(),
  loginApi: vi.fn(),
  meApi: vi.fn(),
  logoutApi: vi.fn(),
}));

import * as authApi from './auth.api';

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>{children}</AuthProvider>
    </QueryClientProvider>
  );
};

describe('AuthProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('initial state', () => {
    it('shows loading state initially', async () => {
      (authApi.meApi as ReturnType<typeof vi.fn>).mockImplementation(
        () => new Promise(() => {}),
      );

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      expect(result.current.isLoading).toBe(true);
    });

    it('sets user to null when not authenticated', async () => {
      (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Not authenticated'),
      );

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));
      expect(result.current.user).toBeNull();
    });

    it('sets user when authenticated', async () => {
      const mockUser = { id: 1, email: 'test@example.com', role: 'user' };
      (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUser });

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));
      expect(result.current.user).toEqual(mockUser);
    });
  });

  describe('register', () => {
    it('registers user successfully', async () => {
      (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({
        data: null,
      });
      (authApi.registerApi as ReturnType<typeof vi.fn>).mockResolvedValue({
        data: { id: 1, email: 'new@example.com', role: 'user' },
      });

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));

      await act(async () => {
        await result.current.register({ email: 'new@example.com', password: 'Password123!' });
      });

      expect(authApi.registerApi).toHaveBeenCalled();
      const callArgs = (authApi.registerApi as ReturnType<typeof vi.fn>).mock.calls[0][0];
      expect(callArgs.email).toBe('new@example.com');
      expect(callArgs.password).toBe('Password123!');
    });

    it('handles registration error', async () => {
      (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Not authenticated'),
      );
      (authApi.registerApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Email already exists'),
      );

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));

      await expect(
        result.current.register({ email: 'existing@example.com', password: 'Password123!' }),
      ).rejects.toThrow('Email already exists');
    });
  });

  describe('login', () => {
    it('logs in user successfully', async () => {
      const mockUser = { id: 1, email: 'test@example.com', role: 'user' };
      (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Not authenticated'),
      );
      (authApi.loginApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUser });

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));

      await act(async () => {
        await result.current.login({ email: 'test@example.com', password: 'Password123!' });
      });

      expect(authApi.loginApi).toHaveBeenCalled();
      const callArgs = (authApi.loginApi as ReturnType<typeof vi.fn>).mock.calls[0][0];
      expect(callArgs.email).toBe('test@example.com');
      expect(callArgs.password).toBe('Password123!');
    });

    it('handles login error', async () => {
      (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Not authenticated'),
      );
      (authApi.loginApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Invalid credentials'),
      );

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));

      await expect(
        result.current.login({ email: 'wrong@example.com', password: 'WrongPassword!' }),
      ).rejects.toThrow('Invalid credentials');
    });
  });

  describe('logout', () => {
    it('logs out user successfully', async () => {
      const mockUser = { id: 1, email: 'test@example.com', role: 'user' };
      (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUser });
      (authApi.logoutApi as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));
      expect(result.current.user).toEqual(mockUser);

      await act(async () => {
        await result.current.logout();
      });

      expect(authApi.logoutApi).toHaveBeenCalled();
    });

    it('handles logout error', async () => {
      const mockUser = { id: 1, email: 'test@example.com', role: 'user' };
      (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUser });
      (authApi.logoutApi as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Logout failed'),
      );

      const { result } = renderHook(() => useAuth(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isLoading).toBe(false));

      await expect(result.current.logout()).rejects.toThrow('Logout failed');
    });
  });
});

describe('session cache hygiene', () => {
  const mockUserA = { id: 1, email: 'a@example.com', role: 'user' };
  const mockUserB = { id: 2, email: 'b@example.com', role: 'user' };

  const createTestQueryClient = () =>
    new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });

  const renderAuth = (queryClient: QueryClient) =>
    renderHook(() => useAuth(), {
      wrapper: ({ children }: { children: React.ReactNode }) => (
        <QueryClientProvider client={queryClient}>
          <AuthProvider>{children}</AuthProvider>
        </QueryClientProvider>
      ),
    });

  it('logout clears every cached query and nulls auth me', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['columns', 1], [{ id: 11, name: 'Secret TODO' }]);
    queryClient.setQueryData(['boards'], [{ id: 5, name: 'A secret board' }]);
    queryClient.setQueryData(['notes'], [{ id: 9, title: 'A private note' }]);
    (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUserA });
    (authApi.logoutApi as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    const { result } = renderAuth(queryClient);
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.logout();
    });

    expect(queryClient.getQueryData(['columns', 1])).toBeUndefined();
    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
    expect(queryClient.getQueryData(['notes'])).toBeUndefined();
    expect(queryClient.getQueryData(['auth', 'me'])).toBeNull();
  });

  it('login as B wipes A cached data and sets me to B', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['columns', 1], [{ id: 11, name: 'Secret TODO' }]);
    queryClient.setQueryData(['boards'], [{ id: 5, name: 'A secret board' }]);
    (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Not authenticated'),
    );
    (authApi.loginApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUserB });

    const { result } = renderAuth(queryClient);
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.login({ email: 'b@example.com', password: 'Password123!' });
    });

    expect(queryClient.getQueryData(['columns', 1])).toBeUndefined();
    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
    // Ordering pin: the new session's me write must survive the clear.
    expect(queryClient.getQueryData(['auth', 'me'])).toEqual({ data: mockUserB });
    // The observer re-attaches on a later effect hop; wait for the context to catch up.
    await waitFor(() => expect(result.current.user).toEqual(mockUserB));
  });

  it('late in-flight query response after logout does not repopulate the cache', async () => {
    const queryClient = createTestQueryClient();
    (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUserA });
    (authApi.logoutApi as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    const box: { resolve?: (value: unknown) => void } = {};
    const boardsQueryFn = vi.fn(
      () =>
        new Promise((resolve) => {
          // Only the original in-flight fetch gets a resolver; any refetch
          // re-triggered by the flush returns a promise that never settles.
          if (!box.resolve) box.resolve = resolve;
        }),
    );

    function BoardsProbe() {
      useQuery({ queryKey: ['boards'], queryFn: boardsQueryFn });
      return null;
    }

    let logoutFn!: () => Promise<void>;
    function Harness() {
      const { logout } = useAuth();
      logoutFn = logout;
      return null;
    }

    render(
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <Harness />
          <BoardsProbe />
        </AuthProvider>
      </QueryClientProvider>,
    );

    await waitFor(() => expect(boardsQueryFn).toHaveBeenCalledTimes(1));
    await act(async () => {
      await logoutFn();
    });

    box.resolve?.([{ id: 5, name: 'A secret board' }]);
    await act(async () => {}); // flush microtasks

    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
  });

  it('mutation settling after logout does not resurrect the cache', async () => {
    const queryClient = createTestQueryClient();
    (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUserA });
    (authApi.logoutApi as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    let resolveMutation!: (value: unknown) => void;
    const mutation = queryClient.getMutationCache().build(queryClient, {
      mutationKey: ['createBoard'],
      mutationFn: () =>
        new Promise((resolve) => {
          resolveMutation = resolve;
        }),
      onSettled: () => {
        // Realistic app pattern: invalidate after a mutation settles. On an
        // empty cache with no observer this must not fetch/resurrect anything.
        queryClient.invalidateQueries({ queryKey: ['boards'] });
      },
    });
    const runPromise = mutation.execute({});

    const { result } = renderAuth(queryClient);
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.logout();
    });
    expect(queryClient.getMutationCache().getAll()).toHaveLength(0);

    resolveMutation({ id: 5 });
    await act(async () => {
      await runPromise;
    });

    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
  });

  it('late bootstrap me response after logout does not repopulate me', async () => {
    const queryClient = createTestQueryClient();
    let resolveMe!: (value: unknown) => void;
    (authApi.meApi as ReturnType<typeof vi.fn>).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveMe = resolve;
        }),
    );
    (authApi.logoutApi as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    const { result } = renderAuth(queryClient);
    await waitFor(() => expect(authApi.meApi).toHaveBeenCalled());

    // Logout lands before the bootstrap me fetch resolves.
    await act(async () => {
      await result.current.logout();
    });
    expect(queryClient.getQueryData(['auth', 'me'])).toBeNull();

    resolveMe({ data: mockUserA });
    await act(async () => {}); // flush microtasks

    // The destroyed bootstrap query never reaches setData — me stays null.
    expect(queryClient.getQueryData(['auth', 'me'])).toBeNull();
    expect(result.current.user).toBeNull();
  });

  it('logout failure keeps the cache (deliberate)', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['boards'], [{ id: 5, name: 'A kept board' }]);
    (authApi.meApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: mockUserA });
    (authApi.logoutApi as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Logout failed'),
    );

    const { result } = renderAuth(queryClient);
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await expect(result.current.logout()).rejects.toThrow('Logout failed');

    // Deliberate contract (spec §Notes item 5): cache is retained on logout failure,
    // because the user is still authenticated. Pin it so a future "clear on error"
    // refactor has to change this test on purpose.
    expect(queryClient.getQueryData(['boards'])).toEqual([{ id: 5, name: 'A kept board' }]);
    expect(queryClient.getQueryData(['auth', 'me'])).toEqual({ data: mockUserA });
  });

  it('register auto-login sets me from the register response without a refetch', async () => {
    const queryClient = createTestQueryClient();
    const newUser = { id: 7, email: 'new@example.com', role: 'user' };
    (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('Not authenticated'),
    );
    (authApi.registerApi as ReturnType<typeof vi.fn>).mockResolvedValue({ data: newUser });

    const { result } = renderAuth(queryClient);
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => {
      await result.current.register({ email: 'new@example.com', password: 'Password123!' });
    });

    // me is written from the register response (ordering: after the clear),
    // so the bootstrap query must not refetch.
    expect(queryClient.getQueryData(['auth', 'me'])).toEqual({ data: newUser });
    expect(authApi.meApi).toHaveBeenCalledTimes(1);
    // The observer re-attaches on a later effect hop; wait for the context to catch up.
    await waitFor(() => expect(result.current.user).toEqual(newUser));
  });
});

describe('AuthProvider on a guest route with the global 401 handler mounted (T043)', () => {
  const unauthorized = () =>
    Object.assign(new Error('Authentication is required.'), { status: 401 });

  it('fires the bootstrap me fetch exactly once on mount — never a refetch loop', async () => {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    (authApi.meApi as ReturnType<typeof vi.fn>).mockRejectedValue(unauthorized());

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/login']}>
          <ToastProvider>
            <SessionExpiryHandler />
            <AuthProvider>
              <div>Guest Page</div>
            </AuthProvider>
          </ToastProvider>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await waitFor(() =>
      expect(
        queryClient.getQueryCache().find({ queryKey: ['auth', 'me'] })?.state.status,
      ).toBe('error'),
    );
    // The fire storm (T043) reproduced hundreds of calls in ~1.5 s; half a
    // second of quiescence after the first fetch is conclusive, and bounded.
    await new Promise((r) => setTimeout(r, 500));
    expect(authApi.meApi).toHaveBeenCalledTimes(1);
  });
});

describe('useAuth', () => {
  it('throws error when used outside AuthProvider', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    expect(() => {
      const { result } = renderHook(() => useAuth());
      if (result.error) throw result.error;
    }).toThrow('useAuth must be used within an AuthProvider');
    consoleError.mockRestore();
  });
});
