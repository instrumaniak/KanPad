import { useCallback, type ReactNode } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { registerApi, loginApi, meApi, logoutApi } from '../auth.api';
import { AuthContext, type AuthContextType } from '../auth-context';
import { resetSessionCaches } from '@/lib/query-client-reset';

export function AuthProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();

  const { data: userResponse, isLoading } = useQuery({
    queryKey: ['auth', 'me'],
    queryFn: meApi,
    retry: false,
    staleTime: 5 * 60 * 1000,
    // The global 401 handler (T014) clears the cache for this 401 too, but the
    // redirect stays AppLayout-owned — exempt redirect/toast only.
    meta: { skipSessionExpiryRedirect: true },
  });

  const registerMutation = useMutation({
    mutationFn: registerApi,
    // Bad-credential 401s are form-level errors, never session-expiry signals.
    meta: { skipSessionExpiry: true },
    onSuccess: async (data) => {
      await resetSessionCaches(queryClient);
      // Register logs the user in and returns the user payload — write it from
      // the response (same ordering rule as login). invalidateQueries here
      // would be a no-op: the clear above emptied the cache.
      queryClient.setQueryData(['auth', 'me'], data);
    },
    onError: () => {
      // Server may have set the session cookie while the client treats the
      // response as an error (non-2xx with cookie, parse failure, aborted
      // request). The fresh login could therefore have happened while stale
      // cache is present — drop it silently. No-op on an already-empty cache.
      void resetSessionCaches(queryClient);
    },
  });

  const loginMutation = useMutation({
    mutationFn: loginApi,
    meta: { skipSessionExpiry: true },
    onSuccess: async (data) => {
      await resetSessionCaches(queryClient);
      // me is written AFTER the clear so it survives (ordering pinned by test).
      queryClient.setQueryData(['auth', 'me'], data);
    },
    onError: () => {
      // Same rationale as register — see above.
      void resetSessionCaches(queryClient);
    },
  });

  const logoutMutation = useMutation({
    mutationFn: logoutApi,
    onSuccess: async () => {
      await resetSessionCaches(queryClient);
      // Belt-and-braces null for observers that re-render between clear and
      // navigate; a cleared cache has no me anyway.
      queryClient.setQueryData(['auth', 'me'], null);
    },
  });

  const register = useCallback<AuthContextType['register']>(
    async (data) => {
      await registerMutation.mutateAsync(data);
    },
    [registerMutation],
  );

  const login = useCallback<AuthContextType['login']>(
    async (data) => {
      await loginMutation.mutateAsync(data);
    },
    [loginMutation],
  );

  const logout = useCallback<AuthContextType['logout']>(async () => {
    await logoutMutation.mutateAsync();
  }, [logoutMutation]);

  return (
    <AuthContext.Provider
      value={{
        user: userResponse?.data ?? null,
        isLoading,
        register,
        login,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}
