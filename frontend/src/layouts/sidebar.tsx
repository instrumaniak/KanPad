import { ChevronLeft, ChevronRight, FolderKanban } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
  projects?: Array<{ id: string; name: string }>;
  activeProjectId?: string;
  onProjectClick?: (projectId: string) => void;
  isMobile?: boolean;
}

export function Sidebar({
  collapsed,
  onToggle,
  projects = [],
  activeProjectId,
  onProjectClick,
  isMobile = false,
}: SidebarProps) {
  const content = (
    <>
      {!isMobile && (
        <div className="flex items-center justify-end p-2">
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onToggle}
            aria-label="Toggle sidebar"
            className="shrink-0 text-sidebar-foreground hover:bg-sidebar-accent"
          >
            {collapsed ? (
              <ChevronRight className="h-4 w-4" />
            ) : (
              <ChevronLeft className="h-4 w-4" />
            )}
          </Button>
        </div>
      )}

      <nav className="flex-1 overflow-y-auto px-2 pb-4">
        <p className="mb-2 px-2 text-xs font-semibold uppercase tracking-wider text-sidebar-foreground/60">
          Projects
        </p>
        <ul className="space-y-0.5">
          {projects.length === 0 ? (
            <li className="px-2 py-1.5 text-sm text-sidebar-foreground/50">
              No projects yet
            </li>
          ) : (
            projects.map((project) => (
              <li key={project.id}>
                <button
                  onClick={() => onProjectClick?.(project.id)}
                  className={cn(
                    'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-sidebar-foreground transition-colors hover:bg-sidebar-accent',
                    activeProjectId === project.id &&
                      'bg-sidebar-accent font-medium',
                  )}
                >
                  <FolderKanban className="h-4 w-4 shrink-0" />
                  <span className="truncate">{project.name}</span>
                </button>
              </li>
            ))
          )}
        </ul>
      </nav>
    </>
  );

  if (isMobile) {
    return <>{content}</>;
  }

  return (
    <aside
      className={cn(
        'flex h-full flex-col border-r border-sidebar-border bg-sidebar transition-[width] duration-300 ease-in-out overflow-hidden',
        collapsed ? 'w-0' : 'w-[240px]',
      )}
    >
      {content}
    </aside>
  );
}
