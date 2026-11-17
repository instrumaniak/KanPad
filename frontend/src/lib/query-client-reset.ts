import { notifyManager, type QueryClient } from '@tanstack/react-query';

/**
 * Monotonic session-boundary counter. `resetSessionCaches()` bumps it BEFORE
 * clearing; mutation callbacks that will later write to the cache capture
 * `currentSessionEpoch()` in `onMutate` and compare here before applying an
 * `onError` rollback — if the epoch changed, a session boundary was crossed
 * while the mutation was in flight and the rollback MUST NOT recreate the
 * previous user's data (the residual R2 path: `MutationCache.clear()` never
 * cancels in-flight mutations, so their `onError`/`onSettled` still run and a
 * concrete-value `setQueryData` would build the destroyed query again).
 */
let sessionEpoch = 0;

export function currentSessionEpoch(): number {
  return sessionEpoch;
}

/**
 * Flushes every cached query and mutation for a session boundary
 * (logout / login / register / global 401).
 *
 * Query keys are not user-scoped, so a whole-cache clear is the minimal
 * correct fix for cross-account data disclosure on shared tabs.
 *
 * Order: bump `sessionEpoch` first (in-flight mutation callbacks must observe
 * the boundary even before `cancelQueries()` completes), then
 * `cancelQueries()` (belt-and-braces — since @tanstack/query-core
 * 5.107.1, `QueryClient.clear()` destroys each query, which cancels its
 * retryer with a `CancelledError`, so a late in-flight response never reaches
 * `setData`), then `clear()`.
 *
 * Callers MUST re-apply the new session's `['auth', 'me']` write AFTER this
 * returns — a write made before the clear is wiped. `MutationCache.clear()`
 * does NOT cancel in-flight mutations; their callbacks still run and must
 * tolerate executing after a session boundary (see `currentSessionEpoch`).
 *
 * `exceptQueryHash` spares a single query from the removal — but only when
 * it carries no data. It exists for the global 401 handler's exempt
 * (clear-only) path: an exempt 401 that destroys the data-less `['auth','me']`
 * bootstrap query makes the mounted observer re-create + refetch it, whose
 * 401 triggers another clear — a self-sustaining fire storm (T043). Sparing
 * the data-less query breaks the cycle. A query that still holds data (e.g.
 * a focus-refetch of `me` that 401s with the previous user's last-good
 * payload, which TanStack keeps on refetch error) is destroyed like
 * everything else: the mounted provider re-creates and refetches it once, it
 * 401s without data, and is spared then — so at most one extra request and
 * zero stale-identity leakage on a shared tab.
 *
 * In all cases every other query is removed and the mutation cache is
 * cleared; the epoch bump and `cancelQueries()` are identical.
 */
export async function resetSessionCaches(
  queryClient: QueryClient,
  options?: { exceptQueryHash?: string },
): Promise<void> {
  sessionEpoch += 1;
  await queryClient.cancelQueries();
  const queryCache = queryClient.getQueryCache();
  const exceptHash = options?.exceptQueryHash;
  const spared =
    exceptHash === undefined
      ? undefined
      : queryCache.getAll().find(
          (query) => query.queryHash === exceptHash && query.state.data === undefined,
        );
  if (spared === undefined) {
    queryClient.clear();
    return;
  }
  // `queryCache.clear()` removes inside `notifyManager.batch(...)` (one
  // notification pass); mirror that here so the spare path doesn't pay N
  // synchronous observer notification passes on a shared tab.
  notifyManager.batch(() => {
    for (const query of queryCache.getAll()) {
      if (query === spared) continue;
      queryCache.remove(query);
    }
    queryClient.getMutationCache().clear();
  });
}
