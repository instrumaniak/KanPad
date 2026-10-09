import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useToastHelpers } from '@/lib/toast-helpers';
import { useCreateBoard, useUpdateBoard } from './use-boards';
import { useCreateProject, useProjects } from '../projects/use-projects';
import { Plus, Pencil } from 'lucide-react';

export type BoardProjectMode = 'select' | 'new';

export const NO_PROJECT_VALUE = 'none';

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
  const [isPending, setIsPending] = useState(false);
  // Shared synchronously with the form content: set at submit start, cleared in
  // `finally`. Covers the pre-re-render window where `isPending` is stale.
  const submittingRef = useRef(false);

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen && (submittingRef.current || isPending)) return;
    onOpenChange(nextOpen);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      {open ? (
        <BoardFormContent
          key={formKey}
          mode={mode}
          initialBoard={initialBoard}
          onOpenChange={handleOpenChange}
          onSuccess={onSuccess}
          // Raw close for programmatic submit paths (success, pristine):
          // nothing is in flight on those paths, and the guarded handler
          // would see the just-set submittingRef and refuse to close.
          onClose={() => onOpenChange(false)}
          onPendingChange={setIsPending}
          submittingRef={submittingRef}
        />
      ) : null}
    </Dialog>
  );
}

interface BoardFormFieldsProps {
  name: string;
  selectedProjectId: number | null;
  projectMode: BoardProjectMode;
  newProjectName: string;
  onNameChange: (value: string) => void;
  onProjectChange: (value: number | null) => void;
  onProjectModeChange: (mode: BoardProjectMode) => void;
  onNewProjectNameChange: (value: string) => void;
  disabled: boolean;
  projects: Array<{ id: number; name: string }>;
}

export function BoardFormFields({
  name,
  selectedProjectId,
  projectMode,
  newProjectName,
  onNameChange,
  onProjectChange,
  onProjectModeChange,
  onNewProjectNameChange,
  disabled,
  projects,
}: BoardFormFieldsProps) {
  const isNewMode = projectMode === 'new';

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
        <div className="flex items-center justify-between">
          <label
            htmlFor={isNewMode ? 'new-project-name' : 'project-select'}
            className="text-sm font-medium"
          >
            {isNewMode ? 'New Project Name' : 'Project (optional)'}
          </label>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onProjectModeChange(isNewMode ? 'select' : 'new')}
            disabled={disabled}
            aria-pressed={isNewMode}
          >
            {isNewMode ? (
              'Select Existing'
            ) : (
              <>
                <Plus className="mr-1 h-4 w-4" />
                New Project
              </>
            )}
          </Button>
        </div>
        {isNewMode ? (
          <Input
            id="new-project-name"
            value={newProjectName}
            onChange={(e) => onNewProjectNameChange(e.target.value)}
            placeholder="Enter project name..."
            disabled={disabled}
          />
        ) : (
          <Select
            value={selectedProjectId != null ? String(selectedProjectId) : NO_PROJECT_VALUE}
            onValueChange={(value) => {
              if (value === NO_PROJECT_VALUE) {
                onProjectChange(null);
                return;
              }
              const id = Number(value);
              onProjectChange(Number.isInteger(id) ? id : null);
            }}
            disabled={disabled}
          >
            <SelectTrigger id="project-select" className="w-full">
              <SelectValue placeholder="No project" />
            </SelectTrigger>
            <SelectContent position="popper">
              <SelectGroup>
                <SelectItem value={NO_PROJECT_VALUE}>No project</SelectItem>
                {projects.map((project) => (
                  <SelectItem key={project.id} value={String(project.id)}>
                    {project.name}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        )}
      </div>
    </div>
  );
}

function BoardFormContent({
  mode,
  initialBoard,
  onOpenChange,
  onSuccess,
  onClose,
  onPendingChange,
  submittingRef,
}: Pick<BoardFormModalProps, 'mode' | 'initialBoard' | 'onOpenChange' | 'onSuccess'> & {
  onClose: () => void;
  onPendingChange: (pending: boolean) => void;
  submittingRef: { current: boolean };
}) {
  const isEdit = mode === 'edit';
  const initialName = isEdit ? (initialBoard?.name ?? '') : '';
  const initialProjectId = isEdit ? (initialBoard?.project_id ?? null) : null;

  const [name, setName] = useState(initialName);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(initialProjectId);
  const [projectMode, setProjectMode] = useState<BoardProjectMode>('select');
  const [newProjectName, setNewProjectName] = useState('');
  // Already-created projects keyed by trimmed name, so a board failure after a
  // successful project creation doesn't orphan projects or create duplicates
  // when the user retries (same name reuses, changed name creates anew).
  const createdProjectsRef = useRef(new Map<string, number>());

  const { showSuccess, showError } = useToastHelpers();
  const createBoard = useCreateBoard();
  const updateBoard = useUpdateBoard();
  const createProject = useCreateProject();
  const { data: projectsData } = useProjects();
  const navigate = useNavigate();

  const isPending = createBoard.isPending || updateBoard.isPending || createProject.isPending;
  const projects = projectsData?.data ?? [];
  const trimmed = name.trim();
  const trimmedNewProject = newProjectName.trim();
  const isNewProjectMode = projectMode === 'new';
  const isPristine =
    isEdit && !isNewProjectMode && trimmed === initialName && selectedProjectId === initialProjectId;
  const isSubmitDisabled =
    !trimmed || (isNewProjectMode && !trimmedNewProject) || isPending || isPristine;

  useEffect(() => {
    onPendingChange(isPending);
  }, [isPending, onPendingChange]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!trimmed || isPending) return;
    if (isNewProjectMode && !trimmedNewProject) return;
    if (isEdit && !initialBoard) {
      // Nothing to submit and nothing created yet: close before engaging guards.
      onClose();
      return;
    }
    // Narrowed for the edit path below (isEdit implies initialBoard here).
    const boardToEdit = isEdit ? initialBoard : undefined;
    if (submittingRef.current) return;
    submittingRef.current = true;

    try {
      let projectId = selectedProjectId;
      if (isNewProjectMode) {
        const cachedId = createdProjectsRef.current.get(trimmedNewProject);
        if (cachedId !== undefined) {
          projectId = cachedId;
        } else {
          try {
            const projectResponse = await createProject.mutateAsync(trimmedNewProject);
            projectId = projectResponse.data.id;
            createdProjectsRef.current.set(trimmedNewProject, projectId);
          } catch (err) {
            showError(
              'Failed to create project',
              err instanceof Error ? err.message : 'Something went wrong',
            );
            return;
          }
        }
      }

      if (boardToEdit) {
        if (isPristine) {
          onClose();
          return;
        }
        try {
          await updateBoard.mutateAsync({
            id: boardToEdit.id,
            data: {
              name: trimmed !== initialName ? trimmed : undefined,
              project_id: projectId !== initialProjectId ? projectId : undefined,
            },
          });
          showSuccess('Board updated');
          onClose();
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
          project_id: projectId,
        });
        showSuccess('Board created');
        onClose();
        onSuccess?.();
        navigate(`/board/${response.data.id}`);
      } catch (err) {
        showError(
          'Failed to create board',
          err instanceof Error ? err.message : 'Something went wrong',
        );
      }
    } finally {
      submittingRef.current = false;
    }
  };

  return (
    <DialogContent
      className="sm:max-w-[425px]"
      onInteractOutside={(e) => {
        if (submittingRef.current || isPending) e.preventDefault();
      }}
      onEscapeKeyDown={(e) => {
        if (submittingRef.current || isPending) e.preventDefault();
      }}
    >
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
          projectMode={projectMode}
          newProjectName={newProjectName}
          onNameChange={setName}
          onProjectChange={setSelectedProjectId}
          onProjectModeChange={setProjectMode}
          onNewProjectNameChange={setNewProjectName}
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
          <Button type="submit" disabled={isSubmitDisabled}>
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
