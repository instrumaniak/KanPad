# Story 2.4: Column Sorting & Bulk Move

Status: done

## Story

As a user,
I want to sort cards within columns and move all cards between columns,
so that I can efficiently organize and reorganize my workflow.

## Acceptance Criteria

1. **Given** I am viewing a column, **When** I click the column menu and select "Sort by Date", **Then** I can choose ascending or descending order, **And** cards reorder immediately based on creation date
2. **Given** I want to move all cards from one column to another, **When** I click "Move All Cards" in the column menu, **Then** I see a dropdown of other columns in the board, **And** selecting a target column moves all cards to that column, **And** a toast confirms "X cards moved"

## Tasks / Subtasks

- [x] Task 1: Backend API - Add card sorting endpoint (AC: #1)
  - [x] Subtask 1.1: Add `PATCH /api/columns/:id/sort` endpoint with `{ order: 'asc' | 'desc' }` payload
  - [x] Subtask 1.2: Implement sorting by `created_at` ascending or descending
  - [x] Subtask 1.3: Update Card entity if needed to ensure `created_at` exists
  - [x] Subtask 1.4: Add authorization check - user must own the board
- [x] Task 2: Backend API - Add bulk move cards endpoint (AC: #2)
  - [x] Subtask 2.1: Add `POST /api/columns/:id/move-all` endpoint with `{ targetColumnId: number }` payload
  - [x] Subtask 2.2: Move all cards from source column to target column
  - [x] Subtask 2.3: Add authorization check - user must own both source and target columns' board
  - [x] Subtask 2.4: Return count of moved cards in response
- [x] Task 3: Frontend - Add column menu items (AC: #1, #2)
  - [x] Subtask 3.1: Update column header menu to include "Sort by Date" option
  - [x] Subtask 3.2: Add sort submenu with "Ascending (Oldest first)" and "Descending (Newest first)" options
  - [x] Subtask 3.3: Add "Move All Cards" option to column menu
- [x] Task 4: Frontend - Sort UI implementation (AC: #1)
  - [x] Subtask 4.1: Create API function `sortCards(columnId, order)` in `columns.api.ts`
  - [x] Subtask 4.2: Add React Query mutation hook in `use-columns.ts`
  - [x] Subtask 4.3: Connect sort menu items to API call
  - [x] Subtask 4.4: Optimistically update card order in UI after successful sort
- [x] Task 5: Frontend - Bulk move UI implementation (AC: #2)
  - [x] Subtask 5.1: Create API function `moveAllCards(sourceColumnId, targetColumnId)` in `columns.api.ts`
  - [x] Subtask 5.2: Add React Query mutation hook in `use-columns.ts`
  - [x] Subtask 5.3: Create dropdown showing other columns in the board (exclude current column)
  - [x] Subtask 5.4: Show toast with "X cards moved" on successful move
- [x] Task 6: Tests (AC: #1, #2)
  - [x] Subtask 6.1: Backend unit tests for sort and bulk move endpoints
  - [x] Subtask 6.2: Frontend tests for sort menu and bulk move dropdown

## Dev Notes

### Database Schema

**Card entity** (already exists from Story 2.3):
```
Card {
  id: number (PK, auto-increment)
  title: string (NOT NULL)
  column_id: number (FK to columns.id, NOT NULL)
  position: number (NOT NULL, default 0) — order within column
  created_at: Date
  updated_at: Date
}
```

**Required:** `created_at` field must exist on Card entity for sorting by date. Verify in `backend/src/cards/entities/card.entity.ts`.

### API Endpoints

**New endpoints for Story 2.4:**

```
PATCH /api/columns/:id/sort      — Sort cards by creation date
POST  /api/columns/:id/move-all — Move all cards to another column
```

**Request/Response examples:**

```typescript
// PATCH /api/columns/:id/sort (200)
Request:  { order: "asc" }  // or "desc"
Response: {
  data: {
    id: 1,
    name: "To Do",
    cards: [ /* cards sorted by created_at ASC */ ]
  },
  message: "Cards sorted"
}

// POST /api/columns/:id/move-all (200)
Request:  { targetColumnId: 3 }
Response: {
  data: {
    movedCount: 5
  },
  message: "5 cards moved to In Progress"
}
```

**Authorization:**
- Both endpoints require authentication
- User must own the board containing the column(s)
- For move-all: user must own both source and target columns' boards

### Frontend API Functions

Add to `frontend/src/features/columns/columns.api.ts`:

```typescript
export const sortCards = async (columnId: number, order: 'asc' | 'desc') => {
  const response = await api.patch(`/api/columns/${columnId}/sort`, { order });
  return response.data;
};

export const moveAllCards = async (sourceColumnId: number, targetColumnId: number) => {
  const response = await api.post(`/api/columns/${sourceColumnId}/move-all`, { targetColumnId });
  return response.data;
};
```

Add to `frontend/src/features/columns/use-columns.ts`:

```typescript
export const useSortCards = () => {
  return useMutation(({ columnId, order }) => sortCards(columnId, order));
};

export const useMoveAllCards = () => {
  return useMutation(({ sourceColumnId, targetColumnId }) => moveAllCards(sourceColumnId, targetColumnId));
};
```

### Column Menu Enhancement

Update column header to include new menu items. Current menu has "Rename" and "Delete". New structure:

```
Column Menu:
├── Rename
├── Sort by Date
│   ├── Ascending (Oldest first)
│   └── Descending (Newest first)
├── Move All Cards →
│   ├── To Do
│   ├── In Progress
│   └── Done
└── Delete
```

**UI Pattern:** Use DropdownMenu with nested submenus for sort options and a separate dropdown for move target selection.

### Previous Story Intelligence

**From Story 2.3 (Column CRUD):**
- Column entity includes `position` field for ordering
- Card entity already exists with `created_at` field
- Column menu component already implemented at `frontend/src/features/columns/column-header.tsx`
- DTO pattern established at `backend/src/columns/dto/`

**From Story 2.2 (Board Archiving):**
- Board entity exists, used for authorization checks

**From Story 2.1 (Board CRUD):**
- Authorization pattern: `board.user_id === session.userId`

### Architecture Compliance

**Naming conventions:**
- DB: `snake_case` (e.g., `created_at`, `column_id`)
- API: `kebab-case` endpoints (e.g., `/api/columns/:id/sort`)
- Code: `camelCase` for fields/properties, `PascalCase` for types/components
- Files: `kebab-case` (e.g., `columns.api.ts`, `use-columns.ts`)

**Module structure:**
```
backend/src/columns/
├── columns.module.ts
├── columns.controller.ts      # Add sort and move-all endpoints
├── columns.service.ts        # Add sortCards and moveAllCards methods
├── columns.service.spec.ts   # Add tests
├── columns.controller.spec.ts
├── entities/
│   └── column.entity.ts
└── dto/
    ├── sort-cards.dto.ts      # New: { order: 'asc' | 'desc' }
    └── move-cards.dto.ts      # New: { targetColumnId: number }
```

```
frontend/src/features/columns/
├── column.tsx                 # Update to handle sort/move menu
├── column-header.tsx         # Update menu with new options
├── columns.api.ts            # Add sortCards, moveAllCards
├── use-columns.ts            # Add useSortCards, useMoveAllCards
└── column.test.tsx           # Add tests
```

### Critical Anti-Patterns

- ❌ NEVER allow sorting without authorization - verify board ownership
- ❌ NEVER allow moving cards to the same column - no-op, show toast "Cards already in this column"
- ❌ NEVER allow moving cards to column in different board - authorization check required
- ❌ NEVER skip toast notification - user needs confirmation of bulk action
- ❌ NEVER use client-side sort only - must persist to backend
- ❌ NEVER forget to include moved card count in toast message

### UX Requirements

**Sort by Date:**
- Click column menu → "Sort by Date" → submenu with Ascending/Descending
- Sort happens immediately on selection
- Toast confirms "Cards sorted" (no undo needed for sort)

**Move All Cards:**
- Click column menu → "Move All Cards" → dropdown of other columns in board
- Selecting target column moves all cards
- Toast shows count: "5 cards moved to In Progress"
- Source column shows empty state after move

**Edge cases:**
- Source column empty: show toast "No cards to move"
- Target column same as source: show toast "Cards already in this column"
- Single column in board: disable "Move All Cards" (no other columns to move to)

### Testing Standards

**Backend (Jest):**
- Test: sort ascending - verify cards ordered by created_at ASC
- Test: sort descending - verify cards ordered by created_at DESC
- Test: bulk move - verify all cards moved to target column
- Test: bulk move count - verify returned count matches actual moved cards
- Test: authorization - verify user cannot sort/move another user's columns
- Test: error handling - 404 on invalid column, 400 on invalid order

**Frontend (Vitest + React Testing Library):**
- Test: sort menu shows ascending and descending options
- Test: clicking sort option calls API with correct order
- Test: move all shows dropdown with other columns
- Test: selecting target column calls API with correct IDs
- Test: toast shows correct card count after move

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-2.4-Column-Sorting-Bulk-Move]
- [Source: _bmad-output/planning-artifacts/architecture.md#API-Response-Format]
- [Source: _bmad-output/planning-artifacts/architecture.md#Naming-Patterns]
- [Source: _bmad-output/planning-artifacts/architecture.md#Data-Boundaries]
- [Source: _bmad-output/implementation-artifacts/2-3-column-crud.md]
- [File: backend/src/cards/entities/card.entity.ts]
- [File: backend/src/columns/columns.controller.ts]
- [File: backend/src/columns/columns.service.ts]
- [File: frontend/src/features/columns/column-header.tsx]
- [File: frontend/src/features/columns/columns.api.ts]
- [File: frontend/src/features/columns/use-columns.ts]

## Dev Agent Record

### Agent Model Used

minimax-m2.5-free

### Debug Log References

### Completion Notes List

### File List

**Backend (New):**
- backend/src/columns/dto/sort-cards.dto.ts
- backend/src/columns/dto/move-cards.dto.ts

**Backend (Modified):**
- backend/src/columns/columns.controller.ts (add sort and move-all endpoints)
- backend/src/columns/columns.service.ts (add sortCards and moveAllCards methods)
- backend/src/cards/entities/card.entity.ts (verify created_at exists)

**Frontend (Modified):**
- frontend/src/features/columns/column-header.tsx (add sort and move-all menu items)
- frontend/src/features/columns/columns.api.ts (add API functions)
- frontend/src/features/columns/use-columns.ts (add mutation hooks)

**Tests (New/Modified):**
- backend/src/columns/columns.service.spec.ts (add sort and move tests)
- backend/src/columns/columns.controller.spec.ts (add endpoint tests)

## Change Log

- 2026-04-30: Story 2.4 context created - Column Sorting & Bulk Move with sort by date and move all cards functionality.