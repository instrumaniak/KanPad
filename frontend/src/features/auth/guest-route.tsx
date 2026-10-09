import { Navigate, Outlet } from 'react-router-dom';
import { useAuth } from '@/features/auth/use-auth';
import { Spinner } from '@/components/spinner';

export function GuestRoute() {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="flex h-dvh items-center justify-center bg-background">
        <Spinner size="md" />
      </div>
    );
  }

  if (user) {
    return <Navigate to="/" replace />;
  }

  return <Outlet />;
}
