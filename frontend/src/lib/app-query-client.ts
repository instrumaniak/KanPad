import { CancelledError, QueryClient } from '@tanstack/react-query';
import { ApiError, TimeoutError } from '@/lib/api/client';

/**
 * App-wide QueryClient defaults (single owner — App.tsx consumes this).
 *
 * Retry policy (R5): idempotent GET queries (useQuery) retry ONCE on
 * network/5xx/timeout-class failures with a flat 1 s delay; 4xx never retry;
 * `CancelledError` never retries (T013's `cancelQueries()` rejection);
 * mutations never retry (they are not idempotent). `staleTime` unchanged.
 */
export function createAppQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 5 * 60 * 1000,
        retry: (failureCount, error) => {
          if (error instanceof CancelledError) return false;
          if (error instanceof ApiError) {
            return error.status >= 500 && failureCount < 1;
          }
          if (error instanceof TimeoutError) {
            // Timeout is network-class: retry once, never more.
            return failureCount < 1;
          }
          // Network TypeError (browser jargon, any vendor), unknowns —
          // network-class; retry once.
          return failureCount < 1;
        },
        retryDelay: 1_000,
      },
      mutations: {
        retry: false,
      },
    },
  });
}
