# T015: Board error states + error boundary resetKeys

> **STATUS: done (2026-11-17).** Wave 0. Closes **R6** (MEDIUM).
> Amended 2026-11-17 per pre-implementation review + industry-standards research (see §Amendments D1–D8).
> Implementation + full gates complete 2026-11-17; §Notes has evidence. No commit without approval (AGENTS.md).

## Goal

A failed board/columns fetch must show an error with retry — never a silently **empty
board** (indistinguishable from a real empty board). And a route error must not stick:
navigation resets the boundary.

1. `board-view.tsx`: add `isError`/`error` branches for **both** the `board` and
   `columns` queries → reuse the existing `ErrorState` + retry pattern from
   `board-list.tsx` (the convention exists; apply it). With two pinned decisions
   (§Amendments): **404** board fetch keeps the "Board not found" UX (permanent, not
   retryable); **401-shaped** errors render the loading state — the global T014
   session-expiry handler owns the 401 UX (clear cache + redirect to `/login`).
2. `App.tsx` → `RouteErrorBoundary`: pass `resetKeys={[location.pathname]}` so
   navigation clears the fallback (component already supports `resetKeys`; App passes
   none today). `App()` renders `<BrowserRouter>`, so `useLocation()` is **not in
   scope** in `App` — the reset key is wired through a small wrapper component that
   renders **inside** the router (industry-standard pattern, see §Amendments D2).
3. Sweep: `note-list.tsx` gets the same `isError` branch (identical defect: failed
   notes fetch renders "No notes yet"). Project page already has its `ErrorState`;
   note detail/edit pages and sidebar/selector surfaces are **out of scope** (see D5).
4. `route-error-boundary.tsx`: port the upstream `react-error-boundary`
   `prevState.didCatch` guard so a crash caught in the same commit as a key change
   does not reset on that commit (D7).

## Depends On

(None — independent of T013/T014; owns `board-view.tsx`, which T021/T022 later touch.)

## Phase

0

## Critical

No — R6 is MEDIUM (misleading UI, no data impact).

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R6 (design + edge cases).
- `specs/arch-hardening-specs/00-overview.md` — finding R6; global DoD.
- `frontend/src/features/boards/board-view/board-view.tsx:167-181` (the defect),
  `frontend/src/features/boards/board-list.tsx:32-40` (the pattern to reuse),
  `frontend/src/components/route-error-boundary.tsx:60-72` (resetKeys support),
  `frontend/src/App.tsx:70-93` (boundary wiring).
- `frontend/src/components/error-state.tsx` (reuse target),
  `frontend/src/lib/api/client.ts:45-66` (`TimeoutError` / `ApiError{status}` shapes).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/features/boards/board-view/board-view.tsx` | `isError` branches for board + columns queries → `ErrorState` with `onRetry` (refetch both); 404 board keeps not-found; 401-shaped renders loading |
| `frontend/src/features/boards/board-view/board-view.test.tsx` | Red-first: columns reject → error UI + retry (not empty board); board reject → error UI (5xx/network); 404 → not-found; 401 → no error UI |
| `frontend/src/components/route-error-boundary.tsx` | Port upstream `prevState.didCatch` guard (D7) |
| `frontend/src/components/route-error-boundary.test.tsx` | Red-first: same-commit crash+key-change does not reset on that commit; keys change after error commit resets |
| `frontend/src/components/route-error-boundary-with-location.tsx` | **NEW** — `RouteErrorBoundaryWithLocation`: `useLocation()` inside the router, passes `resetKeys={[location.pathname]}` (D2) |
| `frontend/src/components/route-error-boundary-with-location.test.tsx` | **NEW** — Red-first: boundary fallback → navigate → children render again (MemoryRouter harness) |
| `frontend/src/App.tsx` | Swap `<RouteErrorBoundary>` for `<RouteErrorBoundaryWithLocation>` |
| `frontend/src/features/notes/note-list.tsx` | `isError` branch → `ErrorState` inside the scroll area (create/header stay usable) |
| `frontend/src/features/notes/note-list.test.tsx` | Red-first: notes reject → error UI + retry, not empty state |
| `docs/lessons/frontend.md` | Dated, tagged lesson |
| `specs/tasks/status.yaml` + this file | Status consistency (D6) |

**Must NOT appear in the diff:** loading/skeleton logic changes, query key changes,
T014's 401 handling (401 routes through T014, not the board error UI — see D4),
note detail/edit pages (D5), `throwOnError` adoption (keeps the codebase's isError-branch convention; industry-documented default — query errors never reach boundaries without it).

## Implementation Steps

TDD: red-first.

1. **Red test:** mock `['columns', boardId]` rejection → render `BoardView` → assert
   error UI visible and board grid NOT rendered as empty (fails today: renders empty).
   Same for `board` query rejection.
2. **Red test:** board rejection with `status: 404` → assert "Board not found" renders
   (not the generic error UI). Board/columns rejection with `status: 401` → assert
   **no** `ErrorState` renders (global T014 handler owns UX).
3. **Red test:** `RouteErrorBoundary` with `resetKeys` → a crash caught in the same
   commit as a key change must leave the fallback visible (upstream
   `prevState.didCatch` guard; port + red-proof).
4. **Red test:** `RouteErrorBoundaryWithLocation` under `MemoryRouter` → crash →
   navigate → children render again without clicking "Try Again" (fails today: App
   passes no resetKeys).
5. Implement both; keep skeleton-while-loading behavior unchanged.
6. **401 coordination (T014 landed 2026-11-17 — the "interim behavior" branch of the
   old step is moot):** 401-shaped errors in the board view render the loading state;
   the global handler clears the cache, soft-navigates to `/login` and toasts once.
   Document the one-frame-flash reasoning in §Notes.
7. **404 semantics:** `ApiError{status:404}` on the board query → existing
   "Board not found" view (no retry button — permanent). All other failures
   (5xx/network/`TimeoutError`/403) → `ErrorState` + retry.
8. **Sweep:** `note-list.tsx` `isError` branch (inline `ErrorState` in the scroll
   area); red-first test. Project page already covered (`project-list.tsx:293`);
   `archived-boards.tsx` already covered. Out of scope: `note-detail-page.tsx`,
   `note-edit-page.tsx` (conflate error with not-found, no retry — separate follow-up),
   `board-notes-sidebar.tsx` / `note-link-selector.tsx` (secondary surfaces).
9. Full vitest (`npm test -- --project=unit` or `vitest run --project unit`) +
   `tsc --noEmit`; lesson.

## Constraints

- Reuse existing `ErrorState` — no new component (shadcn/convention rules).
- `resetKeys` change must not swallow real crashes (keep MAX_RETRIES semantics;
  same-commit crash+key-change must NOT reset on that commit — D7).
- Skeleton/error mutual exclusion (no flicker): loading branch stays first and
  unchanged; error branch only after `isError` is stable (4xx never retried post-T014;
  5xx/network retried once then `isError`).
- 404 → not-found; everything else → ErrorState+retry; 401 → T014's handler. These
  three shapes must not collapse into one another.
- Global DoD; no commit without approval.

## Acceptance Criteria

- [x] Board/columns fetch failure (non-404, non-401) → error UI + working retry
      (refetches both); never an empty-looking board (closes **R6**, red-proof pasted).
- [x] 404 board fetch → "Board not found" unchanged (no misleading retry button).
- [x] 401-shaped rejection → no `ErrorState`; global T014 handler owns the UX
      (decision + rationale documented in §Notes).
- [x] Route navigation resets the error boundary (red-first wiring test green;
      same-commit crash does not self-reset on that commit — D7).
- [x] Loading skeletons unchanged; existing board tests green; `note-list` test green;
      full vitest (**793/793**, 83 files) + `tsc --noEmit` clean.
- [x] 401-vs-error interaction documented in §Notes.
- [x] Lesson appended.

## Amendments (2026-11-17, pre-implementation — review + industry-standards research)

- **D1 — Path drift fixed.** `board-view.tsx` lives at `board-view/board-view.tsx`;
  the spec referenced `features/boards/board-view.tsx` (pre-reorg path); test file path
  corrected likewise. `App.tsx:77-100` → `:70-93` (drifted after T014 mounted
  `<SessionExpiryHandler />`).
- **D2 — `useLocation()` is not in scope in `App()`.** `App()` renders
  `<BrowserRouter>` itself; the boundary is *inside* it, so `location` cannot be read
  in `App()`. Industry standard (react-error-boundary docs, community guides,
  error-recovery.com, Marigold UI): a small wrapper component **inside** the router
  that calls `useLocation()` and passes `resetKeys={[location.pathname]}` — chosen over
  `key={location.pathname}` (remount/demolition, loses subtree state). Verified against
  react-router v7 semantics (`useLocation` must render under the router).
- **D3 — 404 vs other errors must not collapse (mandatory).** `fetchBoard` throws
  `ApiError{status}` on non-OK (`apiFetch`, client.ts). A naive `isError` branch would
  change a genuinely-missing board (404) from the deliberate "Board not found" UX into
  "Failed to load board + Retry" — a permanent error with a useless retry button.
  Industry standard (TanStack Query docs + community guidance: *"A 404 and a 500 should
  not have the same message… Don't retry 4xx"*): 404 → not-found view, no retry;
  5xx/network/`TimeoutError` → `ErrorState` + retry. Red test rejects with 5xx; a
  separate 404 test pins the split.
- **D4 — 401 ownership (mandatory).** T014 **landed 2026-11-17** (retry predicate never
  retries 4xx; `SessionExpiryHandler` = single-flight clear cache → soft navigate
  `/login` → one toast). Component-level generic error UI for 401 is the
  double-handling anti-pattern (axios-auth-refresh lineage; TanStack `QueryCache`
  onError). Decision: board `isError` branch renders the **loading** state for
  401-shaped errors — the handler owns redirect/toast; cache-clear re-pends the queries.
  Suppression avoids the one-frame ErrorState flash; the flash would also be harmless
  (soft nav unmounts) — documented, not relied on.
- **D5 — Sweep scope pinned.** `project-list.tsx:284-297` and `archived-boards.tsx`
  already have `isError` + `ErrorState` (verified). `note-list.tsx:65,155-176` has the
  **identical silent-empty defect** ("No notes yet" on fetch failure) — in scope, inline
  `ErrorState` in the scroll area (header/create stay usable). `note-detail-page.tsx:23`
  / `note-edit-page.tsx` conflate error with not-found and need a product decision
  (retry vs not-found copy) — **out of scope** (follow-up). `board-notes-sidebar.tsx` /
  `note-link-selector.tsx` — out of scope (secondary surfaces).
- **D6 — Status consistency.** `specs/tasks/status.yaml` said `T015: in_progress`; this
  file said `pending`. Both now read consistently.
- **D7 — Port the upstream `prevState.didCatch` guard (new, standards-aligned).**
  Upstream `react-error-boundary` resets only when `didCatch` in **both** prev and
  current state — *"prevents an immediate reset when the key that triggered the error is
  itself inside resetKeys"*. KanPad's hand-written boundary lacks this guard: a crash in
  the same commit as a `resetKeys` change resets immediately → children re-render →
  crash again (bounded, but one whole extra crash of the subtree; violates the spirit of
  this task's "resetKeys must not swallow real crashes" constraint). Port the guard
  (~2 lines) + a same-commit-crash test. This is the reference implementation's exact
  semantics.
- **D8 — Verified-accurate claims.** "Existing 174 board tests green" — counted exactly
  174 `it/test` blocks in board feature tests (verified 2026-11-17). `board-list.tsx:32-40`,
  `route-error-boundary.tsx:60-72` line refs accurate. `ErrorState` props match intended
  reuse. `RouteErrorBoundary` resetKeys element-wise compare means a fresh array literal
  per render is safe (length+`Object.is`, upstream `hasArrayChanged` semantics — confirmed
  against upstream source). TanStack v5: query errors never reach boundaries by default —
  so isError branches + boundary are complementary, both required. No `throwOnError`
  adoption (codebase convention + industry default).

Research sources: `/tanstack/query` docs (throwOnError, retry predicate), Context7
`/bvaughn/react-error-boundary` (componentDidUpdate, `hasArrayChanged`, guard),
react-router v7 docs (`useLocation` scoping), error-recovery.com / Marigold / Flare /
ussue #148 (resetKeys-on-pathname pattern), TanStack discussion #1385 + axios-auth-refresh
(401 single-flight global handler), RFC 9110 status semantics,
ma-x.im/blog/react-playbook-error-handling (404≠500 UX).

## Notes

(filled in during implementation — 2026-11-17)

**Implementation evidence (2026-11-17):**

- **Red-first.** Red run before implementation, 4 test files: `Tests 6 failed | 51 passed (57)`
  — board-view 4 (columns-error, board-error, 401-board, 401-columns), note-list 1,
  route-error-boundary 1 (same-commit guard), plus `route-error-boundary-with-location.test.tsx`
  failing at import (module didn't exist yet — its intent was pinned by the test itself).
  The 404 pin test was green pre-implementation by design (regression guard, intentional).
- **Board view.** `useBoard`/`useColumns` now also destructure `isError`/`error`/`refetch`.
  Render order: loading (unchanged markup, skeleton while loading) → **401-shaped errors
  render the loading state** (see D4 rationale) → **generic error branch** (5xx/network/
  `TimeoutError`/403 → `ErrorState`, title "Failed to load board", message = server
  message with `Error` fallback, `onRetry` refetches **both**) → `!board` → "Board not found".
  Board 404 falls through to not-found via `boardNotFound` (no retry button). Columns-404
  is treated as a generic error (a columns endpoint 404 for an existing board has no
  not-found semantics). Loose `errorStatus()` helper reads `.status` structurally —
  no dependence on `ApiError` class identity across module boundaries (same rationale as
  T014's handler duck-typing).
- **401-vs-error interaction (AC).** T014 landed 2026-11-17: its retry predicate never
  retries 4xx, so a 401 reaches `isError` — but `SessionExpiryHandler` fires from
  `QueryCache`/`MutationCache` `onError` (single-flight; cache clear → soft `navigate('/login')`
  → one toast) **regardless of what the component renders**. The board view rendering its
  own error UI for that same 401 would be the double-handling anti-pattern (and a possible
  one-frame flash of a permanently-retrying error for an action that never succeeds).
  Decision: 401-shaped errors render the loading state; the handler's cache-clear re-pends
  the queries anyway. No new handling in feature code, no swallowed errors — the global
  handler owns the UX.
- **Route boundary.** `App()` renders `<BrowserRouter>`, so `useLocation()` is impossible
  in `App()` — new `components/route-error-boundary-with-location.tsx`
  (`RouteErrorBoundaryWithLocation`) renders **inside** the router and feeds
  `resetKeys={[location.pathname]}`. Chosen over `key={pathname}` (demolition/remount) per
  industry guidance (resetKeys = targeted flag clear; preserves subtree state).
- **Same-commit crash guard (D7).** Ported upstream `prevState.didCatch` parity:
  `componentDidUpdate` now requires `prevState.hasError` — never resets on the commit that
  caught the error. Red-proof: guard neutralized → the new same-commit test fails
  (`componentDidCatch` fires twice — boundary resets on the same commit → children re-render
  → deterministic crash fires again); guard restored → 7/7 boundary tests green.
- **Wiring test fidelity (worth remembering).** React Router v7 (`react-router-dom@^7.18.5`)
  wraps router state updates in `startTransition`, so a bare `act(() => navigate('/safe'))`
  did **not** flush the pathname change to the wrapper — the test showed no re-render at all.
  Driving navigation through a real React event (a `fireEvent.click` on an always-mounted
  "Go to safe page" button, the app-shell pattern) flushes correctly. Test asserts: crash →
  fallback → click → new route renders, alert gone. (Debugging confirmed: wrapper renders
  re-fired with `/safe` only via the event path.)
- **Deterministic effect metric.** React re-invokes the crashing child during its error
  recovery passes, so counting child invocations is meaningless (observed 3 ≠ 1 naive
  expectation). `componentDidCatch` fires once per caught crash — the test asserts on
  `console.error` calls with the exact `RouteErrorBoundary caught:` prefix (React's own
  error logs use different prefixes). This is the metric the upstream guard controls.
- **Sweep.** `note-list.tsx`: `isError` branch renders `ErrorState` inline in the
  `ScrollArea` (header + create button stay usable) with red-first test. `project-list.tsx:293`
  and `archived-boards.tsx` already had `ErrorState`. Note detail/edit pages conflate error
  with not-found (no retry) — deliberately excluded (D5); `board-notes-sidebar` /
  `note-link-selector` out of scope.
- **Independent review (2026-11-17, general agent, verdict "request changes"):** one Major +
  three Minor + nits; all fixed or explicitly accepted:
  - **M1 (fixed, red-first).** React Query v5 preserves `data` on refetch failure — the
    error branch must gate on **data absence** (`boardQueryError && !boardResponse`,
    `columnsQueryError && !columns`; note-list: `isError && !data`), or a background
    invalidation refetch failing would unmount a perfectly good board for a "Failed to
    load board" card. Regression test: cached-data mocks (`isError:true` + `data` present)
    must render the board, no alert — red before the fix, green after (4 new tests:
    cached-columns, cached-board, both-fail, board-404+columns-500).
  - **m1 (accepted, documented).** The 401 carve-out is deliberately coarse: a 401 on one
    query + a 500 on the other both render loading while T014 is mounted (it redirects
    regardless). Code comment added at the branch; §Notes documents the reliance on
    `SessionExpiryHandler` being mounted in `App.tsx`.
  - **m2 (fixed).** `errorStatus` now `typeof status === 'number'` — no unchecked cast.
  - **m3 (fixed).** `note-list.tsx` mirrors the 401 suppression (skeleton, no error card)
    for the D4 rationale — the notes page got the same treatment, not a special case.
  - **m4 (addressed).** Tests added for the M1 scenario, both-queries-fail, board-404 +
    columns-500, notes header-stays-interactive during error, notes-401 suppression,
    explicit `isError: false` in the outer notes beforeEach (n3). App-level render smoke
    test deliberately **not** added — no `App.test.tsx` exists, the App tree pulls lazy
    routes + providers (heavy, brittle); wiring is covered by `tsc` (existence) +
    `route-error-boundary-with-location.test.tsx` (mechanism, red-proofed by stripping
    resetKeys). Accepted gap, noted here.
  - **n1 (fixed).** Element compare uses `Object.is` (exact upstream `hasArrayChanged` parity).
  - **n2 (accepted).** No `.stories.tsx` for the wrapper — wiring, not visual; nothing drifted.
  - **n3 (fixed).** See m4. **n4 (accepted).** "Failed to load board" title regardless of which
    query failed — message carries the detail (board-list convention), parity kept.
  - Post-fix gates: **793/793** (was 786; +4 board-view +3 note-list), `tsc` clean,
    eslint clean on all changed files.
- **Gates (2026-11-17):** `vitest run --project unit` → **793/793 across 83 files**
  (was 777 at T014; +9 new, +7 review-fix); `npx tsc --noEmit` → clean; `npx eslint` on all changed
  files → clean (exit 0). Storybook project not affected (`--project=unit` only).
- **No commit** — awaiting user approval (AGENTS.md).

## Lessons

See `docs/lessons/frontend.md` — "Board error states + navigation-reset route boundary
(2026-11-17, T015)".
