# Story 3.2: Card Editing

Status: completed

## Story

As a user,
I want to edit a card's title,
so that I can correct or update task names.

## Acceptance Criteria

1. **Given** I am viewing a card, **When** I click on the card title, **Then** the title becomes editable inline, **And** changes are saved on Enter or blur, **And** pressing Escape cancels the edit and reverts to original

## Tasks / Subtasks

- [x] Task 1: Backend card update endpoint (AC: #1)
  - [x] Subtask 1.1: Ensure PATCH endpoint exists in cards.controller.ts
  - [x] Subtask 1.2: Add update-card.dto.ts validation
  - [x] Subtask 1.3: Implement title update in cards.service.ts

- [x] Task 2: Frontend card editing UI (AC: #1)
  - [x] Subtask 2.1: Add inline edit state to card.tsx
  - [x] Subtask 2.2: Handle click to activate edit mode
  - [x] Subtask 2.3: Handle Enter/blur to save
  - [x] Subtask 2.4: Handle Escape to cancel
  - [x] Subtask 2.5: Connect to use-cards mutation

- [x] Task 3: Testing (AC: #1)
  - [x] Subtask 3.1: Write unit test for card title update
  - [x] Subtask 3.2: Write component test for inline edit
  - [x] Subtask 3.3: Verify no regressions

---

## Dev Notes

### Story Context

Epic 3, Story 2 - Card title editing. Building on Story 3.1 (Card Creation) which delivered the core card component.

**Business Value:** Users can correct typos or update task names without friction.

**Dependencies:**
- Epic 3, Story 3.1 (Card Creation) - Card component and API already exist

### Previous Story Intelligence

**From Story 3.1 (Card Creation) - COMPLETED:**
- Card component at `frontend/src/features/cards/card.tsx`
- Cards API at `frontend/src/features/cards/cards.api.ts`
- use-cards hook at `frontend/src/features/cards/use-cards.ts`
- Backend cards.controller.ts has GET/POST endpoints - need to add PATCH

**Backend Cards Module Status:**
- cards.controller.ts exists with GET/POST/DELETE (Story 3.1)
- Need to add PATCH /:id for title update

### Architecture Compliance

**Technical Stack:**
- React 18+ with Vite
- Tailwind CSS v4 via `@tailwindcss/vite`
- shadcn/ui (Radix UI primitives)
- React Query (TanStack Query) for server state
- NestJS + TypeORM + MySQL 8.x
- dnd-kit (for Epic 3, Stories 3-4)

**Naming Conventions:**
- DB: `snake_case` (e.g., `created_at`, `updated_at`)
- API: `kebab-case` endpoints (e.g., `/api/cards`)
- Code: `camelCase` for fields, `PascalCase` for components

**Module Structure:**
```
frontend/src/features/cards/
├── card.tsx                  # NEEDS inline edit
├── add-card-input.tsx        # (from 3.1)
├── use-cards.ts              # (from 3.1)
└── cards.api.ts             # (from 3.1)

backend/src/cards/
├── cards.controller.ts     # ADD PATCH/:id
├── cards.service.ts        # ADD update method
├── dto/
│   ├── create-card.dto.ts  # (from 3.1)
│   └── update-card.dto.ts  # NEEDS update validation
└── entities/
    └── card.entity.ts      # (from earlier)
```

**API Response Formats:**
```typescript
{ data: T, message?: string }
{ statusCode: number, message: string | string[], error: string }
```

**Critical Anti-Patterns:**
- ❌ NEVER open modal for simple title edit - inline only per AC
- ❌ NEVER lose focus on Enter - should blur and save
- ❌ NEVER skip API call - persist to backend
- ❌ NEVER skip optimistic UI - update immediately
- ❌ NEVER skip Escape handler - must revert

### UX Requirements

**Inline Edit Flow:**
1. Click on card title → activates edit mode
2. Title becomes `<input>` or `<textarea>`
3. Enter or blur → saves changes, exits edit mode
4. Escape → cancels edit, reverts to original
5. Empty title → validation error or prevent save

**Visual Indication:**
- Cursor changes on hover
- Title text has edit affordance (underline or color)
- Input is same typography as display

### Testing Standards

**Backend (Jest):**
- Test: Update card title with valid ID
- Test: Update card title with invalid ID (404)
- Test: Update unauthorized (401)
- Test: Empty title rejection (400)

**Frontend (Vitest + RTL):**
- Test: Click title enters edit mode
- Test: Enter saves and exits
- Test: Blur saves and exits
- Test: Escape cancels and reverts

---

## References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.2-Card-Editing]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend-Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#API-Communication-Patterns]
- [File: frontend/src/features/cards/card.tsx]
- [File: frontend/src/features/cards/use-cards.ts]
- [File: frontend/src/features/cards/cards.api.ts]
- [File: backend/src/cards/cards.controller.ts]
- [File: backend/src/cards/cards.service.ts]

---

## Dev Agent Record

### Agent Model Used

### Implementation Summary

**Task 1 (Backend):** ALREADY IMPLEMENTED - PATCH endpoint, service method, and DTO validation existed from prior work.

**Task 2 (Frontend):** IMPLEMENTED inline editing in `card.tsx`:
- Click to enter edit mode
- Enter/blur to save
- Escape to cancel
- Empty title reverts to original

**Task 3 (Tests):** CREATED component tests:
- 4 new tests in `card.test.tsx` (all passing)
- 9 existing API tests (all passing)
- TypeScript clean

### Debug Log References

### Completion Notes List

- Backend was fully implemented prior to this story
- Only Card component needed inline edit UI
- Optimistic updates via useUpdateCard hook
- Tests verify Enter, Escape, blur behaviors

### File List

- `frontend/src/features/cards/card.tsx` - MODIFIED (inline edit)
- `frontend/src/features/cards/card.test.tsx` - CREATED (4 tests)