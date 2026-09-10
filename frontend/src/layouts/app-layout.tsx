import { useState, useEffect, useCallback } from 'react';
import { Outlet, Navigate, useParams, useNavigate, Link } from 'react-router-dom';
import { useAuth } from '@/features/auth/use-auth';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';
import { LogOut, ChevronDown, Moon, Sun, FileText } from 'lucide-react';
import { useTheme } from '@/hooks/use-theme';
import { BoardNotesSidebar } from '@/features/notes';
import { Breadcrumbs } from './breadcrumbs';
import type { ListResponse, Project } from '@/features/projects/projects.api';

const STORAGE_KEY = 'sidebar-collapsed';

function getStoredCollapsed(): boolean {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'false') return false;
    return true;
  } catch {
    return true;
  }
}

function persistCollapsed(collapsed: boolean) {
  try {
    localStorage.setItem(STORAGE_KEY, String(collapsed));
  } catch {
    // localStorage unavailable
  }
}

export function AppLayout({ projectsData }: { projectsData?: ListResponse<Project> }) {
  const { user, isLoading, logout } = useAuth();
  const { theme, toggleTheme } = useTheme();
  const navigate = useNavigate();
  const { projectId, boardId } = useParams();
  const [collapsed, setCollapsed] = useState(getStoredCollapsed);
  const [loggingOut, setLoggingOut] = useState(false);

  const toggleSidebar = useCallback(() => {
    setCollapsed((prev) => {
      const next = !prev;
      persistCollapsed(next);
      return next;
    });
  }, []);

  // Auto-collapse on tablet/mobile, re-expand on desktop
  useEffect(() => {
    const tablet = window.matchMedia('(min-width: 640px) and (max-width: 1023px)');
    const mobile = window.matchMedia('(max-width: 639px)');

    const handleResize = () => {
      if (mobile.matches) {
        setCollapsed(true);
        persistCollapsed(true);
      } else if (tablet.matches) {
        setCollapsed(true);
        persistCollapsed(true);
      } else {
        // Desktop: restore stored preference
        setCollapsed(getStoredCollapsed());
      }
    };

    handleResize();
    tablet.addEventListener('change', handleResize);
    mobile.addEventListener('change', handleResize);
    return () => {
      tablet.removeEventListener('change', handleResize);
      mobile.removeEventListener('change', handleResize);
    };
  }, []);

  const handleLogout = async () => {
    setLoggingOut(true);
    try {
      await logout();
      navigate('/login');
    } catch {
      // Logout failed — user can retry
    } finally {
      setLoggingOut(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-border border-t-primary" />
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  // Derive breadcrumb data from route params and projects data
  const activeProject = projectId
    ? projectsData?.data.find((p) => String(p.id) === projectId)
    : undefined;
  const breadcrumbProjectName =
    activeProject?.name ?? (projectId ? decodeURIComponent(projectId) : undefined);
  const breadcrumbBoardName = boardId ? decodeURIComponent(boardId) : undefined;

  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-border bg-background px-4">
        <div className="flex items-center gap-2">
          <Link to="/">
            <h1 className="text-lg font-semibold text-foreground">KanPad</h1>
          </Link>
        </div>

        <div className="hidden items-center gap-2 sm:flex">
          <Breadcrumbs
            projectName={breadcrumbProjectName}
            boardName={breadcrumbBoardName}
            projectId={projectId}
          />
        </div>

        <div className="flex items-center gap-1">
          <Link
            to="/notes"
            className="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm text-foreground transition-colors hover:text-foreground"
          >
            <FileText className="h-4 w-4" />
            Notes
          </Link>
          <Button variant="ghost" size="icon-sm" onClick={toggleTheme} aria-label="Toggle theme">
            {theme === 'dark' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" className="flex items-center gap-2">
                <span className="hidden sm:inline">{user.email}</span>
                <ChevronDown className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>{user.email}</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleLogout} disabled={loggingOut}>
                <LogOut className="mr-2 h-4 w-4" />
                {loggingOut ? 'Logging out...' : 'Log out'}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {boardId && (
          <BoardNotesSidebar
            boardId={Number(boardId)}
            collapsed={collapsed}
            onToggle={toggleSidebar}
          />
        )}
        <main className="flex-1 overflow-y-auto bg-background p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
