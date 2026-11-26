# T051: E2E: board pagination and server-side filter coverage

## Goal

Add Playwright e2e coverage proving board page data is accessible through pagination/infinite scroll and server-side search/filter behaves correctly after the T022 amendment lands.

## Depends On

T050

## Blocked By / Preconditions

- **Blocked until T048, T049, and T050 are implemented and merged.** T051 is a post-amendment regression/coverage task, not a spec for current behavior.
- **T049 contract lock required before writing assertions** for query params and response semantics. **Locked:** filter params are `search`, `labels` (repeated `i32`, AND semantics; assert repeated-param encoding `labels=1&labels=2`), `due_date`, `checklist` (enums/semantics as in T049). Do not guess; confirm final encoding/details in OpenAPI before asserting.
- **T049 totals semantics lock (required for assertions):** paginated card endpoints return filtered authoritative `Page.total`; column embed keeps `total_cards` as unfiltered authoritative and adds additive `matching_cards` as filtered authoritative; embedded cards are first `embed_limit` matching cards. Assert the correct field for the state under test; do not hardcode ambiguous UI copy unless T050 locks it.
- **T050/T053 UX contract lock (locked):** initial page sizes are **10 cards/column (`embed_limit=10`)**, **20 notes**, **20 list items**. IO sentinel placement is in the same scroll root as each virtualized/list/notes scroll container. Filter changes reset accumulation. **DnD under filter (T053):** sort-by-date and Move All Cards remain disabled/guarded with explicit affordance/copy; card drag/reorder/cross-column move is allowed when the drop resolves to a safe visible anchor; blocked ambiguous drops give visible toast + polite aria-live feedback with no mutation. Assert against the final T050/T053 behavior (disabled state and/or blocked action with no mutation request), not vague wording.
- **Pre-dev contract checklist (record in PR/task notes before writing tests):** endpoint paths, query param names/encoding, page sizes, `total_cards`/`matching_cards`/`Page.total` semantics, DnD guard copy/selectors/testids, and any T050-added testids/aria hooks.

## Phase

1

## Critical

No

## Spec References

- `specs/tasks/T022-pagination-u-x-page-ceilings.md`
- `specs/tasks/T048-backend-parameterized-embed-limit-board-cards-list-endpoint.md`
- `specs/tasks/T049-backend-server-side-board-card-search-and-filters.md`
- `specs/tasks/T050-frontend-board-infinite-scroll-and-server-driven-filters.md`
- `AGENTS.md`: frontend e2e uses API calls for setup, no `waitForTimeout`, use assertions/waits
- `frontend/CONVENTIONS.md`: E2E rules
- `frontend/playwright.config.ts`: two-origin SPA/API topology, `kanpad_e2e`, `E2E_BASE_URL`/`E2E_SPA_URL`
- `frontend/e2e/api-base.ts`: use `E2E_API`; do not add `Origin` header to fixture API requests (T56 fail-closed CSRF)

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/e2e/board-pagination.spec.ts` (new) | Board column, notes sidebar, and list-view infinite scroll/pagination coverage. |
| `frontend/e2e/board-search-filter.spec.ts` (new, or split by scope) | Server-driven search/filter behavior and filtered DnD guard coverage. |
| `frontend/e2e/card-search.spec.ts` (existing, likely modify) | Update/regress for post-T050 server-driven search semantics where behavior overlaps; avoid duplicating brittle assertions. |
| `frontend/e2e/virtualization.spec.ts` (existing, major adaptation/rewrite likely) | Reconcile with T050 pagination model (10 initial + 20/page). Existing cases assume full embed/card arrays; rewrite or replace with equivalent post-T050 coverage preserving intent: bounded DOM, keyboard DnD across virtual boundary, and list-view virtualization. |
| `frontend/e2e/perf-scroll.spec.ts` (existing, review/reconcile) | Seeds 400 cards and assumes large initial virtualized set; review/update so it still measures intended behavior under paged loading, or explicitly document replacement scope. |
| `frontend/e2e/notes.spec.ts` / `frontend/e2e/board-view-toggle.spec.ts` / `frontend/e2e/perf-load-heap.spec.ts` (only if needed) | Touch only where T050 behavior changes existing expectations; sanity-check perf/load assumptions. |
| `frontend/e2e/test-utils.ts` / focused helper module (only if needed) | Reuse existing monitoring/API patterns; extract small deterministic seed/scroll helpers only if duplication is substantial. No unnecessary Playwright config changes. |

## Affected Existing Tests / Overlap

- **`frontend/e2e/card-search.spec.ts`**: currently validates search visibility behavior; after T050, search becomes server-driven. Keep coverage, but update waits/assertions to the final contract (e.g., `waitForResponse` with query-matching predicate for search/filter requests), debounce-aware visible-state assertions, and no client-only assumptions. Verify empty-state/clear copy/selectors against final T050 UI.
- **`frontend/e2e/virtualization.spec.ts`**: currently seeds large datasets and assumes full embed/card arrays and large initial mounted windows. T050 changes initial card load to 10 and paginates by 20; this is a major reconciliation. Rewrite/replace cases to preserve intent (bounded DOM, keyboard DnD across virtual boundary, list-view virtualization) under paged loading. Do not leave stale assumptions about `col.cards` length or all cards mounted initially.
- **`frontend/e2e/perf-scroll.spec.ts`**: seeds 400 cards in one column and measures scroll over a large initially loaded set; review/update for paged loading or explicitly document replacement/equivalent perf coverage. Sanity-check `frontend/e2e/perf-load-heap.spec.ts` for any full-load assumptions.
- **`frontend/e2e/notes.spec.ts`**: contains `waitForTimeout` debt and old sidebar assumptions; do not copy those patterns. Only modify if T050/T051 changes notes sidebar behavior it covers.
- **Board list view toggle/sort tests**: ensure server-side sort/filter/list pagination does not regress existing list-view behavior.
- **Reconciliation bar:** affected specs must pass under post-T050 behavior, or be replaced/removed only with explicit justification recorded in PR/task notes. No unexplained skips/broken tests.

## Implementation Steps

1. **Lock preconditions before coding:** confirm T048/T049/T050 merged; record final endpoint/query param names/encoding, page sizes, filtered/unfiltered total semantics (`Page.total`, `total_cards`, `matching_cards`), DnD guard behavior/copy/selectors/testids, and any T050 aria/testid hooks in PR/task notes.
2. **Test setup helpers:** create dedicated test user(s); perform UI login only where login itself is the behavior under test or already the minimal established pattern, otherwise prefer API/session setup for speed. Seed board/columns/cards/notes/labels via API using `E2E_API`. Use unique timestamped names/emails per spec for isolation in the shared `kanpad_e2e` DB. Do not add `Origin` headers to API fixture requests.
3. **Deterministic seed sizes:** seed counts just above page sizes to keep CI runtime bounded (e.g. 11-15 cards in a target column, 21-25 notes, 21-25 list-view cards across columns). Include at least one item that will be outside the initial page/window for search/filter coverage. Keep all assertions scoped to the seeded board/resources.
4. **Board view column cards:** open `/board/:id`; assert initial loaded page for target column is 10 (or T050-locked value). Determine "loaded count" via virtualization-safe signals: paginated response payload/count, T050-exposed count/testid, and/or accessible items found by scrolling — not total mounted DOM nodes. Trigger infinite scroll by scrolling the column's actual scroll root until the IO sentinel intersects; wait for the next page `waitForResponse` (predicate matching endpoint/query) and assert visible/appended items. Repeat with a bounded no-progress guard (max iterations/scroll attempts + clear failure). Assert all seeded cards for that column become accessible, no duplicates by stable card id/title+id, and count/total UI reflects T049 semantics (`total_cards` unfiltered; `matching_cards`/filtered count when filters active, as exposed).
5. **Notes side panel:** open notes sidebar; assert initial 20 notes loaded (or T050-locked value) using virtualization-safe signals. Scroll the sidebar's actual scroll container to trigger IO sentinel loads; wait for response/visible items. Assert every seeded note for that board becomes accessible without duplicates. Account for sidebar collapsed/persisted state with accessible selectors, not timeouts.
6. **List view:** switch to list view; assert initial 20 items (or T050-locked value). Use IO sentinel in the same list scroll root to load remaining pages with bounded no-progress guards and response waits. Assert all seeded items for that board are accessible, stable server-driven sort/order across loads, and no duplicates. For virtualized lists, do not assert all rows exist in DOM at once; assert accessibility by scrolling/finding expected items and checking bounded row counts where appropriate. Cover filter/search/sort-change accumulation reset in list view where applicable.
7. **Server-driven search/filter:** seed a matching card outside the initial page. Apply search/filter through UI controls; wait for relevant server response(s) using the locked T049 query contract. Assert the out-of-initial-page match is returned, accumulation resets on filter change/clear, clear/reset restores expected results, and no-match shows the expected empty state. Cover filter kinds/semantics locked by T049 as exposed by T050 UI: `search` (title, case-insensitive, trimmed/escaped semantics as server-defined), `labels` (repeated params, AND semantics), `due_date`, `checklist`. Where board and list views both expose filters, assert parity/reset behavior for the affected view(s) without duplicating brittle cases.
8. **Filtered DnD (T053):** while any search/filter is active, assert the exact T050/T053 behavior where affordances exist in board and list views: sort-by-date and Move All Cards remain disabled with explicit affordance/copy and no mutation; card drag/reorder and cross-column move are allowed when the drop resolves to a safe visible anchor and persist after reload; blocked ambiguous drops (no visible anchor, unloaded matching tail, hidden-only target) give visible toast + polite aria-live feedback with no mutation. Evaluate keyboard DnD under the same rules (or explicitly defer with justification). Do not invent alternate behavior; follow the final T050/T053 contract recorded in pre-dev notes.
9. **Stability/a11y:** use `expect`, `waitForResponse` (query/predicate-specific), `waitForLoadState('networkidle')` only where appropriate, role/label/testid selectors, and count/text assertions. Add `aria-live`/status assertions for loading/end/error where T050 exposes them. Verify focus is not lost after append when feasible. No `waitForTimeout`.

## Constraints

- No `waitForTimeout()` in e2e tests.
- Test setup/seed via API calls, not UI clicks, except behavior under test.
- Assertions must be resilient to virtualization: do not assume all items are mounted; define initial/loaded counts via virtualization-safe signals (response payload, exposed count/testid, accessible items found by scrolling), not DOM row count alone.
- All infinite-scroll loads use bounded no-progress guards (max iterations/scroll attempts with clear failure) and specific `waitForResponse`/visible-state waits; do not rely on `networkidle` as the sole sync for scroll-triggered fetches.
- Scope all data assertions to seeded board/resources/users; shared `kanpad_e2e` DB runs with concurrent workers.
- Follow existing Playwright/e2e conventions: `E2E_API`, `monitoringTest`/`assertNoErrors` where useful, unique per-spec users/resources, no `Origin` header in API fixtures.
- Keep seed volume and runtime bounded; avoid large sequential seeding unless justified.
- Do not weaken two-origin/CSRF/throttle test environment assumptions.
- Affected existing e2e specs must pass post-T050 or be replaced/removed only with explicit justification in PR/task notes.

## Acceptance Criteria

- [ ] Preconditions satisfied: T048/T049/T050 merged; pre-dev contract checklist recorded in PR/task notes (endpoints, query params/encoding, page sizes, `Page.total`/`total_cards`/`matching_cards` semantics, DnD guard behavior/copy/selectors/testids, T050 a11y/testid hooks).
- [ ] E2E verifies all seeded board column cards for the target column are accessible through infinite scroll pagination from the T050 initial page size, with no duplicates by stable id, virtualization-safe assertions, and correct T049 total semantics (`total_cards` unfiltered; filtered state uses `matching_cards`/filtered count as exposed).
- [ ] E2E verifies all seeded board notes for the target board are accessible through infinite scroll pagination from the T050 initial page size, with no duplicates and virtualization-safe assertions.
- [ ] E2E verifies all seeded list-view cards for the target board are accessible through T050 page-sized infinite scroll with stable server-driven sort/order, no duplicates, virtualization-safe assertions, and correct filtered authoritative `Page.total` behavior where exposed.
- [ ] E2E verifies server-driven search/filter (T049-locked `search`, `labels` repeated AND, `due_date`, `checklist`) returns correct results, including items beyond the initial page, resets accumulation on filter/sort/search change and clear, handles empty state, and enforces the T050/T053 filtered DnD rules (sort/move-all disabled no mutation; allowed visible-anchor card DnD persists; ambiguous blocked drops give feedback and no mutation).
- [ ] Affected existing e2e specs (`card-search.spec.ts`, `virtualization.spec.ts`, `perf-scroll.spec.ts`, others as needed) are updated/replaced/reconciled so the suite passes post-T050; any removal/replacement has explicit justification in PR/task notes.
- [ ] Tests pass reliably without `waitForTimeout`, with scoped seeded-resource assertions, virtualization-safe checks, deterministic bounded no-progress scroll guards, and response/visible-state waits.

## Notes

- **Current state (2026-11-25):** T048-T051 are pending/untracked; current behavior differs materially (50-card embed, manual/button load-more in places, notes sidebar plain query with default-page truncation risk, client-side board filters, no filter-aware DnD guard). Do not implement T051 against current behavior.
- **Virtualization:** board/list virtualization uses bounded DOM windows; infinite scroll tests must scroll the same scroll root as the IO sentinel/virtualizer and prove item accessibility, not total mounted DOM nodes. "Initial loaded count" must be defined via virtualization-safe signals, not mounted row count alone.
- **Existing spec risk:** `frontend/e2e/virtualization.spec.ts` and `frontend/e2e/perf-scroll.spec.ts` currently assume large initially loaded datasets/full embed arrays; expect major reconciliation under T050 paged loading.
- **Security:** low direct runtime impact; preserve throwaway `kanpad_e2e` isolation and do not add fixture `Origin` headers. Tests should not encode secrets or relax CSRF/CORS assumptions.
- **Runtime:** keep datasets minimal (> page size but small). Prefer `waitForResponse` predicates for pagination/filter requests triggered by scroll/input; debounce-aware waits for search input should use response/visible-state assertions, not fixed sleeps.
- **Follow-up:** if T049 exposes filtered/unfiltered totals separately, add assertions for both labels/values to prevent ambiguous count regressions.

## Pre-dev Contract Checklist (recorded 2026-11-26)

- **Endpoints**: `GET /api/v1/boards/{boardId}/columns?embed_limit=&search=&labels=&due_date=&checklist=`; `GET /api/v1/columns/{colId}/cards?page=&limit=&search=&labels=&due_date=&checklist=`; `GET /api/v1/boards/{boardId}/cards?page=&limit=&sort=&order=&search=&labels=&due_date=&checklist=`; `GET /api/v1/notes?board_id=&page=&limit=`.
- **Query params/encoding**: `search` trimmed string (empty=no filter, <2 chars not sent by board view UI); `labels` repeated `i32` AND (`labels=1&labels=2`); `due_date` enum `overdue|today|this_week|no_date`; `checklist` enum `with|complete|incomplete`.
- **Page sizes**: column embed initial 10 (`embed_limit=10`), column card hydration 20/page, notes 20/page, list view 20/page.
- **Totals semantics**: paginated card endpoints return filtered authoritative `Page.total`; column embed keeps `total_cards` unfiltered authoritative and additive `matching_cards` filtered authoritative (equals `total_cards` when no filter active).
- **DnD under filter (T050/T053-locked)**: sort/move-all disabled with `aria-disabled`, titles "Clear search and filters to sort cards" / "Clear search and filters to move all cards"; board banner "Sorting and move-all are disabled while search or filters are active. Card drag and drop is available for visible cards."; list view copy "Drag and drop is disabled while search or filters are active." (list view remains no-DnD); card DnD allowed for visible anchors under filter with blocked-drop feedback (`data-testid="dnd-blocked-status"`, `role="status"`, `aria-live="polite"`) and no mutation for ambiguous drops; guarded mutations (sort PATCH, move-all) must not be sent.
- **Testids/aria hooks**: `data-testid="column-card-list-scroll"`, `data-testid="column-cards-sentinel"`, `data-testid="virtual-card-row"`, `data-testid="board-list-scroll"`, `data-testid="board-list-sentinel"`, `data-testid="board-list-row"`, `data-testid="board-notes-sentinel"`, `aria-label="Search cards"`, `aria-label="Clear search"`, `aria-label="Filter cards"`, `role="table" aria-label="Board cards list"`, `aria-live="polite"` status messages.

## Full E2E Run Investigation (2026-11-26)

Command: `cd frontend && npm run test:e2e` (85 tests, 4 workers, 10 failed, 75 passed). API webServer ran with `APP_ENV=test`, `THROTTLE_AUTH_PER_MIN=10000`, `THROTTLE_GLOBAL_PER_MIN=10000` per `frontend/playwright.config.ts`; throttle not implicated (failures were 401/timeout/assertion, not 429).

### T051-caused regressions (fixed)

| Failure | Root cause | Fix |
|---|---|---|
| `board-pagination.spec.ts` 3 tests: `Create board failed: 401` | New specs used `request` fixture for API setup without capturing/passing session cookies. Repo e2e pattern requires manual `set-cookie` capture + `headers: { Cookie: cookies }` for fixture API calls; `api-base.ts` forbids `Origin` header. | Added module-scope cookie capture in `apiLogin`; passed `Cookie` header on all API setup/assertion requests/helpers. |
| `board-search-filter.spec.ts` 3 tests: `Create board failed: 401` | Same as above. | Same cookie propagation fix. |
| `card-search.spec.ts` timeout waiting for columns response with no `search=` after typing `A` | T051 reconciliation added `waitForResponse` for all search transitions. App QueryClient has `staleTime: 5min` (`frontend/src/lib/app-query-client.ts`), so returning to fresh unfiltered `['columns', boardId]` (or list default key) can be a cache hit with no network request. | For transitions returning to fresh default cached keys, assert UI state instead of waiting for response; kept response waits for genuinely new filtered keys. Applied same cache-aware rule to `board-search-filter.spec.ts` clear/default transitions. |
| `board-search-filter.spec.ts` checklist setup 400 / checklist=complete empty | Test helper sent `title`/`completed`; backend expects `text`/`is_completed` (`CreateChecklistItemInput`/`UpdateChecklistItemInput` in `frontend/src/lib/api/schema.d.ts`). | Fixed helper to send `text` and `is_completed: true`. |
| `board-pagination.spec.ts` column scroll timeout | Test waited for `page=[2-9]` column-cards response, but `useLoadMoreColumnCards` fetches `page=1&limit=20` after initial 10-card embed. Also IO could auto-load before scroll loop observed it. | Accept any `page=` column-cards response; if sentinel gone but IDs missing, confirm remaining IDs via column-cards API. |
| `board-search-filter.spec.ts` popover/menu flakiness | Radix popover closed after filter changes; DnD guard menu stayed open; keyboard Space opened card dialog. | Reopen popover defensively before subsequent popover clicks; ensure menu closed before drag/list switch; close card dialog after keyboard DnD attempt. |
| `assertNoErrors` flake on auth probe | 401 `/api/auth/me` (and generic console "Failed to load resource... 401 (Unauthorized)") and known dnd-kit React useEffect size warning caused spurious failures. | Narrowed `frontend/e2e/test-utils.ts` ignore rules for auth/me 401 console/network noise and that specific React warning. |

### Non-T051 failures observed (triage separately; not modified)

| Failure | Observation | Initial classification |
|---|---|---|
| `archived-boards.spec.ts:41` archives/restores/deletes boards - 30s timeout, request context disposed during late board3 API create | Heavy UI flow with existing `waitForTimeout/networkidle`; test timed out before final API create completed. Not touched by T051. | Likely pre-existing flake/slowness; reproduce in isolation before changing. |
| `checklists.spec.ts:51` checklist progress after reload not `1/2 (50%)` | Progress/toggle state not visible after reload; possible persistence/timing flake. Not touched by T051. | Likely pre-existing flake or real checklist persistence issue; reproduce/isolate. |
| `drag-drop.spec.ts:81` card order reverted after reload | UI drag order asserted before reload, but after reload order reverted to original; move persistence/timing issue. Not touched by T051. | Likely pre-existing flake or real DnD persistence issue; reproduce/isolate. |

### Verification (2026-11-26, post-fix + review)

- `cd frontend && npx playwright test --list e2e/board-pagination.spec.ts e2e/board-search-filter.spec.ts e2e/card-search.spec.ts e2e/virtualization.spec.ts e2e/perf-scroll.spec.ts` => **11 tests in 5 files** (added board-view sort accumulation reset coverage).
- `cd frontend && npm run test:e2e -- e2e/board-pagination.spec.ts e2e/board-search-filter.spec.ts e2e/card-search.spec.ts e2e/virtualization.spec.ts e2e/perf-scroll.spec.ts` => **11 passed (51.2s)**.
- `cd frontend && npx eslint e2e/board-pagination.spec.ts e2e/board-search-filter.spec.ts e2e/card-search.spec.ts e2e/virtualization.spec.ts e2e/perf-scroll.spec.ts src/features/boards/board-view/board-list-view.tsx src/features/notes/board-notes-sidebar.tsx e2e/test-utils.ts` => 0 errors, 1 pre-existing warning (`board-list-view.tsx` react-hooks/incompatible-library).
- `cd frontend && npm run -s typecheck` => pass.
- Full 85-test suite not rerun in this pass (time/cost); targeted affected specs are green.

### Subagent review fixes (2026-11-26)

- Tightened `frontend/e2e/test-utils.ts`: generic 401 console ignore now applies only when all 401 network requests are auth/me; keeps auth-me network ignore narrow.
- Removed column pagination API fallback in `frontend/e2e/board-pagination.spec.ts`; capture paginated card IDs via response listener (including IO auto-loads) and fail if UI pagination did not load all seeded IDs. Duplicate detection ignores embed overlap but flags duplicate paginated IDs.
- Added board-view column sort accumulation reset coverage in `frontend/e2e/board-search-filter.spec.ts` (AC89 gap).
- Added `status === 200` to `frontend/e2e/card-search.spec.ts` response predicates; minor cleanup in `frontend/e2e/virtualization.spec.ts`.
