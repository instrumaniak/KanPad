import { useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { LoadingSkeleton } from '@/components/loading-skeleton';
import { EmptyState } from '@/components/empty-state';
import { ErrorState } from '@/components/error-state';
import { useBoards, useRestoreBoard, type Board } from './use-boards';
import { BoardFormModal } from './board-form-modal';
import { BoardCard, DeleteDialog } from './board-card';
import { useToast } from '@/components/ui/use-toast';
import { Plus, Layout, Archive, FolderKanban } from 'lucide-react';
import { Link } from 'react-router-dom';

export function BoardList() {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingBoard, setEditingBoard] = useState<Board | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; name: string } | null>(null);

  const { data: boardsData, isLoading, isError, error, refetch } = useBoards();
  const restoreMutation = useRestoreBoard();
  const { toast } = useToast();

  const boards = boardsData?.data ?? [];

  const handleEdit = useCallback((board: Board) => {
    setEditingBoard(board);
  }, []);

  const handleDelete = useCallback((id: number, name: string) => {
    setDeleteTarget({ id, name });
  }, []);

  if (isError) {
    return (
      <div className="mx-auto max-w-6xl">
        <h1 className="mb-6 text-2xl font-bold">My Boards</h1>
        <ErrorState
          title="Failed to load boards"
          message={error instanceof Error ? error.message : 'Something went wrong'}
          onRetry={() => refetch()}
        />
      </div>
    );
  }

  const showEmptyState = boards.length === 0 && !showCreateModal && !isLoading;

  if (showEmptyState) {
    return (
      <div className="mx-auto max-w-6xl">
        <div className="mb-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <h1 className="text-2xl font-bold">My Boards</h1>
          <div className="grid grid-cols-2 gap-2 sm:flex">
            <Button variant="outline" className="w-full sm:w-auto" asChild>
              <Link to="/archived-boards">
                <Archive className="mr-1 h-4 w-4" />
                Archived Boards
              </Link>
            </Button>
            <Button className="w-full sm:w-auto" onClick={() => setShowCreateModal(true)}>
              <Plus className="mr-1 h-4 w-4" />
              Create Board
            </Button>
          </div>
        </div>
        <EmptyState
          icon={<Layout className="h-8 w-8 text-muted-foreground" />}
          headline="No boards yet"
          description="Create your first board to start organizing your tasks"
          action={{
            label: 'Create your first board',
            onClick: () => setShowCreateModal(true),
          }}
        />
        <BoardFormModal
          mode="create"
          open={showCreateModal}
          onOpenChange={setShowCreateModal}
          onSuccess={() => refetch()}
        />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-6xl">
      <div className="mb-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-2xl font-bold">My Boards</h1>
        <div className="grid grid-cols-2 gap-2 sm:flex">
          <Button variant="outline" className="w-full sm:w-auto" asChild>
            <Link to="/projects">
              <FolderKanban className="mr-1 h-4 w-4" />
              Projects
            </Link>
          </Button>
          <Button variant="outline" className="w-full sm:w-auto" asChild>
            <Link to="/archived-boards">
              <Archive className="mr-1 h-4 w-4" />
              Archived Boards
            </Link>
          </Button>
          <Button className="col-span-2 w-full sm:col-auto sm:w-auto mt-2 sm:mt-0" onClick={() => setShowCreateModal(true)}>
            <Plus className="mr-1 h-4 w-4" />
            Create Board
          </Button>
        </div>
      </div>

      {isLoading ? <LoadingSkeleton /> : null}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
        {boards.map((board) => (
          <BoardCard key={board.id} board={board} onEdit={handleEdit} onDelete={handleDelete} />
        ))}
      </div>

      <BoardFormModal
        mode="create"
        open={showCreateModal}
        onOpenChange={setShowCreateModal}
        onSuccess={() => refetch()}
      />

      <BoardFormModal
        mode="edit"
        open={editingBoard ? true : false}
        onOpenChange={(open) => {
          if (!open) setEditingBoard(null);
        }}
        onSuccess={() => {
          setEditingBoard(null);
          refetch();
        }}
        initialBoard={
          editingBoard
            ? { id: editingBoard.id, name: editingBoard.name, project_id: editingBoard.project_id }
            : null
        }
      />

      {deleteTarget ? (
        <DeleteDialog
          boardName={deleteTarget.name}
          boardId={deleteTarget.id}
          open={!!deleteTarget}
          onOpenChange={(open) => {
            if (!open) setDeleteTarget(null);
          }}
          onDeleted={() => {
            const deletedId = deleteTarget.id;
            const deletedName = deleteTarget.name;
            setDeleteTarget(null);
            refetch();
            toast({
              title: 'Board archived',
              description: `"${deletedName}" has been archived.`,
              action: {
                label: 'Undo',
                onClick: async () => {
                  try {
                    await restoreMutation.mutateAsync(deletedId);
                    refetch();
                    toast({
                      title: 'Board restored',
                      description: 'The board has been restored successfully.',
                    });
                  } catch (err) {
                    toast({
                      type: 'destructive',
                      title: 'Failed to restore board',
                      description: err instanceof Error ? err.message : 'Something went wrong',
                    });
                  }
                },
              },
            });
          }}
          mode="archive"
        />
      ) : null}
    </div>
  );
}
