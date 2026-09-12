import { useState, useRef } from 'react';
import { useDeleteCard, useCreateCard, type Card as CardType } from './use-cards';
import { CardDraggable } from './card-draggable';
import { CardDetailPanel } from './card-detail-panel';
import { CardPreview } from './card-preview';
import { MoreHorizontal, Trash2 } from 'lucide-react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
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
import { Button } from '@/components/ui/button';
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
  const pointerDownPos = useRef<{ x: number; y: number } | null>(null);
  const deleteCard = useDeleteCard();
  const createCardMutation = useCreateCard();
  const { toast } = useToast();
  const { showSuccess, showError } = useToastHelpers();

  const handleClick = (e: React.MouseEvent) => {
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

  return (
    <>
      <CardDraggable card={card} index={index} isDragDisabled={false}>
        {({ isDragging }) => (
          <div
            role="button"
            tabIndex={0}
            aria-label="Open card details"
            className={`group rounded bg-card p-3 text-sm shadow-sm hover:bg-accent/50 cursor-pointer ${isNew ? 'animate-slide-up' : ''} ${isDragging ? 'shadow-lg scale-[1.02]' : ''}`}
            onClick={handleClick}
            onPointerDown={handlePointerDown}
            onKeyDown={handleKeyDown}
            style={{
              cursor: isDragging ? 'grabbing' : 'grab',
            }}
          >
            <CardPreview
              card={card}
              actions={
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="p-1 h-auto opacity-0 group-hover:opacity-100 focus:opacity-100"
                      aria-label="Card menu"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation();
                        setShowDeleteDialog(true);
                      }}
                      className="text-destructive focus:text-destructive"
                    >
                      <Trash2 className="mr-2 h-4 w-4" />
                      Delete
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              }
            />
          </div>
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
