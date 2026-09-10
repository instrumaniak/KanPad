import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useToast } from '@/components/ui/use-toast';
import { useUpdateBoard, useArchiveBoard, usePermanentDeleteBoard, type Board } from './use-boards';
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
  onEdit: (id: number, name: string) => void;
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
              onEdit(board.id, board.name);
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

interface InlineEditFormProps {
  initialValue: string;
  boardId: number;
  projectId: number | null;
  projects: Array<{ id: number; name: string }>;
  onSave: () => void;
  onCancel: () => void;
  backgroundColor?: string;
}

export function InlineEditForm({
  initialValue,
  boardId,
  projectId,
  projects,
  onSave,
  onCancel,
}: InlineEditFormProps) {
  const [name, setName] = useState(initialValue);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(projectId);
  const updateMutation = useUpdateBoard();
  const { toast } = useToast();

  const handleSave = async () => {
    if (updateMutation.isPending) return;

    const trimmed = name.trim();
    if (!trimmed || (trimmed === initialValue && selectedProjectId !== projectId)) {
      onCancel();
      return;
    }

    try {
      await updateMutation.mutateAsync({
        id: boardId,
        data: {
          name: trimmed !== initialValue ? trimmed : undefined,
          project_id: selectedProjectId !== projectId ? selectedProjectId : undefined,
        },
      });
      toast({ title: 'Board updated', type: 'success' });
      onSave();
    } catch (err) {
      toast({
        title: 'Failed to update board',
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSave();
    } else if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <div className="mb-3">
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleSave}
          aria-label="Board name"
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
          className="font-semibold"
          disabled={updateMutation.isPending}
        />
      </div>

      <div>
        <label className="text-sm font-medium">Project</label>
        <select
          value={selectedProjectId ?? ''}
          onChange={(e) =>
            setSelectedProjectId(e.target.value ? parseInt(e.target.value, 10) : null)
          }
          className="mt-1 flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
        >
          <option value="">No project</option>
          {projects.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>
      </div>
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
  const { toast } = useToast();

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
      toast({
        title: `Failed to ${isArchiveMode ? 'archive' : 'delete'} board`,
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
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
