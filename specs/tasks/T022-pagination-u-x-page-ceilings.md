# T022: Pagination UX + page ceilings

> **STATUS: in_progress.** Wave 1. Closes **P5 page-ceiling gap** (FTS remains T023) and **P6** (MEDIUM-LOW). Closes **P3 hydration half** with T021 (rendering half done; visual half T044).
> **2026-11-24 split:** staged implementation covers original T022 scope. Amendment scope (Option A board-cards endpoint, parameterized embed-limit naming safe vs T044, server-side search/filter, 10/20/20 infinite-scroll targets, board e2e) moved to T048-T051.
> **Depends on T021** (done; shared board-view/column/DnD files — coordinate carefully, do not parallelize conflicting edits).

## Goal

1. **Server: page ceiling (all `PaginationParams` consumers).** `page` has no bound today (`extract.rs:115-139`; unit test at `extract.rs:351-366` explicitly accepts `page=999999`) → deep offsets scan+discard. Add a hard window ceiling: `u64(page) * u64(limit) <= 10_000` (overflow-safe math, named constant near `MAX_LIMIT`) → 400 validation-error problem+json with `errors[]` outside the window. This is a **behavior-tightening contract change** (previously accepted deep pages become 400), not an additive wire-shape change; no OpenAPI shape/typegen change is expected unless a schema/field changes.
2. **Board cards: stop loading everything.** The board loads ALL cards unpaginated via the columns embed (`columns/routes.rs:45-103`, `common/card_summary.rs:128-301`) though a paginated cards API exists (`cards/routes.rs:499-541`, `cards.api.ts:66-126`, `use-cards.ts:28-35`). Keep columns embed for structure; embed only the first **N=50** cards per column by stable card order and expose an additive per-column total-card count so the frontend knows hydration state. Hydrate cards past N via the existing per-column paginated endpoint (`GET /api/v1/columns/{colId}/cards?page&limit`) using IO-driven load-more/infinite fetch.
3. **Lists: load-more UI.** Boards/archived/notes/projects fetch page 1 (limit 50 default) with no pagination UI → items beyond page 1 invisible. Add load-more using existing `Page<T>` envelope (`total` already returned).

## Depends On

T021 (done).

## Phase

1

## Critical

No — P6 is LOW-MED; P5 ceilings are cheap correctness. Board hydration is the main UX/perf risk.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Pagination (design: ceiling value, hydration strategy, open calls: keyset deferral, server-side search interplay).
- `specs/arch-hardening-specs/00-overview.md` — findings P3, P5, P6; global DoD; `backend-rs/src/extract.rs:115-139` (ceiling gap), `backend-rs/src/common/pagination.rs:13-40` (envelope; nested children unpaginated by contract), `backend-rs/src/features/columns/routes.rs:45-103` (all-cards embed), `frontend/src/features/cards/cards.api.ts:66-126` + `use-cards.ts:28-35` (paginated cards API currently unused by UI), `frontend/src/features/boards/use-boards.ts:38-127`.
- T010 note: card positions are computed `sort_key` indexes — card pagination must use stable ordering (`sort_key ASC, id ASC`).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/extract.rs` | Add `MAX_PAGE_WINDOW` (or equivalent) and overflow-safe ceiling validation; update unit tests that currently allow large pages. |
| `backend-rs/src/features/columns/routes.rs`, `backend-rs/src/features/columns/types.rs`, `backend-rs/src/common/card_summary.rs` (and only necessary column/card store/query support) | Embed first N=50 cards per column; add additive per-column `total_cards`/count field (exact name decided in impl, documented in Notes) so structure/counts remain authoritative; keep summary shape; avoid N+1; MySQL 8.4 + MariaDB 11.4 compatible SQL only. |
| `backend-rs/src/features/cards/routes.rs` / store (if needed) | Confirm/reuse paginated card endpoint; no keyset/cursor change. |
| `frontend/src/features/columns/columns.api.ts`, `frontend/src/features/columns/use-columns.ts` | Adapt to additive column count/first-N embed; preserve canonical board cache behavior. |
| `frontend/src/features/columns/column-card-list.tsx`, `frontend/src/features/columns/column.tsx` | IO-driven per-column hydration/load-more; sentinel in same scroll root as virtualizer; loading affordance; cooperate with existing `@tanstack/react-virtual` pinning/range extraction. |
| `frontend/src/features/cards/use-cards.ts` (and/or new focused hook) | Per-column paginated fetch/infinite accumulation for hydration; dedupe by card id; stable merge. Do not create a second conflicting DnD source of truth. |
| `frontend/src/features/cards/drag-drop-context.tsx` | Keep DnD coherent with partial hydration; define/guard drop behavior when target has unloaded pages; preserve dragSource-at-start, optimistic rollback, rAF coalescing semantics. |
| `frontend/src/features/boards/board-view/board-view.tsx`, `frontend/src/features/boards/board-view/board-list-view.tsx` | Counts from authoritative server totals; search/filter behavior made explicit under partial hydration; BoardListView scope handled (hydrate consistently or explicitly document/guard). |
| `frontend/src/features/boards/use-boards.ts`, `frontend/src/features/boards/board-list.tsx`, `frontend/src/features/boards/archived-boards.tsx` | Load-more accumulation; handle archived mixed-result semantics. |
| `frontend/src/features/notes/use-notes.ts`, `frontend/src/features/notes/note-list.tsx` | Load-more accumulation; preserve search/tag/type filter reset behavior. |
| `frontend/src/features/projects/use-projects.ts`, `frontend/src/features/projects/project-list.tsx` | Load-more accumulation. |
| `frontend/src/features/*/*.api.ts` (only if needed) | Query param helpers unchanged unless needed; no ad-hoc fetch wrappers. |
| Tests (backend + frontend unit/e2e) | Ceiling red-proof/boundaries; ordering/tiebreak; load-more dedupe/stop; hydration cache coherence/DnD; search limitation; archived mixed pages; a11y/focus. |
| `docs/lessons/{rust-backend,frontend}.md` | Dated, tagged lessons. |

**May appear only if required and documented:** additive column card-count field in OpenAPI/schema/typegen (`T52` same-PR process, additive-only shape change). 

**Must NOT appear in the diff:** keyset/cursor contract; broad search/filter product redesign beyond explicit limitation/guard; unrelated DnD sensor/animation changes; unrelated layout work owned by T044.

## Implementation Steps

TDD. Keep changes small and verify after each slice.

1. **Red (ceiling):** update/add extractor test proving `page` beyond window is rejected. Current `extract.rs` test accepts `page=999999` - flip it. Add boundary tests: `u64(page)*u64(limit) == 10_000` allowed when otherwise valid; `> 10_000` rejected. Add overflow-safe case (very large page value does not panic/wrap). Assert 400 problem+json `validation-error` with `errors[]`.
2. **Implement ceiling in shared extractor:** add named constant (e.g. `MAX_PAGE_WINDOW: u32 = 10_000` or equivalent) and checked/u64 multiplication in `PaginationParams::from_request_parts`. Keep strict no-clamp semantics and existing field-level errors. Confirm all `PaginationParams` consumers inherit behavior.
3. **Ordering audit/fix for paginated lists:** verify stable ordering + deterministic tiebreaker for every paginated endpoint touched by load-more:
   - cards: `sort_key ASC, id ASC` (already stable) — keep.
   - labels/tags: `id ASC` (already stable).
   - notes: `updated_at DESC, id DESC` already has tiebreaker; document mutable `updated_at` page-drift limitation (offset pagination) and dedupe-by-id on frontend.
   - boards/projects: add deterministic `id` tiebreaker to ordering (e.g. append `, b.id DESC/ASC` consistent with selected direction, or a documented stable rule). Add tests for equal-sort-key/tie rows.
4. **Red (invisible items):** prove list with >1 page has items beyond page 1 unreachable before UI, then reachable via load-more.
5. **List load-more (boards/archived/notes/projects):** use `useInfiniteQuery` where it fits existing React Query patterns, or an equivalent explicit accumulation pattern; prefer `useInfiniteQuery` for lists. Dedupe by id, preserve existing filter/search params as query-key inputs, reset accumulation on filter/search change, stop at `total`, expose loading/error/retry/end states. Accessibility: load-more button has accessible name, disabled/loading state, and focus/ARIA status announcement for loaded/error/end.
6. **Archived boards mixed-result case:** `archived=true` currently means "include archived" and UI filters client-side. Define and implement safe behavior: either accumulate pages until enough archived items/total exhausted with clear stop condition, or adjust API/query semantics additively if needed. Do not show false-empty while more archived items may exist.
7. **Board hydration contract (decide before coding):**
   - Keep `['columns', boardId]` as canonical board render/DnD cache unless a deliberate migration is fully completed; avoid dual source-of-truth bugs.
   - Embed first N=50 cards per column in columns response; add additive `total_cards` (or equivalent, documented) per column. Counts come from authoritative server totals, not loaded-card flatMap.
   - Hydrate remaining cards via paginated cards endpoint per column. Merge by card id, preserve stable order (`sort_key, id` / existing position semantics), never duplicate.
   - Mutations/invalidation: define matrix for create/update/delete/move/reorder/sort/move-all/label changes. After mutations, either refetch affected column pages coherently or reset to first page + visible loaded window with clear UX; no stale duplicates/missing cards.
   - DnD: drag source must be loaded/rendered. Dropping into a column with unloaded pages below the drop point must not silently misplace at end-of-loaded-subset; block with loading affordance or resolve against known server end only with explicit tested behavior. `dragSource`-at-start and optimistic rollback invariants remain.
   - IO sentinel: place in same scroll root as `ColumnCardList` virtualizer; guard against fetch storms (`isFetchingNextPage`, `hasNextPage`, threshold). Verify with nested scroll layout.
8. **Search/filter under partial hydration:** client-side search/filter currently inspects loaded `column.cards`. Do not silently return incomplete results as if complete. Implement explicit behavior: show "searches loaded cards only" / "load more to search remaining cards" indicator when unloaded pages exist, and/or adjust empty state/count copy. Document limitation in Notes. Server-side card search is out of scope unless separately decided.
9. **BoardListView scope:** handle consistently with board view. If it remains over loaded data, make limitation explicit and test it; if it hydrates, reuse same cache/merge rules.
10. **Tests:** red-proof ceiling; boundary/overflow; representative endpoint coverage; ordering/tiebreak; list load-more dedupe/stop/reset-on-filter; archived mixed-page; board hydration first-N + next page; DnD across loaded boundary; mutation after hydration; search limitation behavior; a11y/focus. For every POST/PATCH/DELETE touched by tests, add DB-state/persistence verification via different API call or SQL where applicable.
11. **Gates/evidence:** backend `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic`, `cargo test`/relevant suites, both DB engines if SQL/behavior affected, `cargo sqlx prepare --check` only if SQL changed; frontend `tsc -b`, vitest, relevant Playwright e2e, `gen:api -- --check` if contract changed; coverage floor green. Append lessons.

## Constraints

- **Behavior change is intentional and narrow:** page ceiling 400 for out-of-window pages is accepted; keep error shape consistent with existing PaginationParams validation.
- **Overflow-safe math:** ceiling check must not overflow/wrap `u32`.
- **Ordering stability:** card lists use `sort_key ASC, id ASC`. Other paginated lists must have deterministic tiebreakers; where mutable sort keys exist (notes `updated_at`), document drift and dedupe by id.
- **No keyset/cursor contract** in this task (defer to T026 evidence).
- **Additive contract only for shapes.** Column count field is additive if used; run T52 same-PR process if `openapi.json`/`schema.d.ts` changes.
- **No dual DnD cache.** DnD/render/mutations must not read/write divergent card sources.
- **Client-side search limitation must be explicit** if server-side search is not implemented.
- **React Query conventions:** targeted invalidation where possible; no `waitForTimeout` in e2e; dedupe accumulated pages by id.
- **A11y:** load-more controls keyboard/focus/announcement accessible; virtualized + incremental load keeps stable row semantics.
- Global DoD: TDD, persistence verification, both engines where relevant, gates pasted as evidence, lessons appended, no commit without approval.

## Acceptance Criteria

- [ ] `PaginationParams` enforces `u64(page)*u64(limit) <= 10_000` with overflow-safe math across all consumers; out-of-window requests return 400 problem+json `validation-error` with `errors[]`; red-proof replaces the old "page has no ceiling" case (closes **P5** page-ceiling gap).
- [ ] Paginated lists (boards, archived boards, notes, projects) expose load-more UI; items beyond page 1 reachable; accumulation stops at `total`; dedupe by id; filter/search changes reset accumulation; loading/error/retry/end states are accessible (closes **P6**).
- [ ] Archived boards pagination handles mixed active/archived API results without false-empty/incorrect stop behavior.
- [ ] Board columns embed first N=50 cards per column and expose additive authoritative per-column total-card count; remaining cards hydrate via paginated endpoint; no duplicate cards; stable order preserved (closes **P3 hydration half** with T021).
- [ ] BoardView/BoardListView counts use authoritative server totals; search/filter behavior under partial hydration is explicit and tested (no silent incomplete-search success state).
- [ ] DnD remains correct with hydration: drag source loaded; cross-column/same-column moves, reorder, create/delete/sort interactions keep cache coherent; drop near unloaded boundary has defined, tested behavior; no stale duplicates after invalidation/refetch.
- [ ] Ordering/tiebreakers are deterministic for paginated endpoints touched; boards/projects have id tiebreakers; notes mutable-`updated_at` drift limitation documented and frontend dedupes by id.
- [ ] Gates green: backend fmt/clippy/tests (+ both DB engines if SQL/behavior affected), `sqlx prepare --check` if SQL changed; frontend `tsc -b`, vitest, relevant e2e; `gen:api -- --check` if contract changed; coverage floor green; lessons appended.

## Notes

**Resolved/default decisions (2026-11-24 amendment):**
1. **Ceiling formula:** `u64(page) * u64(limit) <= 10_000` (window cap), strict no-clamp 400. Implement overflow-safe; keep error in existing validation report shape.
2. **Ceiling scope:** all `PaginationParams` consumers, not only cards/boards.
3. **Board hydration shape:** first N embedded + separate per-column paginated card fetches (reuse existing endpoint). Add additive per-column total count. Avoid dual DnD cache.
4. **Search under partial hydration:** out of scope to build server-side card search; limitation must be explicit in UX/tests/notes.
5. **Keyset/cursor:** deferred to T026 evidence.
6. **List load-more pattern:** `useInfiniteQuery` preferred for lists; manual accumulation acceptable only if it preserves dedupe, reset, and targeted invalidation semantics.

**Open calls (resolve before/early in implementation):**
1. **Resolved:** additive field is `total_cards: u64` on `ColumnResponse` / OpenAPI (additive, regenerated via `gen:api`).
2. Canonical cache strategy details for hydrated cards: merge into `['columns', boardId]` vs maintain per-column infinite cache with a single selector consumed by DnD/render. Default: keep `['columns', boardId]` canonical for DnD/render, with controlled merge from per-column fetches.
3. Archived boards: client accumulation over mixed `archived=true` results vs additive backend-only archived query mode. Default: client accumulation with careful stop/empty-state logic unless backend change is trivial and additive.
4. BoardListView: full hydration parity vs explicit loaded-only behavior with indicator. Default: parity with same cache/merge rules.

## Implementation Status (2026-11-24)

- **Staged code covers:** shared `PaginationParams` page-window ceiling; boards/projects deterministic ordering tiebreakers; column embed first 50 cards + additive `total_cards` (OpenAPI/schema regen); list load-more/infinite query for boards/archived/notes/projects; board column hydration/load-more merged into canonical `['columns', boardId]`; DnD unloaded-end guard; partial-hydration search/count indicators.
- **Verified:** `cargo fmt --check`, `cargo clippy -- -D warnings -A clippy::pedantic`, `cargo test --lib` (DB-dependent filtered, 120 passed), `cargo test --no-run`, `tsc -b`, `gen:api -- --check`, targeted vitest for previously failing files (3 env-caused api base failures remain env-related), backend API tests on MySQL 8.4/MariaDB 11.4 tmpfs containers passed.
- **Remaining for original scope:** spec-called behavior tests (embed/total_cards contract, ordering tie cases, load-more dedupe/reset/error/archived mixed-page, hydration merge/DnD unloaded-end/search limitation), full e2e/coverage/gate evidence, and final lessons/evidence cleanup.
- **Split out:** amendment scope tracked in T048-T051.

## Lessons

(pending)
