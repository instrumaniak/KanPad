# Story 4.7: Board View Toggle

Status: ready-for-dev

## Story

As a user,
I want to switch between kanban board view and list view for a board,
so that I can use the same board for different purposes — structured workflow management or simple task tracking.

## Acceptance Criteria

1. **Given** I am viewing a board
   **When** I look at the board header
   **Then** I see a view toggle with two options: "Board" (kanban) and "List"
   **And** the current view is visually indicated
   **And** the board remembers my last selected view (persisted per board)

2. **Given** I am in "Board" (kanban) view
   **When** the board loads
   **Then** I see columns with cards that I can drag-drop between columns
   **And** cards show labels, due dates, and progress bars

3. **Given** I switch to "List" view
   **When** the view changes
   **Then** I see all cards from all columns in a single flat list
   **And** each row shows: title, column name (as a badge), labels, due date, checklist progress
   **And** the list is sortable by: created date, updated date, due date, title
   **And** I can click a card to open the detail panel (same as kanban view)
   **And** I can edit card title inline (same as kanban view)

4. **Given** I switch back to "Board" view
   **When** the view changes
   **Then** I see the standard kanban columns and cards
   **And** my previous board state is preserved (scroll position, filters, etc.)

5. **Given** I am in List view
   **When** I filter or search cards
   **Then** the list updates in real-time (same filter logic as kanban view)

6. **Given** I create a new card in List view
   **When** I click "Add Card"
   **Then** I can select which column to add it to
   **And** the card appears in the list

7. **Given** I delete a card in List view
   **When** I click delete
   **Then** the card is removed (same behavior as kanban view)

## Tasks / Subtasks

- [ ] Backend: Add `view_mode` to Board entity (AC: #1)
  - [ ] Add `@Column({ length: 10, default: 'board' }) view_mode!: 'board' | 'list'` to `backend/src/boards/entities/board.entity.ts` with `@ApiProperty({ example: 'board', enum: ['board', 'list'] })`
  - [ ] Add `view_mode?: 'board' | 'list'` to `UpdateBoardDto` with `@IsIn(['board', 'list'])` + `@IsOptional()` (import from `class-validator`)
  - [ ] Update `BoardsService.update()` in `backend/src/boards/boards.service.ts` to handle `dto.view_mode !== undefined` → `board.view_mode = dto.view_mode`
  - [ ] Update `BoardResponse` interface in `backend/src/boards/boards.controller.ts` to include `view_mode: string`
  - [ ] Include `view_mode: b.view_mode` in ALL response mappings: `findAll`, `findArchived`, `create`, `findOne`, `update`, `archive`, `restore`
  - [ ] Update `@ApiOperation({ summary: 'Update board name, color, project, or view mode' })` on `PATCH :id`
- [ ] Backend: Create migration for `view_mode` (AC: #1)
  - [ ] Create `backend/src/migrations/<timestamp>-AddViewModeToBoards.ts` following `1777411200000-AddIsArchivedToBoards.ts` pattern (hand-written raw SQL, NOT auto-generated)
  - [ ] `up`: `ALTER TABLE \`boards\` ADD \`view_mode\` varchar(10) NOT NULL DEFAULT 'board'`
  - [ ] `down`: `ALTER TABLE \`boards\` DROP COLUMN \`view_mode\``
  - [ ] Register migration in `backend/src/database/typeorm-registry.ts` (import + append to `migrations` array)
- [ ] Frontend: Update Board types + API (AC: #1)
  - [ ] Add `view_mode?: 'board' | 'list'` to `Board` interface in `frontend/src/features/boards/boards.api.ts`
  - [ ] Add `view_mode?: 'board' | 'list'` to `UpdateBoardData` in same file
  - [ ] No new endpoint needed — reuse existing `updateBoard(id, { view_mode })` PATCH
- [ ] Frontend: Create `board-view-toggle.tsx` (AC: #1)
  - [ ] Create `frontend/src/features/boards/board-view/board-view-toggle.tsx`
  - [ ] Props: `{ value: 'board' | 'list'; onChange: (v: 'board' | 'list') => void; disabled?: boolean }`
  - [ ] Use shadcn `ToggleGroup` if available, else two `Button`s in a `div` with `role="group" aria-label="Board view mode"` + `aria-pressed` per option
  - [ ] Icons: `LayoutGrid` for Board, `List` for List (lucide-react, already a dep); verify export before use — `KanbanSquare` was renamed `SquareKanban` in newer lucide versions, so prefer `LayoutGrid` to avoid build break
  - [ ] Active state: filled background (`bg-accent` / `bg-primary text-primary-foreground`), inactive: `ghost`
  - [ ] Place in `BoardView` header next to title (right side), sticky header row
- [ ] Frontend: Create `board-list-view.tsx` + row (AC: #3, #5, #6, #7)
  - [ ] Create `frontend/src/features/boards/board-view/board-list-view.tsx`
  - [ ] Props: `{ boardId: number; columns: BoardColumn[] }` — derive flat card list from existing `useColumns(boardId)` data via embedded `columns[].cards` (`GET /api/boards/:boardId/columns` returns `cards: Card[]` per `columns.api.ts`); do NOT use per-column `useCards` aggregation and do NOT add new fetch — reuse same query (no refetch on toggle). Flat list: `columns.flatMap(c => (c.cards ?? []).map(card => ({ ...card, columnName: c.name })))`
  - [ ] Each row shows: title (inline-editable), column badge (`Badge` variant `secondary`), labels via `LabelBadge` (max 3 + `+N`), due badge via `getDueDateBadge()` from `features/cards/date-utils.ts`, checklist progress via `ProgressBar` from `features/checklists/progress-bar.tsx`
  - [ ] Sort header: buttons for Created / Updated / Due / Title with `aria-sort`, asc/desc toggle, active indicator (`ArrowUp`/`ArrowDown` lucide)
  - [ ] Sort logic (client-side, `useMemo`): `created_at`, `updated_at` (date compare), `due_date` (nulls last), `title` (`localeCompare`); stable, memoized on cards + sort key
  - [ ] Click row → open `CardDetailPanel` (same component as kanban, reuse `card.tsx` pattern with `isPanelOpen` state)
  - [ ] Inline title edit: click title → `Input` with default value, `Enter`/blur → `useUpdateCard().mutate({ id, data: { title } })`, `Escape` cancels; toast on error
  - [ ] Delete: `DropdownMenu` or trash `Button` per row → `AlertDialog` confirm → `useDeleteCard()` (same undo-toast pattern as `card.tsx:65-97`)
  - [ ] "Add Card" in list: `Button` opens `Dialog` with `Input` (title) + `Select` (target column, required, default = first column by `position`) → `useCreateCard().mutate({ title, column_id })` → invalidate `['columns']` + `['cards', columnId]`; empty board still shows Add Card (board always has default columns from `BoardsService.create`)
- [ ] Frontend: Integrate toggle into `BoardView` (AC: #1, #2, #4)
  - [ ] Edit `frontend/src/features/boards/board-view/board-view.tsx`: add `const [view, setView] = useState<'board'|'list'>(board?.view_mode ?? 'board')` synced via `useEffect` when `boardResponse` loads
  - [ ] `useUpdateBoard()` mutation on toggle: optimistic `setView` immediately, PATCH `{ view_mode }` in background, rollback + error toast on failure; disable toggle while `useUpdateBoard.isPending` to prevent rapid-toggle races
  - [ ] Preserve scroll: `useRef<{ board: number; list: number }>` storing `scrollLeft`/`scrollTop` of containers on toggle, restore on switch back
  - [ ] Transition: wrap views in `div` with `transition-opacity duration-300 animate-in fade-in`
  - [ ] Kanban branch unchanged (`DragDropContext` + `Column` + `AddColumnButton`); List branch renders `<BoardListView boardId columns />`
  - [ ] Keep existing loading (`Loading...`) and not-found states intact
- [ ] Tests: Backend unit tests (AC: #1)
  - [ ] Extend `backend/src/boards/boards.service.spec.ts`: update with `view_mode: 'list'` persists; invalid value rejected at DTO level (ValidationPipe)
  - [ ] Verify persistence after PATCH with follow-up `findOne` DB query (not just response) — per project DB safety rule
  - [ ] Test default: newly created board has `view_mode === 'board'`
- [ ] Tests: Frontend unit tests (AC: #1, #3, #4)
  - [ ] `board-view-toggle.test.tsx`: renders Board/List options, indicates active, calls onChange, `aria-pressed` correct
  - [ ] `board-list-view.test.tsx`: renders flat rows from multi-column fixture, shows column badge + labels + due + progress, sorts by each key (asc/desc, nulls last for due), click opens detail panel, inline edit calls update, delete calls delete mutation
  - [ ] Update `board-view.test.tsx`: toggle renders, switching calls `updateBoard` with `{ view_mode }`, kanban preserved when switching back; mock `useUpdateBoard`
  - [ ] Vitest + @testing-library/react + userEvent, QueryClient wrapper with `retry: false`
- [ ] Tests: E2E test (AC: #1-#7)
  - [ ] Create `frontend/e2e/board-view-toggle.spec.ts` following `due-dates.spec.ts` pattern: `test.beforeAll` register user, API login, setup board+columns+cards
  - [ ] Flow: login → open board → assert kanban → toggle List → assert rows + sort each key → reload → assert List persisted (DB-backed) → toggle Board → assert kanban + scroll preserved → Add Card in List with column picker → verify appears → delete → verify removed
  - [ ] Use `monitoringTest` from `test-utils.ts`

## Dev Notes

### Architecture Compliance

- **Stack:** React 19, React Query v5, shadcn/ui (Radix + cva), Tailwind v4, Vite 8, Vitest; NestJS 11, TypeORM 0.3, MySQL 8, Fastify adapter, class-validator, Swagger
- **Naming:** DB `snake_case` (`view_mode`), DTO/frontend-type field `view_mode` to match existing `background_color`/`project_id` convention in this module (NOT camelCase — boards module uses snake_case across layers); files `kebab-case` (`board-view-toggle.tsx`, `board-list-view.tsx`); components `PascalCase`; no `any`
- **Module pattern:** Boards stay in `backend/src/boards/` (entity + dto + service + controller). No new module. Frontend stays in `frontend/src/features/boards/board-view/` — do NOT create new top-level feature folder
- **Server state:** ALL fetches via React Query (`useBoard`, `useColumns`, `useUpdateBoard`, `useCards`, `useUpdateCard`, `useDeleteCard`, `useCreateCard`) — no raw `fetch` in components
- **Swagger:** `@ApiProperty` on entity field, `@ApiOperation` update on PATCH summary

### Migration Strategy

Hand-written raw SQL only (never `synchronize`, never auto-generate). Follow `1777411200000-AddIsArchivedToBoards.ts` exactly:

```typescript
import { MigrationInterface, QueryRunner } from 'typeorm';

export class AddViewModeToBoards<timestamp> implements MigrationInterface {
  name = 'AddViewModeToBoards<timestamp>';

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE \`boards\` ADD \`view_mode\` varchar(10) NOT NULL DEFAULT 'board'`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`ALTER TABLE \`boards\` DROP COLUMN \`view_mode\``);
  }
}
```

- Timestamp prefix must be greater than `1779500000000` (latest: `CreateNotesTagsTables`). Use `Date.now()` e.g. `1779600000000-AddViewModeToBoards.ts`
- Register in `backend/src/database/typeorm-registry.ts` (import + append). Entities array unchanged (Board already registered)
- `down` is lossless (drops only the added column)
- Test migration locally: `npm run migration:run` before committing; commit entity + dto + migration together

### Database Schema

`boards` table addition:

| Column | Type | Constraints |
|--------|------|-------------|
| view_mode | varchar(10) | NOT NULL DEFAULT 'board' |

- App-level allowed values: `'board' \| 'list'` enforced by `UpdateBoardDto` `@IsIn`. No MySQL ENUM (keeps migration simple + portable, consistent with `background_color` varchar approach)
- Existing rows backfill automatically via DEFAULT

### API Endpoints

No new endpoint. Extend existing:

- `PATCH /api/boards/:id` — body now accepts `{ name?, background_color?, project_id?, view_mode?: 'board'|'list' }` → returns full `BoardResponse` including `view_mode`
- `GET /api/boards/:id`, `GET /api/boards`, `GET /api/boards/archived`, `POST /api/boards` — all include `view_mode` in response (update every mapper in `boards.controller.ts`, else frontend gets `undefined` on first load)
- Validation: `ValidationPipe({ transform: true })` already on PATCH — invalid `view_mode` returns 400, never raw DB error

### Frontend Components

**BoardViewToggle (`board-view/board-view-toggle.tsx`):**
```typescript
interface BoardViewToggleProps {
  value: 'board' | 'list';
  onChange: (v: 'board' | 'list') => void;
  disabled?: boolean;
}
// role="group" aria-label="Board view mode", per-button aria-pressed
```

**BoardListView (`board-view/board-list-view.tsx`):**
```typescript
interface BoardListViewProps {
  boardId: number;
  columns: BoardColumn[]; // from useColumns(boardId), each with cards
}
type SortKey = 'created_at' | 'updated_at' | 'due_date' | 'title';
// Local state: sortKey, sortDir: 'asc'|'desc', editingCardId, detailCardId
```

- Reuse, do NOT reinvent: `CardDetailPanel` (`features/cards/card-detail-panel.tsx`), `LabelBadge` (`features/labels/label-badge.tsx`), `getDueDateBadge` (`features/cards/date-utils.ts`), `ProgressBar` (`features/checklists/progress-bar.tsx`), `Badge/Input/Button/Dialog/Select/DropdownMenu/AlertDialog/Toast` from `components/ui/`
- Flat list: `columns.flatMap(c => (c.cards ?? []).map(card => ({ ...card, columnName: c.name })))` — `Column.cards` is embedded (see `frontend/src/features/columns/columns.api.ts:21-29`), no per-column aggregation needed
- List is a pure renderer of the `columns` prop: it applies no independent filtering — current `BoardView` has no filter/search props so AC5 passes vacuously, and future Epic 5 filters applied to `columns` will automatically affect both kanban and list views
- No `DragDropContext` in list branch (flat list is not draggable — spec requires sort, not drag)
- Add Card dialog `Select` lists `columns.map(c => ({ value: String(c.id), label: c.name }))`

### React Query Key Convention

```typescript
// boards (existing in use-boards.ts)
['boards', projectId] / ['board', id] / ['archivedBoards']
// reuse useUpdateBoard() — already invalidates ['boards'] + ['board', id]
// cards (existing in use-cards.ts)
['cards', columnId] / ['card', cardId] / ['columns']
```

- After `view_mode` PATCH: existing `useUpdateBoard.onSuccess` invalidation is sufficient; do NOT add custom cache writes except optimistic `setView` local state (rollback on error)
- After list-view card create/update/delete: existing `useCreateCard`/`useUpdateCard`/`useDeleteCard` invalidations cover it; no new keys

### Previous Story Intelligence

- **4-6 Notes System:** established hand-written migration + `typeorm-registry.ts` pattern; per-feature frontend folders; co-located `.spec.ts`/`.test.tsx`; board sidebar collapse pattern (relevant for scroll-preservation approach); E2E `login → create → verify → reload → verify persistence` flow to copy
- **4-5 Checklists:** `ProgressBar` component API (`completed/total/className`) + `checklist_progress` on Card type — reuse directly in list rows
- **4-4 Due Dates:** `getDueDateBadge(due_date)` memo pattern + `CardDetailPanel` `<Separator/>` section pattern — reuse for due display
- **4-3 Labels:** `LabelBadge` + max-3 + `+N` overflow pattern from `card-preview.tsx:26-37` — copy into list rows
- **4-1 Card Detail Panel:** `CardDetailPanel card open onOpenChange` pattern from `card.tsx:148` — reuse for list row click
- **2-5 Board View Layout:** `board-view.tsx` scroll container (`overflow-x-auto scroll-smooth touch-pan-x`), header-above-scroll structure (header stays fixed), `min-w-[320px]` columns — do NOT break; list branch uses vertical scroll container instead
- **3-3/3-4 Drag-Drop:** `DragDropContext boardId` + `useMoveCard`/`useReorderCard` optimistic patterns — kanban branch untouched; list branch must NOT mount `DragDropContext`

### Git Intelligence

- Recent: `a6e2c35` merge, `7bccf15` note `mediumtext` fix + breaking-change discipline, frontend build optimization
- Stack confirmed in tree: React 19 + Vite 8 + Tailwind v4 + shadcn/Radix + TanStack Query v5 + Vitest/Testing Library + Playwright; NestJS 11 + TypeORM 0.3 + MySQL 8 + Fastify + class-validator + bcryptjs
- Aliases: `@/` → `src/` (Vite + tsconfig); E2E in `frontend/e2e/*.spec.ts` with `test-utils.ts` (`monitoringTest`)
- Convention: co-located tests, conventional commits (`feat(boards): ...`)

### Testing Patterns

**Backend** (`boards.service.spec.ts`, `boards.controller.spec.ts`, mock repositories):
- update `view_mode` board→list→board; default `board` on create; invalid enum → 400; cross-user `findOne` still throws `NotFoundException`
- **Persistence check (mandatory):** after every PATCH, follow-up `findOne` / `GET /api/boards/:id` and assert `view_mode` — never assert response body alone

**Frontend** (Vitest, `retry: false` QueryClient wrapper, mock `useBoard`/`useColumns`/`useUpdateBoard` like `board-view.test.tsx`):
- Toggle: active indication, `aria-pressed`, `onChange` payload
- List: multi-column fixture → flat rows; each sort key asc/desc; `due_date: null` sorts last; column badge text; `+N` labels; progress `completed/total (percent%)`; row click opens panel; inline edit Enter commits / Escape cancels; delete confirm flow; Add Card dialog requires column selection
- BoardView integration: initial view from `board.view_mode`, fallback `board` when undefined (backward compat with unmigrated fixture); toggle triggers PATCH; failure rolls back + toast

**E2E** (`frontend/e2e/board-view-toggle.spec.ts`): register → API seed (board + 2 columns + cards with labels/due/checklists) → UI toggle/sort/reload-persist/Add/delete — full journey with reload persistence assertions

### Accessibility

- Toggle: `role="group"` + `aria-label="Board view mode"`, each option `aria-pressed`, keyboard focusable, visible focus ring
- List: table semantics (`role="table"`/`row`/`columnheader` or native `table`) with `aria-sort` on sortable headers; sort buttons keyboard-operable
- Rows: `role="button"` + `tabIndex={0}` + `aria-label="Open card details: {title}"`, Enter/Space opens (mirror `card.tsx:58-63`)
- Inline edit: `aria-label="Edit card title"`, Escape announces cancel
- Delete: `AlertDialog` with title + description naming the card
- Reduced motion: `motion-safe:` prefix on fade transition or `@media (prefers-reduced-motion)` fallback

### Edge Cases

- **Unmigrated / undefined `view_mode`:** default to `'board'` client-side (`?? 'board'`) so old fixtures/rows never crash
- **Empty board:** list shows empty state ("No cards yet") + working Add Card; sort header disabled when zero rows
- **Null `due_date` in sort:** always last regardless of direction
- **PATCH race (rapid toggling):** disable toggle while `useUpdateBoard.isPending` OR last-write-wins with rollback only for failed request
- **Offline / PATCH failure:** rollback to previous view + error toast with friendly message (no raw error), via `onError` handler
- **Filter/search (Epic 5 future):** list must consume the SAME filtered card array as kanban — implement list as pure renderer of `columns` prop so future filters apply to both automatically
- **Performance:** `useMemo` for flat + sorted arrays; large boards (100+ cards) render without virtualization for MVP, note follow-up if slow
- **Board not found / loading:** keep existing states; toggle hidden until `board` loads

## References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.7: Board View Toggle]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Board View Toggle (Kanban ↔ List)]
- [Source: _bmad-output/planning-artifacts/architecture.md#Pattern Examples / Frontend Organization]
- [Source: _bmad-output/project-context.md#Notes System Rules — Board view toggle preserves scroll position and filters]
- [Source: backend/src/boards/entities/board.entity.ts — no view_mode, add here]
- [Source: backend/src/boards/dto/update-board.dto.ts — add view_mode with IsIn]
- [Source: backend/src/boards/boards.service.ts#update — extend here]
- [Source: backend/src/boards/boards.controller.ts — extend BoardResponse + all mappers]
- [Source: backend/src/migrations/1777411200000-AddIsArchivedToBoards.ts — migration template]
- [Source: backend/src/database/typeorm-registry.ts — register migration here]
- [Source: frontend/src/features/boards/board-view/board-view.tsx — integrate toggle here]
- [Source: frontend/src/features/boards/boards.api.ts — extend Board + UpdateBoardData]
- [Source: frontend/src/features/boards/use-boards.ts — reuse useUpdateBoard]
- [Source: frontend/src/features/cards/card-preview.tsx — LabelBadge + due + ProgressBar reuse pattern]
- [Source: frontend/src/features/cards/card.tsx — detail panel + delete undo-toast pattern]
- [Source: frontend/src/features/cards/date-utils.ts — getDueDateBadge]
- [Source: _bmad-output/implementation-artifacts/4-6-notes-system.md — migration/registry/react-query/E2E patterns]
- [Source: _bmad-output/implementation-artifacts/2-5-board-view-layout.md — scroll/header constraints]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List
