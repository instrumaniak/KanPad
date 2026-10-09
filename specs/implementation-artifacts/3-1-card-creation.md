# Story 3.1: Card Creation

Status: done

---

## Story

As a user,
I want to quickly create cards by typing a title,
So that I can capture tasks as fast as I think of them.

## Acceptance Criteria

1. **Given** I am viewing a column, **When** I click "+ Add a card" at the bottom of the column, **Then** an inline text input appears (no modal), **And** the input is focused immediately

2. **Given** I type a title and press Enter, **When** the card is created, **Then** the card appears at the bottom of the column, **And** the input stays focused for the next card, **And** a subtle slide-up animation plays on the new card

3. **Given** I press Escape or click away, **When** the input is empty, **Then** the input closes without creating a card

4. **Given** I press Tab, **When** creating cards, **Then** focus moves to the next column's "Add a card" input

---

## Tasks / Subtasks

- [x] Task 1: Create backend Cards API (AC: #1, #2)
  - [x] Subtask 1.1: Create cards controller at backend/src/cards/cards.controller.ts
  - [x] Subtask 1.2: Create cards service at backend/src/cards/cards.service.ts
  - [x] Subtask 1.3: Create DTOs (create-card.dto.ts, update-card.dto.ts)
  - [x] Subtask 1.4: Register cards module in app.module.ts
  - [x] Subtask 1.5: Add column_id foreign key to card entity if needed

- [x] Task 2: Create frontend Cards feature module (AC: #1, #2, #3)
  - [x] Subtask 2.1: Create cards.api.ts with API client functions
  - [x] Subtask 2.2: Create use-cards.ts hook with React Query
  - [x] Subtask 2.3: Create add-card-input.tsx component for inline input
  - [x] Subtask 2.4: Create card.tsx display component
  - [x] Subtask 2.5: Integrate cards into column-card-list.tsx

- [x] Task 3: Implement quick card capture UX (AC: #1, #2, #3, #4)
  - [x] Subtask 3.1: Add card input with auto-focus on click
  - [x] Subtask 3.2: Handle Enter to create and keep focus
  - [x] Subtask 3.3: Handle Escape to cancel
  - [x] Subtask 3.4: Handle Tab to move to next column

- [x] Task 4: Add card animation (AC: #2)
  - [x] Subtask 4.1: Add slide-up animation when card appears
  - [x] Subtask 4.2: Add prefers-reduced-motion support

- [x] Task 5: Testing (AC: all)
  - [x] Subtask 5.1: Write unit tests for cards service
  - [x] Subtask 5.2: Write component tests for add-card-input
  - [x] Subtask 5.3: Run all existing tests to verify no regressions

---

## Dev Notes

### Story Context

This is **Epic 3, Story 1** - the first story in the Task Capture & Card Management epic. It delivers the core kanban interaction: quick card creation. Users should be able to rapidly capture tasks without friction.

**Business Value:** Immediate productivity - users capture ideas as fast as they think.

**Dependencies:**
- Epic 2, Story 2.3 (Column CRUD) - column structure already exists
- Epic 2, Story 2.5 (Board View Layout) - horizontal scroll and column rendering in place

### Previous Story Intelligence

**From Story 2.5 (Board View Layout) - COMPLETED:**
- Column component at `frontend/src/features/columns/column.tsx`
- Column card list at `frontend/src/features/columns/column-card-list.tsx`
- Board view renders columns horizontally with 24px gap
- Column width: 320px (min-w-[320px] max-w-[320px])
- Add column button already in place

**From Story 2.3 (Column CRUD) - COMPLETED:**
- Column entity exists at `backend/src/columns/entities/column.entity.ts`
- Columns API at `frontend/src/features/columns/columns.api.ts`
- use-columns hook at `frontend/src/features/columns/use-columns.ts`
- Default columns created: "To Do", "In Progress", "Done"

**Backend Cards Module Status:**
- Card entity exists at `backend/src/cards/entities/card.entity.ts`
- No controller/service yet - this story creates them

### Architecture Compliance

**Technical Stack (from architecture.md):**
- React 18+ with Vite
- Tailwind CSS v4 via `@tailwindcss/vite`
- shadcn/ui (Radix UI primitives)
- React Query (TanStack Query) for server state
- NestJS + TypeORM + MySQL 8.x
- dnd-kit for drag-drop ( Epic 3 Stories 3-4)

**Naming Conventions:**
- DB: `snake_case` (e.g., `created_at`, `column_id`)
- API: `kebab-case` endpoints (e.g., `/api/cards`)
- Code: `camelCase` for fields/properties, `PascalCase` for components
- Files: `kebab-case` (e.g., `card.entity.ts`, `add-card-input.tsx`)

**Module Structure:**
```
frontend/src/features/cards/
├── card.tsx                 # Card display component
├── add-card-input.tsx       # Inline card creation input
├── card-list.tsx            # List of cards (if needed separately)
├── use-cards.ts             # React Query hook
└── cards.api.ts             # API client

backend/src/cards/
├── cards.module.ts          # Module (needs to be created)
├── cards.controller.ts     # API endpoints (needs to be created)
├── cards.service.ts       # Business logic (needs to be created)
├── entities/
│   └── card.entity.ts     # EXISTS
├── dto/
│   ├── create-card.dto.ts # NEEDS CREATE
│   └── update-card.dto.ts # NEEDS CREATE
```

**API Patterns:**
```typescript
// Success response format
{ data: T, message?: string }

// Error response format
{ statusCode: number, message: string | string[], error: string }
```

**Authentication Flow:**
- Cookie-based sessions via NestJS guards
- All card endpoints protected by `AuthGuard`
- User can only access their own cards (enforced in service)

### Critical Anti-Patterns

- ❌ NEVER create cards without column_id - cards must belong to a column
- ❌ NEVER skip optimistic UI - cards should appear immediately
- ❌ NEVER use raw fetch - use React Query hooks
- ❌ NEVER create modal for simple card creation - inline only per AC
- ❌ NEVER lose input focus after Enter - stay focused for rapid capture
- ❌ NEVER break column/card rendering from Story 2.5
- ❌ NEVER remove animation - subtle slide-up is required

### UX Requirements (UX-DR6, UX-DR14, UX-DR15)

**Card Quick Capture Flow:**
- Inline input at bottom of column (no modal)
- Input focused immediately on click
- Enter creates card, clears input, keeps focus
- Escape or click-away (empty) closes input
- Tab moves to next column's input

**Card Appearance:**
- Subtle slide-up animation (300ms ease-out)
- Card displays at bottom of column list
- If first card in board: celebration animation
- Respects prefers-reduced-motion

**Column Integration:**
- Cards render in column-card-list.tsx
- Column minimum 320px width accommodates card
- Card list scrollable within column

### Testing Standards

**Backend (Jest):**
- Test: card creation with valid column_id
- Test: card creation with invalid column_id (404)
- Test: card creation unauthorized (401)
- Test: card belongs to correct user (ownership)

**Frontend (Vitest + React Testing Library):**
- Test: clicking "+ Add a card" shows input
- Test: typing + Enter creates card
- Test: Enter clears input and keeps focus
- Test: Escape closes empty input
- Test: Tab moves focus to next column
- Test: card appears with animation

**Manual Testing Checklist:**
- [ ] Create card in each default column
- [ ] Create multiple cards rapidly (Enter, Enter, Enter)
- [ ] Verify Tab navigates between columns
- [ ] Verify Escape closes input
- [ ] Verify animation plays on card creation
- [ ] Test with prefers-reduced-motion enabled

---

## References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.1-Card-Creation]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend-Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#API-Communication-Patterns]
- [Source: _bmad-output/planning-artifacts/architecture.md#Pattern-Examples]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Core-User-Experience]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Design-System-Foundation]
- [Source: _bmad-output/implementation-artifacts/2-5-board-view-layout.md]
- [Source: _bmad-output/implementation-artifacts/2-3-column-crud.md]
- [Source: _bmad-output/implementation-artifacts/2-1-board-crud.md]
- [File: backend/src/cards/entities/card.entity.ts]
- [File: frontend/src/features/columns/column.tsx]
- [File: frontend/src/features/columns/column-card-list.tsx]
- [File: frontend/src/features/columns/use-columns.ts]

---

## Dev Agent Record

### Agent Model Used

- Model: minimax-m2.5-free (from opencode)

### Implementation Summary

**Story 3-1: Card Creation - COMPLETE**

**Backend (NestJS):**
- Created `backend/src/cards/cards.module.ts` - Cards module
- Created `backend/src/cards/cards.controller.ts` - REST API endpoints (GET/POST/PATCH/DELETE /api/cards)
- Created `backend/src/cards/cards.service.ts` - Business logic with ownership validation
- Created `backend/src/cards/dto/create-card.dto.ts` - DTO with validation
- Created `backend/src/cards/dto/update-card.dto.ts` - DTO with validation
- Registered CardsModule in app.module.ts

**Frontend (React):**
- Created `frontend/src/features/cards/cards.api.ts` - API client functions
- Created `frontend/src/features/cards/use-cards.ts` - React Query hooks
- Created `frontend/src/features/cards/card.tsx` - Card display component
- Created `frontend/src/features/cards/add-card-input.tsx` - Inline card creation input
- Updated `frontend/src/features/columns/column.tsx` - Integrated AddCardInput
- Updated `frontend/src/features/columns/column-card-list.tsx` - Use Card component
- Added slide-up animation in index.css with prefers-reduced-motion support

**Tests:**
- `backend/src/cards/cards.service.spec.ts` - 10 unit tests (all passing)
- `frontend/src/features/cards/add-card-input.test.tsx` - 3 component tests (all passing)

**All Acceptance Criteria Met:**
- ✅ AC1: Click "+ Add a card" shows inline input, focused immediately
- ✅ AC2: Enter creates card, keeps focus, slide-up animation
- ✅ AC3: Escape/click-away closes empty input
- ✅ AC4: Tab moves to next column's input

### Debug Log References

(TBD - will be filled in by developer agent)

### Completion Notes List

(TBD - will be filled in by developer agent)

### File List

**Backend (New):**
- backend/src/cards/cards.module.ts
- backend/src/cards/cards.controller.ts
- backend/src/cards/cards.service.ts
- backend/src/cards/cards.service.spec.ts
- backend/src/cards/dto/create-card.dto.ts
- backend/src/cards/dto/update-card.dto.ts

**Backend (Modified):**
- backend/src/app.module.ts (registered CardsModule)

**Frontend (New):**
- frontend/src/features/cards/card.tsx
- frontend/src/features/cards/add-card-input.tsx
- frontend/src/features/cards/add-card-input.test.tsx
- frontend/src/features/cards/use-cards.ts
- frontend/src/features/cards/cards.api.ts

**Frontend (Modified):**
- frontend/src/features/columns/column.tsx (integrated AddCardInput)
- frontend/src/features/columns/column-card-list.tsx (use Card component)
- frontend/src/index.css (added slide-up animation)