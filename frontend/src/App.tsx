import { useState, lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from './features/auth/auth-provider';
import { ToastProvider } from './components/ui/toast-provider';
import { useProjects } from './features/projects/use-projects';
import { AppLayout } from './layouts/app-layout';

const RegisterForm = lazy(() =>
  import('./features/auth/register-form').then((m) => ({ default: m.RegisterForm }))
);
const LoginForm = lazy(() =>
  import('./features/auth/login-form').then((m) => ({ default: m.LoginForm }))
);
const ProjectList = lazy(() =>
  import('./features/projects/project-list').then((m) => ({ default: m.ProjectList }))
);
const BoardList = lazy(() =>
  import('./features/boards/board-list').then((m) => ({ default: m.BoardList }))
);
const ArchivedBoards = lazy(() =>
  import('./features/boards/archived-boards').then((m) => ({ default: m.ArchivedBoards }))
);
const BoardView = lazy(() =>
  import('./features/boards/board-view/board-view').then((m) => ({ default: m.BoardView }))
);
const NotesPage = lazy(() =>
  import('./features/notes/note-list').then((m) => ({ default: m.NoteList }))
);
const NoteDetailPage = lazy(() =>
  import('./features/notes/note-detail-page').then((m) => ({ default: m.NoteDetailPage }))
);
const NoteEditPage = lazy(() =>
  import('./features/notes/note-edit-page').then((m) => ({ default: m.NoteEditPage }))
);

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
      <div className="animate-spin h-8 w-8 border-4 border-primary border-t-transparent rounded-full" />
    </div>
  );
}

function AppLayoutRoute() {
  const { data: projectsData } = useProjects();
  return <AppLayout projectsData={projectsData} />;
}

function App() {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 5 * 60 * 1000,
            retry: false,
          },
        },
      }),
  );

  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <ToastProvider>
          <AuthProvider>
            <Suspense fallback={<LoadingFallback />}>
              <Routes>
              <Route path="/register" element={<RegisterForm />} />
              <Route path="/login" element={<LoginForm />} />
              <Route path="/forgot-password" element={<ForgotPasswordPage />} />

              <Route element={<AppLayoutRoute />}>
                <Route path="/" element={<BoardList />} />
                <Route path="/archived-boards" element={<ArchivedBoards />} />
                <Route path="/projects" element={<ProjectList />} />
                <Route path="/notes" element={<NotesPage />} />
                <Route path="/notes/:id" element={<NoteDetailPage />} />
                <Route path="/notes/:id/edit" element={<NoteEditPage />} />
                <Route
                  path="/board/:boardId"
                  element={<BoardView />}
                />
              </Route>

              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
            </Suspense>
          </AuthProvider>
        </ToastProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

export default App;
