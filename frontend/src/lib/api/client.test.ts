import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { apiFetch, apiFetchVoid, ApiError, TimeoutError, DEFAULT_TIMEOUT_MS } from './client';

/**
 * jsdom has no real fetch; every test stubs `global.fetch`. The timeout tests
 * assert on the same AbortSignal-shaped contract a real browser fetch follows:
 * the signal fires, the fetch rejects with an AbortError DOMException.
 */
describe('apiFetch', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  describe('default timeout', () => {
    function abortOnSignal() {
      return vi.fn().mockImplementation(
        (_url: string, init?: RequestInit) =>
          new Promise((_resolve, reject) => {
            // Faithful to real fetch: a pre-aborted signal rejects immediately.
            if (init?.signal?.aborted) {
              reject(new DOMException('The operation was aborted.', 'AbortError'));
              return;
            }
            init?.signal?.addEventListener('abort', () => {
              reject(new DOMException('The operation was aborted.', 'AbortError'));
            });
          }),
      );
    }

    it('aborts with a typed TimeoutError after 15s', async () => {
      vi.stubGlobal('fetch', abortOnSignal());
      const promise = apiFetch('/api/v1/boards');
      const assertion = expect(promise).rejects.toBeInstanceOf(TimeoutError);
      await vi.advanceTimersByTimeAsync(DEFAULT_TIMEOUT_MS);
      await assertion;
    });

    it('TimeoutError carries the friendly connection message', async () => {
      vi.stubGlobal('fetch', abortOnSignal());
      const promise = apiFetch('/api/v1/boards');
      const assertion = expect(promise).rejects.toMatchObject({
        name: 'TimeoutError',
        message: 'Network timeout — check your connection',
      });
      await vi.advanceTimersByTimeAsync(DEFAULT_TIMEOUT_MS);
      await assertion;
    });

    it('does not abort before the timeout elapses', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 })),
      );
      await expect(apiFetch('/api/v1/boards')).resolves.toEqual({ ok: true });
    });

    it('a caller-supplied abort is not re-labelled as a TimeoutError', async () => {
      vi.stubGlobal('fetch', abortOnSignal());
      const caller = new AbortController();
      const promise = apiFetch('/api/v1/boards', { signal: caller.signal });
      const assertion = expect(promise).rejects.not.toBeInstanceOf(TimeoutError);
      caller.abort();
      await assertion;
    });

    it('a pre-aborted caller signal aborts immediately and is never a TimeoutError', async () => {
      vi.stubGlobal('fetch', abortOnSignal());
      const caller = new AbortController();
      caller.abort();
      await expect(apiFetch('/api/v1/boards', { signal: caller.signal })).rejects.not.toBeInstanceOf(
        TimeoutError,
      );
      await expect(apiFetch('/api/v1/boards', { signal: caller.signal })).rejects.toMatchObject({
        name: 'AbortError',
      });
    });

    it('clears the internal timer on the caller-abort path', async () => {
      vi.stubGlobal('fetch', abortOnSignal());
      const caller = new AbortController();
      const promise = apiFetch('/api/v1/boards', { signal: caller.signal });
      const assertion = expect(promise).rejects.toBeDefined();
      caller.abort();
      await assertion;
      expect(vi.getTimerCount()).toBe(0);
    });

    it('threads the caller signal through to fetch (chained, not dropped)', async () => {
      const fetchMock = abortOnSignal();
      vi.stubGlobal('fetch', fetchMock);
      const caller = new AbortController();
      const promise = apiFetch('/api/v1/boards', { signal: caller.signal });
      const assertion = expect(promise).rejects.toBeDefined();
      caller.abort();
      await assertion;
      const init = fetchMock.mock.calls[0][1] as RequestInit;
      expect(init.signal).toBeInstanceOf(AbortSignal);
      expect(init.signal).not.toBe(caller.signal); // wrapped by the internal timer controller
    });
  });

  describe('error propagation', () => {
    it('propagates network TypeError unchanged (Failed to fetch)', async () => {
      vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')));
      await expect(apiFetch('/api/v1/boards')).rejects.toThrow(TypeError);
      await expect(apiFetch('/api/v1/boards')).rejects.toThrow('Failed to fetch');
    });

    it('throws ApiError with the server status for non-ok responses', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValue(
          new Response(JSON.stringify({ title: 'Unauthorized', detail: 'Authentication is required.' }), {
            status: 401,
            statusText: 'Unauthorized',
            headers: { 'Content-Type': 'application/problem+json' },
          }),
        ),
      );
      await expect(apiFetch('/api/v1/boards')).rejects.toMatchObject({
        name: 'ApiError',
        status: 401,
      });
    });

    it('returns undefined for 204 responses', async () => {
      vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })));
      await expect(apiFetch('/api/v1/boards')).resolves.toBeUndefined();
    });
  });

  describe('apiFetchVoid', () => {
    it('resolves void for 204', async () => {
      vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })));
      await expect(apiFetchVoid('/api/v1/boards/1', { method: 'DELETE' })).resolves.toBeUndefined();
    });
  });

  it('exposes the 401 status on ApiError so the global handler can duck-type it', () => {
    expect(new ApiError(401, 'Unauthorized').status).toBe(401);
  });
});
