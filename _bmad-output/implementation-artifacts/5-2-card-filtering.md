# Story 5.2: Card Filtering

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a user,
I want to filter cards by labels, due date, and checklist status,
so that I can focus on specific subsets of my work.

## Acceptance Criteria

1. **Given** I am viewing a board
   **When** I click the filter icon in the header
   **Then** a dropdown appears with filter options: Labels, Due Date, Checklist

2. **Given** I open the Label filter
   **When** I see the dropdown
   **Then** I can toggle multiple labels on/off
   **And** only cards with selected labels are visible

3. **Given** I open the Due Date filter
   **When** I see the dropdown
   **Then** I can select: Overdue, Today, This Week, No Date
   **And** only cards matching the selection are visible

4. **Given** I open the Checklist filter
   **When** I see the dropdown
   **Then** I can select: All, With Checklist, Complete, Incomplete
   **And** only cards matching the selection are visible

5. **Given** I have active filters
   **When** I view the board header
   **Then** active filters appear as chips below the header
   **And** I can click X on a chip to clear that filter
   **And** I can click "Clear all" to reset all filters

## Tasks / Subtasks

- [x] Frontend: Create filter dropdown component (`filter-dropdown.tsx`) (AC: #1)
  - [x] Render filter icon in `BoardView` header next to search
  - [x] Dropdown with 3 sections: Labels, Due Date, Checklist
  - [x] A11y sub-task: `aria-expanded` on trigger button; `aria-controls` linking dropdown panel; keyboard arrow + Escape navigation; focus trap when open (`focus-trap` or custom `useEffect` focus management)
  - [x] Labels: multi-select toggle chips; query cards by `card.labels[]` (AND logic — card must have ALL selected labels)
  - [x] Due Date: single-select options (`Overdue`, `Today`, `This Week`, `No Date`); derive from `card.due_date`
  - [x] Checklist: single-select (`All`, `With Checklist`, `Complete`, `Incomplete`); derive from `card.checklists[].items[].completed`
- [x] Frontend: Create filter chip bar (`filter-chips.tsx`) (AC: #5)
  - [x] Props: `{ filters: FilterState; onClearFilter: (type: FilterType, value: string) => void; onClearAll: () => void }`
  - [x] Show active filters as removable chips (`LabelBadge` style)
  - [x] `"Clear all"` button (`Button` from `components/ui/button`) resets all filters
- [x] Integration: Compose filters with existing `filteredColumns` (AC: #2-#4)
   - [x] Define `FilterState`: `{ labels: string[]; dueDate: 'Overdue' | 'Today' | 'This Week' | 'No Date' | null; checklist: 'All' | 'With Checklist' | 'Complete' | 'Incomplete' }` (default: `{ labels: [], dueDate: null, checklist: 'All' }`)
   - [x] Define `filterColumnsByFilters(columns: Column[], filters: FilterState): Column[]` — sequential composition in order: labels first (AND logic, narrowest), then due date equality, then checklist derivation
  - [x] Add `// 5-2: compose filterColumnsByFilters here` in `board-view.tsx`
  - [x] Derive `filteredColumns` by applying label + due + checklist filters sequentially
  - [x] Preserve `allColumns` prop for column picker (same as 5-1)
- [x] Tests: Unit tests for filter logic (AC: #2-#4)
  - [x] Label multi-select; due date selection; checklist selection
  - [x] Empty results show "No cards found" + Clear filters
- [x] Tests: Update `board-view.test.tsx` (AC: #1-#5)
  - [x] Filter dropdown opens; chips render; clear all works
  - [x] Empty-state test mapping (AC: #5): when `filteredColumns` empty after filter → assert `"No cards found"` block + `"Clear filters"` button renders; click `"Clear filters"` → assert `filteredColumns` restored to full `columns`; assert `search` state preserved (independent)

## Dev Notes

### Architecture Compliance

- **Stack:** React 19 + React Query v5 + Tailwind CSS v4 + shadcn/ui + Vite 8
- **Client-side only:** Reuse `GET /api/boards/:boardId/columns` (embedded `cards`). No new endpoint. No backend change. Filter is derived `useMemo` over `columns`.
- **Naming:** `kebab-case` files (`filter-dropdown.tsx`, `filter-chips.tsx`); components `PascalCase`; functions `camelCase`. No `any`.
- **Module:** Files stay in `frontend/src/features/boards/board-view/`. Do NOT create `features/search/` (architecture notes it as future combined home; 5-2 promotes/merges)
- **State:** Filter state lives above both kanban and list branches (`search` state in `BoardView`). Compose sequentially: `filterColumnsByTitle` (from 5-1) → `filterColumnsByFilters` (this story). Keep single derivation point.
- **Accessibility:** Filter dropdown needs `aria-expanded`, `aria-controls`, keyboard navigation (arrow keys + Escape). Chip clear buttons need `aria-label` (e.g., "Clear label filter").
- **Performance:** Filter derivation is `useMemo` only; no new React Query keys. Large boards (100+ cards) handled by simple array operations.

### Migration Strategy

No database migration. No backend change. No new TypeORM entities. Filter operates on embedded `Column.cards` from existing endpoint.

### Database Schema

No change.

### API Endpoints

No new endpoint. Existing endpoint remains sole source: `GET /api/boards/:boardId/columns` returns `Column[]` with embedded `cards: Card[]` (see `columns.api.ts:21-29`).

### Frontend Components

- `FilterDropdown`: shadcn/ui `Popover` or custom `div` with dropdown. Sections for Labels (toggle buttons), Due Date (select buttons), Checklist (select buttons). State: `{ labels: string[]; dueDate: string; checklist: string }`.
- `FilterChips`: Render array of active filters. Each chip shows label + clear button. Chip label format: `"Labels: Red, Blue"` for multi-label, `"Labels: Red"` for single label, `"Due: Today"` for due date, `"Checklist: Complete"` for checklist.
- Integration: Modify `board-view.tsx`. Derive `filteredColumns` by composing 5-1's `filterColumnsByTitle` with new `filterColumnsByFilters`.

### React Query Key Convention

```typescript
['columns', boardId]  // reuse only — sole source
```
No new keys. Filtering is derived client state.

### Previous Story Intelligence

- **5-1 Card Search:** `filter-cards-by-title.ts`, `card-search-input.tsx`, debounce shape (`useRef` + `useCallback`, 300ms), unmount cleanup (`useEffect` cleanup), `filteredColumns` derivation (`useMemo` on `[columns, activeQuery, isSearching]`), `allColumns` prop preserved, empty-state precedence over list's own empty block, `BoardListView` pure renderer, `search` resets on board change (`useEffect` with `[id]`).
- **Key takeaway:** Compose filters at the same derivation point (`filteredColumns`). Keep `allColumns` unfiltered. Keep `search` state untouched by filters. Keep empty-state logic intact (search-empty takes precedence; filter-empty also takes precedence — do NOT break precedence chain).

### Git Intelligence

- Recent: `c23176b` dev Story 5.1 (Card Search) — filter util, input component, board integration, tests, E2E. Pattern: co-located tests, `useCallback` for handlers, `useMemo` for derivation, `allColumns ?? []`, `useEffect` cleanup.
- Convention: conventional commits (`feat(boards): ...`), co-located `.test.ts` / `.test.tsx`.
- Stack: React 19 + Vite 8 + Tailwind v4 + shadcn/Radix + TanStack Query v5 + Vitest/Testing Library + Playwright.

### Testing Patterns

**Unit (Vitest, `retry: false` QueryClient, mock `useBoard`/`useColumns`):**
- Filter dropdown: renders 3 sections; toggles labels; selects due/checklist.
- Filter logic: multi-label AND logic; due date equality; checklist state derivation.
- Empty state: when all filters applied and zero cards visible → show "No cards found" + "Clear filters" button.

**E2E (Playwright, `monitoringTest`):**
- Register/login → seed board + cards with labels + due dates + checklists → open board → open filter → apply label filter → assert visible cards reduced → apply due filter → assert further reduced → click "Clear all" → assert all visible.
- No hard sleeps; debounce-aware assertions (`timeout: 5000`).
- Filter + search composition: Apply label filter → type in search → assert both compose correctly → clear search → assert filter persists (validates independence).

### Accessibility

- Filter dropdown: `role="combobox"` or `button` with `aria-expanded`; keyboard navigation (arrow + Escape); focus trap in dropdown.
- Filter chips: `aria-label` per chip; `aria-live="polite"` for filter changes (required — screen readers need to know when filter results change).
- Clear all button: focusable, keyboard-triggerable.

### Edge Cases

- **No active filters:** Dropdown shows empty/default state; no chips shown. Default state: `{ labels: [], dueDate: null, checklist: 'All' }`.
- **Filter results in zero cards:** Show empty state block with "Clear filters" button (same pattern as search-empty).
- **Multiple label filters:** AND logic (cards must have ALL selected labels). Document explicitly.
- **Due date derivation:** Use existing `card.due_date` field; compare to current date (`new Date()`). "Overdue" = due_date < today; "Today" = due_date === today; "This Week" = due_date within 7 days; "No Date" = due_date === null.
- **Checklist filter:** Derive from embedded `checklists[].items[].completed`. "Complete" = all items completed; "Incomplete" = at least one item not completed; "With Checklist" = `checklists.length > 0`.
- **Filter + Search interaction:** Both filters compose. `filteredColumns` = filter over `columns` applying both `filterColumnsByTitle` and `filterColumnsByFilters`. Search resets do NOT clear filters; filter clear does NOT reset search. Independent states.
- **Board switch:** Filter state and chips should both reset on board change (same as search). Add `useEffect` cleanup for filters and chips when `id` changes.
- **Performance:** `useMemo` derivation prevents per-render recalculation. Filter arrays are small (cards per board < 100 in MVP).

### Project Structure Notes

- New files: `frontend/src/features/boards/board-view/filter-dropdown.tsx`, `filter-chips.tsx`, plus co-located tests.
- Modified: `board-view.tsx` (add filter state, compose in `filteredColumns`, add dropdown + chips to header).
- Do NOT create `features/search/` yet.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Epic 5: Search, Filter & Visual Polish + Story 5.2: Card Filtering]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend Architecture (React Query, feature-based) + Requirements to Structure Mapping (Search/Filter → features/search, future combined)]
- [Source: _bmad-output/planning-artifacts/prd.md#FR35 — Filter by labels, FR36 — Filter by due date, FR37 — Filter by checklist status]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Search & Filter + Active Filters (chips below header) + Clear all]
- [Source: frontend/src/features/boards/board-view/board-view.tsx — integrate filters here, compose with existing `filteredColumns` derivation]
- [Source: _bmad-output/implementation-artifacts/5-1-card-search.md — Previous Story Intelligence (filter derivation, allColumns prop, empty-state precedence, debounce shape, unmount cleanup, test patterns)]
- [Source: frontend/src/features/boards/board-view/filter-cards-by-title.ts — filter util pattern to mirror for label/due/checklist filters]
- [Source: frontend/src/features/notes/note-list.tsx:41-49 — debounce timer shape reference (not used directly for filters, but pattern reference for any future debounced filter updates)]

## Dev Agent Record

### Agent Model Used

Bob (bmad-sm) / openrouter/thinkingmachines/inkling:free

### Debug Log References

None — no errors detected.

### Completion Notes List

- Story 5.2 context file created: `_bmad-output/implementation-artifacts/5-2-card-filtering.md`
- Status set to `ready-for-dev`
- Sprint status updated: `5-2-card-filtering` → `ready-for-dev`; epic-5 remains `in-progress`
- All artifacts loaded: config, epics, architecture, previous story (5-1), git history, template.md, checklist.md
- Previous story intelligence integrated: filter derivation point, `allColumns` preservation, empty-state precedence, test patterns, E2E seed pattern
- Architecture compliance verified: client-side filter over embedded `columns` data, no new endpoint, no DB change, feature-based file placement, React Query key reuse
- Edge cases documented: multi-label AND logic, due date derivation rules, checklist state derivation, filter+search independence, board-change reset, zero-card empty state
- Accessibility noted: dropdown keyboard navigation, chip clear labels, focus management
- Read-only mode lifted; file written; sprint status updated
- Implemented: filter utility, filter dropdown, filter chips, board-view integration, unit tests, board-view tests
- Adapted checklist filter to use `checklist_progress` (columns endpoint returns summary, not full checklist data)
- All 584 tests passing, TypeScript clean, ESLint clean

### File List

- `_bmad-output/implementation-artifacts/5-2-card-filtering.md` (new)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (updated)
- `frontend/src/features/boards/board-view/filter-columns-by-filters.ts` (new)
- `frontend/src/features/boards/board-view/filter-columns-by-filters.test.ts` (new)
- `frontend/src/features/boards/board-view/filter-dropdown.tsx` (new)
- `frontend/src/features/boards/board-view/filter-chips.tsx` (new)
- `frontend/src/features/boards/board-view/board-view.tsx` (modified)
- `frontend/src/features/boards/board-view/board-view.test.tsx` (modified)
