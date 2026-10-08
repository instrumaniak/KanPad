import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useToastHelpers } from '@/lib/toast-helpers';
import { useCreateBoard, useUpdateBoard } from './use-boards';
import { useProjects } from '../projects/use-projects';
import { Plus, Pencil } from 'lucide-react';

export interface BoardFormInitialData {
  id: number;
  name: string;
  project_id: number | null;
}

interface BoardFormModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess?: () => void;
  mode: 'create' | 'edit';
  initialBoard?: BoardFormInitialData | null;
}

export function BoardFormModal({
  open,
  onOpenChange,
  onSuccess,
  mode,
  initialBoard,
}: BoardFormModalProps) {
  const isEdit = mode === 'edit';
  const formKey = isEdit ? `edit-${initialBoard?.id ?? 'none'}` : 'create';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        <BoardFormContent
          key={formKey}
          mode={mode}
          initialBoard={initialBoard}
          onOpenChange={onOpenChange}
          onSuccess={onSuccess}
        />
      ) : null}
    </Dialog>
  );
}

interface BoardFormFieldsProps {
  name: string;
  selectedProjectId: number | null;
  onNameChange: (value: string) => void;
  onProjectChange: (value: number | null) => void;
  disabled: boolean;
  projects: Array<{ id: number; name: string }>;
}

export function BoardFormFields({
  name,
  selectedProjectId,
  onNameChange,
  onProjectChange,
  disabled,
  projects,
}: BoardFormFieldsProps) {
  return (
    <div className="grid gap-4 py-4">
      <div className="grid gap-2">
        <label htmlFor="board-name" className="text-sm font-medium">
          Board name
        </label>
        <Input
          id="board-name"
          value={name}
          onChange={(e) => onNameChange(e.target.value)}
          placeholder="My awesome board"
          disabled={disabled}
          autoFocus
        />
      </div>

      <div className="grid gap-2">
        <label htmlFor="project" className="text-sm font-medium">
          Project (optional)
        </label>
        <select
          id="project"
          value={selectedProjectId ?? ''}
          onChange={(e) =>
            onProjectChange(e.target.value ? parseInt(e.target.value, 10) : null)
          }
          className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={disabled}
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

function BoardFormContent({
  mode,
  initialBoard,
  onOpenChange,
  onSuccess,
}: Pick<BoardFormModalProps, 'mode' | 'initialBoard' | 'onOpenChange' | 'onSuccess'>) {
  const isEdit = mode === 'edit';
  const initialName = isEdit ? (initialBoard?.name ?? '') : '';
  const initialProjectId = isEdit ? (initialBoard?.project_id ?? null) : null;

  const [name, setName] = useState(initialName);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(initialProjectId);

  const { showSuccess, showError } = useToastHelpers();
  const createBoard = useCreateBoard();
  const updateBoard = useUpdateBoard();
  const { data: projectsData } = useProjects();
  const navigate = useNavigate();

  const isPending = createBoard.isPending || updateBoard.isPending;
  const projects = projectsData?.data ?? [];
  const trimmed = name.trim();
  const isPristine = isEdit && trimmed === initialName && selectedProjectId === initialProjectId;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!trimmed || isPending) return;

    if (isEdit) {
      if (!initialBoard) {
        onOpenChange(false);
        return;
      }
      if (isPristine) {
        onOpenChange(false);
        return;
      }
      try {
        await updateBoard.mutateAsync({
          id: initialBoard.id,
          data: {
            name: trimmed !== initialName ? trimmed : undefined,
            project_id: selectedProjectId !== initialProjectId ? selectedProjectId : undefined,
          },
        });
        showSuccess('Board updated');
        onOpenChange(false);
        onSuccess?.();
      } catch (err) {
        showError(
          'Failed to update board',
          err instanceof Error ? err.message : 'Something went wrong',
        );
      }
      return;
    }

    try {
      const response = await createBoard.mutateAsync({
        name: trimmed,
        project_id: selectedProjectId,
      });
      showSuccess('Board created');
      onOpenChange(false);
      onSuccess?.();
      navigate(`/board/${response.data.id}`);
    } catch (err) {
      showError(
        'Failed to create board',
        err instanceof Error ? err.message : 'Something went wrong',
      );
    }
  };

  return (
    <DialogContent className="sm:max-w-[425px]">
      <form onSubmit={handleSubmit}>
        <DialogHeader>
          <DialogTitle>{isEdit ? 'Edit board' : 'Create new board'}</DialogTitle>
          <DialogDescription>
            {isEdit ? 'Update your board details' : 'Give your board a name'}
          </DialogDescription>
        </DialogHeader>

        <BoardFormFields
          name={name}
          selectedProjectId={selectedProjectId}
          onNameChange={setName}
          onProjectChange={setSelectedProjectId}
          disabled={isPending}
          projects={projects}
        />

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isPending}
          >
            Cancel
          </Button>
          <Button type="submit" disabled={!trimmed || isPending || isPristine}>
            {isPending ? (
              <>
                {isEdit ? (
                  <Pencil className="mr-1 h-4 w-4 animate-spin" />
                ) : (
                  <Plus className="mr-1 h-4 w-4 animate-spin" />
                )}
                {isEdit ? 'Saving...' : 'Creating...'}
              </>
            ) : isEdit ? (
              <>
                <Pencil className="mr-1 h-4 w-4" />
                Save changes
              </>
            ) : (
              <>
                <Plus className="mr-1 h-4 w-4" />
                Create Board
              </>
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  );
}
