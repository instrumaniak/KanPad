import { useCallback, useState } from 'react';
import { Button } from '@/components/ui/button';
import { LoadingSkeleton } from '@/components/loading-skeleton';
import { EmptyState } from '@/components/empty-state';
import { ErrorState } from '@/components/error-state';
import {
  useArchivedBoards,
  useRestoreBoard,
  usePermanentDeleteBoard,
  type Board,
} from './use-boards';
import { DeleteDialog } from './board-card';
import { Archive, RotateCcw, Trash2 } from 'lucide-react';
import { useToastHelpers } from '@/lib/toast-helpers';

export function ArchivedBoards() {
  const { data: boardsData, isLoading, isError, error, refetch } = useArchivedBoards();
  const restoreMutation = useRestoreBoard();
  const deleteMutation = usePermanentDeleteBoard();
  const { showSuccess, showError } = useToastHelpers();
  const [deleteDialog, setDeleteDialog] = useState<{ open: boolean; board: Board | null }>({
    open: false,
    board: null,
  });

  const boards = boardsData?.data ?? [];

  const handleRestore = useCallback(
    async (board: Board) => {
      try {
        await restoreMutation.mutateAsync(board.id);
        showSuccess('Board restored', `"${board.name}" has been restored.`);
        refetch();
      } catch (err) {
        showError(
          'Error',
          err instanceof Error ? err.message : 'Failed to restore board',
        );
      }
    },
    [restoreMutation, showSuccess, showError, refetch]
  );

  const handlePermanentDelete = useCallback(
    async (board: Board) => {
      setDeleteDialog({ open: true, board });
    },
    []
  );

  const handlePermanentDeleteConfirm = useCallback(async () => {
    if (!deleteDialog.board) return;
    try {
      await deleteMutation.mutateAsync(deleteDialog.board.id);
      showSuccess('Board deleted', `"${deleteDialog.board.name}" has been permanently deleted.`);
      setDeleteDialog({ open: false, board: null });
      refetch();
    } catch (err) {
      showError(
        'Error',
        err instanceof Error ? err.message : 'Failed to delete board',
      );
    }
  }, [deleteMutation, showSuccess, showError, refetch, deleteDialog.board]);

  if (isLoading) {
    return (
      <div className="mx-auto max-w-6xl">
        <h1 className="mb-6 text-2xl font-bold">Archived Boards</h1>
        <LoadingSkeleton />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="mx-auto max-w-6xl">
        <h1 className="mb-6 text-2xl font-bold">Archived Boards</h1>
        <ErrorState
          title="Failed to load archived boards"
          message={error instanceof Error ? error.message : 'Something went wrong'}
          onRetry={() => refetch()}
        />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-6xl">
      <h1 className="mb-6 text-2xl font-bold">Archived Boards</h1>

      {boards.length === 0 ? (
        <EmptyState
          icon={<Archive className="h-8 w-8 text-muted-foreground" />}
          headline="No archived boards"
          description="Boards you archive will appear here"
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
          {boards.map((board) => (
            <div
              key={board.id}
              className="relative overflow-hidden rounded-lg border bg-card p-4 shadow-sm"
            >
              <h3 className="mb-3 truncate font-semibold">{board.name}</h3>
              {board.project && (
                <p className="mb-4 text-sm text-muted-foreground">
                  Project: {board.project.name}
                </p>
              )}
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant="default"
                  onClick={() => handleRestore(board)}
                  disabled={restoreMutation.isPending}
                >
                  <RotateCcw className="mr-1 h-4 w-4" />
                  Restore
                </Button>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => handlePermanentDelete(board)}
                  disabled={deleteMutation.isPending}
                >
                  <Trash2 className="mr-1 h-4 w-4" />
                  Delete
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      <DeleteDialog
        boardName={deleteDialog.board?.name ?? ''}
        boardId={deleteDialog.board?.id ?? 0}
        open={deleteDialog.open}
        onOpenChange={(open) => setDeleteDialog((prev) => ({ ...prev, open }))}
        onDeleted={handlePermanentDeleteConfirm}
        mode="permanent"
      />
    </div>
  );
}