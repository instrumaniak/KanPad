import { useState, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { flattenNotePages, useNotes } from './use-notes';
import { NoteCard } from './note-card';
import { CreateNoteDialog } from './create-note-dialog';
import { NoteEditor } from './note-editor';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/empty-state';
import { LoadingSkeleton } from '@/components/loading-skeleton';
import { ErrorState } from '@/components/error-state';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { FileText, Plus, Search } from 'lucide-react';
import type { Note } from './notes.api';
import { useDeleteNote } from './use-notes';
import { useToast } from '@/components/ui/use-toast';
import { useToastHelpers } from '@/lib/toast-helpers';

const typeFilters = ['All', 'General', 'Board', 'Project', 'Card'] as const;

// Loose status read — avoids depending on ApiError class identity across module boundaries (same rationale as board-view's helper and T014's handler duck-typing).
function errorStatus(err: unknown): number | undefined {
  if (!err || typeof err !== 'object' || !('status' in err)) return undefined;
  const status = (err as { status?: unknown }).status;
  return typeof status === 'number' ? status : undefined;
}

export function NoteList() {
  const navigate = useNavigate();
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [typeFilter, setTypeFilter] = useState<string>('All');
  const [tagFilter, setTagFilter] = useState<number | undefined>(undefined);
  const [editingNote, setEditingNote] = useState<Note | null>(null);
  const [deletingNote, setDeletingNote] = useState<Note | null>(null);
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const deleteMutation = useDeleteNote();
  const { toast } = useToast();
  const { showSuccess, showError } = useToastHelpers();
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const debounceSearch = useCallback((value: string) => {
    clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => {
      setDebouncedSearch(value);
    }, 300);
  }, []);

  const filters = {
    search: debouncedSearch || undefined,
    type: typeFilter !== 'All' ? typeFilter.toLowerCase() as never : undefined,
    tag_id: tagFilter,
  };

  const handleTagClick = (tagId: number) => {
    setTagFilter((prev) => (prev === tagId ? undefined : tagId));
  };



  const {
    data,
    isLoading,
    isError,
    error,
    refetch,
    hasNextPage,
    fetchNextPage,
    isFetchingNextPage,
  } = useNotes(filters);
  const notes = flattenNotePages(data?.pages);

  const handleDelete = () => {
    if (!deletingNote) return;
    const noteId = deletingNote.id;
    const noteTitle = deletingNote.title;
    setDeletingNote(null);

    deleteTimerRef.current = setTimeout(() => {
      deleteMutation.mutate(
        { id: noteId },
        {
          onSuccess: () => {
            showSuccess('Note deleted');
          },
          onError: () => {
            showError('Failed to delete note');
          },
        },
      );
    }, 5000);

    toast({
      title: `"${noteTitle}" will be deleted`,
      action: {
        label: 'Undo',
        onClick: () => {
          clearTimeout(deleteTimerRef.current);
          toast({ title: 'Delete undone' });
        },
      },
    });
  };

  if (editingNote) {
    return (
      <div className="p-6">
        <NoteEditor
          note={editingNote}
          onSave={() => {
            setEditingNote(null);
          }}
          onCancel={() => setEditingNote(null)}
        />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="p-6 pb-2 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-2xl font-bold">Notes</h2>
          <CreateNoteDialog
            open={showCreateDialog}
            onOpenChange={setShowCreateDialog}
          >
            <Button size="sm">
              <Plus className="h-4 w-4 mr-1" />
              New Note
            </Button>
          </CreateNoteDialog>
        </div>
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search notes..."
            className="pl-8 h-8 text-sm"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              debounceSearch(e.target.value);
            }}
          />
        </div>
        <div className="flex gap-1.5 overflow-x-auto scrollbar-none sticky top-0 bg-background z-10 pb-1">
          {typeFilters.map((type) => (
            <Badge
              key={type}
              variant={typeFilter === type ? 'default' : 'outline'}
              className="cursor-pointer whitespace-nowrap text-xs"
              onClick={() => setTypeFilter(type)}
            >
              {type}
            </Badge>
          ))}
        </div>
      </div>
      <ScrollArea className="flex-1 px-6 pb-6">
        {errorStatus(error) === 401 ? (
          // 401: T014's global handler owns the UX (cache clear → /login) —
          // no error card, just the loading skeleton until the route unmounts.
          <LoadingSkeleton count={4} />
        ) : isError && !data ? (
          // Last-good data wins: React Query keeps `data` on a background
          // refetch failure — keep showing it rather than a blocking error.
          <ErrorState
            title="Failed to load notes"
            message={error instanceof Error ? error.message : 'Something went wrong'}
            onRetry={() => refetch()}
          />
        ) : isLoading ? (
          <LoadingSkeleton count={4} />
        ) : notes.length === 0 ? (
          <EmptyState
            icon={<FileText className="h-6 w-6 text-muted-foreground" />}
            headline="No notes yet"
            description="Create your first note to start documenting."
          />
        ) : (
          <div className="space-y-2">
            {notes.map((note) => (
              <NoteCard
                key={note.id}
                note={note}
                onClick={() => navigate(`/notes/${note.id}`)}
                onEdit={() => setEditingNote(note)}
                onDelete={() => setDeletingNote(note)}
                onTagClick={handleTagClick}
              />
            ))}
            {hasNextPage ? (
              <div className="flex justify-center pt-2">
                <Button
                  variant="outline"
                  onClick={() => void fetchNextPage()}
                  disabled={isFetchingNextPage}
                >
                  {isFetchingNextPage ? 'Loading more...' : 'Load more notes'}
                </Button>
              </div>
            ) : null}
            <p className="sr-only" aria-live="polite" aria-atomic="true">
              {isFetchingNextPage ? 'Loading more notes' : !hasNextPage && notes.length > 0 ? `Showing all ${notes.length} notes` : ''}
            </p>
          </div>
        )}
      </ScrollArea>
      <AlertDialog
        open={!!deletingNote}
        onOpenChange={(open) => !open && setDeletingNote(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete note?</AlertDialogTitle>
            <AlertDialogDescription>
              This will permanently delete "{deletingNote?.title}" and all its content.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={handleDelete} variant="destructive">
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
