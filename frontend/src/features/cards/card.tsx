import { useState, useRef } from 'react';
import { useDeleteCard, useCreateCard, type Card as CardType } from './use-cards';
import { CardDraggable } from './card-draggable';
import { CardDetailPanel } from './card-detail-panel';
import { CardPreview } from './card-preview';
import { Trash2 } from 'lucide-react';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu';
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
import { useToast } from '@/components/ui/use-toast';
import { useToastHelpers } from '@/lib/toast-helpers';

interface CardProps {
  card: CardType;
  index: number;
  isNew?: boolean;
}

export function Card({ card, index, isNew }: CardProps) {
  const [isPanelOpen, setIsPanelOpen] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [contextMenuOpen, setContextMenuOpen] = useState(false);
  const pointerDownPos = useRef<{ x: number; y: number } | null>(null);
  // Suppress only the synthetic click that immediately follows a touch
  // long-press (menu open), not the next legitimate click minutes later.
  const suppressUntilRef = useRef(0);
  const deleteCard = useDeleteCard();
  const createCardMutation = useCreateCard();
  const { toast } = useToast();
  const { showSuccess, showError } = useToastHelpers();

  const handleClick = (e: React.MouseEvent) => {
    if (Date.now() < suppressUntilRef.current) {
      suppressUntilRef.current = 0;
      pointerDownPos.current = null;
      return;
    }
    if (pointerDownPos.current) {
      const dx = e.clientX - pointerDownPos.current.x;
      const dy = e.clientY - pointerDownPos.current.y;
      const distance = Math.sqrt(dx * dx + dy * dy);
      pointerDownPos.current = null;
      if (distance > 5) return;
    } else {
      pointerDownPos.current = null;
    }
    e.stopPropagation();
    setIsPanelOpen(true);
  };

  const handlePointerDown = (e: React.PointerEvent) => {
    pointerDownPos.current = { x: e.clientX, y: e.clientY };
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      setIsPanelOpen(true);
    }
  };

  const handleDelete = () => {
    const deletedCard = { ...card };
    setShowDeleteDialog(false);
    setIsPanelOpen(false);
    deleteCard.mutate(card.id, {
      onSuccess: () => {
        toast({
          title: 'Card deleted',
          type: 'destructive',
          action: {
            label: 'Undo',
            onClick: async () => {
              try {
                await createCardMutation.mutateAsync({
                  title: deletedCard.title,
                  column_id: deletedCard.column_id,
                  position: deletedCard.position,
                  description: deletedCard.description ?? undefined,
                  due_date: deletedCard.due_date ?? undefined,
                });
                showSuccess('Card restored to original position');
              } catch {
                showError('Failed to restore card');
              }
            },
          },
        });
      },
      onError: () => {
        showError('Failed to delete card');
      },
    });
  };

  const openDeleteDialog = () => setShowDeleteDialog(true);

  const handleContextMenuOpenChange = (open: boolean) => {
    setContextMenuOpen(open);
    if (open) {
      // Long-press on touch fires a synthetic click after; suppress clicks
      // only within a short window so later legitimate clicks still work.
      suppressUntilRef.current = Date.now() + 500;
    }
  };

  return (
    <>
      <CardDraggable card={card} index={index} isDragDisabled={contextMenuOpen}>
        {({ isDragging }) => (
          <ContextMenu onOpenChange={handleContextMenuOpenChange}>
            <ContextMenuTrigger asChild disabled={isDragging}>
              <div
                role="button"
                tabIndex={0}
                aria-label="Open card details"
                aria-haspopup="menu"
                title="Right-click for card menu"
                className={`relative rounded bg-card p-3 text-sm shadow-sm hover:bg-accent/50 cursor-pointer outline-none focus-visible:ring-2 focus-visible:ring-ring ${isNew ? 'animate-slide-up' : ''} ${isDragging ? 'shadow-lg scale-[1.02]' : ''}`}
                onClick={handleClick}
                onPointerDown={handlePointerDown}
                onKeyDown={handleKeyDown}
                onContextMenu={(e) => e.stopPropagation()}
                style={{
                  cursor: isDragging ? 'grabbing' : 'grab',
                }}
              >
                <CardPreview card={card} />
              </div>
            </ContextMenuTrigger>
            <ContextMenuContent onClick={(e) => e.stopPropagation()}>
              <ContextMenuItem
                onSelect={openDeleteDialog}
                className="text-destructive focus:text-destructive"
              >
                <Trash2 className="mr-2 h-4 w-4" />
                Delete
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
        )}
      </CardDraggable>
      <CardDetailPanel card={card} open={isPanelOpen} onOpenChange={setIsPanelOpen} />
      <AlertDialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <AlertDialogContent onClick={(e) => e.stopPropagation()}>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete card?</AlertDialogTitle>
            <AlertDialogDescription>
              The card &quot;{card.title || 'this card'}&quot; will be deleted. You can restore it from the notification.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={(e) => e.stopPropagation()}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleteCard.isPending}
              className={deleteCard.isPending ? 'opacity-50 cursor-not-allowed' : ''}
              onClick={(e) => {
                e.stopPropagation();
                handleDelete();
              }}
            >
              {deleteCard.isPending ? 'Deleting...' : 'Delete'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
