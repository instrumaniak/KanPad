# T021: Board render perf — virtualization + dragOver throttle

> **STATUS: pending.** Wave 1. Closes **P3** (HIGH) and **P4** (MEDIUM-HIGH).

## Goal

A board with hundreds of cards must stay smooth (NFR: drag < 100 ms input latency):

1. **Virtualize card lists** — `@tanstack/react-virtual` on `column-card-list` and
   `board-list-view` (today: every card mapped, O(N) `CardDetailPanel` instances, no
   virtualization anywhere).
2. **Throttle dragOver cache writes** — `handleDragOver` writes
   `setQueryData(['columns', boardId])` on **every dragOver event** during cross-column
   drags; rAF-coalesce to ≤ 1 write/frame, and prefer a lightweight local hover state
   (dnd-kit over-id) over rewriting the whole columns cache — the cache write exists
   only to move the card visually; `dragEnd`'s mutation + invalidation is the real sync.
3. **Stabilize identities** — stop recreating `allColumns`/`cards` arrays per write
   (defeats React Compiler); memoize the double-flatMap board counts.

## Depends On

(None. T022 depends on this — both own `board-view.tsx`/column list files.)

## Phase

1

## Critical

No — but P3 is HIGH and it's the core UX at 100+ users with real board sizes.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Board rendering (design: virtualization
  + dnd-kit interplay, rAF coalescing, identity stabilization, edge cases, test plan).
- `specs/arch-hardening-specs/00-overview.md` — findings P3, P4; global DoD.
- `frontend/src/features/cards/drag-drop-context.tsx:100-186` (dragOver defect),
  `frontend/src/features/boards/board-view.tsx:111-112,256,269` (counts + allColumns
  identity), `frontend/src/features/columns/column-card-list.tsx:22-29`,
  `frontend/src/features/boards/board-list-view.tsx:258-350`,
  `frontend/src/features/cards/card.tsx:32-43` (per-card cost).
- dnd-kit + virtualization integration notes (verify against current dnd-kit docs).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/package.json` | + `@tanstack/react-virtual` |
| `frontend/src/features/columns/column-card-list.tsx` | Virtualizer (estimated row height, overscan; dnd-kit `SortableContext` over visible items) |
| `frontend/src/features/boards/board-list-view.tsx` | Virtualize flat card rows |
| `frontend/src/features/cards/drag-drop-context.tsx` | rAF-coalesced dragOver updates; hover via local state/over-id where possible; keep optimistic rollback + `dragSource`-at-start |
| `frontend/src/features/boards/board-view.tsx` | Stabilize `allColumns` identity; `useMemo` counts |
| Tests | Red-first: dragOver write-count ≤ 1/frame; identity stability; existing drag suites green |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** pagination changes (T022), optimistic-rollback
semantics, sensor distances (documented touch/context-menu interplay stays).

## Implementation Steps

TDD.

1. **Red proof (P4):** synthetic cross-column drag sequence → spy `setQueryData` calls
   → count ≫ frames (fails today). Also profile marker: full-board re-render per write
   (React Profiler paste optional but encouraged).
2. **Fix dragOver:** rAF schedule (one pending frame max), derive visual hover from
   dnd-kit `over` id + local state instead of full-cache rewrite where feasible; keep
   same-column fast path; cache write only on dragEnd (plus any correctness need —
   verify optimistic model still works).
3. **Red test (identity):** `allColumns`/`cards` array references stable across dragOver
   writes (structural sharing or memo).
4. **Virtualize:** add `@tanstack/react-virtual`; wire into both lists with dnd-kit
   (SortableContext items = visible/virtual range; DragOverlay already used → preview
   unaffected). Verify keyboard sensor reaches off-screen rows (dnd-kit scrolls — test),
   touch 750 ms delay unaffected, empty/single-card columns.
5. Perf smoke: 500-card board (seeded) — drag smoothness + paste before/after render
   counts or latency note; precise numbers belong to T026.
6. Full vitest + tsc; existing drag-drop unit + e2e suites green; lesson.

## Constraints

- dnd-kit + virtualization conflict is the main risk — if drag preview/keyboard breaks,
  fall back to **conditional virtualization (>50 cards)** and document (open call).
- Optimistic update + rollback + `dragSource` conventions untouched (AGENTS.md).
- Counts must stay correct (server totals arrive with T022 — until then, memoized
  flatMap is acceptable).
- React Compiler stays enabled (don't hand-memo everything — memoize the *inputs*).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] dragOver cache writes ≤ 1 per animation frame (red-first test pasted) (closes
      **P4**).
- [ ] `allColumns`/cards identities stable across dragOver (test green).
- [ ] Card lists virtualized (DOM node count for a 500-card board ≪ N — paste
      `document.querySelectorAll` count) (closes **P3** rendering half).
- [ ] Keyboard sensor, touch delay, empty column, drag preview all verified (tests or
      documented manual pass).
- [ ] Existing drag-drop unit + e2e suites green; full vitest + `tsc --noEmit` clean.
- [ ] Lesson appended.

## Notes

(filled in during/after implementation)

**Open call:** always-virtualize vs >50-card threshold (dnd-kit interplay). *Default:
always, with threshold fallback if keyboard/drag regressions appear.*

## Lessons

(pending)
