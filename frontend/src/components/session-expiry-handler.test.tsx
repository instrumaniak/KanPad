import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
  useMutation,
} from '@tanstack/react-query';
import { ToastProvider } from './ui/toast-provider';
import { SessionExpiryHandler } from './session-expiry-handler';

function unauthorized() {
  return Object.assign(new Error('Authentication is required.'), { status: 401 });
}

function serverError() {
  return Object.assign(new Error('DB down'), { status: 500 });
}

function FailingQuery({
  queryKey,
  meta,
  error,
  testId,
}: {
  queryKey: string[];
  meta?: Record<string, unknown>;
  error?: unknown;
  testId: string;
}) {
  const { refetch, isError } = useQuery({
    queryKey,
    queryFn: () => Promise.reject(error ?? unauthorized()),
    retry: false,
    enabled: false, // fired explicitly via the trigger button
    meta,
  });
  return (
    <>
      <button data-testid={testId} onClick={() => void refetch()}>
        fire
      </button>
      {isError && <span data-testid="query-error" />}
    </>
  );
}

function FailingMutation({
  meta,
  error,
  testId,
}: {
  meta?: Record<string, unknown>;
  error?: unknown;
  testId: string;
}) {
  const mutation = useMutation({
    mutationFn: () => Promise.reject(error ?? unauthorized()),
    retry: false,
    meta,
  });
  return (
    <>
      <button data-testid={testId} onClick={() => mutation.mutate()}>
        fire mutation
      </button>
      {mutation.isError && <span data-testid="mutation-error" />}
    </>
  );
}

function rig(initialPath: string) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  queryClient.setQueryData(['boards'], [{ id: 5, name: 'A secret board' }]);
  queryClient.setQueryData(['notes'], [{ id: 9, title: 'A private note' }]);

  const utils = render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[initialPath]}>
        <ToastProvider>
          <SessionExpiryHandler />
          <Routes>
            <Route
              path="/board"
              element={
                <>
                  <FailingQuery queryKey={['boards']} testId="fire-boards" />
                  <FailingQuery queryKey={['notes']} testId="fire-notes" />
                  <FailingQuery
                    queryKey={['me']}
                    meta={{ skipSessionExpiryRedirect: true }}
                    testId="fire-me"
                  />
                  <FailingQuery queryKey={['broken']} error={serverError()} testId="fire-broken" />
                  <FailingMutation meta={{ skipSessionExpiry: true }} testId="fire-credential-mutation" />
                  <FailingMutation testId="fire-global-mutation" />
                  <div>Protected Page</div>
                </>
              }
            />
            <Route
              path="/login"
              element={
                <>
                  <FailingQuery queryKey={['boards']} testId="fire-boards" />
                  <FailingQuery queryKey={['notes']} testId="fire-notes" />
                  <div>Login Page</div>
                </>
              }
            />
          </Routes>
        </ToastProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { queryClient, ...utils };
}

describe('SessionExpiryHandler', () => {
  it('clears the cache, redirects to /login and shows exactly one toast for a non-exempt 401', async () => {
    const { queryClient } = rig('/board');

    fireEvent.click(screen.getByTestId('fire-boards'));

    await screen.findByText('Login Page');
    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
    expect(queryClient.getQueryData(['notes'])).toBeUndefined();
    expect(screen.getAllByTestId('toast-error')).toHaveLength(1);
    expect(screen.getByText('Session Expired')).toBeInTheDocument();
  });

  it('is idempotent under concurrent 401s — one redirect, one toast', async () => {
    rig('/board');

    fireEvent.click(screen.getByTestId('fire-boards'));
    fireEvent.click(screen.getByTestId('fire-notes'));

    await screen.findByText('Login Page');
    expect(screen.getAllByTestId('toast-error')).toHaveLength(1);
  });

  it('clears the cache but does NOT redirect or toast for the bootstrap [auth,me] query (AppLayout owns the redirect)', async () => {
    const { queryClient } = rig('/board');

    fireEvent.click(screen.getByTestId('fire-me'));

    // The me exemption is redirect/toast only — the cache MUST still be cleared.
    // (No isError assertion: the clear destroys the query and the observer
    // re-attaches to a fresh instance.)
    await waitFor(() => expect(queryClient.getQueryData(['boards'])).toBeUndefined());
    expect(queryClient.getQueryData(['notes'])).toBeUndefined();
    expect(screen.queryByTestId('toast-error')).not.toBeInTheDocument();
    expect(screen.getByText('Protected Page')).toBeInTheDocument();
  });

  it('an actionable 401 racing an exempt (me) 401 in the same synchronous burst still redirects and toasts exactly once', async () => {
    const { queryClient } = rig('/board');

    // Deterministic race: the exempt (me) 401's handler runs FIRST and would
    // hold the single-flight slot in the buggy shape; the actionable 401 is
    // dispatched synchronously right behind it, before any microtask runs.
    const unauthorizedError = Object.assign(new Error('Authentication is required.'), {
      status: 401,
    });
    const fireExempt = () =>
      queryClient.getQueryCache().config.onError?.(unauthorizedError, {
        meta: { skipSessionExpiryRedirect: true },
      } as never);
    const fireActionable = () =>
      queryClient.getQueryCache().config.onError?.(unauthorizedError, {
        meta: undefined,
      } as never);

    fireExempt();
    fireActionable();

    await screen.findByText('Login Page');
    expect(screen.getAllByTestId('toast-error')).toHaveLength(1);
    expect(screen.getByText('Session Expired')).toBeInTheDocument();
    // Exempt path is clear-only — but the actionable 401 must also have
    // cleared the cache exactly as usual.
    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
  });

  it('does nothing for a credential-POST 401 (login/register): no clear, no redirect, no toast', async () => {
    const { queryClient } = rig('/board');

    fireEvent.click(screen.getByTestId('fire-credential-mutation'));

    await screen.findByTestId('mutation-error');
    expect(queryClient.getQueryData(['boards'])).toEqual([{ id: 5, name: 'A secret board' }]);
    expect(queryClient.getQueryData(['notes'])).toEqual([{ id: 9, title: 'A private note' }]);
    expect(screen.queryByTestId('toast-error')).not.toBeInTheDocument();
    expect(screen.queryByText('Login Page')).not.toBeInTheDocument();
  });

  it('clear-only on a guest route — no toast, no redirect', async () => {
    const { queryClient } = rig('/login');

    fireEvent.click(screen.getByTestId('fire-boards'));

    await waitFor(() => expect(queryClient.getQueryData(['boards'])).toBeUndefined());
    expect(screen.queryByTestId('toast-error')).not.toBeInTheDocument();
    expect(screen.getByText('Login Page')).toBeInTheDocument();
  });

  it('ignores non-401 errors entirely', async () => {
    const { queryClient } = rig('/board');

    fireEvent.click(screen.getByTestId('fire-broken'));

    await screen.findByTestId('query-error');
    expect(queryClient.getQueryData(['boards'])).toEqual([{ id: 5, name: 'A secret board' }]);
    expect(screen.queryByTestId('toast-error')).not.toBeInTheDocument();
    expect(screen.getByText('Protected Page')).toBeInTheDocument();
  });

  it('clears the cache and redirects for non-exempt mutation 401s as well', async () => {
    const { queryClient } = rig('/board');

    fireEvent.click(screen.getByTestId('fire-global-mutation'));

    await screen.findByText('Login Page');
    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
  });
});
