import { DndContext, MouseSensor, TouchSensor, KeyboardSensor, useSensor, useSensors, DragOverlay, defaultDropAnimationSideEffects } from '@dnd-kit/core';
import { arrayMove, sortableKeyboardCoordinates } from '@dnd-kit/sortable';
import type { DragEndEvent, DragOverEvent, DragStartEvent, DropAnimation } from '@dnd-kit/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { usePrefersReducedMotion } from '@/hooks/use-prefers-reduced-motion';
import { createPortal } from 'react-dom';
import { useMoveCard, useReorderCard, type Card } from './use-cards';
import type { DragData } from './use-cards';
import { useToast } from '@/components/ui/use-toast';
import { useQueryClient } from '@tanstack/react-query';
import type { Column } from '../columns/columns.api';
import { CardPreview } from './card-preview';
import { moveCardToColumn } from './drag-over-cache';
import { DragDisabledContext } from './drag-disabled-context';

// rAF coalescing helpers (T021 / P4). Kept behind globalThis lookups so tests can
// stub requestAnimationFrame/cancelAnimationFrame deterministically, with a
// setTimeout fallback for environments lacking rAF.
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
}

export function DragDropContext({ boardId, children, disabled = false }: DragDropContextProps) {
  const moveCardMutation = useMoveCard();
  const reorderCardMutation = useReorderCard();
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [activeCard, setActiveCard] = useState<Card | null>(null);
  const [dragSource, setDragSource] = useState<{ columnId: number; index: number } | null>(null);
  const prefersReducedMotion = usePrefersReducedMotion();
  const dropAnimation: DropAnimation = useMemo(() => ({
    duration: prefersReducedMotion ? 1 : 300,
    easing: prefersReducedMotion ? 'linear' : 'cubic-bezier(0.34, 1.56, 0.64, 1)',
    sideEffects: defaultDropAnimationSideEffects({
      styles: {
        active: {
          opacity: '0.5',
        },
      },
    }),
  }), [prefersReducedMotion]);
  const sensors = useSensors(
    useSensor(MouseSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(TouchSensor, {
      activationConstraint: {
        // Must exceed Radix ContextMenu long-press (700ms) so tap-and-hold
        // opens the menu when stationary instead of starting a drag.
        delay: 750,
        tolerance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
      // Deterministic for tests/e2e — smooth scrolling makes keyboard-drag
      // step timing unpredictable (scroll callbacks fire during assertions).
      scrollBehavior: 'auto',
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

  // rAF-coalesced dragOver: dnd-kit fires onDragOver at pointer-move rate, and
  // each un-throttled setQueryData re-rendered the whole board. At most ONE
  // cache write happens per animation frame; the pending event slot always holds
  // the latest event only (earlier ones in the same frame are obsolete).
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
    // A fresh drag supersedes any frame scheduled by a previous drag.
    cancelPendingDragOver();
    const activeData = event.active.data.current as DragData & { index?: number };
    if (activeData?.card) {
      setActiveCard(activeData.card);
      setDragSource({
        columnId: activeData.sourceColumnId,
        index: activeData.index ?? 0,
      });
    }
  }, [cancelPendingDragOver]);

  // Resolves the target column + insert index for a cross-column event and
  // applies the live rearrangement — ONLY when the order actually changed.
  // Same-column events and no-op echoes return the cache untouched (skip write).
  const applyDragOver = useCallback((event: DragOverEvent) => {
    const { active, over } = event;
    if (!over) return;

    const activeData = active.data.current as DragData;
    if (!activeData || !activeData.card) return;

    const cardId = activeData.cardId;
    const columns = getColumns();
    if (!columns) return;

    // Find the column the card is CURRENTLY in (position may have been moved
    // optimistically by an earlier live rearrangement).
    let sourceColumn: Column | undefined;
    for (const col of columns) {
      const idx = col.cards.findIndex((c) => c.id === cardId);
      if (idx !== -1) {
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

    if (!isSameColumn) {
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
      // Structural sharing: only the changed column(s) get new identities;
      // unchanged cards keep their object references; no order change → the
      // helper returns the SAME array reference and we skip setQueryData.
      const next = moveCardToColumn(columns, cardId, targetColumnId, insertIndex, activeData.card);
      if (next === columns) return;
      queryClient.setQueryData<Column[]>(['columns', boardId], next);
    }
  }, [getColumns, queryClient, boardId]);

  const flushPendingDragOver = useCallback(() => {
    rafIdRef.current = null;
    const event = pendingDragOverRef.current;
    pendingDragOverRef.current = null;
    if (event) applyDragOver(event);
  }, [applyDragOver]);

  const handleDragOver = useCallback((event: DragOverEvent) => {
    // Live rearrangement for cross-column drag: update target column cards
    // but do NOT call mutations (those are handled in handleDragEnd).
    pendingDragOverRef.current = event;
    if (rafIdRef.current === null) {
      rafIdRef.current = scheduleFrame(flushPendingDragOver);
    }
  }, [flushPendingDragOver]);

  const handleDragEnd = useCallback(async (event: DragEndEvent) => {
    // The drop supersedes any in-flight coalesced dragOver frame — drop it so a
    // stale pending event can never land after the post-drop cache writes here.
    cancelPendingDragOver();
    const { active, over } = event;
    const animationDuration = prefersReducedMotion ? 1 : 300;

    // Delay clearing activeCard until bounce animation completes
    // so the drag overlay stays visible during the settlement animation
    setTimeout(() => {
      setActiveCard(null);
      setDragSource(null);
    }, animationDuration);

    if (!over) {
      // A cross-column dragOver may have already moved the card optimistically in
      // the cache — the drop landed nowhere, so nothing will sync the server.
      // Roll back to server truth (same semantics as handleDragCancel).
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      return;
    }

    const activeData = active.data.current as DragData & { index?: number };
    if (!activeData || !activeData.card) {
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      return;
    }

    const cardId = activeData.cardId;
    const columns = getColumns();
    if (!columns) {
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      return;
    }

    // Use original source column from drag start (captured in handleDragStart)
    // handleDragOver may have already moved the card optimistically in the cache
    if (!dragSource) {
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      return;
    }
    const sourceColumnId = dragSource.columnId;
    const sourceColumn = columns.find((col) => col.id === sourceColumnId);

    if (!sourceColumn) {
      queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
      return;
    }

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
      const sourceTotal = (sourceColumn as Column & { total_cards?: number }).total_cards ?? cards.length;
      const sourceHasUnloaded = cards.length < sourceTotal;

      let newIndex: number;
      if (overData?.card) {
        const cardIdx = cards.findIndex((c) => c.id === overData.card!.id);
        newIndex = cardIdx !== -1 ? cardIdx : cards.length - 1;
      } else {
        if (sourceHasUnloaded) {
          toast({ title: 'Load more cards before dropping at the end of this column', type: 'error' });
          queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
          return;
        }
        newIndex = cards.length - 1;
      }

      // The dragged card's position in the CURRENT cache may differ from the
      // dragStart snapshot (optimistic live moves may have relocated it) —
      // look it up live; -1 means the drop is out of our hands (invalidate).
      const currentIndex = cards.findIndex((c) => c.id === cardId);
      if (currentIndex === -1) {
        queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
        return;
      }

      if (currentIndex === newIndex) return;

      setColumns((columns) => {
        return columns.map((col) => {
          if (col.id !== sourceColumn.id) return col;
          const reordered = arrayMove(col.cards, currentIndex, newIndex).map((c, i) => (c.position === i ? c : {
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
      // Cross-column move — authoritative for both the "card never moved live"
      // and the "card was live-moved into the target" cases. Shares dragOver's
      // structural sharing: reindex only changed cards; same reference → no write.
      const targetColumn = columns.find((col) => col.id === targetColumnId);
      if (!targetColumn) {
        queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
        return;
      }

      const overData = over.data.current as { card?: Card; index?: number };
      const targetCards = targetColumn.cards;
      const targetTotal = (targetColumn as Column & { total_cards?: number }).total_cards ?? targetCards.length;
      const targetHasUnloaded = targetCards.length < targetTotal;

      let insertIndex: number;
      if (overData?.card) {
        const cardIdx = targetCards.findIndex((c) => c.id === overData.card!.id);
        insertIndex = cardIdx !== -1 ? cardIdx : targetCards.length;
      } else {
        if (targetHasUnloaded) {
          toast({ title: 'Load more cards before dropping at the end of this column', type: 'error' });
          queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
          return;
        }
        insertIndex = targetCards.length;
      }

      setColumns((old) => moveCardToColumn(old, cardId, targetColumnId, insertIndex, activeData.card));

      try {
        await moveCardMutation.mutateAsync({ id: cardId, data: { column_id: targetColumnId, position: insertIndex } });
        queryClient.invalidateQueries({ queryKey: ['board-cards', boardId] });
        toast({ title: 'Card moved', type: 'success' });
      } catch {
        toast({ title: 'Failed to move card', type: 'error' });
        queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
        queryClient.invalidateQueries({ queryKey: ['board-cards', boardId] });
      }
    }
  }, [dragSource, moveCardMutation, reorderCardMutation, queryClient, toast, boardId, getColumns, setColumns, prefersReducedMotion, cancelPendingDragOver]);

  const handleDragCancel = useCallback(() => {
    cancelPendingDragOver();
    setActiveCard(null);
    setDragSource(null);
    queryClient.invalidateQueries({ queryKey: ['columns', boardId] });
  }, [queryClient, boardId, cancelPendingDragOver]);

  return (
    <DragDisabledContext.Provider value={disabled}>
      <DndContext
        sensors={disabled ? [] : sensors}
        onDragStart={disabled ? undefined : handleDragStart}
        onDragOver={disabled ? undefined : handleDragOver}
        onDragEnd={disabled ? undefined : handleDragEnd}
        onDragCancel={disabled ? undefined : handleDragCancel}
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
    </DragDisabledContext.Provider>
  );
}
