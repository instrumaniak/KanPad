import { useState, useRef, useEffect } from 'react';
import { FileText, Plus, ChevronLeft, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useToast } from '@/components/ui/use-toast';
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
import { Sheet, SheetContent, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { cn } from '@/lib/utils';
import { NoteEditor } from './note-editor';
import { BoardNotesSidebarItem } from './board-notes-sidebar-item';
import { useBoardNotes, useDeleteNote, useNote } from './use-notes';

interface BoardNotesSidebarProps {
  boardId: number;
  collapsed: boolean;
  onToggle: () => void;
}

export function BoardNotesSidebar({ boardId, collapsed, onToggle }: BoardNotesSidebarProps) {
  const { data: notesData, isLoading } = useBoardNotes(boardId);
  const deleteNote = useDeleteNote();
  const { toast } = useToast();

  const [editingNoteId, setEditingNoteId] = useState<number | null>(null);
  const { data: editingNoteData, isLoading: isLoadingNote } = useNote(editingNoteId ?? 0);
  const editingNote = editingNoteId ? editingNoteData?.data ?? null : null;
  const [isCreating, setIsCreating] = useState(false);
  const [deleteConfirmId, setDeleteConfirmId] = useState<number | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const notes = notesData?.data ?? [];

  useEffect(() => {
    return () => clearTimeout(deleteTimerRef.current);
  }, []);

  const handleCreate = () => {
    setIsCreating(false);
  };

  const handleUpdate = () => {
    setEditingNoteId(null);
  };

  const handleDelete = async (id: number) => {
    setDeleteConfirmId(null);

    deleteTimerRef.current = setTimeout(async () => {
      try {
        await deleteNote.mutateAsync({ id });
        toast({ title: 'Note deleted', type: 'success' });
      } catch {
        toast({ title: 'Failed to delete note', type: 'error' });
      }
    }, 5000);

    toast({
      title: 'Note will be deleted',
      action: {
        label: 'Undo',
        onClick: () => {
          clearTimeout(deleteTimerRef.current);
          toast({ title: 'Delete undone' });
        },
      },
    });
  };

  return (
    <>
      <aside
        className={cn(
          'flex h-full flex-col border-r border-border bg-background transition-[width] duration-300 ease-in-out overflow-hidden shrink-0',
          collapsed ? 'w-0 border-r-0' : 'w-[280px]',
        )}
      >
        <div className="flex items-center justify-between  px-3 py-2">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium">Notes</span>
            <Button variant="ghost" size="sm" onClick={() => setIsCreating(true)}>
              <Plus className="mr-1 h-4 w-4" />
            </Button>
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onToggle}
            aria-label="Toggle notes sidebar"
            aria-expanded={!collapsed}
          >
            <ChevronLeft className="h-4 w-4" />
          </Button>
        </div>

        <ScrollArea className="flex-1 px-2 py-2">
          {isLoading ? (
            <div className="space-y-2 px-2">
              {[1, 2, 3].map((i) => (
                <div key={i} className="h-16 animate-pulse rounded-md bg-muted" />
              ))}
            </div>
          ) : notes.length === 0 ? (
            <div className="flex flex-col items-center gap-2 px-4 py-8 text-center">
              <FileText className="h-8 w-8 text-muted-foreground/50" />
              <p className="text-sm text-muted-foreground">No notes for this board</p>
            </div>
          ) : (
            <div className="space-y-1">
              {notes.map((note) => (
                <div key={note.id} className="group relative">
                  <div className="relative">
                    <BoardNotesSidebarItem
                      note={note}
                      onClick={() => setEditingNoteId(note.id)}
                      onEdit={() => setEditingNoteId(note.id)}
                      onDelete={() => setDeleteConfirmId(note.id)}
                    />
                  </div>
                </div>
              ))}
            </div>
          )}
        </ScrollArea>
      </aside>

      {collapsed && (
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onToggle}
          aria-label="Show notes sidebar"
          className="absolute left-0 top-1/2"
        >
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        </Button>
      )}

      <Sheet
        open={editingNoteId !== null}
        onOpenChange={(open) => {
          if (!open) setEditingNoteId(null);
        }}
      >
        <SheetContent side="right" className="w-[400px] sm:w-[540px] sm:max-w-[540px] p-4">
          <SheetHeader>
            <SheetTitle></SheetTitle>
          </SheetHeader>
          {isLoadingNote ? (
            <div className="space-y-4 pt-4">
              <div className="h-8 w-48 animate-pulse rounded bg-muted" />
              <div className="h-64 animate-pulse rounded bg-muted" />
            </div>
          ) : editingNote ? (
            <NoteEditor
              note={editingNote}
              onSave={handleUpdate}
              onCancel={() => setEditingNoteId(null)}
              onDelete={() => {
                setDeleteConfirmId(editingNote.id);
                setEditingNoteId(null);
              }}
            />
          ) : null}
        </SheetContent>
      </Sheet>

      <Sheet
        open={isCreating}
        onOpenChange={(open) => {
          if (!open) setIsCreating(false);
        }}
      >
        <SheetContent side="right" className="w-[400px] sm:w-[540px] sm:max-w-[540px] p-4">
          <SheetHeader>
            <SheetTitle></SheetTitle>
          </SheetHeader>
          <NoteEditor
            defaultBoardId={boardId}
            onSave={handleCreate}
            onCancel={() => setIsCreating(false)}
          />
        </SheetContent>
      </Sheet>

      <AlertDialog
        open={deleteConfirmId !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteConfirmId(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Note</AlertDialogTitle>
            <AlertDialogDescription>
              This will permanently delete this note. This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => deleteConfirmId && handleDelete(deleteConfirmId)}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
