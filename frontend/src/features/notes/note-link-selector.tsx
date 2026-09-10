import { useState, useMemo } from 'react';
import { useBoards } from '@/features/boards/use-boards';
import { useProjects } from '@/features/projects/use-projects';
import { useColumns } from '@/features/columns/use-columns';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Link2, X } from 'lucide-react';
import { cn } from '@/lib/utils';

type LinkType = 'board' | 'project' | 'card';

interface NoteLinkSelectorProps {
  linkBoardId: number | undefined;
  linkProjectId: number | undefined;
  linkCardId: number | undefined;
  onLinkBoard: (boardId: number | undefined) => void;
  onLinkProject: (projectId: number | undefined) => void;
  onLinkCard: (cardId: number | undefined) => void;
}

export function NoteLinkSelector({
  linkBoardId,
  linkProjectId,
  linkCardId,
  onLinkBoard,
  onLinkProject,
  onLinkCard,
}: NoteLinkSelectorProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [activeTab, setActiveTab] = useState<LinkType>('board');

  const { data: boardsData, isLoading: boardsLoading } = useBoards();
  const { data: projectsData, isLoading: projectsLoading } = useProjects();
  const { data: columnsData, isLoading: columnsLoading } = useColumns(linkBoardId ?? 0);

  const boards = boardsData?.data ?? [];
  const projects = projectsData?.data ?? [];
  const columns = columnsData ?? [];
  const cards = useMemo(() => columns.flatMap((col) => col.cards ?? []), [columns]);

  const selectedBoard = boards.find((b) => b.id === linkBoardId);
  const selectedProject = projects.find((p) => p.id === linkProjectId);
  const selectedCard = cards.find((c) => c.id === linkCardId);

  const filteredBoards = boards.filter((b) =>
    b.name.toLowerCase().includes(search.toLowerCase()),
  );
  const filteredProjects = projects.filter((p) =>
    p.name.toLowerCase().includes(search.toLowerCase()),
  );
  const filteredCards = cards.filter((c) =>
    c.title.toLowerCase().includes(search.toLowerCase()),
  );

  const currentLinkedName =
    activeTab === 'board'
      ? selectedBoard?.name
      : activeTab === 'project'
        ? selectedProject?.name
        : selectedCard?.title;

  const hasLink = linkBoardId || linkProjectId || linkCardId;

  const handleClear = () => {
    onLinkBoard(undefined);
    onLinkProject(undefined);
    onLinkCard(undefined);
  };

  const handleSelectBoard = (id: number) => {
    onLinkBoard(id);
    onLinkProject(undefined);
    onLinkCard(undefined);
    setOpen(false);
    setSearch('');
  };

  const handleSelectProject = (id: number) => {
    onLinkProject(id);
    onLinkBoard(undefined);
    onLinkCard(undefined);
    setOpen(false);
    setSearch('');
  };

  const handleSelectCard = (id: number) => {
    onLinkCard(id);
    onLinkBoard(undefined);
    onLinkProject(undefined);
    setOpen(false);
    setSearch('');
  };

  return (
    <div className="space-y-1">
      <label className="text-sm font-medium">Linked to</label>
      <div className="flex items-center gap-2">
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="w-full justify-start text-sm font-normal"
            >
              <Link2 className="h-3.5 w-3.5 mr-1.5 text-muted-foreground" />
              {hasLink ? (
                <span className="flex items-center gap-1">
                  <TypeBadge type={linkBoardId ? 'board' : linkProjectId ? 'project' : 'card'} />
                  <span className="truncate">{currentLinkedName}</span>
                </span>
              ) : (
                'None'
              )}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-[280px] p-2" align="start">
            <div className="flex gap-1 mb-2 border-b border-border pb-1">
              {(['board', 'project', 'card'] as LinkType[]).map((tab) => (
                <button
                  key={tab}
                  className={cn(
                    'flex-1 rounded-sm px-2 py-1 text-xs font-medium capitalize transition-colors',
                    activeTab === tab
                      ? 'bg-accent text-accent-foreground'
                      : 'text-muted-foreground hover:bg-accent/50',
                  )}
                  onClick={() => {
                    setActiveTab(tab);
                    setSearch('');
                  }}
                >
                  {tab}
                </button>
              ))}
            </div>
            <Input
              placeholder={`Search ${activeTab}s...`}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-7 text-sm mb-2"
              autoFocus
            />
            <ScrollArea className="max-h-48">
              {activeTab === 'board' && (
                boardsLoading ? (
                  <p className="text-sm text-muted-foreground px-2 py-4 text-center">Loading...</p>
                ) : filteredBoards.length > 0 ? (
                  <div className="space-y-0.5">
                    {filteredBoards.map((board) => (
                      <button
                        key={board.id}
                        className={cn(
                          'flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent text-left',
                          linkBoardId === board.id && 'bg-accent',
                        )}
                        onClick={() => handleSelectBoard(board.id)}
                      >
                        <div
                          className="h-2.5 w-2.5 rounded-full flex-shrink-0"
                          style={{ backgroundColor: board.background_color || '#0079BF' }}
                        />
                        <span className="flex-1 truncate">{board.name}</span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground px-2 py-4 text-center">
                    No boards found
                  </p>
                )
              )}
              {activeTab === 'project' && (
                projectsLoading ? (
                  <p className="text-sm text-muted-foreground px-2 py-4 text-center">Loading...</p>
                ) : filteredProjects.length > 0 ? (
                  <div className="space-y-0.5">
                    {filteredProjects.map((project) => (
                      <button
                        key={project.id}
                        className={cn(
                          'flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent text-left',
                          linkProjectId === project.id && 'bg-accent',
                        )}
                        onClick={() => handleSelectProject(project.id)}
                      >
                        <span className="flex-1 truncate">{project.name}</span>
                        <span className="text-xs text-muted-foreground">
                          {project.boardCount} board{project.boardCount !== 1 ? 's' : ''}
                        </span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground px-2 py-4 text-center">
                    No projects found
                  </p>
                )
              )}
              {activeTab === 'card' && (
                columnsLoading ? (
                  <p className="text-sm text-muted-foreground px-2 py-4 text-center">Loading...</p>
                ) : filteredCards.length > 0 ? (
                  <div className="space-y-0.5">
                    {filteredCards.map((card) => (
                      <button
                        key={card.id}
                        className={cn(
                          'flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent text-left',
                          linkCardId === card.id && 'bg-accent',
                        )}
                        onClick={() => handleSelectCard(card.id)}
                      >
                        <span className="flex-1 truncate">{card.title}</span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground px-2 py-4 text-center">
                    {columns.length === 0 ? 'No columns on this board' : 'No cards found'}
                  </p>
                )
              )}
            </ScrollArea>
          </PopoverContent>
        </Popover>
        {hasLink && (
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={handleClear}
            aria-label="Remove link"
          >
            <X className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>
    </div>
  );
}

function TypeBadge({ type }: { type: LinkType }) {
  const labels: Record<LinkType, string> = {
    board: 'Board',
    project: 'Project',
    card: 'Card',
  };
  return (
    <span className="inline-flex items-center rounded-sm bg-muted px-1.5 py-0.5 text-[10px] font-medium uppercase text-muted-foreground">
      {labels[type]}
    </span>
  );
}
