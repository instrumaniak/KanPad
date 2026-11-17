import type { QueryClient } from '@tanstack/react-query';

/**
 * Flushes every cached query and mutation for a session boundary
 * (logout / login / register).
 *
 * Query keys are not user-scoped, so a whole-cache clear is the minimal
 * correct fix for cross-account data disclosure on shared tabs.
 *
 * Order: `cancelQueries()` first (belt-and-braces — since @tanstack/query-core
 * 5.107.1, `QueryClient.clear()` destroys each query, which cancels its
 * retryer with a `CancelledError`, so a late in-flight response never reaches
 * `setData`), then `clear()`.
 *
 * Callers MUST re-apply the new session's `['auth', 'me']` write AFTER this
 * returns — a write made before the clear is wiped. `MutationCache.clear()`
 * does NOT cancel in-flight mutations; their callbacks still run and must
 * tolerate executing after a session boundary.
 */
export async function resetSessionCaches(queryClient: QueryClient): Promise<void> {
  await queryClient.cancelQueries();
  queryClient.clear();
}
