import { Suspense } from 'react';
import type { Note } from './notes.api';
import { getNoteType } from './notes.api';
import { TagBadge } from '@/features/tags/tag-badge';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { ScrollArea } from '@/components/ui/scroll-area';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { ChevronLeft, Edit3, Trash2 } from 'lucide-react';
import { LazyLoadBoundary } from '@/components/lazy-load-boundary';
import { LazyMarkdownRenderer } from './lazy-markdown-renderer';

const typeStyles: Record<string, string> = {
  general: 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
  board: 'bg-teal-100 text-teal-700 dark:bg-teal-900 dark:text-teal-300',
  project: 'bg-rose-100 text-rose-700 dark:bg-rose-900 dark:text-rose-300',
  card: 'bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300',
};

interface NoteDetailProps {
  note: Note;
  onBack: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

export function NoteDetail({ note, onBack, onEdit, onDelete }: NoteDetailProps) {
  const noteType = getNoteType(note);

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between p-4 pb-2">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ChevronLeft className="h-4 w-4 mr-1" />
          Back
        </Button>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={onEdit}>
            <Edit3 className="h-4 w-4 mr-1" />
            Edit
          </Button>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="destructive" size="sm">
                <Trash2 className="h-4 w-4 mr-1" />
                Delete
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Delete note?</AlertDialogTitle>
                <AlertDialogDescription>
                  This will permanently delete "{note.title}" and all its content.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction onClick={onDelete} variant="destructive">
                  Delete
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
      </div>
      <ScrollArea className="flex-1 px-4 pb-4">
        <article>
          <div className="flex items-center gap-2 mb-2">
            <Badge className={`text-xs px-2 py-0.5 font-medium ${typeStyles[noteType]}`}>
              {noteType}
            </Badge>
          </div>
          <h1 className="text-2xl font-bold mb-2">{note.title}</h1>
          <div className="flex items-center gap-2 text-xs text-muted-foreground mb-4">
            <span>Created {new Date(note.created_at).toLocaleDateString()}</span>
            <span>&middot;</span>
            <span>Updated {new Date(note.updated_at).toLocaleDateString()}</span>
          </div>
          {note.tags && note.tags.length > 0 && (
            <div className="flex gap-1.5 flex-wrap mb-4">
              {note.tags.map((tag) => (
                <TagBadge key={tag.id} name={tag.name} color={tag.color} />
              ))}
            </div>
          )}
          <Separator className="mb-4" />
          <div className="prose prose-sm dark:prose-invert max-w-none">
            <LazyLoadBoundary fallback={<div className="text-xs text-muted-foreground">Failed to load preview</div>}>
              <Suspense fallback={<div className="text-xs text-muted-foreground animate-pulse">Loading...</div>}>
                <LazyMarkdownRenderer content={note.content || ''} />
              </Suspense>
            </LazyLoadBoundary>
          </div>
        </article>
      </ScrollArea>
    </div>
  );
}
