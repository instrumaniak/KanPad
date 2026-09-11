import { DndContext, MouseSensor, TouchSensor, KeyboardSensor, useSensor, useSensors, DragOverlay, defaultDropAnimationSideEffects } from '@dnd-kit/core';
import { sortableKeyboardCoordinates, arrayMove } from '@dnd-kit/sortable';
import type { DragEndEvent, DragStartEvent, DropAnimation } from '@dnd-kit/core';
import { useCallback, useState } from 'react';
import { createPortal } from 'react-dom';
import { useMoveCard, useReorderCard, type Card } from './use-cards';
import type { DragData } from './use-cards';
import { useToast } from '@/components/ui/use-toast';
import { useQueryClient } from '@tanstack/react-query';
import type { Column } from '../columns/columns.api';
import { CardPreview } from './card-preview';

function getCardIdFromDndId(id: string | number): number | undefined {
  if (typeof id === 'number') return id;
  if (id.startsWith('card-')) {
    const parsed = Number(id.slice(5));
    return Number.isNaN(parsed) ? undefined : parsed;
  }
  const parsed = Number(id);
  return Number.isNaN(parsed) ? undefined : parsed;
}

function getColumnIdFromDndId(id: string | number): number | undefined {
  if (typeof id === 'number') return id;
  if (id.startsWith('column-')) {
    const parsed = Number(id.slice(7));
    return Number.isNaN(parsed) ? undefined : parsed;
  }
  return undefined;
}

const dropAnimation: DropAnimation = {
  sideEffects: defaultDropAnimationSideEffects({
    styles: {
      active: {
        opacity: '0.5',
      },
    },
  }),
};

interface DragDropContextProps {
  boardId: number;
  children: React.ReactNode;
}

export function DragDropContext({ boardId, children }: DragDropContextProps) {
  const moveCardMutation = useMoveCard();
  const reorderCardMutation = useReorderCard();
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [activeCard, setActiveCard] = useState<Card | null>(null);

  const sensors = useSensors(
    useSensor(MouseSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(TouchSensor, {
      activationConstraint: {
        delay: 500,
        tolerance: 5,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  const getColumns = useCallback((): Column[] | undefined => {
    return queryClient.getQueryData<Column[]>(['columns', boardId]);
  }, [queryClient, boardId]);

  const setColumns = useCallback((updater: (old: Column[]) => Column[]): void => {
    queryClient.setQueryData<Column[]>(['columns', boardId], (old) => {
      if (!old) return old;
      return updater(old);
    });
  }, [queryClient, boardId]);

  const handleDragStart = useCallback((event: DragStartEvent) => {
    const activeData = event.active.data.current as DragData;
    if (activeData?.card) {
      setActiveCard(activeData.card);
    }
  }, []);

  const handleDragOver = useCallback(() => {
    // We intentionally do NOT update the React Query cache here.
    // Updating the cache during drag causes React to unmount/remount
    // card components between columns, which breaks dnd-kit's internal
    // DOM tracking and prevents handleDragEnd from firing.
    // The DragOverlay provides visual feedback during the drag instead.
  }, []);

  const handleDragEnd = useCallback(async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveCard(null);

    if (!over) return;

    const activeData = active.data.current as DragData;
    if (!activeData || !activeData.card) return;

    const cardId = activeData.cardId;
    const columns = getColumns();
    if (!columns) return;

    // Find which column the card is in and its position
    let sourceColumn: Column | undefined;
    let sourceCardIndex = -1;

    for (const col of columns) {
      const idx = col.cards.findIndex((c) => c.id === cardId);
      if (idx !== -1) {
        sourceColumn = col;
        sourceCardIndex = idx;
        break;
      }
    }

    if (!sourceColumn || sourceCardIndex === -1) return;

    const overId = over.id as string;
    let targetColumnId: number | undefined;

    // Determine target column from the 'over' item
    const overColumnId = over.data.current
      ? (over.data.current as { columnId?: number }).columnId
      : undefined;

    if (overColumnId !== undefined) {
      targetColumnId = overColumnId;
    } else {
      // Find which column the 'over' item belongs to
      const overCardId = getCardIdFromDndId(overId) ?? (over.data.current as { card?: Card })?.card?.id;
      for (const col of columns) {
        if (col.cards.some((c) => c.id === overCardId)) {
          targetColumnId = col.id;
          break;
        }
      }
      if (targetColumnId === undefined) {
        // 'over' might be a column itself
        targetColumnId = getColumnIdFromDndId(overId) ?? sourceColumn.id;
      }

      if (targetColumnId === undefined) return;
    }

    const isSameColumn = sourceColumn.id === targetColumnId;

    if (isSameColumn) {
      // Same-column reorder
      const overData = over.data.current as { card?: Card; index?: number };
      const cards = sourceColumn.cards;

      let newIndex: number;
      if (overData?.card) {
        const cardIdx = cards.findIndex((c) => c.id === overData.card!.id);
        newIndex = cardIdx !== -1 ? cardIdx : cards.length - 1;
      } else {
        newIndex = cards.length - 1;
      }

      if (sourceCardIndex === newIndex) return;

      setColumns((columns) => {
        return columns.map((col) => {
          if (col.id !== sourceColumn.id) return col;
          const reordered = arrayMove(col.cards, sourceCardIndex, newIndex).map((c, i) => ({
            ...c,
            position: i,
          }));
          return { ...col, cards: reordered };
        });
      });

      try {
        await reorderCardMutation.mutateAsync({ id: cardId, position: newIndex });
        toast({ title: 'Card reordered', type: 'success' });
      } catch {
        toast({ title: 'Failed to reorder card', type: 'error' });
        queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      }
    } else {
      // Cross-column move
      const targetColumn = columns.find((col) => col.id === targetColumnId);
      if (!targetColumn) return;

      const overData = over.data.current as { card?: Card; index?: number };
      const targetCards = targetColumn.cards;

      let insertIndex: number;
      if (overData?.card) {
        const cardIdx = targetCards.findIndex((c) => c.id === overData.card!.id);
        insertIndex = cardIdx !== -1 ? cardIdx : targetCards.length;
        if (insertIndex === -1) insertIndex = targetCards.length;
      } else {
        insertIndex = targetCards.length;
      }

      setColumns((columns) => {
        return columns.map((col) => {
          if (col.id === sourceColumn.id) {
            return { ...col, cards: col.cards.filter((c) => c.id !== cardId) };
          }
          if (col.id === targetColumnId) {
            const movedCard = { ...activeData.card, column_id: targetColumnId, position: insertIndex };
            const newCards = [...col.cards];
            newCards.splice(insertIndex, 0, movedCard);
            return { ...col, cards: newCards.map((c, i) => ({ ...c, position: i })) };
          }
          return col;
        });
      });

      try {
        await moveCardMutation.mutateAsync({ id: cardId, data: { column_id: targetColumnId, position: insertIndex } });
        toast({ title: 'Card moved', type: 'success' });
      } catch {
        toast({ title: 'Failed to move card', type: 'error' });
        queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      }
    }
  }, [moveCardMutation, reorderCardMutation, queryClient, toast, boardId, getColumns, setColumns]);

  const handleDragCancel = useCallback(() => {
    setActiveCard(null);
    queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
  }, [queryClient, boardId]);

  return (
    <DndContext
      sensors={sensors}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragEnd={handleDragEnd}
      onDragCancel={handleDragCancel}
    >
      {children}
      {createPortal(
        <DragOverlay dropAnimation={dropAnimation}>
          {activeCard ? (
            <div className="rounded border bg-card p-3 text-sm shadow-xl opacity-90 scale-105 cursor-grabbing w-full max-w-[280px]">
              <CardPreview card={activeCard} />
            </div>
          ) : null}
        </DragOverlay>,
        document.body
      )}
    </DndContext>
  );
}
