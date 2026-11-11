# T015: Board error states + error boundary resetKeys

> **STATUS: pending.** Wave 0. Closes **R6** (MEDIUM).

## Goal

A failed board/columns fetch must show an error with retry — never a silently **empty
board** (indistinguishable from a real empty board). And a route error must not stick:
navigation resets the boundary.

1. `board-view.tsx`: add `isError`/`error` branches for **both** the `board` and
   `columns` queries → reuse the existing `ErrorState` + retry pattern from
   `board-list.tsx` (the convention exists; apply it).
2. `App.tsx` → `RouteErrorBoundary`: pass `resetKeys={[location.pathname]}` so
   navigation clears the fallback (component already supports `resetKeys`; App passes
   none today).
3. Sweep: same missing-error-state check on notes list / project page if the pattern
   applies cheaply (board view is the AC).

## Depends On

(None — independent of T013/T014; owns `board-view.tsx`, which T021/T022 later touch.)

## Phase

0

## Critical

No — R6 is MEDIUM (misleading UI, no data impact).

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R6 (design + edge cases).
- `specs/arch-hardening-specs/00-overview.md` — finding R6; global DoD.
- `frontend/src/features/boards/board-view.tsx:167-181` (the defect),
  `frontend/src/features/boards/board-list.tsx:32-40` (the pattern to reuse),
  `frontend/src/components/route-error-boundary.tsx:60-72` (resetKeys support),
  `frontend/src/App.tsx:77-100` (boundary wiring).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/features/boards/board-view.tsx` | `isError` branches for board + columns queries → `ErrorState` with `onRetry` (refetch both) |
| `frontend/src/App.tsx` | `resetKeys` prop on `RouteErrorBoundary` |
| Optional: notes/project pages | Same pattern only if trivially applicable |
| Tests: `board-view.test.tsx`, `route-error-boundary.test.tsx` | Red-first: rejected columns query → error UI + retry (not empty board); error → navigate → routes render again |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** loading/skeleton logic changes, query key changes,
T014's 401 handling (401 should route through T014, not show as a board error — see
edge cases).

## Implementation Steps

TDD: red-first.

1. **Red test:** mock `['columns', boardId]` rejection → render `BoardView` → assert
   error UI visible and board grid NOT rendered as empty (fails today: renders empty).
   Same for `board` query rejection.
2. **Red test:** boundary renders fallback on child error → change route → assert
   children render again without clicking "Try Again" (fails today: no resetKeys).
3. Implement both; keep skeleton-while-loading behavior unchanged.
4. Edge pass: ensure a 401 during board load doesn't flash the error UI before T014's
   redirect (guard: if error is 401-shaped, let the global handler win — coordinate
   with T014's handler; if T014 not landed yet, document the interim behavior).
5. Full vitest + `tsc --noEmit`; lesson.

## Constraints

- Reuse existing `ErrorState` — no new component (shadcn/convention rules).
- `resetKeys` change must not swallow real crashes (keep MAX_RETRIES semantics).
- Skeleton/error mutual exclusion (no flicker).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Board/columns fetch failure → error UI + working retry; never an empty-looking
      board (closes **R6**, red-proof pasted).
- [ ] Route navigation resets the error boundary (red-first test green).
- [ ] Loading skeletons unchanged; existing 174 board tests green; full vitest +
      `tsc --noEmit` clean.
- [ ] 401-vs-error interaction documented in §Notes (or coordinated with T014).
- [ ] Lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
