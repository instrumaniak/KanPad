import { describe, it, expect } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { resetSessionCaches } from './query-client-reset';

const createClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

describe('resetSessionCaches', () => {
  it('clears every cached query', async () => {
    const queryClient = createClient();
    queryClient.setQueryData(['columns', 1], [{ id: 11, name: 'Secret TODO' }]);
    queryClient.setQueryData(['boards'], [{ id: 5, name: 'A secret board' }]);
    queryClient.setQueryData(['notes'], [{ id: 9, title: 'A private note' }]);

    await resetSessionCaches(queryClient);

    expect(queryClient.getQueryData(['columns', 1])).toBeUndefined();
    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
    expect(queryClient.getQueryData(['notes'])).toBeUndefined();
  });

  it('clears the mutation cache', async () => {
    const queryClient = createClient();
    queryClient.getMutationCache().build(queryClient, {
      mutationKey: ['createBoard'],
      mutationFn: () => new Promise(() => {}), // never settles
    });
    expect(queryClient.getMutationCache().getAll()).toHaveLength(1);

    await resetSessionCaches(queryClient);

    expect(queryClient.getMutationCache().getAll()).toHaveLength(0);
  });

  it('is a no-op on an empty cache (double logout safe)', async () => {
    const queryClient = createClient();
    await expect(resetSessionCaches(queryClient)).resolves.toBeUndefined();
    await expect(resetSessionCaches(queryClient)).resolves.toBeUndefined();
  });

  it('does not let a late in-flight query response repopulate the cache', async () => {
    const queryClient = createClient();
    let resolveFetch!: (value: unknown) => void;
    const deferred = new Promise((resolve) => {
      resolveFetch = resolve;
    });
    const fetchPromise = queryClient.fetchQuery({
      queryKey: ['boards'],
      queryFn: () => deferred,
    });

    await resetSessionCaches(queryClient);
    resolveFetch([{ id: 5, name: 'A secret board' }]);
    // The destroyed query's retryer rejects with CancelledError — the late
    // response never reaches setData.
    await expect(fetchPromise).rejects.toThrow();

    expect(queryClient.getQueryData(['boards'])).toBeUndefined();
  });
});
