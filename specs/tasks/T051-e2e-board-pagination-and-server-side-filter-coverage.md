# T051: E2E: board pagination and server-side filter coverage

## Goal

Add Playwright e2e coverage proving board page data is accessible through pagination/infinite scroll and server-side search/filter behaves correctly after the T022 amendment lands.

## Depends On

T050

## Blocked By / Preconditions

- **Blocked until T048, T049, and T050 are implemented and merged.** T051 is a post-amendment regression/coverage task, not a spec for current behavior.
- **T049 contract lock required before writing assertions** for query params and response semantics. **Locked:** filter params are `search`, `labels` (repeated `i32`, AND semantics), `due_date`, `checklist` (enums/semantics as in T049). Do not guess; also confirm any final encoding details in OpenAPI before asserting.
- **T050 UX contract lock (locked):** initial page sizes are **10 cards/column (`embed_limit=10`)**, **20 notes**, **20 list items**. IO sentinel placement is in the same scroll root as each virtualized/list/notes scroll container. Filter changes reset accumulation. **DnD guard:** when any search/filter is active, drag/reorder/move-all/sort/keyboard/cross-column/list drag paths are disabled/guarded with explicit user-visible affordance/copy (minimum expectation per T050). Assert against that behavior (disabled state and/or blocked action with no mutation request), not vague wording.

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
| `frontend/e2e/virtualization.spec.ts` (existing, likely modify) | Reconcile with T050 pagination model (10 initial + 20/page) if existing assumptions break; keep bounded-DOM virtualization assertions. |
| `frontend/e2e/notes.spec.ts` / `frontend/e2e/board-view-toggle.spec.ts` (only if needed) | Touch only where T050 behavior changes existing expectations. |
| `frontend/e2e/test-utils.ts` / focused helper module (only if needed) | Reuse existing monitoring/API patterns; extract small deterministic seed/scroll helpers only if duplication is substantial. No unnecessary Playwright config changes. |

## Affected Existing Tests / Overlap

- **`frontend/e2e/card-search.spec.ts`**: currently validates search visibility behavior; after T050, search becomes server-driven. Keep coverage, but update waits/assertions to the final contract (e.g., `waitForResponse` for search/filter requests where useful) and avoid client-only assumptions.
- **`frontend/e2e/virtualization.spec.ts`**: currently seeds large datasets and assumes current virtualization/window behavior. T050 changes initial card load to 10 and paginates by 20; confirm whether existing 120/500-card cases still represent intended behavior or need adaptation.
- **`frontend/e2e/notes.spec.ts`**: contains `waitForTimeout` debt and old sidebar assumptions; do not copy those patterns. Only modify if T050/T051 changes notes sidebar behavior it covers.
- **Board list view toggle/sort tests**: ensure server-side sort/filter/list pagination does not regress existing list-view behavior.

## Implementation Steps

1. **Lock preconditions before coding:** confirm T048/T049/T050 merged; record final endpoint/query param names, filtered total semantics, DnD guard behavior, and UI affordance labels/testids in the PR/task notes.
2. **Test setup helpers:** create dedicated test user(s), login via UI only where needed, and seed board/columns/cards/notes via API using `E2E_API`. Use unique timestamped names/emails per spec for isolation in the shared `kanpad_e2e` DB. Do not add `Origin` headers to API fixture requests.
3. **Deterministic seed sizes:** seed counts just above page sizes to keep CI runtime bounded (e.g. 11-15 cards in a target column, 21-25 notes, 21-25 list-view cards across columns). Include at least one item that will be outside the initial page/window for search/filter coverage.
4. **Board view column cards:** open `/board/:id`; assert initial loaded count for target column is 10 (or T050-locked value). Trigger infinite scroll by scrolling the column's actual scroll root until the IO sentinel intersects; wait for the next page response/assert visible new items. Repeat with a no-progress guard to avoid infinite loops. Assert all seeded cards become accessible, no duplicates by card title/id, and count/total UI reflects authoritative totals.
5. **Notes side panel:** open notes sidebar; assert initial 20 notes loaded (or T050-locked value). Scroll the sidebar scroll container to trigger IO sentinel loads; wait for responses/visible items. Assert every seeded note becomes accessible without duplicates. Account for sidebar collapsed/persisted state with accessible selectors, not timeouts.
6. **List view:** switch to list view; assert initial 20 items (or T050-locked value). Use IO sentinel in the same list scroll root to load remaining pages. Assert all seeded items accessible, stable server-driven sort/order across loads, and no duplicates. For virtualized lists, do not assert all rows exist in DOM at once; assert accessibility by scrolling/finding expected items and checking bounded row counts where appropriate.
7. **Server-driven search/filter:** seed a matching card outside the initial page. Apply search/filter through UI controls; wait for the relevant server response(s) using the locked T049 query contract. Assert the out-of-initial-page match is returned, visible state resets accumulation appropriately, clear/reset restores expected results, and no-match shows the expected empty state. Cover at least the filter kinds/semantics locked by T049 (title `q`/search, labels AND, due date, checklist) as applicable to available UI.
8. **Filtered DnD guard:** while any search/filter is active, assert the T050-defined guard behavior. At minimum cover card drag/reorder, move-all, sort, keyboard DnD, and cross-column drop/list-view drag as applicable; controls must be disabled or blocked with explicit user-visible affordance/copy. Assert no mutation request is sent for guarded actions.
9. **Stability/a11y:** use `expect`, `waitForResponse`, `waitForLoadState('networkidle')` only where appropriate, role/label/testid selectors, and count/text assertions. Add `aria-live`/status assertions for loading/end/error where T050 exposes them. Verify focus is not lost after append when feasible. No `waitForTimeout`.

## Constraints

- No `waitForTimeout()` in e2e tests.
- Test setup/seed via API calls, not UI clicks, except behavior under test.
- Assertions must be resilient to virtualization: do not assume all items are mounted; scroll and assert by accessible content/IDs/totals.
- Use deterministic no-progress guards for scroll loops; fail with a clear message if sentinel loading stalls.
- Follow existing Playwright/e2e conventions: `E2E_API`, `monitoringTest`/`assertNoErrors` where useful, unique per-spec users/resources, no `Origin` header in API fixtures.
- Keep seed volume and runtime bounded; avoid large sequential seeding unless justified.
- Do not weaken two-origin/CSRF/throttle test environment assumptions.

## Acceptance Criteria

- [ ] Preconditions satisfied: T048/T049/T050 merged and T049 filter/query contract documented in test comments or task notes.
- [ ] E2E verifies all seeded board column cards are accessible through infinite scroll pagination from the T050 initial page size, with no duplicates and authoritative count/total behavior.
- [ ] E2E verifies all seeded board notes in side panel are accessible through infinite scroll pagination from the T050 initial page size.
- [ ] E2E verifies all seeded list-view cards are accessible through T050 page-sized infinite scroll with stable server-driven sort/order and no duplicates.
- [ ] E2E verifies server-driven search/filter returns correct results, including items beyond the initial page, resets accumulation on filter change/clear, handles empty state, and enforces the T050 filtered DnD guard (no guarded mutation requests).
- [ ] Existing affected e2e specs (`card-search.spec.ts`, `virtualization.spec.ts`, others as needed) are updated or explicitly reconciled so the suite remains coherent after T050.
- [ ] Tests pass reliably without `waitForTimeout`, with virtualization-safe assertions and deterministic scroll/loading guards.

## Notes

- **Current state (2026-11-25):** T048-T051 are pending/untracked; current behavior differs materially (50-card embed, manual/button load-more in places, notes sidebar plain query with default-page truncation risk, client-side board filters, no filter-aware DnD guard). Do not implement T051 against current behavior.
- **Virtualization:** board/list virtualization uses bounded DOM windows; infinite scroll tests must scroll the same scroll root as the IO sentinel/virtualizer and prove item accessibility, not total mounted DOM nodes.
- **Security:** low direct runtime impact; preserve throwaway `kanpad_e2e` isolation and do not add fixture `Origin` headers. Tests should not encode secrets or relax CSRF/CORS assumptions.
- **Runtime:** keep datasets minimal (> page size but small). Prefer `waitForResponse` for pagination/filter requests triggered by scroll/input; debounce-aware waits for search input should use response/visible-state assertions, not fixed sleeps.
- **Follow-up:** if T049 exposes filtered/unfiltered totals separately, add assertions for both labels/values to prevent ambiguous count regressions.
