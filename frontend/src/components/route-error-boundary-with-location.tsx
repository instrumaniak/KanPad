import type { ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import { RouteErrorBoundary } from './route-error-boundary';

/**
 * Navigation-aware boundary wiring for `<RouteErrorBoundary>`.
 *
 * `useLocation()` only works below the router, and `App()` renders
 * `<BrowserRouter>` itself — so the boundary (which lives inside it) cannot be
 * keyed off a location read in `App()`. This wrapper renders inside the router
 * and feeds `location.pathname` as the reset key, so navigation clears a
 * crashed boundary automatically (resetKeys, not a `key` remount — the softer,
 * state-preserving reset).
 */
export function RouteErrorBoundaryWithLocation({ children }: { children: ReactNode }) {
  const location = useLocation();
  return (
    <RouteErrorBoundary resetKeys={[location.pathname]}>
      {children}
    </RouteErrorBoundary>
  );
}
