import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useBoard, useUpdateBoard, type BoardViewMode } from '../use-boards';
import { useColumns, useCreateColumn } from '../../columns/use-columns';
import { Column } from '../../columns/column';
import { AddColumnButton } from '../../columns/add-column-button';
import { useToastHelpers } from '@/lib/toast-helpers';
import { DragDropContext } from '../../cards/drag-drop-context';
import { BoardViewToggle } from './board-view-toggle';
import { BoardListView } from './board-list-view';
import { CardSearchInput } from './card-search-input';
import { filterColumnsByTitle } from './filter-cards-by-title';
import { FilterDropdown } from './filter-dropdown';
import { FilterChips } from './filter-chips';
import { useLabels } from '../../labels/use-labels';
import { DEFAULT_FILTER_STATE, type FilterState, filterColumnsByFilters } from './filter-columns-by-filters';
import { useBreakpoint } from '@/hooks/use-breakpoint';
import { usePrefersReducedMotion } from '@/hooks/use-prefers-reduced-motion';
import { cn } from '@/lib/utils';

export function BoardView() {
  const { boardId } = useParams<{ boardId: string }>();
  const navigate = useNavigate();
  const id = boardId ? parseInt(boardId, 10) : 0;

  const { data: boardResponse, isLoading: boardLoading } = useBoard(id);
  const { data: columns, isLoading: columnsLoading } = useColumns(id);
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
  const hasActiveFilters = filters.labels.length > 0 || filters.dueDate !== null || filters.checklist !== 'All';
  // 5-2: compose filterColumnsByFilters here (single derivation point).
  const filteredColumns = useMemo(() => {
    let result = columns ?? [];
    if (isSearching) result = filterColumnsByTitle(result, activeQuery);
    if (hasActiveFilters) result = filterColumnsByFilters(result, filters);
    return result;
  }, [columns, activeQuery, isSearching, hasActiveFilters, filters]);

  const visibleCount = filteredColumns.flatMap((c) => c.cards ?? []).length;
  const totalCount = (columns ?? []).flatMap((c) => c.cards ?? []).length;
  const isFilteredEmpty =
    !boardLoading && !columnsLoading && !!board && hasActiveFilters && filteredColumns.every((c) => (c.cards ?? []).length === 0);
  const isSearchEmpty =
    !boardLoading && !columnsLoading && !!board && isSearching && filteredColumns.every((c) => (c.cards ?? []).length === 0);
  const isEmpty = isSearchEmpty || isFilteredEmpty;

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

  if (boardLoading || columnsLoading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
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
    <div className="flex flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 px-4 py-4 sm:gap-4 sm:px-6">
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
            availableLabels={allLabels ?? []}
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
      />
      <p aria-live="polite" className="sr-only">
        {isSearching || hasActiveFilters ? `${visibleCount} of ${totalCount} cards` : ''}
      </p>

      <div className={cn(
        "transition-opacity animate-in fade-in motion-safe:animate-in motion-safe:fade-in",
        prefersReducedMotion && "duration-0"
      )}>
        {isEmpty ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-16">
            <h2 className="text-lg font-semibold">No cards found</h2>
            <p className="text-sm text-muted-foreground">
              {isSearchEmpty
                ? 'No cards match your search. Try a different term or clear the search.'
                : 'No cards match your filters. Try different filters or clear them.'}
            </p>
            <div className="flex gap-2">
              {isSearchEmpty && (
                <Button variant="secondary" onClick={handleClearSearch}>
                  Clear search
                </Button>
              )}
              {isFilteredEmpty && (
                <Button variant="secondary" onClick={() => setFilters(DEFAULT_FILTER_STATE)}>
                  Clear filters
                </Button>
              )}
            </div>
          </div>
        ) : view === 'board' ? (
          isMobile ? (
            <div className="flex flex-1 overflow-hidden">
              <div className="flex-1 overflow-x-auto touch-pan-x" ref={boardScrollRef}>
                <DragDropContext boardId={id}>
                  <div className="flex h-full min-w-max gap-4 p-4 pb-4">
                    {filteredColumns.map((column) => (
                      <Column key={column.id} column={column} allColumns={columns ?? []} />
                    ))}
                    <AddColumnButton onClick={handleAddColumn} />
                  </div>
                </DragDropContext>
              </div>
            </div>
          ) : (
            <div className="flex flex-1 overflow-hidden">
              <div className="flex-1 overflow-x-auto" ref={boardScrollRef}>
                <DragDropContext boardId={id}>
                  <div className="flex h-full gap-6 p-6 pb-6">
                    {filteredColumns.map((column) => (
                      <Column key={column.id} column={column} allColumns={columns ?? []} />
                    ))}
                    <AddColumnButton onClick={handleAddColumn} />
                  </div>
                </DragDropContext>
              </div>
            </div>
          )
        ) : (
          <div ref={listScrollRef} className="flex flex-1 flex-col overflow-hidden">
            <BoardListView boardId={id} columns={filteredColumns} />
          </div>
        )}
      </div>
    </div>
  );
}
