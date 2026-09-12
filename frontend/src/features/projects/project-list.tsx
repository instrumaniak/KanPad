import { useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { useToast } from '@/components/ui/use-toast';
import { useToastHelpers } from '@/lib/toast-helpers';
import {
  useProjects,
  useCreateProject,
  useUpdateProject,
  useDeleteProject,
} from './use-projects';
import { recreateProject } from './projects.api';
import { Plus, Pencil, Trash2, FolderKanban } from 'lucide-react';
import { EmptyState } from '@/components/empty-state';
import { ErrorState } from '@/components/error-state';
import { LoadingSkeleton } from '@/components/loading-skeleton';

function InlineCreateForm({ onSuccess, onCancel }: { onSuccess: () => void; onCancel: () => void }) {
  const [name, setName] = useState('');
  const createMutation = useCreateProject();
  const { showSuccess, showError } = useToastHelpers();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;

    try {
      await createMutation.mutateAsync(trimmed);
      showSuccess('Project created');
      setName('');
      onSuccess();
    } catch (err) {
      showError(
        'Failed to create project',
        err instanceof Error ? err.message : 'Something went wrong',
      );
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <form onSubmit={handleSubmit} className="flex items-center gap-2">
      <Input
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={handleKeyDown}
        autoFocus
        placeholder="Project name"
        aria-label="Project name"
        className="max-w-xs"
        disabled={createMutation.isPending}
      />
      <Button type="submit" size="sm" disabled={!name.trim() || createMutation.isPending}>
        {createMutation.isPending ? 'Saving...' : 'Save'}
      </Button>
      <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
        Cancel
      </Button>
    </form>
  );
}

function InlineEdit({
  initialValue,
  projectId,
  onSave,
  onCancel,
}: {
  initialValue: string;
  projectId: number;
  onSave: () => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(initialValue);
  const updateMutation = useUpdateProject();
  const { showSuccess, showError } = useToastHelpers();

  const handleSave = async () => {
    if (updateMutation.isPending) return;

    const trimmed = name.trim();
    if (!trimmed || trimmed === initialValue) {
      onCancel();
      return;
    }

    try {
      await updateMutation.mutateAsync({ id: projectId, name: trimmed });
      showSuccess('Project renamed');
      onSave();
    } catch (err) {
      showError(
        'Failed to rename project',
        err instanceof Error ? err.message : 'Something went wrong',
      );
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
    <Input
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleSave}
        aria-label="Project name"
        autoFocus
        onFocus={(e) => e.currentTarget.select()}
        className="h-7 max-w-[200px] text-sm"
        disabled={updateMutation.isPending}
      />
  );
}

function DeleteDialog({
  projectName,
  projectId,
  open,
  onOpenChange,
  onDeleted,
}: {
  projectName: string;
  projectId: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted: () => void;
}) {
  const deleteMutation = useDeleteProject();
  const { toast } = useToast();
  const { showError } = useToastHelpers();

  const handleDelete = async () => {
    const deletedName = projectName;
    try {
      await deleteMutation.mutateAsync(projectId);
      onOpenChange(false);
      onDeleted();
      toast({
        title: 'Project deleted',
        type: 'destructive',
        action: {
          label: 'Undo',
          onClick: async () => {
            try {
              await recreateProject(deletedName);
              toast({ title: 'Project restored', type: 'success' });
            } catch (err) {
              toast({
                title: 'Failed to restore project',
                description: err instanceof Error ? err.message : 'Something went wrong',
                type: 'error',
              });
            }
          },
        },
      });
    } catch (err) {
      showError(
        'Failed to delete project',
        err instanceof Error ? err.message : 'Something went wrong',
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Delete project</DialogTitle>
          <DialogDescription>
            Delete project &quot;{projectName}&quot;? This will delete all boards and cards within
            it.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={handleDelete} disabled={deleteMutation.isPending}>
            {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ProjectCard({
  id,
  name,
  boardCount,
  onEdit,
  onDelete,
}: {
  id: number;
  name: string;
  boardCount: number;
  onEdit: (id: number, name: string) => void;
  onDelete: (id: number, name: string) => void;
}) {
  return (
    <div className="group flex items-center justify-between rounded-lg border border-border bg-card p-4 transition-colors hover:bg-accent/50">
      <div className="flex items-center gap-3">
        <FolderKanban className="h-5 w-5 text-muted-foreground" />
        <div>
          <p className="font-medium">{name}</p>
          <p className="text-sm text-muted-foreground">
            {boardCount} {boardCount === 1 ? 'board' : 'boards'}
          </p>
        </div>
      </div>
      <div className="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onEdit(id, name)}
          aria-label={`Edit project ${name}`}
        >
          <Pencil className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onDelete(id, name)}
          aria-label={`Delete project ${name}`}
          className="text-destructive hover:bg-destructive/10"
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}



export function ProjectList() {
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingName, setEditingName] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; name: string } | null>(null);
  const { data, isLoading, isError, error, refetch } = useProjects();

  const handleEdit = useCallback((id: number, name: string) => {
    setEditingId(id);
    setEditingName(name);
  }, []);

  const handleEditSave = useCallback(() => {
    setEditingId(null);
  }, []);

  const handleEditCancel = useCallback(() => {
    setEditingId(null);
  }, []);

  const handleDelete = useCallback((id: number, name: string) => {
    setDeleteTarget({ id, name });
  }, []);

  const projects = data?.data ?? [];

  if (isLoading) {
    return (
      <div className="mx-auto max-w-2xl">
        <h1 className="mb-6 text-2xl font-bold">My Projects</h1>
        <LoadingSkeleton />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="mx-auto max-w-2xl">
        <h1 className="mb-6 text-2xl font-bold">My Projects</h1>
        <ErrorState
          title="Failed to load projects"
          message={error instanceof Error ? error.message : 'Something went wrong'}
          onRetry={() => refetch()}
        />
      </div>
    );
  }

  if (projects.length === 0 && !showCreateForm) {
    return (
      <div className="mx-auto max-w-2xl">
        <h1 className="mb-6 text-2xl font-bold">My Projects</h1>
        <EmptyState
          icon={<FolderKanban className="h-8 w-8 text-muted-foreground" />}
          headline="Start organizing"
          description="Create your first project to get started"
          action={{
            label: 'Create your first project',
            onClick: () => setShowCreateForm(true),
          }}
        />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">My Projects</h1>
        {!showCreateForm && (
          <Button onClick={() => setShowCreateForm(true)}>
            <Plus className="mr-1 h-4 w-4" />
            Create Project
          </Button>
        )}
      </div>

      {showCreateForm && (
        <div className="mb-4">
          <InlineCreateForm
            onSuccess={() => setShowCreateForm(false)}
            onCancel={() => setShowCreateForm(false)}
          />
        </div>
      )}

      <div className="space-y-3">
        {projects.map((project) =>
          editingId === project.id ? (
            <div key={project.id} className="rounded-lg border border-border bg-card p-4">
              <InlineEdit
                initialValue={editingName}
                projectId={project.id}
                onSave={handleEditSave}
                onCancel={handleEditCancel}
              />
            </div>
          ) : (
            <ProjectCard
              key={project.id}
              id={project.id}
              name={project.name}
              boardCount={project.boardCount}
              onEdit={handleEdit}
              onDelete={handleDelete}
            />
          ),
        )}
      </div>

      {deleteTarget && (
        <DeleteDialog
          projectName={deleteTarget.name}
          projectId={deleteTarget.id}
          open={!!deleteTarget}
          onOpenChange={(open) => {
            if (!open) setDeleteTarget(null);
          }}
          onDeleted={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
