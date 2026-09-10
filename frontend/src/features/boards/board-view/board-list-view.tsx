import { useMemo, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
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

  const { toast } = useToast();
  const updateCard = useUpdateCard();
  const deleteCardMutation = useDeleteCard();
  const createCardMutation = useCreateCard();

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

  const handleSort = (key: BoardListSortKey) => {
    if (key === sortKey) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      setSortDir('asc');
    }
  };

  const startTitleEdit = (card: Card) => {
    titleCommitGuard.current = null;
    setEditingCardId(card.id);
    setEditTitle(card.title);
  };

  const commitTitleEdit = (card: Card) => {
    if (titleCommitGuard.current === card.id || updateCard.isPending) return;
    const trimmed = editTitle.trim();
    if (!trimmed || trimmed === card.title) {
      titleCommitGuard.current = card.id;
      setEditingCardId(null);
      return;
    }
    titleCommitGuard.current = card.id;
    updateCard.mutate(
      { id: card.id, data: { title: trimmed } },
      {
        onSuccess: () => setEditingCardId(null),
        onError: (err) => {
          titleCommitGuard.current = null;
          toast({
            title: 'Failed to update card',
            description: err instanceof Error ? err.message : 'Something went wrong',
            type: 'error',
          });
        },
      },
    );
  };

  const cancelTitleEdit = (card: Card) => {
    titleCommitGuard.current = card.id;
    setEditingCardId(null);
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
                  onSuccess: () => toast({ title: 'Card restored', type: 'success' }),
                  onError: () => toast({ title: 'Failed to restore card', type: 'error' }),
                },
              );
            },
          },
        });
      },
      onError: () => {
        setDetailCard((d) => d ?? prevDetail);
        toast({ title: 'Failed to delete card', type: 'error' });
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
      toast({ title: 'Selected column is no longer available', type: 'error' });
      return;
    }
    createCardMutation.mutate(
      { title: trimmed, column_id: columnId },
      {
        onSuccess: () => {
          setShowAddDialog(false);
          setNewTitle('');
          toast({ title: 'Card created', type: 'success' });
        },
        onError: (err) =>
          toast({
            title: 'Failed to create card',
            description: err instanceof Error ? err.message : 'Something went wrong',
            type: 'error',
          }),
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
    <div className="flex flex-1 flex-col overflow-hidden">
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

      <div className="flex-1 overflow-y-auto px-6 pb-6" data-testid="board-list-scroll">
        {sortedCards.length === 0 ? (
          <p className="py-8 text-center text-muted-foreground">No cards yet</p>
        ) : (
          <div role="table" aria-label="Board cards list">
            <div role="row" className="sr-only">
              {SORT_OPTIONS.map(({ key, label }) => (
                <div
                  key={key}
                  role="columnheader"
                  aria-sort={
                    sortKey === key
                      ? sortDir === 'asc'
                        ? 'ascending'
                        : 'descending'
                      : 'none'
                  }
                >
                  {label}
                </div>
              ))}
            </div>
            {sortedCards.map((card) => {
              const dueBadge = getDueDateBadge(card.due_date);
              return (
                <div
                  key={card.id}
                  role="row"
                  data-testid="board-list-row"
                  className="flex items-center gap-3 border-b py-2"
                >
                  <div
                    role="button"
                    tabIndex={0}
                    aria-label={`Open card details: ${card.title}`}
                    className="flex flex-1 cursor-pointer flex-col gap-1 outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    onClick={() => setDetailCard(card)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        setDetailCard(card);
                      }
                    }}
                  >
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
                      <span
                        className="break-words font-medium"
                        onClick={(e) => {
                          e.stopPropagation();
                          startTitleEdit(card);
                        }}
                      >
                        {card.title}
                      </span>
                    )}
                    <span className="flex flex-wrap items-center gap-1">
                      <Badge variant="secondary">{card.columnName}</Badge>
                      {(card.labels ?? []).slice(0, 3).map((label) => (
                        <LabelBadge key={label.id} label={label} />
                      ))}
                      {(card.labels?.length ?? 0) > 3 && (
                        <span className="inline-flex items-center rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                          +{(card.labels?.length ?? 0) - 3}
                        </span>
                      )}
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
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`Delete card ${card.title}`}
                    onClick={() => setDeleteCard(card)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {detailCard && (
        <CardDetailPanel
          card={
            detailCard as unknown as React.ComponentProps<typeof CardDetailPanel>['card']
          }
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
