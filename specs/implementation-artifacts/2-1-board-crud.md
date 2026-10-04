# Story 2.1: Board CRUD

Status: done (after code review)

## Story

As a user,
I want to create, view, edit, and delete boards,
so that I can organize different workflows for different purposes.

## Acceptance Criteria

1. **Given** I am on the boards page (homepage), **When** I click "Create Board", **Then** a modal form appears with board name, background color picker, and optional project selector
2. **And** the color picker shows 8 preset colors
3. **And** the project selector shows my existing projects (or "No project" as default)
4. **And** submitting creates the board with default columns: "To Do", "In Progress", "Done"
5. **And** I am navigated to the new board view
6. **Given** I have boards, **When** I view the boards page, **Then** I see all my boards listed with name, background color preview, and project label (if assigned)
7. **And** boards are sorted by most recently updated
8. **Given** I want to rename a board, **When** I click edit on the board card or header, **Then** I can modify the name inline
9. **And** changes are saved on Enter or blur
10. **Given** I want to change board background, **When** I click the color picker in board settings, **Then** I can select a new color
11. **And** the board background updates immediately (optimistic UI)
12. **Given** I want to assign a board to a project (or remove from project), **When** I open board settings, **Then** I can select a project or choose "No project"
13. **And** the change saves immediately
14. **Given** I want to delete a board, **When** I click delete and confirm, **Then** the board and all its columns/cards are deleted (cascade)
15. **And** a success toast appears with undo option (5 seconds)

## Tasks / Subtasks

- [x] Task 1: Create Board entity and database migration (AC: #1, #2, #3, #4)
  - [x] Subtask 1.1: Create `backend/src/boards/entities/board.entity.ts` with `user_id` (NOT NULL) and `project_id` (NULLable)
  - [x] Subtask 1.2: Create `backend/src/columns/entities/column.entity.ts` with Board relationship
  - [x] Subtask 1.3: Generate and run migration for boards and columns tables
- [x] Task 2: Create Boards backend module (AC: #1, #6, #7)
  - [x] Subtask 2.1: Create `backend/src/boards/boards.module.ts`
  - [x] Subtask 2.2: Create `backend/src/boards/boards.service.ts` with CRUD methods
  - [x] Subtask 2.3: Create `backend/src/boards/boards.controller.ts` with REST endpoints
- [x] Task 3: Implement default columns creation (AC: #3)
  - [x] Subtask 3.1: On board creation, automatically create "To Do", "In Progress", "Done" columns
- [x] Task 4: Create frontend board feature (AC: #1, #2, #3, #6, #7)
  - [x] Subtask 4.1: Create board list component (homepage)
  - [x] Subtask 4.2: Create board creation modal with color picker and optional project selector
  - [x] Subtask 4.3: Create board card component with color preview and project label
  - [x] Subtask 4.4: Handle redirect to board view after creation
- [x] Task 5: Implement board editing (AC: #8, #9, #10, #11, #12, #13)
  - [x] Subtask 5.1: Inline edit for board name
  - [x] Subtask 5.2: Color picker for background change
  - [x] Subtask 5.3: Project assignment in board settings
- [x] Task 6: Implement board deletion (AC: #14, #15)
  - [x] Subtask 6.1: Delete confirmation dialog
  - [x] Subtask 6.2: Cascade delete columns and cards
  - [x] Subtask 6.3: Toast with undo option
- [x] Task 7: Add tests (AC: all)
  - [x] Subtask 7.1: Backend unit tests for boards service
  - [x] Subtask 7.2: Backend unit tests for boards controller
  - [x] Subtask 7.3: Frontend tests for board components

## Fix Log (Post-Implementation)

### Fix 1: Navigation to board view after creation (AC #5)
- **Issue**: After creating a board, user was not navigated to the new board view
- **Root Cause**: CreateBoardModal missing navigation call after successful creation
- **Fix**: Added `useNavigate` hook and `navigate('/board/${response.data.id}')` after board creation success
- **Files Changed**: `frontend/src/features/boards/create-board-modal.tsx`

### Fix 2: BoardCard click behavior (AC #5)
- **Issue**: Clicking board card navigated to edit mode instead of board view
- **Root Cause**: BoardCard onClick handler called `onEdit` instead of router navigation
- **Fix**: Added `useNavigate` to BoardCard, changed onClick to `navigate(\`/board/${board.id}\`)`
- **Files Changed**: `frontend/src/features/boards/board-card.tsx`

### Test Updates
- Added MemoryRouter wrapper to board-card.test.tsx and create-board-modal.test.tsx
- Updated BoardCard test to verify navigation on click

## Dev Notes

### Access Control

Boards are owned directly by users via `user_id`. Users can only see/access their own boards. Optional project grouping does not affect access control.

**Authorization flow:**
1. User requests boards (optionally filtered by `projectId`)
2. Backend verifies `board.user_id === session.userId`
3. If `projectId` filter provided, also verify `project.user_id === session.userId`
4. If not authorized → `ForbiddenException`

### Navigation Note

AC#5: "I am navigated to the new board view"

This is a placeholder for Story 2.5 (Board View Layout). For now, redirect to the boards page or show a simple board placeholder. Full board view with columns comes later.

### Previous Story Intelligence

**From Story 1.9 (CLI Superadmin Creation):**
- Board and Column entities follow the same patterns as User and Project entities
- Use TypeORM with `synchronize: false` — migrations only
- Strict TypeScript with `!` definite assignment for entity properties
- Tests co-located: `.spec.ts` (backend), `.test.tsx` (frontend)

**From Story 1.7 (Project CRUD):**
- Project entity exists at `backend/src/projects/entities/project.entity.ts` with OneToMany to boards (to be added)
- Frontend uses shadcn/ui components, never modified directly
- Feature-based organization: `frontend/src/features/boards/`

### Architecture Compliance

**Entity Patterns:**

Board entity must have:
- id (PK auto-increment)
- name (string)
- background_color (string - hex code)
- user_id (FK to users, NOT NULL)
- project_id (FK to projects, NULLable)
- created_at, updated_at timestamps

Column entity must have:
- id (PK auto-increment)
- name (string)
- position (integer - for ordering)
- board_id (FK to boards)
- created_at, updated_at timestamps

**API Endpoints:**
- `POST /api/boards` - Create board with default columns (optional `project_id` in body)
- `GET /api/boards` - List all user's boards (optional `?projectId=` query param)
- `GET /api/boards/:id` - Get single board
- `PATCH /api/boards/:id` - Update board name/color/project
- `DELETE /api/boards/:id` - Delete board (cascade to columns/cards)

**Response Format:**
```typescript
// Success
{ data: Board, message?: string }

// List
{ data: Board[], message?: string }

// Error
{ statusCode: number, message: string | string[], error: string }
```

### API Response Examples

**POST /api/boards** (Create)
```typescript
// Request (project_id is optional)
{ name: "My Board", background_color: "#0079BF" }
// or with project
{ name: "My Board", background_color: "#0079BF", project_id: 1 }

// Response (201)
{
  data: {
    id: 1,
    name: "My Board",
    project_id: null,
    background_color: "#0079BF",
    created_at: "2026-04-05T10:00:00Z",
    updated_at: "2026-04-05T10:00:00Z",
    columns: [
      { id: 1, name: "To Do", position: 0, board_id: 1 },
      { id: 2, name: "In Progress", position: 1, board_id: 1 },
      { id: 3, name: "Done", position: 2, board_id: 1 }
    ]
  },
  message: "Board created successfully"
}
```

**GET /api/boards** (List)
```typescript
// Response (200)
{
  data: [
    { id: 1, name: "My Board", project_id: null, background_color: "#0079BF", ... },
    { id: 2, name: "Another Board", project_id: 1, background_color: "#D29034", ... }
  ]
}
```

**PATCH /api/boards/:id** (Update)
```typescript
// Request
{ name: "Updated Board Name" } // or { background_color: "#FFAB00" }

// Response (200)
{
  data: { id: 1, name: "Updated Board Name", background_color: "#0079BF", ... },
  message: "Board updated"
}
```

**DELETE /api/boards/:id** (Delete)
```typescript
// Response (200)
{ message: "Board deleted" }

// Error (403 - unauthorized)
{ statusCode: 403, message: "Access denied", error: "Forbidden" }

// Error (404 - not found)
{ statusCode: 404, message: "Board not found", error: "Not Found" }
```

### Critical Anti-Patterns

- ❌ NEVER set `synchronize: true` in TypeORM
- ❌ NEVER return raw database errors to frontend — use NestJS exceptions
- ❌ NEVER use `any` type — strict TypeScript
- ❌ NEVER skip cascade delete handling for boards → columns → cards
- ❌ NEVER create board without default columns
- ❌ NEVER require a project to create a board — `project_id` must be NULLable
- ❌ NEVER cascade delete boards when a project is deleted — set `project_id` to NULL instead

### Default Columns

When creating a board, automatically create these columns in order:
1. "To Do" (position: 0)
2. "In Progress" (position: 1)
3. "Done" (position: 2)

### Color Picker Options

8 preset colors for board background:
- #0079BF (Trello Blue)
- #D29034 (Orange)
- #519839 (Green)
- #B61C26 (Red)
- #F5D6CC (Peach)
- #C0B6F2 (Purple)
- #FFAB00 (Yellow)
- #838C91 (Gray)

### Project Structure After This Story

```
backend/src/
├── boards/
│   ├── boards.module.ts
│   ├── boards.service.ts
│   ├── boards.controller.ts
│   ├── boards.service.spec.ts
│   ├── boards.controller.spec.ts
│   └── entities/
│       └── board.entity.ts
├── columns/
│   └── entities/
│       └── column.entity.ts

frontend/src/features/boards/
├── board-list.tsx          (homepage board list)
├── board-card.tsx          (board preview card with color + project label)
├── create-board-modal.tsx  (modal with name, color, optional project)
├── edit-board-form.tsx
├── board-settings.tsx      (includes project assignment)
├── board.api.ts
└── use-boards.ts
```

### Testing Standards Summary

- Backend: Jest with `describe`/`it` blocks
- Frontend: Vitest with React Testing Library
- Test coverage: CRUD operations, validation, error handling

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-2.1-Board-CRUD]
- [Source: _bmad-output/planning-artifacts/architecture.md#Database-Naming-Conventions]
- [Source: _bmad-output/planning-artifacts/architecture.md#Implementation-Patterns-Naming-Patterns]
- [Source: _bmad-output/planning-artifacts/architecture.md#API-Response-Format]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Board-component]
- [Source: _bmad-output/implementation-artifacts/1-7-project-crud.md]

## Dev Agent Record

### Agent Model Used

minimax-m2.5-free

### Debug Log References

- Migration: 1745862000000-CreateBoardsColumns.ts

### Completion Notes List

- Created Board entity with user_id and optional project_id relationships
- Created BoardColumn entity with position for ordering
- Implemented full CRUD API: POST/GET/PATCH/DELETE /api/boards
- Default columns ("To Do", "In Progress", "Done") auto-created on board creation
- Frontend: Board list, board card, create modal, inline edit, delete dialog
- 8 preset colors for board backgrounds
- Project assignment optional for boards
- Cascading delete for board → columns (cards come later in Story 3)

### File List

backend/src/boards/entities/board.entity.ts
backend/src/boards/boards.module.ts
backend/src/boards/boards.service.ts
backend/src/boards/boards.controller.ts
backend/src/boards/boards.service.spec.ts
backend/src/boards/boards.controller.spec.ts
backend/src/boards/dto/create-board.dto.ts
backend/src/boards/dto/update-board.dto.ts
backend/src/columns/entities/column.entity.ts
backend/src/migrations/1745862000000-CreateBoardsColumns.ts
backend/src/app.module.ts (BoardsModule import)
backend/src/users/entities/user.entity.ts (updated imports)
backend/src/projects/entities/project.entity.ts (updated imports)
frontend/src/features/boards/boards.api.ts
frontend/src/features/boards/use-boards.ts
frontend/src/features/boards/board-list.tsx
frontend/src/features/boards/board-card.tsx
frontend/src/features/boards/create-board-modal.tsx
frontend/src/App.tsx (updated routes)