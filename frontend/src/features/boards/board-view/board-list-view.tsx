import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer, defaultRangeExtractor, type Range } from '@tanstack/react-virtual';
import { ArrowDown, ArrowUp, Plus, Trash2, Letters } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { fetchCards } from '../../cards/cards.api';
import { COLUMN_CARD_PAGE_SIZE } from '../../columns/use-columns';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
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
import type { Column as BoardColumn, Card } from '../../columns/columns.api';
import {
  sortFlatCards,
  type BoardListSortKey,
  type FlatCard,
  type SortDir,
} from './board-list-sort';
import { useCreateCard, useDeleteCard, useUpdateCard } from '../../cards/use-cards';
import { CardDetailPanel } from '../../cards/card-detail-panel';
import { LabelBadge } from '../../labels/label-badge';
import { getDueDateBadge } from '../../cards/date-utils';
import { ProgressBar } from '../../checklists/progress-bar';

interface BoardListViewProps {
  boardId: number;
  columns: BoardColumn[];
}

const SORT_OPTIONS: { key: BoardListSortKey; label: string }[] = [
  { key: 'created_at', label: 'Created' },
  { key: 'updated_at', label: 'Updated' },
  { key: 'due_date', label: 'Due' },
  { key: 'title', label: 'Title' },
];

// Upper bound of a list-row height (py-2 + title line + badge row + optional
// checklist-progress row). Dynamic rows are measured per-row via measureElement.
const ESTIMATED_ROW_HEIGHT = 96;
const ROW_OVERSCAN = 5;

export function BoardListView({ boardId, columns }: BoardListViewProps) {
  const [sortKey, setSortKey] = useState<BoardListSortKey>('created_at');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const [editingCardId, setEditingCardId] = useState<number | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [detailCard, setDetailCard] = useState<Card | null>(null);
  const [deleteCard, setDeleteCard] = useState<Card | null>(null);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [targetColumnId, setTargetColumnId] = useState<string>('');
  const titleCommitGuard = useRef<number | null>(null);
  // Focused row pinned in the virtual window — a focused, non-editing row that
  // scrolls out of the window would otherwise unmount underneath the user and
  // drop focus to <body> (WCAG 2.4.3 focus order).
  const [focusedCardId, setFocusedCardId] = useState<number | null>(null);
  const pendingFocusRestoreRef = useRef<number | null>(null);
  useEffect(() => () => {
    pendingFocusRestoreRef.current = null;
  }, []);

  const { toast } = useToast();
  const { showSuccess, showError } = useToastHelpers();
  const updateCard = useUpdateCard();
  const deleteCardMutation = useDeleteCard();
  const createCardMutation = useCreateCard();
  const queryClient = useQueryClient();
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const hasUnloadedCards = useMemo(
    () =>
      columns.some(
        (c) => ((c as BoardColumn & { total_cards?: number }).total_cards ?? c.cards.length) > c.cards.length,
      ),
    [columns],
  );

  const loadMoreCards = useCallback(async () => {
    if (isLoadingMore) return;
    setIsLoadingMore(true);
    try {
      const targets = columns.filter(
        (c) => ((c as BoardColumn & { total_cards?: number }).total_cards ?? c.cards.length) > c.cards.length,
      );
      for (const col of targets) {
        const total = (col as BoardColumn & { total_cards?: number }).total_cards ?? col.cards.length;
        if (col.cards.length >= total) continue;
        const nextPage = Math.floor(col.cards.length / COLUMN_CARD_PAGE_SIZE) + 1;
        const res = await fetchCards(col.id, nextPage, COLUMN_CARD_PAGE_SIZE);
        queryClient.setQueryData<BoardColumn[]>(['columns', boardId], (old) => {
          if (!old) return old;
          return old.map((c) => {
            if (c.id !== col.id) return c;
            const seen = new Set(c.cards.map((card) => card.id));
            const merged = [...c.cards];
            for (const card of res.data) {
              if (seen.has(card.id)) continue;
              seen.add(card.id);
              merged.push(card as BoardColumn['cards'][number]);
            }
            return { ...c, cards: merged };
          });
        });
      }
    } catch (err) {
      showError('Failed to load more cards', err instanceof Error ? err.message : 'Something went wrong');
    } finally {
      setIsLoadingMore(false);
    }
  }, [boardId, columns, isLoadingMore, queryClient]);

  const sortedColumns = useMemo(
    () => [...columns].sort((a, b) => a.position - b.position),
    [columns],
  );

  const flatCards = useMemo<FlatCard[]>(
    () => columns.flatMap((c) => (c.cards ?? []).map((card) => ({ ...card, columnName: c.name }))),
    [columns],
  );

  const sortedCards = useMemo(
    () => sortFlatCards(flatCards, sortKey, sortDir),
    [flatCards, sortKey, sortDir],
  );

  const scrollRef = useRef<HTMLDivElement>(null);
  // Keep the inline-edit row AND the focused row mounted even when they scroll
  // out of the virtual window — unmounting either mid-interaction destroys
  // the input/button and drops focus.
  const editingIndex = useMemo(
    () => sortedCards.findIndex((c) => c.id === editingCardId),
    [sortedCards, editingCardId],
  );
  const focusedIndex = useMemo(
    () => sortedCards.findIndex((c) => c.id === focusedCardId),
    [sortedCards, focusedCardId],
  );
  const rangeExtractor = useCallback(
    (range: Range) => {
      const base = defaultRangeExtractor(range);
      const missing = [...new Set([editingIndex, focusedIndex])].filter(
        (i) => i >= 0 && !base.includes(i),
      );
      if (missing.length === 0) return base;
      return [...base, ...missing].sort((a, b) => a - b);
    },
    [editingIndex, focusedIndex],
  );
  const getItemKey = useCallback(
    (index: number) => {
      const card = sortedCards[index];
      return card ? `${card.id}` : `__i_${index}`;
    },
    [sortedCards],
  );
  const virtualizer = useVirtualizer({
    count: sortedCards.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: ROW_OVERSCAN,
    getItemKey,
    rangeExtractor,
  });

  const handleSort = (key: BoardListSortKey) => {
    if (key === sortKey) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      setSortDir('asc');
    }
  };

  const titleRestoreRef = (cardId: number) => (el: HTMLButtonElement | null) => {
    if (el && pendingFocusRestoreRef.current === cardId) {
      pendingFocusRestoreRef.current = null;
      el.focus();
    }
  };

  const startTitleEdit = (card: Card) => {
    titleCommitGuard.current = null;
    setFocusedCardId(card.id);
    setEditingCardId(card.id);
    setEditTitle(card.title);
  };

  const restoreFocusAfterEdit = (card: Card) => {
    // Keep the row pinned and restore focus to its rename button once it
    // remounts — committing/cancelling must not strand the user on <body>.
    pendingFocusRestoreRef.current = card.id;
    setFocusedCardId(card.id);
  };

  const commitTitleEdit = (card: Card) => {
    if (titleCommitGuard.current === card.id || updateCard.isPending) return;
    const trimmed = editTitle.trim();
    if (!trimmed || trimmed === card.title) {
      titleCommitGuard.current = card.id;
      setEditingCardId(null);
      restoreFocusAfterEdit(card);
      return;
    }
    titleCommitGuard.current = card.id;
    updateCard.mutate(
      { id: card.id, data: { title: trimmed } },
      {
        onSuccess: () => {
          setEditingCardId(null);
          restoreFocusAfterEdit(card);
        },
        onError: (err) => {
          titleCommitGuard.current = null;
          showError(
            'Failed to update card',
            err instanceof Error ? err.message : 'Something went wrong',
          );
        },
      },
    );
  };

  const cancelTitleEdit = (card: Card) => {
    titleCommitGuard.current = card.id;
    setEditingCardId(null);
    restoreFocusAfterEdit(card);
  };

  const handleDelete = () => {
    if (!deleteCard) return;
    const card = deleteCard;
    const prevDetail = detailCard;
    const deletedSnapshot = { ...card };
    setDeleteCard(null);
    setDetailCard((d) => (d?.id === card.id ? null : d));
    deleteCardMutation.mutate(card.id, {
      onSuccess: () => {
        toast({
          title: 'Card deleted',
          type: 'success',
          action: {
            label: 'Undo',
            onClick: () => {
              createCardMutation.mutate(
                {
                  title: deletedSnapshot.title,
                  column_id: deletedSnapshot.column_id,
                  position: deletedSnapshot.position,
                  ...(deletedSnapshot.due_date ? { due_date: deletedSnapshot.due_date } : {}),
                },
                {
                  onSuccess: () => showSuccess('Card restored'),
                  onError: () => showError('Failed to restore card'),
                },
              );
            },
          },
        });
      },
      onError: () => {
        setDetailCard((d) => d ?? prevDetail);
        showError('Failed to delete card');
      },
    });
  };

  const parseColumnId = (raw: string): number => {
    if (!/^\d+$/.test(raw)) return NaN;
    const id = Number(raw);
    return Number.isSafeInteger(id) ? id : NaN;
  };

  const handleCreate = () => {
    const trimmed = newTitle.trim();
    const columnId = parseColumnId(targetColumnId);
    if (!trimmed || Number.isNaN(columnId)) return;
    const target = sortedColumns.find((c) => c.id === columnId);
    if (!target || (target.board_id !== undefined && target.board_id !== boardId)) {
      showError('Selected column is no longer available');
      return;
    }
    createCardMutation.mutate(
      { title: trimmed, column_id: columnId },
      {
        onSuccess: () => {
          setShowAddDialog(false);
          setNewTitle('');
          showSuccess('Card created');
        },
        onError: (err) =>
          showError(
            'Failed to create card',
            err instanceof Error ? err.message : 'Something went wrong',
          ),
      },
    );
  };

  const openAddDialog = () => {
    const first = sortedColumns[0];
    setTargetColumnId(first ? String(first.id) : '');
    setNewTitle('');
    setShowAddDialog(true);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div
        className="flex shrink-0 flex-wrap items-center gap-2 px-6 py-2"
        role="toolbar"
        aria-label="List controls"
      >
        {SORT_OPTIONS.map(({ key, label }) => {
          const active = sortKey === key;
          const Icon = active && sortDir === 'desc' ? ArrowDown : ArrowUp;
          return (
            <Button
              key={key}
              variant={active ? 'default' : 'ghost'}
              size="sm"
              aria-pressed={active}
              disabled={sortedCards.length === 0}
              onClick={() => handleSort(key)}
            >
              {label}
              <Icon className="ml-1 h-3 w-3" />
            </Button>
          );
        })}
        <div className="ml-auto">
          <Button size="sm" onClick={openAddDialog}>
            <Plus className="mr-1 h-4 w-4" />
            Add Card
          </Button>
        </div>
      </div>
      {hasUnloadedCards ? (
        <div className="flex items-center gap-2 px-6 py-1 text-xs text-muted-foreground" role="status">
          <span>Showing loaded cards only. Some columns have more cards.</span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void loadMoreCards()}
            disabled={isLoadingMore}
          >
            {isLoadingMore ? 'Loading...' : 'Load more cards'}
          </Button>
        </div>
      ) : null}

      <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto px-6 pb-6" data-testid="board-list-scroll">
        {sortedCards.length === 0 ? (
          <p className="py-8 text-center text-muted-foreground">No cards yet</p>
        ) : (
          <div role="table" aria-label="Board cards list" aria-rowcount={sortedCards.length + 1}>
            <div role="row" aria-rowindex={1} className="sr-only">
              {/* Header row mirrors the two data cells: the card column carries
                  the active sort (aria-sort), the actions column is labelled. */}
              <div
                role="columnheader"
                aria-sort={sortDir === 'asc' ? 'ascending' : 'descending'}
              >
                Cards sorted by {SORT_OPTIONS.find((o) => o.key === sortKey)?.label}
              </div>
              <div role="columnheader" aria-label="Actions" />
            </div>
            <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const card = sortedCards[virtualRow.index];
              if (!card) return null;
              const dueBadge = getDueDateBadge(card.due_date);
              return (
                <div
                  key={virtualRow.key}
                  ref={virtualizer.measureElement}
                  data-index={virtualRow.index}
                  role="row"
                  aria-rowindex={virtualRow.index + 2}
                  data-testid="board-list-row"
                  className="absolute left-0 top-0 flex w-full items-center gap-3 border-b py-2"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                  onFocusCapture={() => setFocusedCardId(card.id)}
                  onBlurCapture={(e) => {
                    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
                      setFocusedCardId(null);
                    }
                  }}
                >
                  <div role="cell" className="flex min-w-0 flex-1 flex-col gap-1">
                    {editingCardId === card.id ? (
                      <Input
                        value={editTitle}
                        autoFocus
                        aria-label="Edit card title"
                        onChange={(e) => setEditTitle(e.target.value)}
                        onClick={(e) => e.stopPropagation()}
                        onKeyDown={(e) => {
                          e.stopPropagation();
                          if (e.key === 'Enter') commitTitleEdit(card);
                          if (e.key === 'Escape') cancelTitleEdit(card);
                        }}
                        onBlur={() => commitTitleEdit(card)}
                      />
                    ) : (
                      <button
                        ref={titleRestoreRef(card.id)}
                        type="button"
                        aria-label={`Edit title of card: ${card.title}`}
                        className="flex items-center gap-1 break-words text-left font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          startTitleEdit(card);
                        }}
                      >
                        <span className="flex-1">{card.title}</span>
                        {card.has_description && (
                          <Letters
                            role="img"
                            className="h-4 w-4 shrink-0 text-muted-foreground"
                            aria-label="Has description"
                            data-testid="card-description-icon"
                          />
                        )}
                      </button>
                    )}
                    <button
                      type="button"
                      aria-label={`Open card details: ${card.title}`}
                      className="flex cursor-pointer flex-col gap-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm"
                      onClick={() => setDetailCard(card)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          setDetailCard(card);
                        }
                      }}
                    >
                      <span className="flex flex-wrap items-center gap-1">
                        <Badge variant="secondary">{card.columnName}</Badge>
                        {(card.labels ?? []).map((label) => (
                          <LabelBadge key={label.id} label={label} />
                        ))}
                        {dueBadge && (
                          <span
                            className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-medium ${dueBadge.className}`}
                          >
                            {dueBadge.text}
                          </span>
                        )}
                      </span>
                      {card.checklist_progress && (
                        <span className="flex items-center gap-2">
                          <ProgressBar
                            completed={card.checklist_progress.completed}
                            total={card.checklist_progress.total}
                            className="flex-1"
                          />
                          <span className="whitespace-nowrap text-xs text-muted-foreground">
                            {card.checklist_progress.completed}/{card.checklist_progress.total} (
                            {card.checklist_progress.percent}%)
                          </span>
                        </span>
                      )}
                    </button>
                  </div>
                  <div role="cell">
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`Delete card ${card.title}`}
                      onClick={() => setDeleteCard(card)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              );
            })}
            </div>
          </div>
        )}
      </div>

      {detailCard && (
        <CardDetailPanel
          card={detailCard}
          open={!!detailCard}
          onOpenChange={(o) => !o && setDetailCard(null)}
        />
      )}

      <AlertDialog open={!!deleteCard} onOpenChange={(o) => !o && setDeleteCard(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete card?</AlertDialogTitle>
            <AlertDialogDescription>
              The card &quot;{deleteCard?.title || 'this card'}&quot; will be deleted.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={handleDelete}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Dialog open={showAddDialog} onOpenChange={setShowAddDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add Card</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <Input
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              placeholder="Card title"
              aria-label="Card title"
              autoFocus
            />
            <label className="flex flex-col gap-1 text-sm font-medium">
              Column
              <select
                value={targetColumnId}
                onChange={(e) => setTargetColumnId(e.target.value)}
                aria-label="Target column"
                className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
              >
                {sortedColumns.map((c) => (
                  <option key={c.id} value={String(c.id)}>
                    {c.name}
                  </option>
                ))}
              </select>
            </label>
            {sortedColumns.length === 0 && (
              <p className="text-sm text-muted-foreground">
                No columns yet — add a column in Board view first.
              </p>
            )}
          </div>
          <DialogFooter>
            <Button
              onClick={handleCreate}
              disabled={!newTitle.trim() || !targetColumnId || createCardMutation.isPending}
            >
              {createCardMutation.isPending ? 'Creating...' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
