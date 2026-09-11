import { useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { LoadingSkeleton } from '@/components/loading-skeleton';
import { EmptyState } from '@/components/empty-state';
import { useBoards, useRestoreBoard } from './use-boards';
import { useProjects } from '../projects/use-projects';
import { CreateBoardModal } from './create-board-modal';
import { BoardCard, InlineEditForm, DeleteDialog } from './board-card';
import { useToast } from '@/components/ui/use-toast';
import { Plus, Layout, Archive, FolderKanban } from 'lucide-react';
import { Link } from 'react-router-dom';

export function BoardList() {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingBoard, setEditingBoard] = useState<{ id: number; name: string } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; name: string } | null>(null);

  const { data: boardsData, isLoading, isError, error, refetch } = useBoards();
  const { data: projectsData } = useProjects();
  const restoreMutation = useRestoreBoard();
  const { toast } = useToast();

  const boards = boardsData?.data ?? [];
  const projects = projectsData?.data ?? [];

  const handleEdit = useCallback((id: number, name: string) => {
    setEditingBoard({ id, name });
  }, []);

  const handleEditSave = useCallback(() => {
    setEditingBoard(null);
    refetch();
  }, [refetch]);

  const handleEditCancel = useCallback(() => {
    setEditingBoard(null);
  }, []);

  const handleDelete = useCallback((id: number, name: string) => {
    setDeleteTarget({ id, name });
  }, []);

  if (isError) {
    return (
      <div className="mx-auto max-w-6xl">
        <h1 className="mb-6 text-2xl font-bold">My Boards</h1>
        <div className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive">
          <p className="text-sm font-medium">Failed to load boards</p>
          <p className="text-sm opacity-80">
            {error instanceof Error ? error.message : 'Something went wrong'}
          </p>
          <Button variant="outline" size="sm" className="mt-3" onClick={() => refetch()}>
            Retry
          </Button>
        </div>
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
        <CreateBoardModal
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

      {showCreateModal && (
        <div className="mb-6">
          <CreateBoardModal
            open={showCreateModal}
            onOpenChange={setShowCreateModal}
            onSuccess={() => refetch()}
          />
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
        {boards.map((board) =>
          editingBoard?.id === board.id ? (
            <InlineEditForm
              key={board.id}
              initialValue={editingBoard.name}
              boardId={board.id}
              projectId={board.project_id}
              projects={projects}
              onSave={handleEditSave}
              onCancel={handleEditCancel}
            />
          ) : (
            <BoardCard key={board.id} board={board} onEdit={handleEdit} onDelete={handleDelete} />
          ),
        )}
      </div>

      {deleteTarget &&
        (() => {
          const archivedBoard = { id: deleteTarget.id, name: deleteTarget.name };
          return (
            <DeleteDialog
              boardName={deleteTarget.name}
              boardId={deleteTarget.id}
              open={!!deleteTarget}
              onOpenChange={(open) => {
                if (!open) setDeleteTarget(null);
              }}
              onDeleted={() => {
                setDeleteTarget(null);
                refetch();
                toast({
                  title: 'Board archived',
                  description: `"${archivedBoard.name}" has been archived.`,
                  action: {
                    label: 'Undo',
                    onClick: async () => {
                      try {
                        await restoreMutation.mutateAsync(archivedBoard.id);
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
          );
        })()}
    </div>
  );
}
