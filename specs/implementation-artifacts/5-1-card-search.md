# Story 5.1: Card Search

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a user,
I want to search for cards by title,
so that I can quickly find specific tasks on a busy board.

## Acceptance Criteria

1. **Given** I am viewing a board
   **When** I look at the board header
   **Then** I see a search input field

2. **Given** I type in the search field
   **When** I enter at least 2 characters
   **Then** cards are filtered in real-time (debounced 300ms)
   **And** only cards matching the search term in their title are visible
   **And** non-matching cards are hidden (not removed)

3. **Given** I clear the search field
   **When** the field is empty
   **Then** all cards become visible again

4. **Given** no cards match my search
   **When** the filter results are empty
   **Then** I see "No cards found" with a "Clear search" button

## Tasks / Subtasks

- [x] Frontend: Create `filter-cards-by-title.ts` util (AC: #2)
  - [x] Create `frontend/src/features/boards/board-view/filter-cards-by-title.ts`
  - [x] `import type { Column } from '../../columns/columns.api'` — use real `Column`/`Card` types as sole data source; do NOT invent a generic `{title: string}` structural type
  - [x] Export `matchesTitle(cardTitle: string, query: string): boolean` — trim, `toLowerCase()`, `includes()`; empty query → `true`
  - [x] Export `filterColumnsByTitle(columns: Column[], query: string): Column[]` — returns new column array with `cards` filtered; preserves column object identity for untouched columns where possible; never mutates input
  - [x] Threshold single-source-of-truth: util is defensive (returns input unchanged if `query.trim().length < 2`); caller gates with `isSearching` — util never throws on short query
  - [x] No `any` — strict TypeScript
- [x] Frontend: Create `card-search-input.tsx` (AC: #1, #2, #3)
  - [x] Create `frontend/src/features/boards/board-view/card-search-input.tsx`
  - [x] Props: `{ value: string; onChange: (v: string) => void; onClear: () => void }` — no count props (`aria-live` counter in `BoardView` is the single source for totals)
  - [x] Use shadcn `Input` from `@/components/ui/input` — do NOT build custom input
  - [x] Layout: `relative` wrapper, `Search` lucide icon absolute left (`left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground`), `Input` with `pl-8 pr-11 h-9 w-[140px] sm:w-[240px]` (base `w-[140px]` mobile-first; `sm:w-[240px]` expands — clarified), clear `Button variant="ghost" size="icon"` absolute right (`absolute right-1 top-1/2 -translate-y-1/2`) when `value` non-empty (`aria-label="Clear search"`); hit-slop for 44px touch target: `min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0` (or `p-2 -m-2` pattern); visual `X` icon stays `h-4 w-4`
  - [x] A11y: `role="search"`, `Input` has `aria-label="Search cards"`, `placeholder="Search cards..."`, `type="search"`; clear button keyboard-focusable; `Escape` key clears (call `onClear`); `onClear` must be idempotent (clearTimeout + reset both states — safe to call twice, covers WebKit native `type=search` Escape double-fire; do NOT add `preventDefault` hacks)
  - [x] Controlled component only — NO internal debounce here (debounce lives in `BoardView` mirroring `note-list.tsx:41-49` timer shape: `useRef` + `useCallback` + 300ms); `onChange` fires synchronously on every keystroke
- [x] Frontend: Integrate search into `BoardView` (AC: #1-#4)
  - [x] Edit `frontend/src/features/boards/board-view/board-view.tsx`: add `const [search, setSearch] = useState('')` + `const [debouncedSearch, setDebouncedSearch] = useState('')` + `searchTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)`
  - [x] Copy debounce from `frontend/src/features/notes/note-list.tsx:41-49` timer shape (`useRef` + `useCallback` + 300ms): `debounceSearch` with `clearTimeout` + `setTimeout(..., 300)` → `setDebouncedSearch(value)`; ADD unmount cleanup the source omits: `useEffect(() => () => clearTimeout(searchTimerRef.current), [])`
  - [x] Handler: `handleSearchChange(v: string) { setSearch(v); debounceSearch(v); }`; `handleClearSearch` wrapped in `useCallback` (idempotent: clearTimeout + reset both states)
  - [x] Reset search on board change — inline body to satisfy `exhaustive-deps` (do NOT call handler): `useEffect(() => { clearTimeout(searchTimerRef.current); setSearch(''); setDebouncedSearch(''); }, [id])` — prevents stale query leaking between boards
  - [x] Derive `activeQuery = debouncedSearch.trim()`; `isSearching = activeQuery.length >= 2`
  - [x] Derive `filteredColumns = useMemo(() => isSearching ? filterColumnsByTitle(columns ?? [], activeQuery) : (columns ?? []), [columns, activeQuery, isSearching])`
  - [x] Header layout: place `<CardSearchInput>` in header row between title and toggle: `<div className="ml-auto flex items-center gap-2">` containing search + existing `BoardViewToggle`; on mobile (`<sm`) search shrinks (`w-[140px]`) — never push toggle off-screen
  - [x] Kanban branch: render `filteredColumns.map(...)` instead of `columns.map(...)`; pass `filteredColumns` to `Column` (keep `allColumns={columns ?? []}` unfiltered so move-all-cards column picker + `column.tsx:17` next-column logic still list all columns); `ColumnHeader` count badge shows filtered count — intended (reinforces feedback); do NOT invent `X of Y` dual-count in header (board-level `aria-live` covers totals)
  - [x] List branch: render `<BoardListView boardId columns={filteredColumns} />` — list is pure renderer so it automatically respects search; do NOT add filtering inside `BoardListView`; empty columns MUST stay in the array or the List Add dialog (`board-list-view.tsx:66-69` derives picker from `columns` prop) loses targets
  - [x] Preserve search across view toggle — `search` state lives above both branches, untouched by `handleViewChange`; existing `scrollRef` preservation stays intact
  - [x] Empty state: compute search-empty ONLY after `board-view.tsx:99-113` loading/not-found gates pass; when `isSearching && filteredColumns.every(c => (c.cards ?? []).length === 0)` → render board-level empty block (both views): headline "No cards found", `Button variant="secondary"` "Clear search" → `handleClearSearch()`; intercept BEFORE `<BoardListView/>` (search-empty takes precedence over its "No cards yet" — do NOT edit list component); in kanban hide column card lists but keep column headers + Add Card visible; creating a card while filtered needs no special handling (toast still fires, card appears after Clear)
  - [x] Results announcement: `<p aria-live="polite" className="sr-only">{isSearching ? `${visibleCount} of ${totalCount} cards` : ''}</p>` where `visibleCount = filteredColumns.flatMap(c => c.cards ?? []).length`
- [x] Tests: Frontend unit tests (AC: #1-#4)
  - [x] `filter-cards-by-title.test.ts` (co-located): empty query pass-through; 1-char pass-through (caller rule); case-insensitive match; trim handling; non-match hidden; original array not mutated; column with zero cards preserved
  - [x] `card-search-input.test.tsx`: renders search input with placeholder; shows clear button only when value non-empty; `onChange` fires on type; `onClear` on X click and on Escape; `aria-label="Search cards"` present; `role="search"` present
  - [x] Update `board-view.test.tsx`: search input renders in header; FIRST upgrade mocks (existing `Column` mock ignores `cards`, `BoardListView` mock ignores `columns` prop — tests would fail otherwise): `Column` mock renders `column.cards.map(c => c.title)` + accepts `allColumns`, `BoardListView` mock renders `( {columns} )` flat titles; then assert typing ≥2 chars filters, 1-char shows all, clear restores all; "No cards found" + Clear search button appears on zero matches and Clear resets; search preserved when toggling board/list view; search resets on boardId change (remount with different param); debounce 300ms (use fake timers); prefer real `CardSearchInput` with mocked `useColumns` data like existing tests
  - [x] Vitest + @testing-library/react + userEvent, QueryClient wrapper with `retry: false` (follow `board-view.test.tsx` mock pattern: mock `useBoard`/`useColumns`/`useUpdateBoard`/`useCreateColumn`, mock `Column` + `BoardListView` where appropriate)
- [x] Tests: E2E test (AC: #1-#4)
  - [x] Create `frontend/e2e/card-search.spec.ts` following `board-view-toggle.spec.ts` pattern: `import { monitoringTest } from './test-utils'`; `beforeAll` register user; API login with cookie auth; seed board + 2 columns + cards via `request.post('/api/cards', { data: { title, column_id, position: 0 }, headers: { Cookie } })` ("Alpha task", "Beta task", "Zulu task")
  - [x] Flow (use explicit `fill`, never `type` — `type` appends): login → open board → assert search input visible → `fill('Al')` → assert only "Alpha task" visible → `fill('A')` (1 char) → assert all 3 visible → `fill('')` → assert all visible → `fill('zzz-no-match')` → assert "No cards found" + click "Clear search" → assert all visible → toggle List view → `fill('Beta')` → assert list shows only Beta row → `fill('')` clear
  - [x] Use `monitoringTest` from `./test-utils`; debounce-aware waits with minimum `toBeVisible({ timeout: 5000 })` (absorbs 300ms debounce), no fixed sleeps

## Dev Notes

### Architecture Compliance

- **Stack:** React 19, React Query v5, shadcn/ui (Radix + cva), Tailwind v4, Vite 8, Vitest; no backend change (client-side filter over existing `GET /api/boards/:boardId/columns` data)
- **Naming:** files `kebab-case` (`card-search-input.tsx`, `filter-cards-by-title.ts`); components `PascalCase` (`CardSearchInput`); functions `camelCase` (`matchesTitle`, `filterColumnsByTitle`); no `any`
- **Module pattern:** Search lives in `frontend/src/features/boards/board-view/` — do NOT create `features/search/` yet (architecture lists it as future home for combined search+filter; 5-2 will promote/merge — note this in code comment to prevent a second divergent search implementation)
- **Server state:** NO new fetch, NO new React Query key — reuse existing `useColumns(boardId)` `['columns', boardId]` data; filter with `useMemo` only. No raw `fetch` in components.
- **shadcn:** `Input` from `@/components/ui/input`, `Button` from `@/components/ui/button` — never modify `components/ui/` directly

### Migration Strategy

No migration. No entity change. No backend change. Pure frontend story.

### Database Schema

No change.

### API Endpoints

No new endpoint. No contract change. Existing `GET /api/boards/:boardId/columns` (returns `Column[]` with embedded `cards: Card[]` per `columns.api.ts:21-29`) is the sole data source.

### Frontend Components

**CardSearchInput (`board-view/card-search-input.tsx`):** see Tasks for exact props (no drift — `value`/`onChange`/`onClear` only, no count props).

**Filter util (`board-view/filter-cards-by-title.ts`):** see Tasks for exact signatures (no drift — real `Column` type, defensive `<2` pass-through).

**BoardView integration:**
- State above both view branches: `search` (immediate) + `debouncedSearch` (300ms) — see Tasks for exact timer shape
- `filteredColumns` via `useMemo` on `[columns, activeQuery, isSearching]` — single derivation point where 5-2 label/due/checklist filters will compose later (document with code comment: `// 5-2: compose filterColumnsByFilters here`)
- Kanban: `filteredColumns.map(...)`; `allColumns` prop stays unfiltered
- List: `<BoardListView columns={filteredColumns} />`
- Search-empty block takes precedence over list's own "No cards yet"
- MVP exception to architecture "Card endpoint query params" mapping: client-side `useMemo` filter for FR34 title-only (small boards, debounced, no new key); server query params deferred to 5-2 if boards grow

### React Query Key Convention

```typescript
// reuse only:
['columns', boardId]  // via useColumns(boardId) — sole source
['board', id]         // unchanged
```
- No invalidation added. No new keys. Filtering is derived client state, not server state.

### Previous Story Intelligence

- **4-7 Board View Toggle:** `board-view.tsx` header (`flex shrink-0 items-center gap-4 px-6 py-4`, toggle in `ml-auto` div), scroll-preservation via `scrollRef` + `boardScrollRef`/`listScrollRef`, `BoardListView` is pure renderer of `columns` prop with `columns.flatMap(c => (c.cards ?? []).map(...))` — search MUST feed the same `columns` prop so both views filter identically (4-7 AC5 explicitly requires this). Keep `?? 'board'` fallback, `isPending` toggle guard, `transition-opacity` wrapper untouched.
- **4-7 list sort:** `board-list-sort.ts` extracted for `react-refresh` lint rule — follow same precedent: pure logic in `filter-cards-by-title.ts`, component in `card-search-input.tsx`.
- **4-6 Notes System:** debounce precedent (`note-list.tsx:41-49` — `useRef` timer + `useCallback`, 300ms) — mirror shape, ADD unmount cleanup the source omits. E2E `login → create → verify → reload → verify` flow to mirror.
- **4-3 Labels / 4-5 Checklists / 4-4 Due Dates:** list rows render `LabelBadge`, `getDueDateBadge`, `ProgressBar` — search does NOT touch these; title-only matching keeps 5-1 scope tight (labels/due/checklist filtering is 5-2).
- **2-5 Board View Layout:** header-above-scroll structure, `min-w-[320px]` columns — do NOT break; search input must not overflow header on mobile.
- **3-3/3-4 Drag-Drop:** `DragDropContext boardId` wraps kanban only — filtered (hidden) cards stay mounted nowhere (excluded from array), so drag operates only on visible cards; no DnD change needed.

### Git Intelligence

- Recent: `65929fd` dev Story 4.7, `83f53b9` Create Story 4.7, merge `a6e2c35`; stack in tree: React 19 + Vite 8 + Tailwind v4 + shadcn/Radix + TanStack Query v5 + Vitest/Testing Library + Playwright; NestJS 11 + TypeORM 0.3 + MySQL 8
- Aliases: `@/` → `src/`; E2E in `frontend/e2e/*.spec.ts` with `./test-utils` (`monitoringTest`)
- Convention: co-located tests, conventional commits (`feat(boards): ...`)

### Testing Patterns

**Unit** (Vitest, `retry: false` QueryClient wrapper, mock `useBoard`/`useColumns` like `board-view.test.tsx`):
- Util: pass-through, case-insensitive, trim, no-mutation, empty-column safety
- Input: placeholder, clear visibility, onChange/onClear, Escape, a11y roles
- BoardView: ≥2 filters, 1-char shows all, clear restores, empty block + Clear button, debounce with fake timers (`vi.useFakeTimers` + `advanceTimersByTime(300)`), search preserved across view toggle, reset on board change
- **Persistence check N/A** (no backend write — client state only; explicitly note why DB rule doesn't apply)

**E2E** (`frontend/e2e/card-search.spec.ts`): register → API seed (board + 2 cols + 3 cards) → UI type/filter/clear/empty/clear → toggle List → filter → clear. Debounce-aware assertions, no hard sleeps.

### Accessibility

- Wrapper `role="search"`; input `aria-label="Search cards"` + `placeholder="Search cards..."`
- Clear button `aria-label="Clear search"`, keyboard-focusable, visible focus ring, 44px touch hit-area via `min-h/min-w` or `p-2 -m-2` hit-slop (visual icon stays `h-4 w-4`)
- `Escape` clears input and returns focus to input (`onClear` idempotent — safe on double-fire)
- Results announced via `aria-live="polite"` sr-only counter (`X of Y cards`); empty block uses heading + descriptive text, Clear button focusable
- Touch target: clear button 44px hit-area (see Tasks); input `h-9` text field acceptable alongside it
- Reduced motion: no animation added — none needed

### Edge Cases

- **1-char input:** shows ALL cards (threshold ≥2 per AC) — not an error, intentional
- **Whitespace-only / leading-trailing spaces:** trimmed before match and before threshold check
- **Case:** case-insensitive (`toLowerCase` both sides); Unicode titles work via `includes`
- **Empty board + search:** search input still renders; typing shows "No cards found" + Clear (not "No cards yet")
- **Special regex chars in query:** use `includes`, never `RegExp` — no injection/escape bug
- **Column with zero cards:** preserved in array (header + Add Card stay visible in kanban)
- **Drag while filtered:** hidden cards excluded from `SortableContext` items — no phantom drops; clearing search restores full order from server cache (no position corruption)
- **Board switch:** search resets (stale query never leaks across boards)
- **Performance:** `useMemo` on filtered array; 300ms debounce prevents re-filter per keystroke; large boards (100+ cards) fine for MVP string match
- **5-2 forward-compat:** `filteredColumns` derivation point is where label/due/checklist filters will compose later — keep single derivation (`filterColumnsByTitle` then future `filterColumnsByFilters`) and document with code comment

### Project Structure Notes

- Alignment: new files stay in `frontend/src/features/boards/board-view/` (`card-search-input.tsx`, `filter-cards-by-title.ts` + co-located tests) — matches feature-based organization; shared `Input`/`Button` from `components/ui/`; no new top-level feature folder until 5-2 promotes search+filter together
- No conflicts: `features/search/` from architecture does NOT exist in tree — do not create it in this story

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.1: Card Search]
- [Source: _bmad-output/planning-artifacts/prd.md#FR34 — Users can search cards by title]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Search & Filter + Loading States (debounced 300ms, no spinner) + Empty States (No cards found / Clear search)]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend Architecture (React Query, feature-based) + Requirements to Structure Mapping (Search/Filter → features/search)]
- [Source: _bmad-output/project-context.md#Framework-Specific Rules (React Query for ALL server state, co-located tests, kebab-case files)]
- [Source: frontend/src/features/boards/board-view/board-view.tsx — integrate search here]
- [Source: frontend/src/features/boards/board-view/board-list-view.tsx — pure renderer, pass filtered columns]
- [Source: frontend/src/features/columns/column.tsx + column-card-list.tsx — kanban render path]
- [Source: frontend/src/features/columns/columns.api.ts:21-29 — Column.cards embedded]
- [Source: frontend/src/features/notes/note-list.tsx:41-49 — debounce timer shape to mirror (plus added unmount cleanup)]
- [Source: frontend/src/features/boards/board-view/board-view.test.tsx — test mock pattern]
- [Source: frontend/e2e/board-view-toggle.spec.ts — E2E seed + monitoringTest pattern]
- [Source: _bmad-output/implementation-artifacts/4-7-board-view-toggle.md — Previous Story Intelligence]

## Dev Agent Record

### Agent Model Used

Amelia (bmad-dev) / openrouter/thinkingmachines/inkling:free

### Debug Log References

### Completion Notes List

- All 5 task groups completed: util, component, board integration, unit tests, E2E.
- Tests verified: vitest 56 passed; playwright E2E 1 passed.
- E2E locator fixed (`getByText('Clear search', { exact: true })`) for strict-mode compliance.
- No backend change; pure frontend client-side filter over `GET /api/boards/:boardId/columns`.
- Search reset on board change implemented (`useEffect` with `[id]` dependency).
- Unmount cleanup (`clearTimeout`) added following note-list.tsx pattern.
- 5-2 forward-compat comment (`// 5-2: compose filterColumnsByFilters here`) preserved in board-view.tsx.
- All 7 patches applied: `allColumns ?? []` (line 201), `isSearchEmpty` gated (line 92), Escape focus (`inputRef` + `.focus()`), WebKit guard (`no-native-search-cancel`), `matchesTitle` aligned in `filterColumnsByTitle`, `handleSearchChange` wrapped in `useCallback`, E2E seed explicit (`colResA`/`colResB`).
- Spec amended (bad_spec resolved): sub-checkboxes synced to [x]; layout width clarified (`w-[140px]` mobile-first, `sm:w-[240px]`).
- Code review (CR) executed: Blind Hunter + Edge Case + Acceptance Auditor; 0 failed layers; 2 rejected, 0 intent_gap, 2 bad_spec (story checkbox contradiction; layout width ambiguity), 7 patch (`allColumns` `?? []`, `isSearchEmpty` before gates, Escape focus, WebKit guard, redundant `matchesTitle` export, `useCallback` missing, E2E locator fragility), 2 defer (timer leaks, stress tests).

### File List

- frontend/src/features/boards/board-view/filter-cards-by-title.ts
- frontend/src/features/boards/board-view/filter-cards-by-title.test.ts
- frontend/src/features/boards/board-view/card-search-input.tsx
- frontend/src/features/boards/board-view/card-search-input.test.tsx
- frontend/src/features/boards/board-view/board-view.tsx (modified)
- frontend/src/features/boards/board-view/board-view.test.tsx (modified)
- frontend/e2e/card-search.spec.ts
- _bmad-output/implementation-artifacts/5-1-card-search.md (updated with review record)
- /tmp/diff_output.txt (review artifact, temporary)
- _bmad-output/implementation-artifacts/sprint-status.yaml (updated)
