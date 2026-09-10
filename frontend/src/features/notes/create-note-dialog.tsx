import { useState } from 'react';
import { NoteEditor } from './note-editor';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';

interface CreateNoteDialogProps {
  children: React.ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  defaultBoardId?: number;
  defaultProjectId?: number;
  defaultCardId?: number;
}

export function CreateNoteDialog({ children, open, onOpenChange, defaultBoardId, defaultProjectId, defaultCardId }: CreateNoteDialogProps) {
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = open !== undefined;
  const isOpen = isControlled ? open : internalOpen;
  const setIsOpen = isControlled ? (v: boolean) => onOpenChange?.(v) : setInternalOpen;

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogTrigger asChild>
        {children}
      </DialogTrigger>
      <DialogContent className="sm:max-w-2xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Create Note</DialogTitle>
        </DialogHeader>
        <NoteEditor
          onSave={() => setIsOpen(false)}
          onCancel={() => setIsOpen(false)}
          defaultBoardId={defaultBoardId}
          defaultProjectId={defaultProjectId}
          defaultCardId={defaultCardId}
        />
      </DialogContent>
    </Dialog>
  );
}
