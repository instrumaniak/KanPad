import { memo, useCallback, useMemo, useRef, useState } from 'react';
import { useDndMonitor } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { useVirtualizer, defaultRangeExtractor, type Range } from '@tanstack/react-virtual';
import type { Card } from '../cards/use-cards';
import { PerfProfiler } from '@/lib/perf-profiler';
import { Card as CardComponent } from '../cards/card';

// Upper bound of a typical card face height (p-3 padding + title line + up to
// two badge rows + a checklist row). Only used until the row is measured —
// dynamic rows are measured per-row via measureElement, and underestimating
// causes scroll jumpiness (T021 spec).
const ESTIMATED_CARD_HEIGHT = 128;
const CARD_GAP = 8;
const OVERSCAN = 5;

const EMPTY_CARDS: Card[] = [];

interface ColumnCardListProps {
  cards: Card[];
  columnId?: number;
  newCardId?: number;
}

export const ColumnCardList = memo(function ColumnCardList({ cards, columnId, newCardId }: ColumnCardListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  // Track only the active drag id. useDndContext() re-renders on every
  // drag move/over; useDndMonitor fires callbacks without subscribing to
  // high-frequency context updates.
  const [activeDragId, setActiveDragId] = useState<string | number | null>(null);
  useDndMonitor({
    onDragStart: (e) => setActiveDragId(e.active.id),
    onDragEnd: () => setActiveDragId(null),
    onDragCancel: () => setActiveDragId(null),
  });
  // Focused row pinned in the virtual window — a focused, non-dragged row that
  // scrolls out of the window would otherwise unmount underneath the user and
  // drop focus to <body> (WCAG 2.4.3 focus order).
  const [focusedCardId, setFocusedCardId] = useState<number | null>(null);
  const cardList = cards ?? EMPTY_CARDS;
  // Pin the in-flight dragged row in the rendered range so dnd-kit never loses
  // its measured node mid-drag (react-virtual has no stickyIndices). For
  // cross-column drags the card is removed from this column and pinned in the
  // target column instead; the pin mainly covers same-column + keyboard drags.
  const draggedIndex = useMemo(
    () => cardList.findIndex((c) => `card-${c.id}` === activeDragId),
    [cardList, activeDragId],
  );
  const focusedIndex = useMemo(
    () => cardList.findIndex((c) => c.id === focusedCardId),
    [cardList, focusedCardId],
  );
  const sortableIds = useMemo(() => cardList.map((c) => `card-${c.id}`), [cardList]);
  // Stable reference + string-typed fallback key (never a bare index that could
  // numerically collide with a card id).
  const getItemKey = useCallback(
    (index: number) => cardList[index]?.id != null ? `${cardList[index].id}` : `__i_${index}`,
    [cardList],
  );
  const rangeExtractor = useCallback(
    (range: Range) => {
      const base = defaultRangeExtractor(range);
      const missing = [...new Set([draggedIndex, focusedIndex])].filter(
        (i) => i >= 0 && !base.includes(i),
      );
      if (missing.length === 0) return base;
      return [...base, ...missing].sort((a, b) => a - b);
    },
    [draggedIndex, focusedIndex],
  );

  // Hooks must stay unconditional — the empty-column early return lives below.
  const virtualizer = useVirtualizer({
    count: cardList.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ESTIMATED_CARD_HEIGHT,
    overscan: OVERSCAN,
    gap: CARD_GAP,
    getItemKey,
    rangeExtractor,
    // Scroll-only updates write item transforms/container size directly to DOM,
    // avoiding React rerenders on every scroll offset change. Requires
    // virtualizer-owned main-axis styles and containerRef below.
    directDomUpdates: true,
  });

  if (cardList.length === 0) {
    return null;
  }

  const virtualItems = virtualizer.getVirtualItems();

  const perfMeta = columnId !== undefined ? { columnId } : undefined;

  return (
    <PerfProfiler id="ColumnCardList" meta={perfMeta}>
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto p-2 kanpad-column-scroll" data-testid="column-card-list-scroll">
      <SortableContext
        items={sortableIds}
        strategy={verticalListSortingStrategy}
      >
        <div
          ref={virtualizer.containerRef}
          className="relative w-full"
          data-card-count={cardList.length}
        >
          {virtualItems.map((virtualRow) => {
            const card = cardList[virtualRow.index];
            if (!card) return null;
            return (
              <div
                key={virtualRow.key}
                ref={virtualizer.measureElement}
                data-index={virtualRow.index}
                data-testid="virtual-card-row"
                className="absolute left-0 top-0 w-full"
                onFocusCapture={() => setFocusedCardId(card.id)}
                onBlurCapture={(e) => {
                  if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
                    setFocusedCardId(null);
                  }
                }}
              >
                <CardComponent
                  card={card}
                  index={virtualRow.index}
                  isNew={card.id === newCardId}
                />
              </div>
            );
          })}
        </div>
      </SortableContext>
    </div>
    </PerfProfiler>
  );
});
