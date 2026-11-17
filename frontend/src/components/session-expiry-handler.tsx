import { useEffect, useRef } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { useToast } from '@/components/ui/use-toast';
import { resetSessionCaches } from '@/lib/query-client-reset';
import { getErrorMessage } from '@/lib/error-messages';

const GUEST_PATHS = ['/login', '/register', '/forgot-password'];

function isUnauthorized(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    (error as { status?: unknown }).status === 401
  );
}

interface SessionExpiryMeta {
  /** Handler does nothing at all (credential POSTs — bad credentials are form-level errors). */
  skipSessionExpiry?: unknown;
  /** Cache is cleared, but no redirect / toast (bootstrap `me`: AppLayout owns the redirect). */
  skipSessionExpiryRedirect?: unknown;
}

function metaOf(meta: unknown): SessionExpiryMeta {
  return (typeof meta === 'object' && meta !== null ? meta : {}) as SessionExpiryMeta;
}

/**
 * Global 401 → session-expiry handling (R5). Mounted once inside the providers.
 *
 * Assigns `onError` on the query and mutation caches (TanStack-documented
 * single-assignment seam — one global handler per cache; restored on
 * unmount). Any non-exempt 401 does:
 *
 *   resetSessionCaches()  →  navigate('/login')  →  exactly one toast
 *
 * Idempotent under concurrent 401s (single-flight flag held until the cache
 * reset settles). Single source of truth for the session-expiry UX; feature
 * code must never sniff 401s itself.
 *
 * Exemptions (via `meta` on the query/mutation):
 * - `skipSessionExpiry`        — no handler actions at all.
 * - `skipSessionExpiryRedirect` — clear-only; no navigate/toast.
 * On guest routes the handler is also clear-only (nothing to redirect to).
 */
export function SessionExpiryHandler() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const location = useLocation();
  const { toast } = useToast();
  const inFlightRef = useRef(false);
  // The cache callbacks outlive renders — always call through the latest
  // refs. NOTE: refs are not written during render (react-hooks/refs); the
  // sync effect below runs before the registration effect.
  const latestRef = useRef({ navigate, location, toast });
  useEffect(() => {
    latestRef.current = { navigate, location, toast };
  }, [navigate, location, toast]);

  useEffect(() => {
    const queryCache = queryClient.getQueryCache();
    const mutationCache = queryClient.getMutationCache();
    const previousQueryOnError = queryCache.config.onError;
    const previousMutationOnError = mutationCache.config.onError;

    const handle = (error: unknown, meta: unknown) => {
      if (metaOf(meta).skipSessionExpiry === true) return;
      if (!isUnauthorized(error)) return;
      const { navigate: nav, location: loc, toast: show } = latestRef.current;
      const clearOnly =
        metaOf(meta).skipSessionExpiryRedirect === true ||
        GUEST_PATHS.some((p) => loc.pathname === p || loc.pathname.startsWith(`${p}/`));

      // Exempt (me/guest-route) 401s take NO single-flight slot: concurrent
      // clears are idempotent, and an actionable 401 racing them must not be
      // dropped (it owns the navigate/toast below).
      if (clearOnly) {
        void resetSessionCaches(queryClient).catch(() => {
          // Best effort — an exempt 401 never blocks on the clear.
        });
        return;
      }

      if (inFlightRef.current) return;
      inFlightRef.current = true;

      // Clear is best-effort: even if it rejects, the user is routed out.
      void resetSessionCaches(queryClient)
        .catch(() => undefined)
        .then(() => {
          nav('/login', { replace: true });
          const msg = getErrorMessage(error);
          show({ title: msg.title, description: msg.message, type: 'error' });
        })
        .finally(() => {
          inFlightRef.current = false;
        });
    };

    queryCache.config.onError = (error, query) => {
      handle(error, query.meta);
    };
    mutationCache.config.onError = (error, _variables, _onMutateResult, mutation) => {
      handle(error, mutation.meta);
    };

    return () => {
      queryCache.config.onError = previousQueryOnError;
      mutationCache.config.onError = previousMutationOnError;
    };
  }, [queryClient]);

  return null;
}
