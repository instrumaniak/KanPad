import { useState, useRef, useEffect } from 'react';
import { MoreHorizontal, ChevronRight, ArrowUpDown, ArrowUp, ArrowDown, MoveHorizontal } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useUpdateColumn, useDeleteColumn, useSortCards, useMoveAllCards, type Column } from './use-columns';
import { useToast } from '@/components/ui/use-toast';
import { useBreakpoint } from '@/hooks/use-breakpoint';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

interface ColumnHeaderProps {
  column: Column;
  allColumns?: Column[];
  onDeleted?: () => void;
}

export function ColumnHeader({ column, allColumns = [], onDeleted }: ColumnHeaderProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState(column.name);
  const [showMenu, setShowMenu] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [showSortMenu, setShowSortMenu] = useState(false);
  const [showMoveMenu, setShowMoveMenu] = useState(false);

  const sortMenuRef = useRef<HTMLDivElement>(null);
  const moveMenuRef = useRef<HTMLDivElement>(null);
  const updateMutation = useUpdateColumn();
  const deleteMutation = useDeleteColumn();
  const sortMutation = useSortCards();
  const moveMutation = useMoveAllCards();
  const { toast } = useToast();
  const breakpoint = useBreakpoint();
  const isMobile = breakpoint === 'mobile';

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (sortMenuRef.current && !sortMenuRef.current.contains(event.target as Node)) {
        setShowSortMenu(false);
      }
      if (moveMenuRef.current && !moveMenuRef.current.contains(event.target as Node)) {
        setShowMoveMenu(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleStartEdit = () => {
    setEditValue(column.name);
    setIsEditing(true);
    setShowMenu(false);
  };

  const handleSave = async () => {
    const trimmed = editValue.trim();
    if (!trimmed || trimmed === column.name) {
      setIsEditing(false);
      setEditValue(column.name);
      return;
    }

    try {
      await updateMutation.mutateAsync({ id: column.id, data: { name: trimmed } });
      toast({ title: 'Column renamed', type: 'success' });
      setIsEditing(false);
    } catch (err) {
      toast({
        title: 'Failed to rename column',
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
      setEditValue(column.name);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSave();
    } else if (e.key === 'Escape') {
      setIsEditing(false);
      setEditValue(column.name);
    }
  };

  const handleDelete = async () => {
    try {
      await deleteMutation.mutateAsync(column.id);
      setShowDeleteDialog(false);
      toast({ title: 'Column deleted', type: 'success' });
      onDeleted?.();
    } catch (err) {
      toast({
        title: 'Failed to delete column',
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
    }
  };

  const handleSort = async (order: 'asc' | 'desc') => {
    try {
      await sortMutation.mutateAsync({ columnId: column.id, order });
      toast({ title: order === 'asc' ? 'Sorted: Oldest first' : 'Sorted: Newest first', type: 'success' });
      setShowSortMenu(false);
      setShowMenu(false);
    } catch (err) {
      toast({
        title: 'Failed to sort cards',
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
    }
  };

  const handleMoveAll = async (targetColumnId: number) => {
    try {
      const result = await moveMutation.mutateAsync({ sourceColumnId: column.id, targetColumnId });
      toast({ title: `${result.data.movedCount} cards moved`, type: 'success' });
      setShowMoveMenu(false);
      setShowMenu(false);
    } catch (err) {
      toast({
        title: 'Failed to move cards',
        description: err instanceof Error ? err.message : 'Something went wrong',
        type: 'error',
      });
    }
  };

  const otherColumns = allColumns.filter((c) => c.id !== column.id);

  return (
    <div className="flex items-center justify-between px-3 py-2">
      {isEditing ? (
        <input
          value={editValue}
          onChange={(e) => setEditValue(e.target.value)}
          onBlur={handleSave}
          onKeyDown={handleKeyDown}
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
          className={`flex-1 rounded border border-input bg-background px-2 py-1 text-sm font-semibold ${isMobile ? 'min-h-[48px]' : ''}`}
        />
      ) : (
        <button
          onClick={handleStartEdit}
          className={`flex-1 truncate text-left text-sm font-semibold hover:bg-muted/50 rounded px-2 py-1 ${isMobile ? 'min-h-[48px]' : ''}`}
        >
          {column.name}
        </button>
      )}

      <div className="relative">
        <Button
          variant="ghost"
          size="icon-sm"
          className={isMobile ? 'h-12 w-12' : 'h-7 w-7'}
          onClick={() => setShowMenu(!showMenu)}
        >
          <MoreHorizontal className="h-4 w-4" />
        </Button>

        {showMenu && (
          <div className="absolute right-0 top-full z-10 mt-1 w-48 rounded-md border bg-popover p-1 shadow-md">
            <button
              onClick={handleStartEdit}
              className={`flex w-full items-center rounded px-2 py-1.5 text-sm hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
            >
              Rename
            </button>
            <div className="relative">
              <button
                onClick={() => setShowSortMenu(!showSortMenu)}
                className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-sm hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
              >
                <span className="flex items-center gap-2">
                  <ArrowUpDown className="h-4 w-4" />
                  Sort by Date
                </span>
                <ChevronRight className="h-4 w-4" />
              </button>
              {showSortMenu && (
                <div ref={sortMenuRef} className={`absolute z-20 w-40 rounded-md border bg-popover p-1 shadow-md ${isMobile ? 'top-full left-0 mt-1' : 'left-full top-0 ml-1'}`}>
                  <button
                    onClick={() => handleSort('asc')}
                    className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
                  >
                    <ArrowUp className="h-4 w-4" />
                    Ascending (Oldest first)
                  </button>
                  <button
                    onClick={() => handleSort('desc')}
                    className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
                  >
                    <ArrowDown className="h-4 w-4" />
                    Descending (Newest first)
                  </button>
                </div>
              )}
            </div>
            {otherColumns.length > 0 && (
              <div className="relative">
                <button
                  onClick={() => setShowMoveMenu(!showMoveMenu)}
                  className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-sm hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
                >
                  <span className="flex items-center gap-2">
                    <MoveHorizontal className="h-4 w-4" />
                    Move All Cards
                  </span>
                  <ChevronRight className="h-4 w-4" />
                </button>
                {showMoveMenu && (
                  <div ref={moveMenuRef} className={`absolute z-20 w-40 rounded-md border bg-popover p-1 shadow-md ${isMobile ? 'top-full left-0 mt-1' : 'left-full top-0 ml-1'}`}>
                    {otherColumns.map((col) => (
                      <button
                        key={col.id}
                        onClick={() => handleMoveAll(col.id)}
                        className={`flex w-full items-center rounded px-2 py-1.5 text-sm hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
                      >
                        {col.name}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
            <button
              onClick={() => {
                setShowMenu(false);
                setShowDeleteDialog(true);
              }}
              className={`flex w-full items-center rounded px-2 py-1.5 text-sm text-destructive hover:bg-accent ${isMobile ? 'min-h-[48px]' : ''}`}
            >
              Delete
            </button>
          </div>
        )}
      </div>

      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete column?</DialogTitle>
            <DialogDescription>
              All cards in this column will be permanently deleted.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowDeleteDialog(false)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={deleteMutation.isPending}
            >
              {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
