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
import { useCreateBoard } from './use-boards';
import { useProjects } from '../projects/use-projects';
import { Plus } from 'lucide-react';

interface CreateBoardModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess?: () => void;
}

export function CreateBoardModal({ open, onOpenChange, onSuccess }: CreateBoardModalProps) {
  const [name, setName] = useState('');
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      setName('');
      setSelectedProjectId(null);
    }
    onOpenChange(nextOpen);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <CreateBoardForm
        name={name}
        selectedProjectId={selectedProjectId}
        setName={setName}
        setSelectedProjectId={setSelectedProjectId}
        onOpenChange={handleOpenChange}
        onSuccess={onSuccess}
      />
    </Dialog>
  );
}

function CreateBoardForm({
  name,
  selectedProjectId,
  setName,
  setSelectedProjectId,
  onOpenChange,
  onSuccess,
}: Pick<CreateBoardModalProps, 'onOpenChange' | 'onSuccess'> & {
  name: string;
  selectedProjectId: number | null;
  setName: (value: string) => void;
  setSelectedProjectId: (value: number | null) => void;
}) {
  const { showSuccess, showError } = useToastHelpers();
  const createBoard = useCreateBoard();
  const { data: projectsData } = useProjects();
  const navigate = useNavigate();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;

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

  const projects = projectsData?.data ?? [];

  return (
    <DialogContent className="sm:max-w-[425px]">
      <form onSubmit={handleSubmit}>
        <DialogHeader>
          <DialogTitle>Create new board</DialogTitle>
          <DialogDescription>
            Give your board a name
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-4 py-4">
          <div className="grid gap-2">
            <label htmlFor="board-name" className="text-sm font-medium">
              Board name
            </label>
            <Input
              id="board-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My awesome board"
              disabled={createBoard.isPending}
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
                setSelectedProjectId(e.target.value ? parseInt(e.target.value, 10) : null)
              }
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              disabled={createBoard.isPending}
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

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={createBoard.isPending}
          >
            Cancel
          </Button>
          <Button
            type="submit"
            disabled={!name.trim() || createBoard.isPending}
          >
            {createBoard.isPending ? (
              <>
                <Plus className="mr-1 h-4 w-4 animate-spin" />
                Creating...
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
