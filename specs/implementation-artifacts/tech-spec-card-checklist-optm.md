---
title: 'Card & Checklist Performance Optimization'
slug: 'card-checklist-performance-optimization'
created: '2026-06-15'
status: 'ready-for-dev'
steps_completed: ['party-mode-review']
tech_stack: ['React 19', 'NestJS 11', 'TypeORM 0.3', 'MySQL 8', 'TanStack Query v5', 'TypeScript']
files_to_modify:
  [
    'backend/src/columns/columns.service.ts',
    'backend/src/columns/columns.service.spec.ts',
    'backend/src/cards/cards.controller.ts',
    'backend/src/cards/cards.controller.spec.ts',
    'backend/src/cards/dto/card-response.dto.ts',
    'backend/src/columns/columns.controller.ts',
    'backend/src/columns/columns.controller.spec.ts',
    'frontend/src/features/cards/cards.api.ts',
    'frontend/src/features/cards/cards.api.test.ts',
    'frontend/src/features/cards/use-cards.ts',
    'frontend/src/features/cards/use-cards.test.tsx',
    'frontend/src/features/cards/card-detail-panel.tsx',
    'frontend/src/features/cards/card-detail-panel.test.tsx',
    'frontend/src/features/cards/card-preview.tsx',
    'frontend/src/features/cards/card-preview.test.tsx',
    'frontend/src/features/columns/columns.api.ts',
    'frontend/src/features/columns/columns.api.test.ts',
    'frontend/src/features/checklists/use-checklists.ts',
  ]
test_patterns:
  [
    'Backend: Vitest with manual repository mocks',
    'Frontend: Vitest + @testing-library/react with vi.mock for API/hook modules',
    'E2E: Playwright with UI interactions + page.reload() persistence verification',
  ]
---

# Tech-Spec: Card & Checklist Performance Optimization

**Created:** 2026-06-15

## Overview

### Problem Statement

The kanban board view (`GET /api/boards/:boardId/columns`) eagerly loads every card with ALL checklists and all checklist items from the database, even though the board view only needs card titles, labels, due dates, and checklist progress percentages. This causes:

1. **~80% of transferred data is unnecessary** — checklists and items are serialized into the response but never rendered on card tiles
2. **Cascading re-renders** — full card objects (with nested checklists) are passed 7 component levels deep through `BoardView → Column → ColumnCardList → Card → CardDetailPanel → ChecklistSection → Checklist → ChecklistItem`
3. **Cache invalidation bloat** — toggling a single checklist item invalidates ALL `['cards']` and `['columns']` queries, triggering a full board refetch
4. **Stale card detail data** — `CardDetailPanel` uses the card object passed as a prop (pre-loaded with column data) instead of fetching fresh data on open

### Acceptance Criteria

| Phase | Metric | Target |
|-------|--------|--------|
| All | Board column response size | **< 60% reduction** in payload bytes vs current |
| **Phase 1** | Board view load time | **< 200ms** p95 for boards with 50+ cards, measured on warm cache (subsequent navigations, not cold start) |
| **Phase 2** | Card detail panel time-to-content | **< 300ms** from click to rendered checklists |
| **Phase 3** | Checklist toggle cascade | **0 full-board refetches** — only single-card and columns queries affected |
| **Phase 4** | Card tile render cost | **No description DOM** in card tiles — confirmed by snapshot |
| **All** | Existing E2E pass rate | **100%** — all existing Playwright tests pass without modification |

### Solution

Four phases of targeted optimization:

1. **Backend: Sparse board-view query** — Strip `description`, `checklists[]`, `checklist_items[]` from the column endpoint response. Compute only `checklist_progress` on the server via batch aggregation.
2. **Frontend: Lazy-load card detail** — `CardDetailPanel` fetches `GET /api/cards/:id` when opened instead of relying on stale prop data.
3. **Frontend: Targeted cache invalidation** — All mutations invalidate granular `['card', cardId]` keys instead of broad `['cards']` + `['columns']`.
4. **Frontend: Remove description from card tiles** — Description is only useful in the detail panel, not on the board view.

### Files to create

- `frontend/src/features/cards/__tests__/` — *(optional, tests co-located with source)*

---

### Existing Architecture

#### Data Flow (current)

```
Browser                          Backend
  │                                │
  │  GET /api/boards/:id/columns   │
  │ ──────────────────────────────>│  columns.service.ts:35-45
  │                                │  - loads ALL card fields
  │                                │  - loads checklists + items for every card
  │  Response: Column[] with       │
  │  Card[] containing:            │
  │  - id, title, column_id,       │
  │    position, description,      │
  │    due_date, labels[],         │
  │    checklists[].items[]        │
  │ <──────────────────────────────│
  │                                │
  │  Card click → Side panel opens │
  │  Uses same card object from    │
  │  column response (stale)       │
```

#### Component Tree (relevant paths)

```
BoardView
  └─ useColumns(boardId) → fetches ALL data
      └─ Column
          └─ ColumnCardList
              └─ Card (receives full Card object with checklists)
                  ├─ CardPreview (renders title, desc, labels, due_date, progress)
                  └─ CardDetailPanel
                      └─ ChecklistSection
                          └─ Checklist
                              └─ ChecklistItem
```

---

### Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Keep TanStack Query** for server state — no Redux/Zustand needed | TanStack Query already handles caching, deduplication, and optimistic updates. Adding a client store for server state would add complexity without benefit since React Compiler auto-memoizes. |
| **Add `['card', cardId]` query key** for detail fetches | Enables granular invalidation — a checklist toggle refetches only that card's data, not every column |
| **Batch-aggregate progress** on backend | Single SQL query computes `completed/total/percent` for all cards in a column without loading individual items |
| **Keep react-compiler** and skip manual memoization | Already configured via `babel-plugin-react-compiler` v1.0.0 — handles auto-memoization at build time |
| **No virtual scrolling** for now | Profile first. Only needed if columns exceed ~100 cards per column |

---

## Implementation Plan

### Phase 1: Backend — Sparse board-view cards

**Context:** `columns.service.ts:35-45` currently loads full card data with all checklists/items:

```typescript
// CURRENT — columns.service.ts findAllByBoardId
return this.columnRepository.find({
  where: { board_id: boardId },
  relations: [
    'cards',
    'cards.cardLabels',
    'cards.cardLabels.label',
    'cards.checklists',
    'cards.checklists.items',
  ],
  order: { position: 'ASC' },
});
```

After this phase, the column endpoint returns cards with: `id, title, column_id, position, due_date, labels[], checklist_progress` — no `description`, no `checklists[]`, no `checklists.items[]`.

**Task 1.1** — Add `toCardSummaryResponse()` in `cards/dto/card-response.dto.ts`

```typescript
// Add to card-response.dto.ts
export interface CardSummaryResponse {
  id: number;
  title: string;
  column_id: number;
  position: number;
  due_date: string | null;
  created_at: string;
  updated_at: string;
  labels: { id: number; name: string; color: string }[];
  checklist_progress?: { completed: number; total: number; percent: number };
}

export function toCardSummaryResponse(card: Card): CardSummaryResponse {
  // Same as toCardResponse but WITHOUT description, WITHOUT checklists array
  // checklist_progress is pre-computed by enrichWithProgress() and attached
  // as a virtual property on the card entity — NOT computed here from card.checklists
  const progress = (card as any).checklist_progress as
    | { completed: number; total: number; percent: number }
    | undefined;

  return {
    id: card.id,
    title: card.title,
    column_id: card.column_id,
    position: card.position,
    due_date: card.due_date ? card.due_date.toISOString() : null,
    created_at: card.created_at.toISOString(),
    updated_at: card.updated_at.toISOString(),
    labels: card.cardLabels?.map((cl) => ({
      id: cl.label.id,
      name: cl.label.name,
      color: cl.label.color,
    })) || [],
    ...(progress ? { checklist_progress: progress } : {}),
  };
}
```

**Task 1.2** — Modify `columns.service.ts:findAllByBoardId` to use lighter query + mapping

Change the query to NOT load `cards.checklists` and `cards.checklists.items`:

```typescript
// NEW — columns.service.ts findAllByBoardId
async findAllByBoardId(boardId: number, userId: number): Promise<BoardColumn[]> {
  const board = await this.boardRepository.findOne({
    where: { id: boardId, user_id: userId },
  });
  if (!board) throw new NotFoundException('Board not found');

  const columns = await this.columnRepository.find({
    where: { board_id: boardId },
    relations: [
      'cards',
      'cards.cardLabels',
      'cards.cardLabels.label',
    ],
    order: { position: 'ASC' },
  });

  // Batch compute checklist progress for all cards
  await this.enrichWithProgress(columns);

  return columns;
}

private async enrichWithProgress(columns: BoardColumn[]): Promise<void> {
  // Collect all card IDs
  const cardIds = columns.flatMap((col) => col.cards.map((c) => c.id));
  if (cardIds.length === 0) return;

  // Single aggregated query: per-card completed/total counts
  // Use parameterized query to avoid SQL injection (cardIds are numeric PKs but
  // using parameterized placeholders is the correct pattern regardless)
  const placeholders = cardIds.map(() => '?').join(',');
  const progressRaw: { card_id: number; total: number; completed: number }[] =
    await this.cardRepository.query(
      `SELECT cl.card_id, COUNT(ci.id) AS total, COALESCE(SUM(ci.is_completed), 0) AS completed
       FROM checklists cl
       LEFT JOIN checklist_items ci ON ci.checklist_id = cl.id
       WHERE cl.card_id IN (${placeholders})
       GROUP BY cl.card_id`,
      cardIds,
    );

  const progressMap = new Map<number, { completed: number; total: number; percent: number }>();
  for (const row of progressRaw) {
    const total = Number(row.total);
    const completed = Number(row.completed);
    progressMap.set(row.card_id, {
      completed,
      total,
      percent: total === 0 ? 0 : Math.round((completed / total) * 100),
    });
  }

  // Attach progress metadata to each card as a virtual property
  // This is read by toCardSummaryResponse() — not computed from card.checklists
  for (const column of columns) {
    for (const card of column.cards) {
      (card as any).checklist_progress = progressMap.get(card.id);
    }
  }
}
```

> **Note on the raw query approach:** TypeORM's `QueryBuilder` with `.loadRelationCountAndMap()` would also work but generates subqueries per card. A single raw `GROUP BY` query is more efficient for batch loading. Parameterized placeholders (`?`) are used instead of string interpolation to prevent SQL injection. If `cardIds` is very large (1000+), batch in chunks of 500.

> **Note on `toCardResponse` vs `toCardSummaryResponse`:** `toCardResponse` remains unchanged and is still used by `cards.controller.ts:findOne` for the card detail endpoint (`GET /api/cards/:id`). That endpoint returns full card data including `description`, `checklists[]`, and `checklists.items[]`. Only the board view endpoints (`columns.controller.ts:findAll`, `columns.controller.ts:sort`) switch to `toCardSummaryResponse`.

> **Note on `sortCards` controller alignment:** The `sort` endpoint (`PATCH columns/:id/sort`) at `columns.controller.ts:142` also maps cards through `toCardResponse(c)`. It must be updated to use `toCardSummaryResponse` alongside `findAll`. The `sortCards` service method already uses relations `['cards', 'cards.cardLabels', 'cards.cardLabels.label']` (no checklists), so the service side requires no changes — only the controller mapping.

**Task 1.3** — Update `columns.controller.ts` to use `toCardSummaryResponse` for serialization

Modify the controller to map cards through `toCardSummaryResponse`:

```typescript
// columns.controller.ts — findAll
@Get()
async findAll(
  @Param('boardId', ParseIntPipe) boardId: number,
  @Session() session: SessionData,
) {
  const columns = await this.columnsService.findAllByBoardId(boardId, session.userId);
  const result = columns.map((col) => ({
    ...col,
    cards: col.cards.map((card) => toCardSummaryResponse(card)),
  }));
  return { data: result };
}
```

Apply the same change to the `sort` endpoint:

```typescript
// columns.controller.ts — sort (previously used toCardResponse(c))
@Patch('columns/:id/sort')
async sort(
  @Session() session: SessionData,
  @Param('id', ParseIntPipe) id: number,
  @Body() dto: SortCardsDto,
): Promise<{ data: ColumnResponse; message: string }> {
  const column = await this.columnsService.sortCards(id, session.userId, dto.order);
  const data: ColumnResponse = {
    id: column.id,
    name: column.name,
    position: column.position,
    board_id: column.board_id,
    cards: column.cards
      ? [...column.cards].sort((a, b) => a.position - b.position).map((c) => toCardSummaryResponse(c))
      : [],
    created_at: column.created_at.toISOString(),
    updated_at: column.updated_at.toISOString(),
  };
  return { data, message: 'Cards sorted' };
}
```

**Task 1.4** — Update `columns.api.ts` frontend `Card` interface to match sparse shape

```typescript
// frontend/src/features/columns/columns.api.ts
// Remove `description` from Card interface in column context
export interface Card {
  id: number;
  title: string;
  column_id: number;
  position: number;
  due_date: string | null;
  labels?: Label[];
  checklist_progress?: { completed: number; total: number; percent: number };
  created_at: string;
  updated_at: string;
}
```

> **Note:** `cards.api.ts` still has the full `Card` interface with `description` and `checklists` — that's the correct detail type used by the card detail panel.

---

### Phase 2: Frontend — Lazy-load card detail on panel open

**Context:** `CardDetailPanel` currently receives `card` as a prop from `Card` component. This card object is part of the column data from Phase 1 — it has no `description` or `checklists`. The panel should fetch its own data.

**Task 2.1** — Add `fetchCard(id)` to `cards.api.ts`

```typescript
// frontend/src/features/cards/cards.api.ts
export async function fetchCard(id: number): Promise<ApiResponse<Card>> {
  let response: Response;
  try {
    response = await fetch(`/api/cards/${id}`, FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}
```

**Task 2.2** — Add `useCard(id)` hook to `use-cards.ts`

```typescript
// frontend/src/features/cards/use-cards.ts
export function useCard(id: number) {
  return useQuery({
    queryKey: ['card', id],
    queryFn: () => fetchCard(id).then((res) => res.data),
    enabled: !!id,
  });
}
```

> **Type safety note:** The `enabled: !!id` guard prevents fetching with ID 0. If `id` could be `number | undefined`, use `enabled: id !== undefined && id > 0` instead.

Also update the query key factory:

```typescript
const cardKeys = {
  byColumn: (columnId: number) => ['cards', columnId] as const,
  byId: (cardId: number) => ['card', cardId] as const,
  all: () => ['cards'] as const,
};
```

**Task 2.3** — Update `CardDetailPanel` to fetch card by ID on open

Replace card prop usage with `useCard` hook:

```typescript
// frontend/src/features/cards/card-detail-panel.tsx
import { useCard } from './use-cards';

export function CardDetailPanel({ card, open, onOpenChange }: CardDetailPanelProps) {
  const { data: cardDetail, isLoading, isError, refetch } = useCard(open ? card.id : 0);
  // Use cardDetail (from API) instead of card (from props) for rendering
  // Show skeleton loading state when isLoading
  // Show error state with retry when isError

  // Critical: still use card prop for id, title fallback, column_id
  // Use cardDetail for full description, checklists, labels, due_date
  const displayCard = cardDetail ?? card;
  // ... rest of component uses displayCard
}
```

Key integration points:
- `open` prop controls whether the query fires (`enabled: open && !!card.id`)
- Title input still uses prop `card.title` for initial value (instant), but saves against `cardDetail` after load
- Description, labels, due date, checklists all use `cardDetail` once loaded
- `cardDetail` auto-refreshes via TanStack Query cache

Additional changes required in `CardDetailPanel`:

**2.3.1 — Use `displayCard` for `LabelPicker` and `SheetTitle`**

Replace raw prop references with `displayCard`:

```typescript
// Before:
<SheetTitle className="sr-only">Card details: {card.title}</SheetTitle>
<LabelPicker card={card} />

// After:
<SheetTitle className="sr-only">Card details: {displayCard.title}</SheetTitle>
<LabelPicker card={displayCard} />
```

This ensures that once `cardDetail` loads from the API, the label picker and accessibility title reflect fresh data.

**2.3.2 — Rework prop-sync `useEffect` to not depend on `card.description`**

The current sync effect (lines 55-60 in `card-detail-panel.tsx`) watches `card.description` which will be `undefined` after Phase 1. The effect should only sync title (which is still available on the prop), and skip description sync entirely since it will come from `cardDetail`:

```typescript
// Updated effect — removes card.description from deps
useEffect(() => {
  if (!isDirtyRef.current && !pendingSaveRef.current) {
    setTitle(card.title);
    // description is no longer synced from props — it comes from cardDetail via useCard
    // if cardDetail is loaded, its description will set this via a separate effect
  }
}, [card.title, card.id]);
```

Add a companion effect to populate description when `cardDetail` first loads:

```typescript
// When cardDetail loads from API, populate the description field
useEffect(() => {
  if (cardDetail && !isDirtyRef.current && !pendingSaveRef.current) {
    setDescription(cardDetail.description ?? '');
  }
}, [cardDetail?.description, cardDetail?.id]);
```

**Loading State UI Spec:**

When `isLoading` is true, render a skeleton placeholder matching the panel layout:
- Title: `<Skeleton className="h-8 w-3/4" />` (gray bar, 75% width)
- Description: 3x `<Skeleton className="h-4 w-full" />` (3 gray lines)
- Labels: `<Skeleton className="h-6 w-1/2" />` (small bar)
- Due Date: `<Skeleton className="h-6 w-1/3" />`
- Checklists: 2x `<Skeleton className="h-20 w-full" />` (checklist card placeholders)

When `isError`, render an inline error message with a retry button:
```tsx
<div className="p-4 text-center text-sm text-muted-foreground">
  <p>Failed to load card details.</p>
  <Button variant="outline" size="sm" onClick={() => refetch()} className="mt-2">
    Retry
  </Button>
</div>
```

---

### Phase 3: Frontend — Targeted cache invalidation

**Context:** Currently every mutation invalidates ALL `['cards']` and `['columns']` queries, causing full board refetches for even atomic operations like toggling a checklist item.

**Task 3.1** — Update `use-cards.ts` mutation invalidation

```typescript
// useUpdateCard — invalidate by card ID + board columns
export function useUpdateCard() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateCardData }) =>
      updateCard(id, data),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.id] });
      // Don't invalidate all ['cards'] — only the specific card
      // Still invalidate columns so board view shows updated title/position
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

// useMoveCard / useReorderCard
// onSettled: invalidate ['card', cardId] + ['columns']
// Do NOT invalidate all ['cards']

// useDeleteCard
// onSettled: invalidate ['columns'] only (card no longer exists)
// Do NOT invalidate all ['cards']

// useAssignCardLabel / useRemoveCardLabel
// onSuccess: invalidate ['card', cardId] + ['columns']
// Do NOT invalidate all ['cards']
```

**Task 3.2** — Update `use-checklists.ts` mutation invalidation

Every checklist mutation needs to know the `cardId`. Currently hooks don't receive it explicitly.

```typescript
// useCreateChecklist — receives cardId from API path
export function useCreateChecklist() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: CreateChecklistData) => createChecklist(data),
    onSuccess: (_data, variables) => {
      // variables contains card_id
      queryClient.invalidateQueries({ queryKey: ['card', variables.card_id] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

// useUpdateChecklistItem — needs cardId context
// Approach: accept cardId as part of mutationFn params
// NOTE: The existing code has an optimistic update that walks ['cards'] query data.
// This optimistic update must be refactored to target ['card', cardId] instead.
// Remove or update the onMutate to operate on the card-specific query key.
export function useUpdateChecklistItem() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ itemId, data, cardId }: { itemId: number; data: UpdateChecklistItemData; cardId: number }) =>
      updateChecklistItem(itemId, data),
    onMutate: async ({ itemId, data, cardId }) => {
      await queryClient.cancelQueries({ queryKey: ['card', cardId] });

      const previousCardData = queryClient.getQueryData(['card', cardId]);

      queryClient.setQueryData(['card', cardId], (old: unknown) => {
        if (!old) return old;
        const card = (old as { checklists?: Array<Checklist & { items: ChecklistItem[] }> });
        return {
          ...card,
          checklists: card.checklists?.map((cl) => ({
            ...cl,
            items: cl.items.map((item) =>
              item.id === itemId ? { ...item, ...data } : item
            ),
          })),
        };
      });

      return { previousCardData };
    },
    onError: (_error: Error, _variables, context) => {
      if (context?.previousCardData) {
        queryClient.setQueryData(['card', _variables.cardId], context.previousCardData);
      }
    },
    onSettled: (_data, _err, variables) => {
      queryClient.invalidateQueries({ queryKey: ['card', variables.cardId] });
      queryClient.invalidateQueries({ queryKey: ['columns'] });
    },
  });
}

// Same pattern for useCreateChecklistItem, useDeleteChecklistItem
// All need cardId passed through and invalidate ['card', cardId] + ['columns']
```

This requires updating the components that call these hooks to pass `cardId`. The prop interface changes are:

In addition, the **`ChecklistItem` undo function** (in `checklist-item.tsx:83-84`) has a hardcoded broad invalidation that must be updated:

```typescript
// Before (checklist-item.tsx — undo handler):
queryClient.invalidateQueries({ queryKey: ['cards'] });
queryClient.invalidateQueries({ queryKey: ['columns'] });

// After:
queryClient.invalidateQueries({ queryKey: ['card', cardId] });
queryClient.invalidateQueries({ queryKey: ['columns'] });
```

This requires the `ChecklistItem` component to receive `cardId` as a prop (see prop interface below).

```typescript
// ChecklistSection — new prop
interface ChecklistSectionProps {
  cardId: number;
  // ... existing props
}

// Checklist — new prop
interface ChecklistProps {
  cardId: number;
  // ... existing props
}

// ChecklistItem — new prop
interface ChecklistItemProps {
  cardId: number;
  // ... existing props
}

// AddChecklistForm — new prop
interface AddChecklistFormProps {
  cardId: number;
  // ... existing props
}

// AddChecklistItemForm — new props
interface AddChecklistItemFormProps {
  cardId: number;
  checklistId: number;
  // ... existing props
}
```

These props flow from `CardDetailPanel` (which receives `card.id` from the column data) down to each checklist subcomponent. No prop-drilling library needed — max depth is 4 levels.

---

### Phase 4: Frontend — Remove description from card tiles

**Grep-verify precondition:** Before making any changes, run:
```
grep -rn 'card\.description' frontend/src/features/ --include='*.tsx' --include='*.ts'
```
to confirm that `CardPreview` is the **only** consumer of `card.description` from the column-context `Card` type. If any other component references it, update that component first or keep the field in the interface.

**Task 4.1** — Remove description rendering from `CardPreview`

```typescript
// frontend/src/features/cards/card-preview.tsx
// Delete the following block (lines 26-33):
{card.description?.trim() && (
  <p className="mt-1 ..." style={{ display: '-webkit-box', ... }}>
    {card.description}
  </p>
)}
```

`CardPreview` now only shows: title, labels, due date badge, checklist progress bar.

---

## Task List / Subtasks

### Phase 1: Backend — Sparse board-view cards

- [ ] **1.1** Add `toCardSummaryResponse()` DTO + function in `backend/src/cards/dto/card-response.dto.ts`
- [ ] **1.2** Modify `columns.service.ts:findAllByBoardId` — remove checklist relations, add `enrichWithProgress` batch aggregation
- [ ] **1.3** Update `columns.controller.ts:findAll` — map cards through `toCardSummaryResponse`
- [ ] **1.3.5** Update `columns.controller.ts:sort` — also map cards through `toCardSummaryResponse` (currently uses `toCardResponse`)
- [ ] **1.4** Update `frontend/src/features/columns/columns.api.ts` — remove `description` from column-context `Card` type
- [ ] **1.5** **Test:** Update `columns.service.spec.ts` — update `findAllByBoardId` relations assertion; add test for progress aggregation
- [ ] **1.6** **Test:** Update `columns.controller.spec.ts` — mock response shape to use sparse card format
- [ ] **1.7** **Test:** Update `columns.api.test.ts` — remove `description` from mock card data
- [ ] **1.8** **Entity dependency audit** — Verify no other service method (e.g., `cards.service`, `search.service`) depends on `cards.checklists` or `cards.checklists.items` being loaded as part of column-level relations

### Phase 2: Frontend — Lazy-load card detail on panel open

- [ ] **2.1** Add `fetchCard(id)` to `frontend/src/features/cards/cards.api.ts`
- [ ] **2.2** Add `useCard(id)` hook with `['card', cardId]` query key to `use-cards.ts`
- [ ] **2.3** Update `CardDetailPanel` — call `useCard(id)` on panel open; use `cardDetail` for description, checklists, labels; fallback to prop `card` while loading
- [ ] **2.3.1** Update `LabelPicker` and `SheetTitle` to use `displayCard` instead of raw prop `card`
- [ ] **2.3.2** Rework prop-sync `useEffect` — remove `card.description` from deps, add companion effect that syncs description when `cardDetail` loads
- [ ] **2.4** Update `CardDetailPanel` — add skeleton loading state and error state with retry button while card detail is being fetched (see Loading State UI Spec above)
- [ ] **2.5** **Test:** Add `fetchCard` test in `cards.api.test.ts` — success + error + network error
- [ ] **2.6** **Test:** Add `useCard` hook test in `use-cards.test.tsx` — fetches on mount, returns data
- [ ] **2.7** **Test:** Update `card-detail-panel.test.tsx` — mock `useCard` hook instead of prop data; test loading state, fetch success, fetch error

### Phase 3: Frontend — Targeted cache invalidation

- [ ] **3.1** Update `use-cards.ts` — all mutations invalidate `['card', cardId]` + `['columns']` instead of broad `['cards']`
- [ ] **3.2** Update `use-checklists.ts` — all mutations accept `cardId` param, invalidate `['card', cardId]` + `['columns']` instead of `['cards']`
- [ ] **3.2.1** Refactor `useUpdateChecklistItem` optimistic update — target `['card', cardId]` instead of broad `['cards']`
- [ ] **3.2.2** Fix `ChecklistItem` undo handler — replace broad `['cards']` invalidation with `['card', cardId]`
- [ ] **3.3** Update `ChecklistSection`, `Checklist`, `ChecklistItem`, `AddChecklistForm`, `AddChecklistItemForm` to pass `cardId` through to hooks
- [ ] **3.4** **Test:** Update `use-cards.test.tsx` — verify invalidation targets `['card', id]` not `['cards']`
- [ ] **3.5** **Test:** Update `use-checklists` test (create if none exists) — verify invalidation targets `['card', cardId]`

### Phase 4: Frontend — Remove description from card tiles

- [ ] **4.1** Remove description `<p>` block from `CardPreview`
- [ ] **4.2** **Test:** Update `card-preview.test.tsx` — remove assertions that check for description rendering

### Acceptance Criteria Verification

- [ ] **AC.1** Measure board column response size — confirm **< 60% reduction** in payload bytes
- [ ] **AC.2** Profile board view p95 load time — confirm **< 200ms** for 50+ card boards
- [ ] **AC.3** Measure card detail panel time-to-content — confirm **< 300ms** from click to checklists
- [ ] **AC.4** Verify checklist toggle triggers **0 full-board refetches** (check Network tab)
- [ ] **AC.5** Snapshot card tiles — confirm **no description DOM** renders on the board view

### E2E Verification

- [ ] **E.1** Run full e2e suite — verify checklists, drag-drop, due-dates, columns, labels all pass
- [ ] **E.2** **Manual check:** Verify card detail panel opens quickly with checklist data
- [ ] **E.3** **Manual check:** Verify board view loads noticeably faster (check DevTools Network tab)

---

## Test Strategy

### Backend Tests

| Existing test | What must change |
|---------------|------------------|
| `columns.service.spec.ts:104-132` `findAllByBoardId` | `relations` assertion must NOT include `cards.checklists`, `cards.checklists.items`. Add assertion that `enrichWithProgress` was called or that cards have `checklist_progress`. |
| `columns.service.spec.ts` `findAllByBoardId: board not found` | Should still pass unchanged |
| `columns.controller.spec.ts:70-88` `findAll` | Mock response should use sparse card shape (no `description`) |
| `cards.service.spec.ts:findById` | Should still pass — this endpoint still returns full data |
| `cards.controller.spec.ts:findOne` | Should still pass — detail endpoint unchanged |

**New backend tests:**

```typescript
// columns.service.spec.ts — add test for enrichWithProgress
it('should attach checklist_progress to cards via batch aggregation', async () => {
  // Mock query() to return progress rows
  mockCardRepository.query.mockResolvedValueOnce([
    { card_id: 1, total: 3, completed: 1 },
  ]);
  // ... verify result cards have checklist_progress
});

it('should handle cards with zero checklists gracefully', async () => {
  // Cards that have no checklists at all — query returns no rows for them
  mockCardRepository.query.mockResolvedValueOnce([]);
  // ... verify progressMap.get(card.id) is undefined / cards have no checklist_progress
});

it('should handle cards with empty checklists (no items)', async () => {
  // Checklists exist but have zero items
  mockCardRepository.query.mockResolvedValueOnce([
    { card_id: 1, total: 0, completed: 0 },
  ]);
  // ... verify checklist_progress = { completed: 0, total: 0, percent: 0 }
});

it('should handle cards with all items completed', async () => {
  mockCardRepository.query.mockResolvedValueOnce([
    { card_id: 1, total: 5, completed: 5 },
  ]);
  // ... verify checklist_progress = { completed: 5, total: 5, percent: 100 }
});
```

### Frontend Tests

| Existing test | What must change |
|---------------|------------------|
| `cards.api.test.ts` | Add `fetchCard` test block |
| `use-cards.test.tsx` | Add `useCard` test; update invalidation assertions |
| `card-detail-panel.test.tsx` | Major rework — mock `useCard`; test loading/error states |
| `card-preview.test.tsx` | Remove description assertions |
| `columns.api.test.ts` | Remove `description` from mock card data |

### E2E Tests

All existing e2e tests should pass without modification:

| Test file | Confidence |
|-----------|------------|
| `checklists.spec.ts` | High — card detail API unchanged (`GET /api/cards/:id`), panel now fetches via this endpoint instead of props; mock server handles both patterns |
| `drag-drop.spec.ts` | High — drag uses same data shape |
| `columns.spec.ts` | High — cards still display on board |
| `due-dates.spec.ts` | High — due dates still show on tiles |

---

## Rollback Plan

If any phase causes issues:

1. **Phase 1 issues** (board view broken): Revert `columns.service.ts` and `columns.controller.ts` to original. The board view will work as before but without progress bars until Phase 2 loads them via card detail.
2. **Phase 2 issues** (panel not loading): Revert `CardDetailPanel` to use prop-based card data. The panel will show the (now sparse) column card data without description/checklists.
3. **Phase 3 issues** (stale data): Revert invalidation to broad `['cards']` + `['columns']`. More refetches but correct data.
4. **Phase 4 issues** (description missing): Re-add description block to `CardPreview`.

---

## References

- [Source: 4-5-checklists-with-progress.md] — Existing checklists feature, response DTOs, component structure
- [Source: AGENTS.md — Backend E2E Test Pattern] — Must verify DB persistence for all data modification endpoints
- [Source: frontend/src/features/cards/card-detail-panel.tsx] — Target file for Phase 2 changes
- [Source: frontend/src/features/columns/columns.api.ts] — Card interface for column context
- [Source: frontend/src/features/cards/cards.api.ts] — Full Card interface for detail context
- [Source: backend/src/columns/columns.service.ts:35-45] — Current eager loading query
- [Source: backend/src/cards/dto/card-response.dto.ts] — `toCardResponse` and `toCardDetailResponse`

---

## Pre-Implementation Readiness Checklist

These items must be confirmed **before** development begins:

- [ ] **R1. Acceptance criteria verified** — Measurable targets accepted by PM/stakeholders
- [ ] **R2. Entity dependency audit complete** — No other service depends on `cards.checklists` being loaded via column relations
- [ ] **R3. Grep-verify complete** — No component besides `CardPreview` references `card.description` from column-context type
- [ ] **R4. Prop interface changes documented** — All 5 checklist components have `cardId` prop defined (see Phase 3)
- [ ] **R5. Security review** — `enrichWithProgress` uses parameterized queries, not string interpolation
- [ ] **R6. Rollback plan acknowledged** — Per-phase revert strategy confirmed with team
- [ ] **R7. Test file existence** — Verify `frontend/src/features/checklists/use-checklists.test.ts` exists (or create it) before Phase 3.5

---

## Dev Agent Record

### Implementation Log

*To be filled after implementation.*

### File List

*To be filled after implementation.*

Backend:
- `backend/src/columns/columns.service.ts` — modified
- `backend/src/columns/columns.service.spec.ts` — modified
- `backend/src/columns/columns.controller.ts` — modified
- `backend/src/columns/columns.controller.spec.ts` — modified
- `backend/src/cards/dto/card-response.dto.ts` — modified
- `backend/src/cards/cards.controller.ts` — verified (unchanged)

Frontend:
- `frontend/src/features/cards/cards.api.ts` — modified
- `frontend/src/features/cards/cards.api.test.ts` — modified
- `frontend/src/features/cards/use-cards.ts` — modified
- `frontend/src/features/cards/use-cards.test.tsx` — modified
- `frontend/src/features/cards/card-detail-panel.tsx` — modified
- `frontend/src/features/cards/card-detail-panel.test.tsx` — modified
- `frontend/src/features/cards/card-preview.tsx` — modified
- `frontend/src/features/cards/card-preview.test.tsx` — modified
- `frontend/src/features/columns/columns.api.ts` — modified
- `frontend/src/features/columns/columns.api.test.ts` — modified
- `frontend/src/features/checklists/use-checklists.ts` — modified
