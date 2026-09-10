import type { Note } from './notes.api';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from '@/components/ui/dropdown-menu';
import { FileText, MoreHorizontal, Edit3, Trash2 } from 'lucide-react';

interface BoardNotesSidebarItemProps {
  note: Note;
  onClick: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onTagClick?: (tagId: number) => void;
}

const NOTE_ITEM_MAX_LENGTH = 28;

export function BoardNotesSidebarItem({
  note,
  onClick,
  onEdit,
  onDelete,
}: BoardNotesSidebarItemProps) {
  const hasLongTitle = note.title?.length > NOTE_ITEM_MAX_LENGTH;
  return (
    <div
      role="article"
      aria-label={`Note: ${note.title}`}
      className="group p-2 transition-shadow hover:bg-accent/50 cursor-pointer"
      onClick={onClick}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex items-center gap-2">
          <FileText className="h-4 w-4 text-muted-foreground" />
          <h3 className="font-medium text-sm truncate">
            {note.title?.substring(0, NOTE_ITEM_MAX_LENGTH)}
            {hasLongTitle && '..'}
          </h3>
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
      <p className="text-xs text-muted-foreground">
        {new Date(note.updated_at).toLocaleDateString(undefined, {
          month: 'short',
          day: 'numeric',
          year: 'numeric',
        })}
      </p>
    </div>
  );
}
