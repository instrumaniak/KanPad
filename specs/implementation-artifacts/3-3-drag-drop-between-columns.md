# Story 3.3: Drag-Drop Between Columns

Status: review

---

## Story

**Epic 3: Task Capture & Card Management**

As a user,
I want to drag cards between columns,
So that I can move tasks through my workflow stages.

## Acceptance Criteria

1. **Given** I am viewing a board with cards, **When** I hover over a card, **Then** the cursor changes to grab hand, **And** a subtle lift shadow appears (2px elevation, 150ms transition)

2. **Given** I click and drag a card, **When** the drag starts (after 5px movement), **Then** the card lifts with elevated shadow (shadow-lg), **And** a ghost placeholder remains in the original position (opacity: 0.5), **And** the card follows the cursor smoothly (60fps)

3. **Given** I drag toward another column, **When** hovering over a column, **Then** the drop zone highlights with a border glow (border-teal-500/50), **And** an insertion point line shows where the card will land (2px, teal-500/60), **And** neighboring cards shift to make room (200ms translateY)

4. **Given** I release the card in a valid drop zone, **When** the drop completes, **Then** the card settles with a subtle bounce animation (300ms spring), **And** if moved to a "Done" column, a brief checkmark animation plays (400ms scale)

5. **Given** I drag to an invalid zone, **When** I release, **Then** the card snaps back to its original position with a slight shake (400ms, 3 oscillations, ±4px)

6. **Given** I press Escape during drag, **When** the drag is cancelled, **Then** the card returns to its original position (250ms spring) - no API call

---

## Developer Context Section

### ⚠️ CRITICAL IMPLEMENTATION REQUIREMENTS

This story delivers the core kanban drag-drop interaction. The user expects smooth, intuitive card movement between workflow stages.

**Business Value:** Users move tasks through their workflow by physically dragging cards between columns - the defining Kanban interaction.

### Dependencies

- **Epic 3, Story 3.1 (Card Creation)** - Card component and data structure exist
- **Epic 3, Story 3.2 (Card Editing)** - Card component has inline edit capability
- **Epic 2, Story 2.3 (Column CRUD)** - Column entity and API exist
- **Epic 2, Story 2.5 (Board View Layout)** - Board renders columns horizontally

### ⚠️ CRITICAL IMPLEMENTATION BLOCKERS (Resolved via Party Mode Review)

#### Blocker 1: Drag-vs-Click Conflict (RESOLVED)
**Location:** `frontend/src/features/cards/card.tsx` lines 31-36
**Issue:** The card component has `onClick` for inline edit mode. Adding dnd-kit's `listeners` (drag handler) will conflict - drag may trigger edit mode.
**Resolution:** Wrap card with `card-draggable.tsx` component that uses `useDraggable` with proper activation constraints. Use dnd-kit's `activationConstraint` to require minimum drag distance (e.g., 5px) before initiating drag, preventing accidental drags from clicks.

#### Blocker 2: API Route Consolidation (RESOLVED)
**Decision:** Reuse existing `PATCH /:id` endpoint with `columnId` in body instead of creating new `/move` route. This follows Story 3.2's pattern and reduces route proliferation.
**Implementation:** Add `columnId` field to the existing update DTO. The service method will detect if `columnId` changed and handle as a move operation.

#### Blocker 3: Missing Test Coverage (RESOLVED)
**Addition:** Add integration test for drag-vs-edit conflict scenario.

### Pre-Implementation Checklist

- [ ] Review card component at `frontend/src/features/cards/card.tsx`
- [ ] Review column component at `frontend/src/features/columns/column.tsx`
- [ ] Review columns card list at `frontend/src/features/columns/column-card-list.tsx`
- [ ] Review cards API at `frontend/src/features/cards/cards.api.ts`
- [ ] Review use-cards hook at `frontend/src/features/cards/use-cards.ts`
- [ ] Verify backend PATCH endpoint at `backend/src/cards/cards.controller.ts`
- [ ] Check column entity at `backend/src/columns/entities/column.entity.ts`
- [ ] Confirm dnd-kit is installed: `npm install @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities`

---

## Technical Requirements

### Library Requirements

**dnd-kit** is required for accessible, touch-supportive drag interactions.

```bash
# Install dnd-kit core packages
npm install @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
```

**Version:** Latest stable (check via `npm view @dnd-kit/core version`)

**Usage Pattern (from dnd-kit docs):**
```jsx
import {DragDropProvider, useDraggable, useDroppable} from '@dnd-kit/core';
import {useDraggable} from '@dnd-kit/core';

// Card becomes Draggable
function Card({id}) {
  const {ref, attributes, listeners, transform} = useDraggable({id});
  return <div ref={ref} {...listeners} {...attributes}>Card</div>;
}

// Column becomes Droppable
function Column({id, children}) {
  const {ref, isDropTarget} = useDroppable({id});
  return <div ref={ref}>{children}</div>;
}

// Wrap board with DragDropProvider
function BoardView() {
  return (
    <DragDropProvider onDragEnd={handleDragEnd}>
      {/* columns and cards */}
    </DragDropProvider>
  );
}
```

### Backend API Requirements

**PATCH /api/cards/:id** (UPDATED - consolidate with existing)

**Request:**
```typescript
{
  title?: string;        // Existing - card title
  columnId?: number;  // NEW - target column ID for move
  position?: number;  // Optional: position within column
}
```

**Response:**
```typescript
{
  data: Card;
  message: "Card moved successfully" | "Card updated";
}
```

**Required in:**
- `backend/src/cards/cards.controller.ts` - Extend existing `PATCH /:id` to handle `columnId`
- `backend/src/cards/cards.service.ts` - Add move logic in existing `update()` method

### Frontend Component Requirements

**Files to Create:**
- `frontend/src/features/cards/drag-drop-context.tsx` - DragDropProvider wrapper
- `frontend/src/features/cards/card-draggable.tsx` - Draggable card wrapper with conflict resolution
- `frontend/src/features/columns/column-droppable.tsx` - Droppable column wrapper

**Files to Modify:**
- `frontend/src/features/cards/card.tsx` - Wrap with card-draggable component
- `frontend/src/features/columns/column.tsx` - Add useDroppable
- `frontend/src/features/columns/column-card-list.tsx` - Handle drag-over state
- `frontend/src/features/cards/use-cards.ts` - Extend existing updateCard mutation
- `frontend/src/features/cards/cards.api.ts` - Reuse update API (no new call needed)

**⚠️ CRITICAL: Drag-vs-Edit Conflict Resolution**

The `card-draggable.tsx` wrapper MUST use dnd-kit's activation constraint:
```typescript
const {ref, ...} = useDraggable({
  id: card.id,
  data: { cardId: card.id },
  activationConstraint: {
    distance: 5,  // Require 5px drag before initiating
  },
});
```

This ensures clicks for edit mode don't accidentally start drags. Only after moving 5px does drag mode activate.

### State Management

- **Optimistic Updates:** Move card immediately in React Query cache before API response
- **Rollback on Error:** If API fails, restore original position and show error toast
- **Position Tracking:** Store card order in column (array index or explicit position field)

### UX Animation Specifications (Added via Party Mode Review)

| Animation | Duration | Easing | Token Reference |
|-----------|----------|--------|--------------|
| Card hover lift shadow | 150ms | ease-out | `shadow-md` → `shadow-lg` |
| Drag start lift | 200ms | cubic-bezier(0.34, 1.56, 0.64, 1) | Spring bounce |
| Drop zone border glow | 150ms | ease-in-out | Border: Teal 500 at 50% opacity |
| Insertion point line | 100ms | linear | 2px height, Teal 500 |
| Neighbor card shift | 200ms | ease-out | translateY(20px) |
| Drop settle bounce | 300ms | cubic-bezier(0.34, 1.56, 0.64, 1) | Spring bounce |
| Checkmark animation | 400ms | ease-out | Scale 0 → 1.2 → 1 |
| Invalid drop shake | 400ms | linear | translateX: ±4px, 3 oscillations |
| Snap back | 250ms | cubic-bezier(0.34, 1.56, 0.64, 1) | Spring return |

**Design System Tokens:**
- Lift shadow: `shadow-md` (2px) → `shadow-lg` (4px)
- Border glow: `border-teal-500/50`
- Insertion line: `bg-teal-500` at 60% opacity
- Checkmark icon: `lucide-react` checkCircle with `text-green-500`

**Accessibility:**
- All animations must respect `prefers-reduced-motion` media query - disable animations when true
- Touch: long-press (300ms) initiates drag on mobile
- Keyboard: Space to grab, arrow keys to move, Space to drop

---

## Architecture Compliance

### Technical Stack (from architecture.md)

| Component | Technology | Version |
|-----------|------------|---------|
| Frontend | React + Vite | 18+ |
| Styling | Tailwind CSS | v4 |
| UI Components | shadcn/ui | Latest |
| State | React Query | TanStack Query v5 |
| Drag-Drop | dnd-kit | Latest |
| Backend | NestJS | Latest |
| ORM | TypeORM | Latest |
| Database | MySQL | 8.x |

### Naming Conventions

| Layer | Convention | Example |
|-------|------------|---------|
| Database | snake_case | column_id, position |
| API | kebab-case | /api/cards/:id/move |
| Code | camelCase | moveCard(), targetColumnId |
| Components | PascalCase | CardDraggable, ColumnDroppable |
| Files | kebab-case | card-draggable.tsx |

### Module Structure

```
frontend/src/features/cards/
├── card.tsx                      # (exists) - add draggable wrapper
├── card-draggable.tsx            # NEW - Draggable wrapper with dnd-kit
├── drag-drop-context.tsx        # NEW - DragDropProvider + handlers
├── use-cards.ts                  # (exists) - add moveCard mutation
├── cards.api.ts                  # (exists) - add moveCard API call
└── card.test.tsx                 # (exists) - add drag-drop tests

frontend/src/features/columns/
├── column.tsx                    # (exists) - add droppable wrapper
├── column-droppable.tsx         # NEW - Droppable wrapper with dnd-kit
├── column-card-list.tsx         # (exists) - handle drag-over state
└── column.test.tsx             # (exists) - add droppable tests

backend/src/cards/
├── cards.controller.ts          # (exists) - add PATCH /:id/move
├── cards.service.ts            # (exists) - add moveCard method
├── dto/
│   └── move-card.dto.ts         # NEW - DTO for move operation
└── cards.service.spec.ts       # (exists) - add moveCard tests
```

### API Response Formats

```typescript
// Success
{ data: Card, message?: string }

// Error
{ statusCode: number, message: string | string[], error: string }
```

### Authentication

- All card endpoints protected by `AuthGuard` (from architecture.md)
- User can only move their own cards (ownership enforced in service)
- Session via httpOnly cookie (from architecture.md)

---

## Testing Requirements

### Unit Tests (Backend)

**cards.service.spec.ts - Add:**
- Test: moveCard with valid card and column IDs
- Test: moveCard with invalid card ID (404)
- Test: moveCard with invalid column ID (404)
- Test: moveCard unauthorized (401)
- Test: moveCard with card owned by different user (403)

### Component Tests (Frontend)

**card.test.tsx - Add:**
- Test: card shows grab cursor on hover
- Test: card lifts with shadow during drag
- Test: card returns to original position on Escape
- Test: card returns to original position on invalid drop

**column.test.tsx - Add:**
- Test: column highlights on drag-over
- Test: insertion point shows during drag-over

### Integration Tests

- Test: Drag card from "To Do" to "In Progress" - API called, UI updates
- Test: Drag card to invalid zone - card returns, no API call
- Test: Drag card to "Done" column - checkmark animation plays
- **NEW:** Test: Card in edit mode - drag initiates only after 5px movement (drag vs click)

### Manual Testing Checklist

- [ ] Drag card from column A to column B - card moves
- [ ] Drag card to invalid area - card snaps back
- [ ] Press Escape during drag - card returns
- [ ] Drag to "Done" column - checkmark animation
- [ ] API failure - card returns, error toast shows
- [ ] Multiple cards - other cards shift to make room
- [ ] Touch device - long-press initiates drag
- [ ] Keyboard - Space to grab, arrows to move, Space to drop
- [ ] prefers-reduced-motion - animations disabled

---

## Previous Story Intelligence

### From Story 3-1 (Card Creation) - COMPLETED

**Key Files:**
- `frontend/src/features/cards/card.tsx` - Card display component
- `frontend/src/features/cards/add-card-input.tsx` - Inline creation
- `frontend/src/features/cards/use-cards.ts` - React Query hooks
- `frontend/src/features/cards/cards.api.ts` - API client
- `frontend/src/features/columns/column.tsx` - Column component
- `frontend/src/features/columns/column-card-list.tsx` - Card list in column

**Learnings:**
- Card component already renders in column-card-list.tsx
- use-cards.ts has createCard, updateCard, deleteCard mutations
- cards.api.ts has createCard, updateCard, deleteCard API calls
- Card entity has columnId relationship
- Animation already exists: slide-up in index.css

**Critical:** Card already has styling - preserve while adding drag behavior.

### From Story 3-2 (Card Editing) - COMPLETED

**Key Files:**
- `frontend/src/features/cards/card.tsx` - Modified with inline edit
- `frontend/src/features/cards/use-cards.ts` - Has updateCard mutation
- `backend/src/cards/cards.controller.ts` - Has PATCH endpoint
- `backend/src/cards/cards.service.ts` - Has update method

**Learnings:**
- Card component uses local state for edit mode
- Backend PATCH /:id already exists - can use same pattern for move
- Optimistic updates via useCards mutation
- card.test.tsx exists with 4 tests

**Critical:** Backend already has PATCH endpoint - can follow same pattern for move.

### From Epic 2 Stories

**Story 2.3 (Column CRUD):**
- Column entity at `backend/src/columns/entities/column.entity.ts`
- Columns have `id`, `name`, `boardId`, `position`

**Story 2.4 (Column Sorting & Bulk Move):**
- Bulk move endpoint at `backend/src/columns/columns.controller.ts`
- `PATCH /:id/move-cards` moves all cards
- Can reference for column-based API patterns

**Critical:** Column already has position - can use similar pattern for card position within column.

---

## Git Intelligence Summary

### Recent Commits

(Scan recent commits for patterns - check git log for files in this epic)

**Expected patterns from Epic 3:**
- Card creation files: 3-1-card-creation.md
- Card editing files: 3-2-card-editing.md
- Frontend pattern: React Query mutations with optimistic updates
- Backend pattern: NestJS service methods with ownership checks

### Key Patterns to Follow

1. **Frontend:** Use React Query `useMutation` with `onMutate` for optimistic updates
2. **Backend:** Return `{data: Card, message: string}` format
3. **Testing:** Co-located tests next to components
4. **Animation:** CSS animations in index.css with prefers-reduced-motion

---

## Latest Technical Information

### dnd-kit Usage (Latest)

**Core Hooks:**
- `useDraggable({id})` - Makes element draggable
- `useDroppable({id})` - Makes element a drop target
- `DragDropProvider` - Wraps entire board, handles drag state

**Key API:**
```typescript
// Draggable
const {ref, transform, isDragging} = useDraggable({
  id: 'card-1',
  data: { cardId: 1 }, // Optional data passed to handlers
});

// Droppable
const {ref, isOver} = useDroppable({
  id: 'column-1',
  accept: 'card', // Optional: only accept specific types
});

// DragDropProvider
<DragDropProvider
  onDragStart={(event) => {...}}
  onDragOver={(event) => {...}} // For moving between containers
  onDragEnd={(event) => {...}}
>
```

**Accessibility:**
- Keyboard support built-in: Space to grab, arrows to move, Space to drop
- Screen reader announcements via dnd-kit's built-in a11y

---

## Project Context Reference

**Project:** KanbanFlow
**Type:** Full-stack SPA (React/Vite + NestJS + MySQL)
**User:** Raziur (intermediate skill level)

**From project-context.md:**
- Design system: Soft Teal color palette (Teal 600 primary)
- Theming: CSS variables for light/dark mode
- UX patterns: Inline editing, optimistic updates, toast notifications
- Naming: snake_case (DB), kebab-case (API), camelCase/PascalCase (code)

---

## Story Completion Status

**Status:** ready-for-dev
**Ultimate context engine analysis completed - comprehensive developer guide created**

---

## References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.3-Drag-Drop-Between-Columns]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend-Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#API-Communication-Patterns]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md]
- [Context7: /clauderic/dnd-kit - drag-drop between columns implementation]
- [File: frontend/src/features/cards/card.tsx]
- [File: frontend/src/features/cards/use-cards.ts]
- [File: frontend/src/features/cards/cards.api.ts]
- [File: frontend/src/features/columns/column.tsx]
- [File: frontend/src/features/columns/column-card-list.tsx]
- [File: backend/src/cards/cards.controller.ts]
- [File: backend/src/cards/cards.service.ts]
- [File: backend/src/cards/entities/card.entity.ts]
- [File: backend/src/columns/entities/column.entity.ts]
- [Implementation: _bmad-output/implementation-artifacts/3-1-card-creation.md]
- [Implementation: _bmad-output/implementation-artifacts/3-2-card-editing.md]
- [Implementation: _bmad-output/implementation-artifacts/2-3-column-crud.md]
- [Implementation: _bmad-output/implementation-artifacts/2-4-column-sorting-bulk-move.md]
- [Implementation: _bmad-output/implementation-artifacts/2-5-board-view-layout.md]