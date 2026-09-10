import { useEffect, useRef, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useBoard, useUpdateBoard, type BoardViewMode } from '../use-boards';
import { useColumns, useCreateColumn } from '../../columns/use-columns';
import { Column } from '../../columns/column';
import { AddColumnButton } from '../../columns/add-column-button';
import { useToast } from '@/components/ui/use-toast';
import { DragDropContext } from '../../cards/drag-drop-context';
import { BoardViewToggle } from './board-view-toggle';
import { BoardListView } from './board-list-view';

export function BoardView() {
  const { boardId } = useParams<{ boardId: string }>();
  const navigate = useNavigate();
  const id = boardId ? parseInt(boardId, 10) : 0;

  const { data: boardResponse, isLoading: boardLoading } = useBoard(id);
  const { data: columns, isLoading: columnsLoading } = useColumns(id);

  const board = boardResponse?.data;
  const boardName = boardResponse?.data?.name;
  const createColumnMutation = useCreateColumn();
  const updateBoardMutation = useUpdateBoard();
  const { toast } = useToast();

  const [view, setView] = useState<BoardViewMode>('board');
  const scrollRef = useRef<{ board: number; list: number }>({ board: 0, list: 0 });
  const boardScrollRef = useRef<HTMLDivElement>(null);
  const listScrollRef = useRef<HTMLDivElement>(null);
  const lastServerView = useRef<BoardViewMode | undefined>(undefined);
  const viewRequestId = useRef(0);

  // Sync local view when the persisted server value arrives/changes.
  // Guarded by ref so we only set state on actual server changes
  // (never clobbers an optimistic pending view on unrelated refetches).
  useEffect(() => {
    const serverView = board?.view_mode;
    if (
      (serverView === 'board' || serverView === 'list') &&
      serverView !== lastServerView.current
    ) {
      lastServerView.current = serverView;
      setView(serverView);
    }
  }, [board?.view_mode]);

  const getListScroller = (): Element | null =>
    listScrollRef.current?.querySelector('[data-testid="board-list-scroll"]') ?? null;

  const handleViewChange = (next: BoardViewMode) => {
    if (next === view || updateBoardMutation.isPending) return;
    if (boardScrollRef.current) scrollRef.current.board = boardScrollRef.current.scrollLeft;
    const listScroll = getListScroller();
    if (listScroll) scrollRef.current.list = listScroll.scrollTop;
    const prev = view;
    const requestId = ++viewRequestId.current;
    setView(next);
    updateBoardMutation.mutate(
      { id, data: { view_mode: next } },
      {
        onError: (err) => {
          if (viewRequestId.current !== requestId) return;
          setView(prev);
          toast({
            title: 'Failed to save view',
            description: err instanceof Error ? err.message : 'Something went wrong',
            type: 'error',
          });
        },
      },
    );
  };

  useEffect(() => {
    if (view === 'board' && boardScrollRef.current) {
      boardScrollRef.current.scrollLeft = scrollRef.current.board;
    }
    if (view === 'list') {
      const listScroll = getListScroller();
      if (listScroll) listScroll.scrollTop = scrollRef.current.list;
    }
  }, [view]);

  const handleAddColumn = async () => {
    try {
      await createColumnMutation.mutateAsync({ boardId: id, data: {} });
      toast({ title: 'Column added', type: 'success' });
    } catch (err) {
      toast({
        title: 'Failed to add column',
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
    }
  };

  if (boardLoading || columnsLoading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (!board) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-muted-foreground">Board not found</p>
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <div className="flex shrink-0 items-center gap-4 px-6 py-4">
        <Button variant="ghost" size="icon" onClick={() => navigate('/')}>
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <h1 className="text-xl font-semibold">{boardName}</h1>
        <div className="ml-auto">
          <BoardViewToggle
            value={view}
            onChange={handleViewChange}
            disabled={updateBoardMutation.isPending}
          />
        </div>
      </div>

      <div className="transition-opacity duration-300 animate-in fade-in motion-safe:animate-in motion-safe:fade-in">
        {view === 'board' ? (
          <div className="flex flex-1 overflow-hidden">
            <div className="flex-1 overflow-x-auto" ref={boardScrollRef}>
              <DragDropContext boardId={id}>
                <div className="flex h-full gap-6 p-6 pb-6">
                  {columns?.map((column) => (
                    <Column key={column.id} column={column} allColumns={columns} />
                  ))}
                  <AddColumnButton onClick={handleAddColumn} />
                </div>
              </DragDropContext>
            </div>
          </div>
        ) : (
          <div ref={listScrollRef} className="flex flex-1 flex-col overflow-hidden">
            <BoardListView boardId={id} columns={columns ?? []} />
          </div>
        )}
      </div>
    </div>
  );
}
