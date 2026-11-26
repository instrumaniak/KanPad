import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer, defaultRangeExtractor, type Range } from '@tanstack/react-virtual';
import { ArrowDown, ArrowUp, Plus, Trash2, Letters } from 'lucide-react';
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
import type { Card } from '../../columns/columns.api';
import type { CardListFilters, BoardCardListItem } from '../../cards/cards.api';
import { useBoardCards, flattenBoardCardPages, useCreateCard, useDeleteCard, useUpdateCard } from '../../cards/use-cards';
import { useColumns } from '../../columns/use-columns';
import { CardDetailPanel } from '../../cards/card-detail-panel';
import { LabelBadge } from '../../labels/label-badge';
import { getDueDateBadge } from '../../cards/date-utils';
import { ProgressBar } from '../../checklists/progress-bar';
import type { BoardListSortKey, SortDir } from './board-list-sort';

interface BoardListViewProps {
  boardId: number;
  filters: CardListFilters;
  filterActive: boolean;
}

const SORT_OPTIONS: { key: BoardListSortKey; label: string }[] = [
  { key: 'created_at', label: 'Created' },
  { key: 'updated_at', label: 'Updated' },
  { key: 'due_date', label: 'Due' },
  { key: 'title', label: 'Title' },
];

const ESTIMATED_ROW_HEIGHT = 96;
const ROW_OVERSCAN = 5;

function toCard(item: BoardCardListItem): Card {
  return {
    id: item.id,
    title: item.title,
    has_description: item.has_description,
    column_id: item.column_id,
    position: item.position,
    due_date: item.due_date,
    created_at: item.created_at,
    updated_at: item.updated_at,
    labels: item.labels,
    checklist_progress: item.checklist_progress,
  } as Card;
}

export function BoardListView({ boardId, filters, filterActive }: BoardListViewProps) {
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

  const { data: columnsData } = useColumns(boardId);
  const sortedColumns = useMemo(
    () => [...(columnsData ?? [])].sort((a, b) => a.position - b.position),
    [columnsData],
  );

  const {
    data,
    isLoading,
    isError,
    error,
    refetch,
    hasNextPage,
    fetchNextPage,
    isFetchingNextPage,
  } = useBoardCards(boardId, { ...filters, sort: sortKey, order: sortDir });

  const items = useMemo(() => flattenBoardCardPages(data?.pages), [data?.pages]);
  const total = data?.pages?.[0]?.total ?? 0;

  const scrollRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!hasNextPage || isFetchingNextPage) return;
    const root = scrollRef.current;
    const sentinel = sentinelRef.current;
    if (!root || !sentinel) return;
    if (typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && hasNextPage && !isFetchingNextPage) {
          void fetchNextPage();
        }
      },
      { root, rootMargin: '200px 0px' },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage, items.length]);

  const editingIndex = useMemo(
    () => items.findIndex((c) => c.id === editingCardId),
    [items, editingCardId],
  );
  const focusedIndex = useMemo(
    () => items.findIndex((c) => c.id === focusedCardId),
    [items, focusedCardId],
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
      const card = items[index];
      return card ? `${card.id}` : `__i_${index}`;
    },
    [items],
  );
  const virtualizer = useVirtualizer({
    count: items.length,
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

  const startTitleEdit = (card: BoardCardListItem) => {
    titleCommitGuard.current = null;
    setFocusedCardId(card.id);
    setEditingCardId(card.id);
    setEditTitle(card.title);
  };

  const restoreFocusAfterEdit = (card: BoardCardListItem) => {
    pendingFocusRestoreRef.current = card.id;
    setFocusedCardId(card.id);
  };

  const commitTitleEdit = (card: BoardCardListItem) => {
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

  const cancelTitleEdit = (card: BoardCardListItem) => {
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
              disabled={items.length === 0 && !isLoading}
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
      {filterActive ? (
        <p className="px-6 pb-1 text-xs text-muted-foreground" role="status">
          Drag and drop is disabled while search or filters are active.
        </p>
      ) : null}

      <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto px-6 pb-6" data-testid="board-list-scroll">
        {isLoading ? (
          <p className="py-8 text-center text-muted-foreground" role="status">Loading cards...</p>
        ) : isError ? (
          <div className="flex flex-col items-center gap-2 py-8 text-center" role="status">
            <p className="text-sm text-muted-foreground">
              {error instanceof Error ? error.message : 'Failed to load cards'}
            </p>
            <Button variant="outline" size="sm" onClick={() => void refetch()}>Retry</Button>
          </div>
        ) : items.length === 0 ? (
          <p className="py-8 text-center text-muted-foreground">No cards yet</p>
        ) : (
          <div role="table" aria-label="Board cards list" aria-rowcount={items.length + 1}>
            <div role="row" aria-rowindex={1} className="sr-only">
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
              const item = items[virtualRow.index];
              if (!item) return null;
              const card = toCard(item);
              const dueBadge = getDueDateBadge(item.due_date);
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
                  onFocusCapture={() => setFocusedCardId(item.id)}
                  onBlurCapture={(e) => {
                    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
                      setFocusedCardId(null);
                    }
                  }}
                >
                  <div role="cell" className="flex min-w-0 flex-1 flex-col gap-1">
                    {editingCardId === item.id ? (
                      <Input
                        value={editTitle}
                        autoFocus
                        aria-label="Edit card title"
                        onChange={(e) => setEditTitle(e.target.value)}
                        onClick={(e) => e.stopPropagation()}
                        onKeyDown={(e) => {
                          e.stopPropagation();
                          if (e.key === 'Enter') commitTitleEdit(item);
                          if (e.key === 'Escape') cancelTitleEdit(item);
                        }}
                        onBlur={() => commitTitleEdit(item)}
                      />
                    ) : (
                      <button
                        ref={titleRestoreRef(item.id)}
                        type="button"
                        aria-label={`Edit title of card: ${item.title}`}
                        className="flex items-center gap-1 break-words text-left font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          startTitleEdit(item);
                        }}
                      >
                        <span className="flex-1">{item.title}</span>
                        {item.has_description && (
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
                      aria-label={`Open card details: ${item.title}`}
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
                        <Badge variant="secondary">{item.column_name}</Badge>
                        {(item.labels ?? []).map((label) => (
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
                      {item.checklist_progress && (
                        <span className="flex items-center gap-2">
                          <ProgressBar
                            completed={item.checklist_progress.completed}
                            total={item.checklist_progress.total}
                            className="flex-1"
                          />
                          <span className="whitespace-nowrap text-xs text-muted-foreground">
                            {item.checklist_progress.completed}/{item.checklist_progress.total} (
                            {item.checklist_progress.percent}%)
                          </span>
                        </span>
                      )}
                    </button>
                  </div>
                  <div role="cell">
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`Delete card ${item.title}`}
                      onClick={() => setDeleteCard(card)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              );
            })}
            </div>
            {hasNextPage ? (
              <>
                <div ref={sentinelRef} data-testid="board-list-sentinel" />
                <div className="flex justify-center py-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => void fetchNextPage()}
                    disabled={isFetchingNextPage}
                    aria-label="Load more cards"
                  >
                    {isFetchingNextPage ? 'Loading more cards...' : 'Load more cards'}
                  </Button>
                </div>
              </>
            ) : null}
            <p className="sr-only" role="status" aria-live="polite">
              {isFetchingNextPage ? 'Loading more cards' : hasNextPage ? 'More cards available' : `End of list, ${items.length} of ${total} cards`}
            </p>
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
