import { useState, useEffect, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClientProvider } from '@tanstack/react-query';
import { createAppQueryClient } from './lib/app-query-client';
import { AuthProvider } from './features/auth/auth-provider';
import { ToastProvider } from './components/ui/toast-provider';
import { GuestRoute } from './features/auth/guest-route';
import { AppLayout } from './layouts/app-layout';
import { Spinner } from './components/spinner';
import { RouteErrorBoundaryWithLocation } from './components/route-error-boundary-with-location';
import { SessionExpiryHandler } from './components/session-expiry-handler';
import { RegisterForm } from './features/auth/register-form';
import { LoginForm } from './features/auth/login-form';
import { ProjectList } from './features/projects/project-list';
import { BoardList } from './features/boards/board-list';
import { ArchivedBoards } from './features/boards/archived-boards';
import { BoardView } from './features/boards/board-view/board-view';
import { NoteList } from './features/notes/note-list';
import { NoteDetailPage } from './features/notes/note-detail-page';
import { NoteEditPage } from './features/notes/note-edit-page';

function ForgotPasswordPage() {
  return (
    <div className="flex min-h-screen items-center justify-center">
      <div className="text-center">
        <h1 className="text-2xl font-bold">Forgot Password</h1>
        <p className="text-muted-foreground">Coming soon</p>
      </div>
    </div>
  );
}

function LoadingFallback() {
  return (
    <div className="flex min-h-screen items-center justify-center">
      <Spinner size="lg" />
    </div>
  );
}

function App() {
  const [queryClient] = useState(() => createAppQueryClient());

  // Idle prefetch the lazy markdown payload after first paint. Silent on
  // failure — the lazy boundary still loads on demand.
  useEffect(() => {
    const prefetch = () => {
      void import('./features/notes/markdown-renderer').catch(() => {});
    };
    if ('requestIdleCallback' in window) {
      const id = window.requestIdleCallback(prefetch, { timeout: 3000 });
      return () => window.cancelIdleCallback(id);
    }
    const t = setTimeout(prefetch, 1500);
    return () => clearTimeout(t);
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <ToastProvider>
          <SessionExpiryHandler />
          <AuthProvider>
            <Suspense fallback={<LoadingFallback />}>
              <RouteErrorBoundaryWithLocation>
                <Routes>
                  <Route element={<GuestRoute />}>
                    <Route path="/register" element={<RegisterForm />} />
                    <Route path="/login" element={<LoginForm />} />
                    <Route path="/forgot-password" element={<ForgotPasswordPage />} />
                  </Route>

                  <Route element={<AppLayout />}>
                    <Route path="/" element={<BoardList />} />
                    <Route path="/archived-boards" element={<ArchivedBoards />} />
                    <Route path="/projects" element={<ProjectList />} />
                    <Route path="/notes" element={<NoteList />} />
                    <Route path="/notes/:id" element={<NoteDetailPage />} />
                    <Route path="/notes/:id/edit" element={<NoteEditPage />} />
                    <Route
                      path="/board/:boardId"
                      element={<BoardView />}
                    />
                  </Route>

                  <Route path="*" element={<Navigate to="/" replace />} />
                </Routes>
              </RouteErrorBoundaryWithLocation>
            </Suspense>
          </AuthProvider>
        </ToastProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

export default App;
