import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ErrorState } from '@/components/error-state';
import { useBoard, useUpdateBoard, type BoardViewMode } from '../use-boards';
import { useColumns, useCreateColumn, type Column as ColumnData } from '../../columns/use-columns';
import { Column } from '../../columns/column';
import { AddColumnButton } from '../../columns/add-column-button';
import { useToastHelpers } from '@/lib/toast-helpers';
import { DragDropContext } from '../../cards/drag-drop-context';
import { BoardViewToggle } from './board-view-toggle';
import { BoardListView } from './board-list-view';
import { CardSearchInput } from './card-search-input';
import { FilterDropdown } from './filter-dropdown';
import { FilterChips } from './filter-chips';
import { useLabels } from '../../labels/use-labels';
import { DEFAULT_FILTER_STATE, hasActiveFilters, toServerCardFilters, type FilterState } from './filter-columns-by-filters';
import { useBreakpoint } from '@/hooks/use-breakpoint';
import { usePrefersReducedMotion } from '@/hooks/use-prefers-reduced-motion';
import { cn } from '@/lib/utils';
import { PerfProfiler } from '@/lib/perf-profiler';

// Loose status read on unknown errors — avoids depending on ApiError class identity across module boundaries (same rationale as T014's handler).
function errorStatus(err: unknown): number | undefined {
  if (!err || typeof err !== 'object' || !('status' in err)) return undefined;
  const status = (err as { status?: unknown }).status;
  return typeof status === 'number' ? status : undefined;
}

// Module-level immutable empty array — an inline `columns ?? []` would mint a
// fresh reference per render and defeat identity checks downstream (T021).
const EMPTY_COLUMNS: ColumnData[] = [];

export function BoardView() {
  const { boardId } = useParams<{ boardId: string }>();
  const navigate = useNavigate();
  const id = boardId ? parseInt(boardId, 10) : 0;

  const { data: boardResponse, isLoading: boardLoading, isError: boardQueryError, error: boardError, refetch: refetchBoard } = useBoard(id);
  const { data: allLabels } = useLabels();

  const board = boardResponse?.data;
  const boardName = boardResponse?.data?.name;
  const createColumnMutation = useCreateColumn();
  const updateBoardMutation = useUpdateBoard();
  const { showSuccess, showError } = useToastHelpers();

  const [view, setView] = useState<BoardViewMode>('board');
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [filters, setFilters] = useState<FilterState>(DEFAULT_FILTER_STATE);

  const scrollRef = useRef<{ board: number; list: number }>({ board: 0, list: 0 });
  const boardScrollRef = useRef<HTMLDivElement>(null);
  const listScrollRef = useRef<HTMLDivElement>(null);
  const lastServerView = useRef<BoardViewMode | undefined>(undefined);
  const viewRequestId = useRef(0);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const breakpoint = useBreakpoint();
  const isMobile = breakpoint === 'mobile';
  const prefersReducedMotion = usePrefersReducedMotion();

  // Sync local view when the persisted server value arrives/changes.
  // Guarded by ref so we only set state on actual server changes
  // (never clobbers an optimistic pending view on unrelated refetches).
  useEffect(() => {
    const serverView = board?.view_mode;
    if (
      (serverView === 'board' || serverView === 'list') &&
      serverView !== lastServerView.current
    ) {
      lastServerView.current = serverView;
      setView(serverView);
    }
  }, [board?.view_mode]);

  const debounceSearch = useCallback((value: string) => {
    clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => {
      setDebouncedSearch(value);
    }, 300);
  }, []);

  useEffect(() => () => clearTimeout(searchTimerRef.current), []);

  const handleSearchChange = useCallback((v: string) => {
    setSearch(v);
    debounceSearch(v);
  }, [debounceSearch]);

  const handleClearSearch = useCallback(() => {
    clearTimeout(searchTimerRef.current);
    setSearch('');
    setDebouncedSearch('');
  }, []);

  // Reset search on board change — inline body (do NOT call handler) to satisfy exhaustive-deps.
  // Stale query must not leak across boards; cascading render here is intentional and cheap.
  useEffect(() => {
    clearTimeout(searchTimerRef.current);
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSearch('');
    setDebouncedSearch('');
    setFilters(DEFAULT_FILTER_STATE);
  }, [id]);

  const activeQuery = debouncedSearch.trim();
  const isSearching = activeQuery.length >= 2;
  const filterActive = hasActiveFilters(filters) || isSearching;
  const serverFilters = useMemo(
    () => toServerCardFilters(filters, isSearching ? activeQuery : ''),
    [filters, isSearching, activeQuery],
  );

  const { data: columns, isLoading: columnsLoading, isError: columnsQueryError, error: columnsError, refetch: refetchColumns } = useColumns(id, serverFilters);

  // Server-driven: columns already contain authoritative filtered results.
  const filteredColumns = columns ?? EMPTY_COLUMNS;

  // Stable column metadata for child props. Drag commits create new column arrays
  // but rarely change column names/positions; keying on metadata keeps
  // moveTargets/nextColumnId references stable so Column/ColumnHeader can skip rerenders.
  const columnMetaKey = (columns ?? EMPTY_COLUMNS)
    .map((c) => `${c.id}:${c.name}:${c.position}`)
    .join('|');
  const moveTargets = useMemo(
    () => (columns ?? EMPTY_COLUMNS).map((c) => ({ id: c.id, name: c.name })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [columnMetaKey],
  );
  const nextColumnIdById = useMemo(() => {
    const map = new Map<number, number>();
    const cols = columns ?? EMPTY_COLUMNS;
    for (const c of cols) {
      const next = cols.find((n) => n.position === c.position + 1);
      if (next) map.set(c.id, next.id);
    }
    return map;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [columnMetaKey]);

  const columnsForCounts = columns ?? EMPTY_COLUMNS;
  const visibleCount = useMemo(
    () => filteredColumns.reduce((n, c) => n + (c.cards ?? []).length, 0),
    [filteredColumns],
  );
  const totalCount = useMemo(
    () =>
      columnsForCounts.reduce((n, c) => {
        const total = filterActive
          ? (c as typeof c & { matching_cards?: number }).matching_cards ?? c.cards.length
          : (c as typeof c & { total_cards?: number }).total_cards ?? c.cards.length;
        return n + total;
      }, 0),
    [columnsForCounts, filterActive],
  );
  const isFilteredEmpty =
    !boardLoading && !columnsLoading && !!board && hasActiveFilters(filters) && filteredColumns.every((c) => ((c as typeof c & { matching_cards?: number }).matching_cards ?? (c.cards ?? []).length) === 0);
  const isSearchEmpty =
    !boardLoading && !columnsLoading && !!board && isSearching && !hasActiveFilters(filters) && filteredColumns.every((c) => ((c as typeof c & { matching_cards?: number }).matching_cards ?? (c.cards ?? []).length) === 0);
  const isSearchAndFilterEmpty =
    !boardLoading && !columnsLoading && !!board && isSearching && hasActiveFilters(filters) && filteredColumns.every((c) => ((c as typeof c & { matching_cards?: number }).matching_cards ?? (c.cards ?? []).length) === 0);
  const isEmpty = isSearchEmpty || isFilteredEmpty || isSearchAndFilterEmpty;

  const getListScroller = (): Element | null =>
    listScrollRef.current?.querySelector('[data-testid="board-list-scroll"]') ?? null;

  const handleViewChange = (next: BoardViewMode) => {
    if (next === view || updateBoardMutation.isPending) return;
    if (boardScrollRef.current) scrollRef.current.board = boardScrollRef.current.scrollLeft;
    const listScroll = getListScroller();
    if (listScroll) scrollRef.current.list = listScroll.scrollTop;
    const prev = view;
    const requestId = ++viewRequestId.current;
    setView(next);
    updateBoardMutation.mutate(
      { id, data: { view_mode: next } },
      {
        onError: (err) => {
          if (viewRequestId.current !== requestId) return;
          setView(prev);
          showError(
            'Failed to save view',
            err instanceof Error ? err.message : 'Something went wrong',
          );
        },
      },
    );
  };

  useEffect(() => {
    if (view === 'board' && boardScrollRef.current) {
      boardScrollRef.current.scrollLeft = scrollRef.current.board;
    }
    if (view === 'list') {
      const listScroll = getListScroller();
      if (listScroll) listScroll.scrollTop = scrollRef.current.list;
    }
  }, [view]);

  const handleAddColumn = async () => {
    try {
      await createColumnMutation.mutateAsync({ boardId: id, data: {} });
      showSuccess('Column added');
    } catch (err) {
      showError(
        'Failed to add column',
        err instanceof Error ? err.message : 'Something went wrong',
      );
    }
  };

  // 401-shaped errors: T014's global session-expiry handler owns the UX
  // (single-flight cache clear → soft navigate /login → one toast). Render the
  // loading state — not an error — until the handler unmounts this route.
  // NB: deliberately coarse — a 401 on one query and a genuine failure on the
  // other both land here while T014 is mounted (it redirects regardless).
  // Depends on SessionExpiryHandler being mounted in App.tsx; reliance documented in T015 §Notes.
  if (boardLoading || columnsLoading || errorStatus(boardError) === 401 || errorStatus(columnsError) === 401) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  // A 404 board fetch is a permanent "not found", not a retryable failure —
  // fall through to the not-found view (no misleading retry button).
  const boardNotFound = boardQueryError && errorStatus(boardError) === 404;

  // Last-good data wins: React Query preserves `data` on a background/refetch
  // failure — a full-screen error would hide a perfectly good board. Only fail
  // hard when there is nothing to show.
  const boardFailed = boardQueryError && !boardResponse && !boardNotFound;
  const columnsFailed = columnsQueryError && !columns;

  if (boardFailed || columnsFailed) {
    const activeError = columnsFailed ? columnsError : boardError;
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <ErrorState
          title="Failed to load board"
          message={activeError instanceof Error ? activeError.message : 'Something went wrong'}
          onRetry={() => {
            void refetchBoard();
            void refetchColumns();
          }}
        />
      </div>
    );
  }

  if (!board) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-muted-foreground">Board not found</p>
      </div>
    );
  }

  return (
    <PerfProfiler id="BoardView">
      <div className="flex h-full min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 px-4 py-2 sm:gap-4 sm:px-6">
        <Button variant="ghost" size="icon" onClick={() => navigate('/')}>
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <h1 className="min-w-0 truncate text-xl font-semibold">{boardName}</h1>
        <div className={cn(
          'ml-auto flex flex-wrap items-center justify-end gap-1 sm:gap-2',
          isMobile && 'ml-0 w-full flex-nowrap',
        )}>
          <CardSearchInput
            value={search}
            onChange={handleSearchChange}
            onClear={handleClearSearch}
          />
          <FilterDropdown
            filters={filters}
            onFiltersChange={setFilters}
            availableLabels={allLabels?.data ?? []}
          />
          <BoardViewToggle
            value={view}
            onChange={handleViewChange}
            disabled={updateBoardMutation.isPending}
          />
        </div>
      </div>
      <FilterChips
        filters={filters}
        onClearFilter={(type) => {
          if (type === 'labels') setFilters((prev) => ({ ...prev, labels: [] }));
          else if (type === 'dueDate') setFilters((prev) => ({ ...prev, dueDate: null }));
          else if (type === 'checklist') setFilters((prev) => ({ ...prev, checklist: 'All' }));
        }}
        onClearAll={() => setFilters(DEFAULT_FILTER_STATE)}
        availableLabels={allLabels?.data ?? []}
      />
      {filterActive ? (
        <p className="px-4 text-xs text-muted-foreground sm:px-6" role="status">
          Sorting and move-all are disabled while search or filters are active. Card drag and drop is available for visible cards.
        </p>
      ) : null}
      <p aria-live="polite" aria-atomic="true" className="sr-only">
        {filterActive ? `${visibleCount} of ${totalCount} matching cards` : ''}
      </p>

      <div className={cn(
        "flex min-h-0 flex-1 flex-col transition-opacity animate-in fade-in motion-safe:animate-in motion-safe:fade-in",
        prefersReducedMotion && "duration-0"
      )}>
        {isEmpty ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-16">
            <h2 className="text-lg font-semibold">No cards found</h2>
            <p className="text-sm text-muted-foreground">
              {isSearching && hasActiveFilters(filters)
                ? 'No cards match your search and filters. Try different terms or clear them.'
                : isSearching
                  ? 'No cards match your search. Try a different term or clear the search.'
                  : 'No cards match your filters. Try different filters or clear them.'}
            </p>
            <div className="flex gap-2">
              {isSearching && (
                <Button variant="secondary" onClick={handleClearSearch}>
                  Clear search
                </Button>
              )}
              {hasActiveFilters(filters) && (
                <Button variant="secondary" onClick={() => setFilters(DEFAULT_FILTER_STATE)}>
                  Clear filters
                </Button>
              )}
            </div>
          </div>
        ) : view === 'board' ? (
          isMobile ? (
            <div className="flex min-h-0 flex-1 overflow-hidden">
              <div className="flex h-full min-h-0 flex-1 overflow-x-auto overflow-y-hidden touch-pan-x" ref={boardScrollRef}>
                <DragDropContext boardId={id} filters={serverFilters}>
                  <div className="flex h-full min-w-max gap-4 p-4 pb-4">
                    {filteredColumns.map((column) => (
                      <Column
                        key={column.id}
                        column={column}
                        boardId={id}
                        nextColumnId={nextColumnIdById.get(column.id)}
                        moveTargets={moveTargets}
                        filterActive={filterActive}
                        filters={serverFilters}
                      />
                    ))}
                    <AddColumnButton onClick={handleAddColumn} />
                  </div>
                </DragDropContext>
              </div>
            </div>
          ) : (
            <div className="flex min-h-0 flex-1 overflow-hidden">
              <div className="flex h-full min-h-0 flex-1 overflow-x-auto overflow-y-hidden" ref={boardScrollRef}>
                <DragDropContext boardId={id} filters={serverFilters}>
                  <div className="flex h-full gap-6 p-6 pb-6">
                    {filteredColumns.map((column) => (
                      <Column
                        key={column.id}
                        column={column}
                        boardId={id}
                        nextColumnId={nextColumnIdById.get(column.id)}
                        moveTargets={moveTargets}
                        filterActive={filterActive}
                        filters={serverFilters}
                      />
                    ))}
                    <AddColumnButton onClick={handleAddColumn} />
                  </div>
                </DragDropContext>
              </div>
            </div>
          )
        ) : (
          <div ref={listScrollRef} className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <BoardListView boardId={id} filters={serverFilters} filterActive={filterActive} />
          </div>
        )}
      </div>
    </div>
    </PerfProfiler>
  );
}
