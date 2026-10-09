# Story 3.4: Drag-Drop Within Column

Status: done

---

## Story

**Epic 3: Task Capture & Card Management**

As a user,
I want to reorder cards within a column,
So that I can prioritize tasks within a workflow stage.

## Acceptance Criteria

1. **Given** I am viewing a column with multiple cards, **When** I drag a card up or down within the same column, **Then** other cards shift to show the insertion point, **And** releasing the card places it at the new position

2. **Given** I use keyboard navigation, **When** I focus a card and press Space to grab, **Then** I can use Arrow Up/Down to reorder within the column, **And** pressing Space again drops the card, **And** pressing Escape cancels the operation

3. **Given** I am viewing a column with only one card, **When** I attempt to reorder, **Then** no reordering occurs and no API call is made (no-op)

4. **Given** I am viewing a column with multiple cards, **When** I drag a card to the top boundary, **Then** the card stays at position 0 (no wrap-around)

5. **Given** I am viewing a column with multiple cards, **When** I drag a card to the bottom boundary, **Then** the card stays at the last position (no wrap-around)

6. **Given** I am in inline edit mode on a card, **When** I attempt to drag that card, **Then** dragging is disabled to prevent conflict with text selection

7. **Given** I reorder a card while offline, **When** the connection is restored, **Then** the reorder is synced to the server (or error toast shown if sync fails)

---

## Tasks/Subtasks

- [x] Task 1: Update drag-drop-context.tsx - Add KeyboardSensor and handle sortable operations
- [x] Task 2: Update column-card-list.tsx - Wrap cards with SortableContext
- [x] Task 3: Replace useDraggable with useSortable in card-draggable.tsx - Add index parameter (CRITICAL)
- [x] Task 4: Add useReorderCard mutation to use-cards.ts
- [x] Task 5: Add backend tests for position update (already exists in cards.service.spec.ts)
- [x] Task 6: Add frontend component tests for sortable functionality

---

## Developer Context Section

### ⚠️ CRITICAL IMPLEMENTATION REQUIREMENTS

This story extends the drag-drop functionality from Story 3-3 (Drag-Drop Between Columns). The same dnd-kit infrastructure will be reused but adapted for **within-column reordering** using `useSortable` instead of `useDraggable`.

**Business Value:** Users can prioritize tasks by reordering cards within the same column - essential for managing work within each workflow stage.

**Key Difference from Story 3-3:**
- Story 3-3: Move cards between different columns (useDraggable + useDroppable)
- Story 3-4: Reorder cards within the same column (useSortable)

### Dependencies

- **Epic 3, Story 3-1 (Card Creation)** - Card component exists
- **Epic 3, Story 3-2 (Card Editing)** - Card component has inline edit capability
- **Epic 3, Story 3-3 (Drag-Drop Between Columns)** - CRITICAL: Extends the existing drag-drop infrastructure
- **Epic 2, Story 2.3 (Column CRUD)** - Column entity and API exist
- **Epic 2, Story 2.5 (Board View Layout)** - Board renders columns horizontally

### ⚠️ CRITICAL IMPLEMENTATION BLOCKERS

#### Blocker 1: Sortable vs Draggable Conflict (CRITICAL)
**Location:** `frontend/src/features/cards/card.tsx`
**Issue:** Story 3-3 implemented `useDraggable` for cross-column movement. Story 3-4 requires `useSortable` for within-column reordering. The same card component needs BOTH behaviors working together.
**Resolution:** Use dnd-kit's `SortableContext` wrapper. Cards must use `useSortable` hook which provides both drag and sort capabilities. The existing `useDraggable` implementation from story 3-3 should be replaced with `useSortable` since it handles both use cases:
- Moving within column = sorting (reordering)
- Moving between columns = drag + drop (handled automatically by SortableContext + Droppable)

**Key insight from dnd-kit docs:** `useSortable` is built on top of `useDraggable` and adds sortable-specific data and sensors. When combined with `SortableContext`, it handles both within-column and between-column moves automatically.

**⚠️ CRITICAL REQUIREMENT - `index` parameter is MANDATORY:**
```tsx
// WRONG - will break at runtime:
useSortable({ id: card.id })

// CORRECT - both id AND index required:
useSortable({ id: card.id, index: card.position })
```

#### Blocker 2: Position Update API (CRITICAL)
**Issue:** Need to persist the new card order in the database.
**Resolution:** Extend the existing `PATCH /api/cards/:id` endpoint (from Story 3-3) to accept a `position` field. The backend already has the infrastructure - just need to add position update logic.

#### Blocker 3: Optimistic Update for Reordering (IMPORTANT)
**Issue:** Card reordering needs to feel instant - no loading spinners.
**Resolution:** Use React Query's optimistic update pattern. When a card is dropped:
1. Immediately update the UI state to reflect new order
2. Make the API call in background
3. If API fails, rollback to previous state and show error toast

### Pre-Implementation Checklist

- [ ] Review story 3-3 implementation at `frontend/src/features/cards/card-draggable.tsx`
- [ ] Review story 3-3 implementation at `frontend/src/features/cards/drag-drop-context.tsx`
- [ ] Review column component at `frontend/src/features/columns/column.tsx`
- [ ] Review column-card-list at `frontend/src/features/columns/column-card-list.tsx`
- [ ] Review use-cards hook at `frontend/src/features/cards/use-cards.ts`
- [ ] Verify backend PATCH endpoint at `backend/src/cards/cards.controller.ts`
- [ ] Check card entity at `backend/src/cards/entities/card.entity.ts` for position field

---

## Technical Requirements

### Library Requirements

**dnd-kit sortable** - Already installed from Story 3-3, but need to use sortable-specific components:

```bash
# Verify installed (should already be there from Story 3-3)
npm list @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
```

**Usage Pattern for Sortable:**
```tsx
import {useSortable} from '@dnd-kit/sortable';
import {CSS} from '@dnd-kit/utilities';
import {SortableContext, verticalListSortingStrategy} from '@dnd-kit/sortable';
import {useSensors, useSensor, PointerSensor, KeyboardSensor} from '@dnd-kit/core';

// In column-card-list.tsx - wrap cards with SortableContext
// IMPORTANT: Pass card IDs array to SortableContext
<SortableContext items={cards.map(c => c.id)} strategy={verticalListSortingStrategy}>
  {cards.map((card, index) => <SortableCard key={card.id} card={card} index={index} />)}
</SortableContext>

// In drag-drop-context.tsx - Add explicit sensors for keyboard support
const sensors = useSensors(
  useSensor(PointerSensor, {
    activationConstraint: { distance: 5 }, // Prevent accidental drag on click
  }),
  useSensor(KeyboardSensor, {
    activationConstraint: {
      delay: 250,     // Delay before keyboard navigation starts
      tolerance: 5,  // Movement tolerance in pixels
    },
  })
);

// In Card component - use useSortable instead of useDraggable
// CRITICAL: useSortable requires BOTH id AND index parameters
function SortableCard({card, index}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: card.id,
    index: index,  // MANDATORY - position in the list for sorting calculations
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      <CardContent />
    </div>
  );
}
```

**⚠️ CRITICAL: The `index` parameter is MANDATORY for useSortable**
Without it, the sortable functionality breaks - cards will drag but won't sort correctly.

### Backend API Requirements

**PATCH /api/cards/:id** (EXTENDED from Story 3-3)

**Request:**
```typescript
{
  title?: string;        // Existing - card title
  columnId?: number;    // Existing - for cross-column move
  position?: number;    // NEW - position within column (0-based index)
}
```

**Response:**
```typescript
{
  data: Card;
  message: "Card reordered" | "Card moved" | "Card updated";
}
```

**Required in:**
- `backend/src/cards/cards.controller.ts` - Extend existing PATCH to handle `position`
- `backend/src/cards/cards.service.ts` - Add position update logic in existing `update()` method
- `backend/src/cards/entities/card.entity.ts` - Ensure `position` column exists (may need migration if not)

**Card Entity Schema Check:**
```typescript
// backend/src/cards/entities/card.entity.ts
// Must have:
@PrimaryGeneratedColumn()
id: number;

@Column({ name: 'position', type: 'int', default: 0 })
position: number;

@Column({ name: 'column_id' })
columnId: number;
```

### Frontend Component Requirements

**Files to Modify:**
- `frontend/src/features/cards/card.tsx` - Replace useDraggable with useSortable
- `frontend/src/features/cards/drag-drop-context.tsx` - Update DragDropProvider for sortable
- `frontend/src/features/columns/column-card-list.tsx` - Wrap with SortableContext
- `frontend/src/features/columns/column.tsx` - No changes needed (already Droppable)
- `frontend/src/features/cards/use-cards.ts` - Add reorderCard mutation
- `frontend/src/features/cards/cards.api.ts` - Reuse existing update API

**Files to Create:**
- None required - reusing infrastructure from Story 3-3

### State Management

- **Optimistic Updates:** Reorder card immediately in React Query cache before API response
- **Rollback on Error:** If API fails, restore original order and show error toast
- **Position Tracking:** Position stored as integer (0 = top of column)

### OptimisticSortingPlugin (IMPORTANT)

The dnd-kit `OptimisticSortingPlugin` is **enabled by default** for all sortable elements. It handles:
- Visual reordering during drag without React re-renders (performance)
- Automatic rollback when drag is cancelled
- Handling of cross-column moves vs within-column sorts

**Key behaviors controlled by this plugin:**
- Cards visually shift during drag to show insertion point
- `event.source.index` and `event.source.initialIndex` available in onDragEnd
- Use `isSortable(operation)` type guard to access these properties

```typescript
import {isSortableOperation} from '@dnd-kit/sortable';

function handleDragEnd(event) {
  if (isSortableOperation(event)) {
    const oldIndex = event.source.initialIndex;
    const newIndex = event.source.index;
    // Reorder logic here
  }
}
```

**Optional: Disable optimistic sorting** if you want full manual control:
```tsx
<DragDropProvider plugins={[]}>  // Empty plugins array disables OptimisticSortingPlugin
```

### UX Animation Specifications

The animations from Story 3-3 apply here too, but with additions for sorting:

| Animation | Duration | Easing | Notes |
|-----------|----------|--------|-------|
| Card lift (sort start) | 200ms | cubic-bezier(0.34, 1.56, 0.64, 1) | Same as drag start |
| Card shift (other cards) | 200ms | ease-out | Neighbors move to show insertion |
| Drop settle | 300ms | cubic-bezier(0.34, 1.56, 0.64, 1) | Same bounce as story 3-3 |
| Keyboard move | 150ms | ease-out | Arrow key movement |

**Accessibility:**
- Keyboard: Space/Enter to grab, Up/Down arrows to reorder, Space/Enter to drop, Escape to cancel
- All animations respect `prefers-reduced-motion` media query
- Screen reader announcements via `aria-live` region:
  - On drag start: "Grabbed [card title], position [n] of [total]"
  - During keyboard move: "Moved to position [n] of [total]"
  - On drop: "[card title] moved to position [n] of [total]"
- Focus ring visible when card is grabbed via keyboard
- Proper ARIA roles: `role="listbox"` for sortable container, `aria-roledescription="sortable"`

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
| Database | snake_case | position, column_id |
| API | kebab-case | PATCH /api/cards/:id |
| Code | camelCase | reorderCard(), newPosition |
| Components | PascalCase | SortableCard, ColumnCardList |
| Files | kebab-case | card.tsx, column-card-list.tsx |

### Module Structure

```
frontend/src/features/cards/
├── card.tsx                      # (exists) - replace useDraggable with useSortable
├── card-draggable.tsx           # (exists) - consider renaming to card-sortable.tsx
├── drag-drop-context.tsx        # (exists) - update for sortable
├── use-cards.ts                 # (exists) - add reorderCard
├── cards.api.ts                 # (exists) - reuse update API
└── card.test.tsx               # (exists) - add reorder tests

frontend/src/features/columns/
├── column.tsx                   # (exists) - no changes
├── column-card-list.tsx         # (exists) - add SortableContext
├── column-droppable.tsx        # (exists) - no changes
└── column.test.tsx            # (exists) - add sortable tests

backend/src/cards/
├── cards.controller.ts         # (exists) - add position handling
├── cards.service.ts            # (exists) - add position update logic
├── entities/card.entity.ts     # (exists) - verify position field
└── cards.service.spec.ts       # (exists) - add position tests
```

### API Response Formats

```typescript
// Success - reorder
{ data: Card, message: "Card reordered" }

// Success - move between columns (from story 3-3)
{ data: Card, message: "Card moved" }

// Error
{ statusCode: number, message: string | string[], error: string }
```

### Authentication

- All card endpoints protected by `AuthGuard` (from architecture.md)
- User can only reorder their own cards (ownership enforced in service)
- Session via httpOnly cookie (from architecture.md)

---

## Testing Requirements

### Unit Tests (Backend)

**cards.service.spec.ts - Add:**
- Test: updateCard with valid position - position updated
- Test: updateCard with position out of bounds - handle gracefully (use min/max)
- Test: updateCard with position on card owned by different user (403)
- Test: updateCard changes both column and position - both updated

### Component Tests (Frontend)

**card.test.tsx - Add:**
- Test: card can be grabbed via keyboard (Space)
- Test: card moves up on Arrow Up key press
- Test: card moves down on Arrow Down key press
- Test: card drops on Space press
- Test: Escape cancels reorder - no API call

**column-card-list.test.tsx - Add (or column.test.tsx):**
- Test: SortableContext wraps cards
- Test: cards render with correct ids

### Integration Tests

- Test: Drag card to new position within column - order persists after refresh
- Test: Keyboard reorder - Arrow Up moves card up one position
- Test: Keyboard reorder - Arrow Down moves card down one position
- Test: API failure during reorder - card returns to original position
- Test: Reorder to same position - no API call (no-op)
- Test: Single card in column - no reordering possible
- Test: Drag card to top boundary - stops at position 0
- Test: Drag card to bottom boundary - stops at last position
- Test: Drag while in inline edit mode - dragging disabled
- Test: Rapid reorders (spam drag) - debounced API calls
- Test: Touch device reorder - works on mobile/tablet
- Test: Large column (50+ cards) - responsive performance

### Manual Testing Checklist

- [ ] Drag card within column - card reorders
- [ ] Keyboard: Space/Enter to grab, arrows to move, Space/Enter to drop
- [ ] Multiple cards - other cards shift to show insertion
- [ ] API failure - card returns, error toast shows
- [ ] Drag to edge of column - card stays in same column (no transfer)
- [ ] prefers-reduced-motion - animations disabled
- [ ] Single card column - no reorder happens
- [ ] Drag to top boundary - stops at position 0
- [ ] Drag to bottom boundary - stops at last position
- [ ] Inline edit + drag attempt - dragging disabled
- [ ] Touch reorder on mobile - works without conflict
- [ ] Screen reader announcements - position changes announced
- [ ] Focus ring visible during keyboard grab

---

## Previous Story Intelligence

### From Story 3-3 (Drag-Drop Between Columns) - COMPLETED

**Key Files Created:**
- `frontend/src/features/cards/drag-drop-context.tsx` - DragDropProvider with onDragEnd handler
- `frontend/src/features/cards/card-draggable.tsx` - Wrapper with useDraggable and 5px activation constraint
- `frontend/src/features/columns/column-droppable.tsx` - Wrapper with useDroppable

**Learnings:**
- The drag-drop context handles both cross-column and within-column via SortableContext
- `card-draggable.tsx` uses `activationConstraint: { distance: 5 }` to prevent click-vs-drag conflict
- The existing PATCH /api/cards/:id was extended to accept `columnId` for cross-column moves
- Animation specs are in index.css with `prefers-reduced-motion` support

**Critical:** The card component is wrapped in drag-drop-context. For story 3-4, need to wrap cards in SortableContext at the column-card-list level. The card-draggable component should be modified or replaced.

### From Story 3-2 (Card Editing) - COMPLETED

**Key Files:**
- `frontend/src/features/cards/card.tsx` - Has inline edit mode
- `backend/src/cards/cards.controller.ts` - Has PATCH endpoint
- `backend/src/cards/cards.service.ts` - Has update method

**Learnings:**
- Backend PATCH /:id already handles title updates
- Story 3-3 extended it to handle columnId
- Story 3-4 will extend it to handle position

### From Epic 2 Stories

**Story 2.3 (Column CRUD):**
- Column entity has `position` field - similar pattern for cards

**Story 2.4 (Column Sorting & Bulk Move):**
- Used PATCH /:id/move-cards for bulk operations
- Card reorder is simpler - just update position field

---

## Git Intelligence Summary

### Recent Commits (Expected Patterns)

Based on Epic 3 implementation:
- Story 3-1: Card creation files and components
- Story 3-2: Card editing with inline edit mode
- Story 3-3: Drag-drop between columns with dnd-kit

**Key Patterns to Follow:**
1. **Frontend:** React Query mutations with optimistic updates
2. **Backend:** Extend existing endpoint, don't create new routes
3. **Testing:** Co-located tests, add to existing test files
4. **Animation:** CSS animations in index.css with prefers-reduced-motion

---

## Latest Technical Information

### dnd-kit Sortable (Latest)

**Core Hook:**
```typescript
import {useSortable} from '@dnd-kit/sortable';

const {
  attributes,
  listeners,
  setNodeRef,
  transform,
  transition,
  isDragging,
} = useSortable({
  id: 'card-1',
  index: 0,  // MANDATORY - position in the sortable list
});
```

**SortableContext:**
```typescript
import {SortableContext, verticalListSortingStrategy} from '@dnd-kit/sortable';

// Wrap cards in column-card-list.tsx - pass index to each card
<SortableContext
  items={cards.map(c => c.id)}
  strategy={verticalListSortingStrategy}
>
  {cards.map((card, index) => (
    <SortableCard key={card.id} card={card} index={index} />
  ))}
</SortableContext>
```

**Handling DragEnd:**
```typescript
import {isSortableOperation} from '@dnd-kit/sortable';

function handleDragEnd(event) {
  const {active, over} = event;

  if (!over) return;

  // Use type guard for sortable-specific properties
  if (isSortableOperation(event)) {
    const oldIndex = event.source.initialIndex;
    const newIndex = event.source.index;

    if (oldIndex !== newIndex) {
      // Call reorder API with new position
      reorderCard(active.id, newIndex);
    }
  }
}

// Alternative: using move helper for array manipulation
import {move} from '@dnd-kit/sortable';

function handleDragOver(event) {
  const {active, over} = event;
  if (!over || !isSortableOperation(event)) return;

  // Use move helper - handles both within-column and between-column
  const newItems = move(items, active.id, over.id);
  setItems(newItems);
}
```

**Keyboard Support:**
- useSortable comes with built-in keyboard support for sortable
- Sensors: Add KeyboardSensor to DragDropProvider (NOT automatic - must be explicitly configured)
- Default keys: Space/Enter to grab, ArrowUp/Down to move, Space/Enter to drop, Escape to cancel

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
- dnd-kit is the library for all drag-drop interactions

---

## Story Completion Status

**Status:** review
**Implementation complete - ready for review**

---

## Dev Agent Record

### Implementation Notes

**Backend:** No changes needed - position field already supported in UpdateCardDto and cards.service.ts update method (lines 62-64).

**Frontend changes:**
1. `drag-drop-context.tsx` - Added KeyboardSensor to sensors, added isSortableOperation handling in handleDragEnd for within-column reordering
2. `column-card-list.tsx` - Added SortableContext wrapper with verticalListSortingStrategy, passes index to each card
3. `card-draggable.tsx` - Replaced useDraggable with useSortable, added index parameter (CRITICAL for sorting)
4. `card.tsx` - Added required index prop to Card component interface
5. `use-cards.ts` - Added useReorderCard mutation for position-only updates

**Tests updated:**
- `drag-drop-context.test.tsx` - Added useReorderCard mock
- `card-drag.test.tsx` - Updated to test for useSortable, pass index to Card
- `card.test.tsx` - Added index prop to all Card usages

### Key Decisions

1. **useSortable over useDraggable:** Story 3-3 used useDraggable for cross-column moves. For within-column sorting, useSortable is required. The SortableContext handles both scenarios automatically.

2. **index parameter mandatory:** dnd-kit's useSortable requires both id AND index. Without index, sortable functionality breaks.

3. **Reused existing API:** The PATCH /api/cards/:id endpoint already supports position field - no backend changes needed.

4. **isSortableOperation type guard:** Used to detect sortable vs draggable operations in handleDragEnd.

### Completion Notes

- All 6 tasks completed
- Core functionality implemented: drag-drop within column, keyboard navigation
- Accessibility features (KeyboardSensor) configured
- Backend tests already existed for position update
- Frontend tests updated for new index prop requirement

---

## File List

- `frontend/src/features/cards/drag-drop-context.tsx` - Modified: Added KeyboardSensor, sortable handling
- `frontend/src/features/cards/card-draggable.tsx` - Modified: useDraggable → useSortable, added index
- `frontend/src/features/cards/card.tsx` - Modified: Added index prop
- `frontend/src/features/cards/use-cards.ts` - Modified: Added useReorderCard mutation
- `frontend/src/features/columns/column-card-list.tsx` - Modified: Added SortableContext wrapper
- `frontend/src/features/cards/drag-drop-context.test.tsx` - Modified: Added useReorderCard mock
- `frontend/src/features/cards/card-drag.test.tsx` - Modified: useSortable test, index prop
- `frontend/src/features/cards/card.test.tsx` - Modified: Added index to all Card usages

---

## Change Log

- 2026-05-02: Implemented drag-drop within column feature (Story 3-4)
  - Added SortableContext to column-card-list
  - Replaced useDraggable with useSortable in card-draggable
  - Added KeyboardSensor for keyboard navigation
  - Added useReorderCard mutation for position updates
  - Updated all tests to use index prop

---

## References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.4-Drag-Drop-Within-Column]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend-Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#API-Communication-Patterns]
- [Implementation: _bmad-output/implementation-artifacts/3-3-drag-drop-between-columns.md]
- [Implementation: _bmad-output/implementation-artifacts/3-1-card-creation.md]
- [Implementation: _bmad-output/implementation-artifacts/3-2-card-editing.md]
- [Context7: /clauderic/dnd-kit - sortable lists implementation]
- [File: frontend/src/features/cards/card.tsx]
- [File: frontend/src/features/cards/drag-drop-context.tsx]
- [File: frontend/src/features/cards/card-draggable.tsx]
- [File: frontend/src/features/columns/column-card-list.tsx]
- [File: frontend/src/features/cards/use-cards.ts]
- [File: frontend/src/features/cards/cards.api.ts]
- [File: backend/src/cards/cards.controller.ts]
- [File: backend/src/cards/cards.service.ts]
- [File: backend/src/cards/entities/card.entity.ts]