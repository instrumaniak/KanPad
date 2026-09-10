import type { Note } from './notes.api';
import { getNoteType } from './notes.api';
import { TagBadge } from '@/features/tags/tag-badge';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from '@/components/ui/dropdown-menu';
import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from '@/components/ui/tooltip';
import { MoreHorizontal, Edit3, Trash2 } from 'lucide-react';

const typeStyles: Record<string, string> = {
  general: 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
  board: 'bg-teal-100 text-teal-700 dark:bg-teal-900 dark:text-teal-300',
  project: 'bg-rose-100 text-rose-700 dark:bg-rose-900 dark:text-rose-300',
  card: 'bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300',
};

interface NoteCardProps {
  note: Note;
  onClick: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onTagClick?: (tagId: number) => void;
}

export function NoteCard({ note, onClick, onEdit, onDelete, onTagClick }: NoteCardProps) {
  const noteType = getNoteType(note);

  return (
    <div
      role="article"
      aria-label={`Note: ${note.title}`}
      className="group rounded-lg border border-border bg-card p-3 transition-shadow hover:shadow-sm cursor-pointer"
      onClick={onClick}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <h3 className="font-medium text-sm truncate">{note.title}</h3>
          {note.content && (
            <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
              {note.content.replace(/[#*>`_~[\]]/g, '').slice(0, 120)}
            </p>
          )}
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 w-6 p-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100"
              onClick={(e) => e.stopPropagation()}
            >
              <MoreHorizontal className="h-3.5 w-3.5" />
              <span className="sr-only">Actions</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              onClick={(e) => {
                e.stopPropagation();
                onEdit();
              }}
            >
              <Edit3 className="h-3.5 w-3.5 mr-2" />
              Edit
            </DropdownMenuItem>
            <DropdownMenuItem
              className="text-destructive"
              onClick={(e) => {
                e.stopPropagation();
                onDelete();
              }}
            >
              <Trash2 className="h-3.5 w-3.5 mr-2" />
              Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="flex items-center gap-2 mt-2 flex-wrap">
        <Badge className={`text-[10px] px-1.5 py-0 font-medium ${typeStyles[noteType]}`}>
          {noteType}
        </Badge>
        {note.tags && note.tags.length > 0 && (
          <div className="flex gap-1 overflow-x-auto max-w-[200px] scrollbar-none">
                <TooltipProvider>
              {note.tags.map((tag) => (
                <Tooltip key={tag.id}>
                  <TooltipTrigger asChild>
                    <span>
                      <TagBadge
                        name={tag.name}
                        color={tag.color}
                        onClick={onTagClick ? () => onTagClick(tag.id) : undefined}
                      />
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>{tag.name}</p>
                  </TooltipContent>
                </Tooltip>
              ))}
            </TooltipProvider>
          </div>
        )}
      </div>
      <p className="text-xs text-muted-foreground mt-2">
        {new Date(note.updated_at).toLocaleDateString(undefined, {
          month: 'short',
          day: 'numeric',
          year: 'numeric',
        })}
      </p>
    </div>
  );
}
