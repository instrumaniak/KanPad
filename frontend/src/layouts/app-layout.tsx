import { useState, useCallback } from 'react';
import { Outlet, Navigate, useParams, useNavigate, Link } from 'react-router-dom';
import { useAuth } from '@/features/auth/use-auth';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/spinner';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';
import { LogOut, ChevronDown, Moon, Sun, FileText, Menu } from 'lucide-react';
import { useTheme } from '@/hooks/use-theme';
import { useBreakpoint } from '@/hooks/use-breakpoint';
import { BoardNotesSidebar } from '@/features/notes';
import { MobileBottomSheet } from '@/components/mobile-bottom-sheet';
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
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === 'undefined') return true;
    const stored = getStoredCollapsed();
    const width = window.innerWidth;
    if (width < 1024) return true;
    return stored;
  });
  const [loggingOut, setLoggingOut] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const breakpoint = useBreakpoint();
  const isMobile = breakpoint === 'mobile';

  const toggleSidebar = useCallback(() => {
    setCollapsed((prev) => {
      const next = !prev;
      persistCollapsed(next);
      return next;
    });
  }, []);

  // Auto-collapse on tablet/mobile, re-expand on desktop
  // Use derived state from breakpoint instead of setState in effect
  const effectiveCollapsed = breakpoint === 'mobile' || breakpoint === 'tablet' ? true : collapsed;

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
      <div className="flex h-dvh items-center justify-center bg-background">
        <Spinner size="md" />
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
    <div className="flex h-dvh flex-col">
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-border bg-background px-4 pt-[env(safe-area-inset-top)]">
        <div className="flex items-center gap-2">
          {isMobile && boardId && (
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => setMobileSidebarOpen(true)}
              aria-label="Open sidebar"
              className="min-h-[48px] min-w-[48px]"
            >
              <Menu className="h-5 w-5" />
            </Button>
          )}
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

      <div className="flex flex-1 overflow-hidden pb-[env(safe-area-inset-bottom)]">
        {boardId && isMobile && (
          <MobileBottomSheet
            open={mobileSidebarOpen}
            onOpenChange={setMobileSidebarOpen}
            title="Projects"
          >
            <BoardNotesSidebar
              boardId={Number(boardId)}
              collapsed={false}
              onToggle={() => setMobileSidebarOpen(false)}
              isMobile
            />
          </MobileBottomSheet>
        )}
        {boardId && !isMobile && (
          <BoardNotesSidebar
            boardId={Number(boardId)}
            collapsed={effectiveCollapsed}
            onToggle={toggleSidebar}
          />
        )}
        <main className="flex-1 overflow-y-auto bg-background p-4 sm:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
