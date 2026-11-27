import { DndContext, MouseSensor, TouchSensor, KeyboardSensor, useSensor, useSensors, DragOverlay, defaultDropAnimationSideEffects } from '@dnd-kit/core';
import { sortableKeyboardCoordinates } from '@dnd-kit/sortable';
import type { DragEndEvent, DragOverEvent, DragStartEvent, DropAnimation } from '@dnd-kit/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { usePrefersReducedMotion } from '@/hooks/use-prefers-reduced-motion';
import { createPortal } from 'react-dom';
import { useAnchorMoveCard, type Card } from './use-cards';
import type { DragData } from './use-cards';
import { useToast } from '@/components/ui/use-toast';
import { useQueryClient } from '@tanstack/react-query';
import type { Column } from '../columns/columns.api';
import { CardPreview } from './card-preview';
import { moveCardToColumn } from './drag-over-cache';
import { DragDisabledContext } from './drag-disabled-context';
import { columnQueryKey, hasBoardColumnFilters, type BoardColumnFilters } from '../columns/use-columns';
import { resolveCardDrop } from './drop-resolver';

function scheduleFrame(cb: () => void): number {
  if (typeof globalThis.requestAnimationFrame === 'function') {
    return globalThis.requestAnimationFrame(cb);
  }
  return globalThis.setTimeout(cb, 16) as unknown as number;
}

function cancelFrame(id: number): void {
  if (typeof globalThis.cancelAnimationFrame === 'function') {
    globalThis.cancelAnimationFrame(id);
    return;
  }
  globalThis.clearTimeout(id);
}

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

interface DragDropContextProps {
  boardId: number;
  children: React.ReactNode;
  disabled?: boolean;
  filters?: BoardColumnFilters;
}

export function DragDropContext({ boardId, children, disabled = false, filters }: DragDropContextProps) {
  const anchorMove = useAnchorMoveCard(boardId);
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [activeCard, setActiveCard] = useState<Card | null>(null);
  const [dragSource, setDragSource] = useState<{ columnId: number; index: number } | null>(null);
  const [blockedStatus, setBlockedStatus] = useState('');
  const blockedNonceRef = useRef(0);
  const activeKeyRef = useRef<readonly unknown[]>(columnQueryKey(boardId, filters));
  const filterActive = hasBoardColumnFilters(filters);
  const prefersReducedMotion = usePrefersReducedMotion();

  // NB: activeKeyRef is refreshed ONLY in handleDragStart (T053 locked rule —
  // "derive activeKey at drag start"). Refreshing it on filter change would let
  // a mid-drag filter change silently re-target this drag's cache reads/writes;
  // board-scoped prefix invalidation on settle converges instead.

  const dropAnimation: DropAnimation = useMemo(() => ({
    duration: prefersReducedMotion ? 1 : 300,
    easing: prefersReducedMotion ? 'linear' : 'cubic-bezier(0.34, 1.56, 0.64, 1)',
    sideEffects: defaultDropAnimationSideEffects({
      styles: {
        active: { opacity: '0.5' },
      },
    }),
  }), [prefersReducedMotion]);

  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 5 } }),
    useSensor(TouchSensor, { activationConstraint: { delay: 750, tolerance: 8 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
      scrollBehavior: 'auto',
    }),
  );

  const getColumns = useCallback((): Column[] | undefined => {
    return queryClient.getQueryData<Column[]>(activeKeyRef.current as unknown[]);
  }, [queryClient]);

  const invalidateBoard = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
    queryClient.invalidateQueries({ queryKey: ['board-cards', boardId] });
  }, [queryClient, boardId]);

  const announceBlocked = useCallback((reason?: string) => {
    if (filterActive) {
      const nonce = ++blockedNonceRef.current;
      const msg = `Can't drop here while filters are active.${nonce % 2 === 0 ? '\u200b' : ''}`;
      setBlockedStatus(msg);
      toast({ title: "Can't drop here while filters are active." });
    } else {
      const title = reason === 'unloaded-tail'
        ? 'Load more cards before dropping at the end of this column'
        : "Can't drop here";
      toast({ title, type: 'error' });
    }
  }, [toast, filterActive]);

  const pendingDragOverRef = useRef<DragOverEvent | null>(null);
  const rafIdRef = useRef<number | null>(null);

  const cancelPendingDragOver = useCallback(() => {
    pendingDragOverRef.current = null;
    if (rafIdRef.current !== null) {
      cancelFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
  }, []);

  useEffect(() => () => cancelPendingDragOver(), [cancelPendingDragOver]);

  const handleDragStart = useCallback((event: DragStartEvent) => {
    cancelPendingDragOver();
    activeKeyRef.current = columnQueryKey(boardId, filters);
    const activeData = event.active.data.current as DragData & { index?: number };
    if (activeData?.card) {
      setActiveCard(activeData.card);
      setDragSource({
        columnId: activeData.sourceColumnId,
        index: activeData.index ?? 0,
      });
    }
  }, [cancelPendingDragOver, boardId, filters]);

  const applyDragOver = useCallback((event: DragOverEvent) => {
    const { active, over } = event;
    if (!over) return;
    const activeData = active.data.current as DragData;
    if (!activeData?.card) return;

    const cardId = activeData.cardId;
    const columns = getColumns();
    if (!columns) return;

    let sourceColumn: Column | undefined;
    for (const col of columns) {
      if (col.cards.some((c) => c.id === cardId)) {
        sourceColumn = col;
        break;
      }
    }
    if (!sourceColumn) return;

    const overId = over.id as string;
    let targetColumnId: number | undefined;
    const overColumnId = over.data.current
      ? (over.data.current as { columnId?: number }).columnId
      : undefined;

    if (overColumnId !== undefined) {
      targetColumnId = overColumnId;
    } else {
      const overCardId = getCardIdFromDndId(overId) ?? (over.data.current as { card?: { id: number } })?.card?.id;
      for (const col of columns) {
        if (col.cards.some((c) => c.id === overCardId)) {
          targetColumnId = col.id;
          break;
        }
      }
      if (targetColumnId === undefined) {
        targetColumnId = getColumnIdFromDndId(overId) ?? sourceColumn.id;
      }
    }
    if (targetColumnId === undefined) return;

    const isSameColumn = sourceColumn.id === targetColumnId;
    if (isSameColumn) return;

    const targetColumn = columns.find((col) => col.id === targetColumnId);
    if (!targetColumn) return;

    const overData = over.data.current as { card?: { id: number }; index?: number };
    const targetCards = targetColumn.cards;
    let insertIndex: number;
    if (overData?.card) {
      const cardIdx = targetCards.findIndex((c) => c.id === overData.card!.id);
      insertIndex = cardIdx !== -1 ? cardIdx : targetCards.length;
    } else {
      insertIndex = targetCards.length;
    }
    const next = moveCardToColumn(columns, cardId, targetColumnId, insertIndex, activeData.card, { filterActive });
    if (next === columns) return;
    queryClient.setQueryData<Column[]>(activeKeyRef.current as unknown[], next);
  }, [getColumns, queryClient, filterActive]);

  const flushPendingDragOver = useCallback(() => {
    rafIdRef.current = null;
    const event = pendingDragOverRef.current;
    pendingDragOverRef.current = null;
    if (event) applyDragOver(event);
  }, [applyDragOver]);

  const handleDragOver = useCallback((event: DragOverEvent) => {
    pendingDragOverRef.current = event;
    if (rafIdRef.current === null) {
      rafIdRef.current = scheduleFrame(flushPendingDragOver);
    }
  }, [flushPendingDragOver]);

  const handleDragEnd = useCallback(async (event: DragEndEvent) => {
    cancelPendingDragOver();
    const { active, over } = event;
    const animationDuration = prefersReducedMotion ? 1 : 300;
    setTimeout(() => {
      setActiveCard(null);
      setDragSource(null);
    }, animationDuration);

    if (!over) {
      invalidateBoard();
      return;
    }

    const activeData = active.data.current as DragData & { index?: number };
    if (!activeData?.card || !dragSource) {
      invalidateBoard();
      return;
    }

    const cardId = activeData.cardId;
    const columns = getColumns();
    if (!columns) {
      invalidateBoard();
      return;
    }

    const sourceColumnId = dragSource.columnId;
    const sourceColumn = columns.find((col) => col.id === sourceColumnId);
    if (!sourceColumn) {
      invalidateBoard();
      return;
    }

    const overId = over.id as string;
    let targetColumnId: number | undefined;
    const overColumnId = over.data.current
      ? (over.data.current as { columnId?: number }).columnId
      : undefined;

    if (overColumnId !== undefined) {
      targetColumnId = overColumnId;
    } else {
      const overCardId = getCardIdFromDndId(overId) ?? (over.data.current as { card?: Card })?.card?.id;
      for (const col of columns) {
        if (col.cards.some((c) => c.id === overCardId)) {
          targetColumnId = col.id;
          break;
        }
      }
      if (targetColumnId === undefined) {
        targetColumnId = getColumnIdFromDndId(overId) ?? sourceColumn.id;
      }
    }
    if (targetColumnId === undefined) {
      invalidateBoard();
      return;
    }

    const overData = over.data.current as { card?: Card; index?: number };
    const overCardId = overData?.card?.id ?? getCardIdFromDndId(overId);
    const overIsColumnContainer = getColumnIdFromDndId(overId) !== undefined || (overColumnId !== undefined && !overData?.card);

    const resolution = resolveCardDrop({
      activeCardId: cardId,
      sourceColumnId,
      targetColumnId,
      overCardId: overData?.card ? overData.card.id : (overIsColumnContainer ? undefined : overCardId),
      overIsColumnContainer: !!overIsColumnContainer,
      columns: columns.map((c) => ({
        id: c.id,
        cards: c.cards,
        total_cards: (c as Column & { total_cards?: number }).total_cards,
        matching_cards: (c as Column & { matching_cards?: number }).matching_cards,
      })),
      filterActive,
    });

    if (resolution.kind === 'noop') {
      // No-op: but if live rearrangement happened during dragOver, rollback.
      invalidateBoard();
      return;
    }

    if (resolution.kind === 'blocked') {
      announceBlocked(resolution.reason);
      invalidateBoard();
      return;
    }

    // Compute optimistic target index for visual placement.
    const targetColumn = columns.find((col) => col.id === targetColumnId);
    if (!targetColumn) {
      invalidateBoard();
      return;
    }

    const payload = resolution.payload;
    const isSameColumn = sourceColumnId === targetColumnId;

    const targetWithoutActive = targetColumn.cards.filter((c) => c.id !== cardId);
    let insertIndex = targetWithoutActive.length;
    if ('before_card_id' in payload && payload.before_card_id !== undefined) {
      const idx = targetWithoutActive.findIndex((c) => c.id === payload.before_card_id);
      insertIndex = idx === -1 ? targetWithoutActive.length : idx;
    } else if ('after_card_id' in payload && payload.after_card_id !== undefined) {
      const idx = targetWithoutActive.findIndex((c) => c.id === payload.after_card_id);
      insertIndex = idx === -1 ? targetWithoutActive.length : idx + 1;
    }

    if (isSameColumn) {
      const currentIndex = sourceColumn.cards.findIndex((c) => c.id === cardId);
      if (currentIndex === insertIndex) {
        invalidateBoard();
        return;
      }
    }

    // Optimistic update on captured active key only.
    const activeKey = activeKeyRef.current;
    const optimistic = (old: Column[] | undefined) => {
      if (!old) return old;
      return moveCardToColumn(old, cardId, targetColumnId, insertIndex, activeData.card, { filterActive });
    };

    try {
      await anchorMove.mutateAsync({
        id: cardId,
        data: payload,
        activeKey,
        optimistic,
      });
      toast({ title: isSameColumn ? 'Card reordered' : 'Card moved', type: 'success' });
    } catch {
      toast({ title: isSameColumn ? 'Failed to reorder card' : 'Failed to move card', type: 'error' });
      invalidateBoard();
    }
  }, [dragSource, anchorMove, toast, getColumns, prefersReducedMotion, cancelPendingDragOver, invalidateBoard, announceBlocked, filterActive]);

  const handleDragCancel = useCallback(() => {
    cancelPendingDragOver();
    setActiveCard(null);
    setDragSource(null);
    invalidateBoard();
  }, [cancelPendingDragOver, invalidateBoard]);

  return (
    <DragDisabledContext.Provider value={disabled}>
      <DndContext
        sensors={sensors}
        onDragStart={disabled ? undefined : handleDragStart}
        onDragOver={disabled ? undefined : handleDragOver}
        onDragEnd={disabled ? undefined : handleDragEnd}
        onDragCancel={disabled ? undefined : handleDragCancel}
      >
        {children}
        <div
          data-testid="dnd-blocked-status"
          role="status"
          aria-live="polite"
          aria-atomic="true"
          className="sr-only"
        >
          {blockedStatus}
        </div>
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
    </DragDisabledContext.Provider>
  );
}
