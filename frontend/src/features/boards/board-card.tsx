import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { useToastHelpers } from '@/lib/toast-helpers';
import { useArchiveBoard, usePermanentDeleteBoard, type Board } from './use-boards';
import { Pencil, Trash2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

interface BoardCardProps {
  board: Board;
  onEdit: (board: Board) => void;
  onDelete: (id: number, name: string) => void;
}

export function BoardCard({ board, onEdit, onDelete }: BoardCardProps) {
  const navigate = useNavigate();

  return (
    <div
      className="group relative flex h-28 cursor-pointer flex-col justify-between rounded-lg border border-border bg-card p-4 transition-all hover:scale-[1.02] hover:shadow-sm"
      onClick={() => navigate(`/board/${board.id}`)}
    >
      <div className="flex items-start justify-between">
        <div className="flex-1 truncate font-semibold text-foreground">{board.name}</div>
        <div className="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          <Button
            variant="ghost"
            size="icon-sm"
            className="h-7 w-7"
            onClick={(e) => {
              e.stopPropagation();
              onEdit(board);
            }}
            aria-label={`Edit board ${board.name}`}
          >
            <Pencil className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            className="h-7 w-7"
            onClick={(e) => {
              e.stopPropagation();
              onDelete(board.id, board.name);
            }}
            aria-label={`Delete board ${board.name}`}
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </div>
      {board.project && (
        <div className="text-xs text-muted-foreground">{board.project.name}</div>
      )}
    </div>
  );
}

export function DeleteDialog({
  boardName,
  boardId,
  open,
  onOpenChange,
  onDeleted,
  mode = 'archive',
}: {
  boardName: string;
  boardId: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted: () => void;
  mode?: 'archive' | 'permanent';
}) {
  const archiveMutation = useArchiveBoard();
  const permanentDeleteMutation = usePermanentDeleteBoard();
  const { showError } = useToastHelpers();

  const isArchiveMode = mode === 'archive';
  const mutation = isArchiveMode ? archiveMutation : permanentDeleteMutation;

  const handleDelete = async () => {
    try {
      if (isArchiveMode) {
        await archiveMutation.mutateAsync(boardId);
      } else {
        await permanentDeleteMutation.mutateAsync(boardId);
      }
      onOpenChange(false);
      onDeleted();
    } catch (err) {
      showError(
        `Failed to ${isArchiveMode ? 'archive' : 'delete'} board`,
        err instanceof Error ? err.message : 'Something went wrong',
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isArchiveMode ? 'Archive board' : 'Delete board permanently'}</DialogTitle>
          <DialogDescription>
            {isArchiveMode
              ? `Archive board "${boardName}"? The board will be moved to archived boards and can be restored later.`
              : `Permanently delete board "${boardName}"? This action cannot be undone and all columns and cards will be deleted.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            variant={isArchiveMode ? 'default' : 'destructive'}
            onClick={handleDelete}
            disabled={mutation.isPending}
          >
            {mutation.isPending
              ? isArchiveMode
                ? 'Archiving...'
                : 'Deleting...'
              : isArchiveMode
                ? 'Archive'
                : 'Delete Permanently'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
