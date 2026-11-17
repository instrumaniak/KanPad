import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { CancelledError } from '@tanstack/react-query';
import { createAppQueryClient } from './app-query-client';
import { ApiError, TimeoutError } from '@/lib/api/client';

const uniqueKey = (prefix: string) => [prefix, Math.random().toString(36)] as const;

describe('createAppQueryClient retry policy', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function runQuery(client: ReturnType<typeof createAppQueryClient>, error: unknown) {
    let calls = 0;
    const promise = client.fetchQuery({
      queryKey: uniqueKey('retry-test'),
      queryFn: () => {
        calls += 1;
        return Promise.reject(error);
      },
    });
    const assertion = expect(promise).rejects.toThrow();
    // Advance past the single retry delay (flat 1 s).
    await vi.advanceTimersByTimeAsync(2_000);
    await assertion;
    return calls;
  }

  it('retries a query once on a 500 (2 attempts total)', async () => {
    const client = createAppQueryClient();
    expect(await runQuery(client, new ApiError(500, 'DB down'))).toBe(2);
  });

  it('retries a query once on a network TypeError', async () => {
    const client = createAppQueryClient();
    expect(await runQuery(client, new TypeError('Failed to fetch'))).toBe(2);
  });

  it('retries a query once on a TimeoutError', async () => {
    const client = createAppQueryClient();
    expect(await runQuery(client, new TimeoutError())).toBe(2);
  });

  it('does NOT retry a query on a 400', async () => {
    const client = createAppQueryClient();
    expect(await runQuery(client, new ApiError(400, 'Bad request'))).toBe(1);
  });

  it('does NOT retry a query on a 401', async () => {
    const client = createAppQueryClient();
    expect(await runQuery(client, new ApiError(401, 'Unauthorized'))).toBe(1);
  });

  it('does NOT retry a query cancelled via cancelQueries (CancelledError)', async () => {
    const client = createAppQueryClient();
    expect(await runQuery(client, new CancelledError())).toBe(1);
  });

  it('never retries mutations, even on a 500', async () => {
    const client = createAppQueryClient();
    const mutationFn = vi.fn().mockRejectedValue(new ApiError(500, 'boom'));
    await expect(
      client.getMutationCache().build(client, { mutationFn }).execute(undefined),
    ).rejects.toThrow('boom');
    expect(mutationFn).toHaveBeenCalledTimes(1);
  });
});
