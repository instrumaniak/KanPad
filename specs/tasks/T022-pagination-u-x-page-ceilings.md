# T022: Pagination UX + page ceilings

> **STATUS: pending.** Wave 1. Closes **P5** (ceilings half) and **P6** (MEDIUM-LOW).
> **Depends on T021** (shared board-view/column files — never parallel).

## Goal

1. **Server: page ceiling.** `page` has no bound today (`extract.rs:115-135`; a test
   literally asserts "page has no ceiling") → deep offsets scan+discard. Cap
   `page * limit ≤ 10_000` → 400 outside (strict-no-clamp convention).
2. **Board cards: stop loading everything.** The board loads ALL cards unpaginated via
   the columns embed (`columns/routes.rs:69-70`) though a paginated cards API exists
   unused (`useCards(page,limit)`). Minimal-risk design: keep the columns embed for
   structure/counts, hydrate past the first N (e.g. 50) per column via the paginated
   endpoint with **infinite scroll / load-more**.
3. **Lists: load-more UI.** boards/archived/notes/projects fetch backend defaults
   (limit 50) with no pagination UI → items beyond page 1 invisible. Add load-more
   using the existing `Page<T>` envelope (`total` already returned).

## Depends On

T021.

## Phase

1

## Critical

No — P6 is LOW-MED; P5 ceilings are cheap correctness.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Pagination (design: ceiling value,
  hydration strategy, open calls: keyset deferral, server-side search interplay).
- `specs/arch-hardening-specs/00-overview.md` — findings P5, P6; global DoD;
  `backend-rs/src/extract.rs:115-135` (ceiling gap), `backend-rs/src/common/pagination.rs:13-40`
  (envelope; nested children unpaginated by contract), `frontend/src/features/columns/columns.api.ts:52-54`
  (all-cards embed), `frontend/src/features/cards/cards.api.ts:117-124` +
  `use-cards.ts` (unused paginated API), `frontend/src/features/boards/use-boards.ts:38-46`.
- T010 note: positions are now computed `sort_key` indexes — pagination must use the
  same stable ordering (sort_key + id tiebreak).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/extract.rs` | Page ceiling validation (400 outside bound) |
| `backend-rs/src/features/columns/` (+ cards routes) | First-N embedded + hydrate endpoint usage (or page param on columns embed — decide per §Notes open call) |
| `frontend/src/features/columns/column-card-list.tsx` (+ hooks) | Infinite/load-more hydration past first N (coordinate with T021's virtualizer — intersection observer feeds the fetch) |
| `frontend/src/features/boards/{board-list,archived-boards}.tsx`, notes/projects lists | Load-more controls using `Page<T>.total` |
| `frontend/src/features/boards/board-view.tsx` | Counts from server totals (retires memoized flatMap from T021 interim) |
| Tests | Ceiling 400; page-2 dedupe; drag across loaded boundary; lists load-more stop-at-total |
| `docs/lessons/{rust-backend,frontend}.md` | Dated, tagged lessons |

**Must NOT appear in the diff:** keyset/cursor contract (deferred — see open call),
search/filter rework beyond what pagination forces (document limitation if server-side
search not in scope).

## Implementation Steps

TDD.

1. **Red (ceiling):** `page=999999` → 400 (fails today — accepted).
2. Implement ceiling in the shared extract path (one owner).
3. **Red (invisible items):** list of 60 boards with limit 50 → item 51+ reachable via
   load-more (fails today: no UI).
4. Load-more on boards/archived/notes/projects (button or infinite — default: button
   for lists, IO-driven for board columns); dedupe by id; stop at `total`.
5. Board hydration: first 50 embedded; fetch next pages per column on scroll; drag
   source must be loaded (guard: can't drag an unloaded card — it isn't rendered);
   drop into a column mid-load shows loading affordance, no data loss on completion.
6. Counts: switch `BoardView` to server totals.
7. Tests: ceiling, dedupe, boundary drag, stop-at-total, existing sanitizer tests green.
8. Gates (both engines if SQL touched); `sqlx prepare --check`; typegen if contract
   additive; lesson.

## Constraints

- Ordering must be stable (T010 sort_key + id) — no page drift between fetches.
- Additive contract only (optional params) → T52 same-PR if `openapi.json` changes.
- No keyset cursors unless T026 later proves need (open call — record, don't build).
- Search/filter: current board search is client-side over loaded cards — if hydration
   changes what's loaded, **document the limitation** in §Notes (server-side search is
   a product decision, not silently half-done here).
- T021's virtualizer + this task's observer must cooperate (depends edge exists;
  verify in final pass).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] `page` beyond ceiling → 400 strict (red-proof) (closes ceilings half of **P5**).
- [ ] Lists beyond page 1 reachable via load-more; stops at `total`; dedupe proven
      (closes **P6**).
- [ ] Board: >100-card column renders first screen fast; cards past first N load on
      scroll; boundary drag works (test or documented pass) (closes hydration half of
      **P3** — with T021).
- [ ] Counts correct from server totals.
- [ ] Stable ordering (sort_key, id) across pages; existing pagination sanitizer tests
      green; full suites + typegen gates green.
- [ ] Search limitation (if any) documented in §Notes; lesson appended.

## Notes

(filled in during/after implementation)

**Open calls:**
1. Hydration shape: page param on columns embed vs separate per-column card pages.
   *Default: separate paginated card fetches (reuses existing endpoint + useCards).*
2. Server-side search for cards: in-scope? *Default: no — document limitation; revisit
   with T023/product input.*
3. Keyset cursors: defer to T026 evidence. *Default: defer.*

## Lessons

(pending)
