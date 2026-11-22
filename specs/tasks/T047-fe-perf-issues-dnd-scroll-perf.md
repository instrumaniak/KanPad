# T047: fe perf issues: dnd, scroll perf

## Goal

Investigate and reduce frontend rendering/performance issues exposed by the `@perf` e2e suite, especially drag-and-drop and column scroll performance in board view. Keep fixes behavior-preserving and covered by existing/new tests.

## Depends On

- T046 (Performance harness & tooling)

## Phase:

Post-T046 performance follow-up (board view / DnD / virtualization)

## Critical:

No (not a release blocker by itself), but perf regressions/jank are user-visible in dense boards.

## Spec References

- `specs/project-context.md`
- `frontend/e2e/perf-drag.spec.ts`
- `frontend/e2e/perf-scroll.spec.ts`
- `frontend/e2e/perf-load-heap.spec.ts`
- `frontend/e2e/perf-helpers.ts`
- `frontend/e2e/test-utils.ts`
- `frontend/src/features/boards/board-view/board-view.tsx`
- `frontend/src/features/cards/drag-drop-context.tsx`
- `frontend/src/features/cards/drag-over-cache.ts`
- `frontend/src/features/columns/column-card-list.tsx`

## Files to Create/Modify

### Modified so far

- `frontend/src/features/cards/use-cards.ts` - seed card detail cache on update success.
- `frontend/src/features/columns/column-card-list.tsx` - `useDndMonitor` for active drag id; `React.memo`; `directDomUpdates: true`; columnId perf meta.
- `frontend/src/features/columns/column.tsx` - removed `allColumns` cascade; added `nextColumnId`/`moveTargets`; `React.memo`; pass `columnId` to list.
- `frontend/src/features/columns/column-header.tsx` - accepts `moveTargets` instead of full `allColumns`.
- `frontend/src/features/boards/board-view/board-view.tsx` - stable `moveTargets`/`nextColumnIdById` memoized by column metadata key.
- `frontend/src/features/cards/card.tsx` - extracted memoized `CardFace` perf boundary to isolate `useSortable` rerenders from card content.
- `frontend/src/lib/perf-profiler.tsx` - added optional `meta`, marks, back-compat store fields.
- `frontend/e2e/perf-helpers.ts` - added perf mark/meta helpers and `perfColumnRenderCounts`.
- `frontend/e2e/perf-drag.spec.ts` - throttled-4x timeout/run mitigation; per-column/CardFace diagnostics.
- `frontend/e2e/perf-scroll.spec.ts` - per-column/CardFace diagnostics.
- `frontend/e2e/due-dates.spec.ts` - more robust calendar day selection (role + keyboard Enter).
- `frontend/src/features/boards/board-view/render-budget.test.tsx` - mock `useDndMonitor`.
- `frontend/src/features/columns/column-card-list-pin.test.tsx` - `useDndMonitor` mock; memo-safe rerender.
- `frontend/src/features/columns/column-card-list.test.tsx` - mock `useDndMonitor`.
- `frontend/src/features/columns/column.test.tsx` - updated for `nextColumnId` prop.
- `frontend/src/features/boards/board-view/board-view.test.tsx` - updated Column mock props.

### Key files to inspect for remaining work

- `frontend/src/features/boards/board-view/board-view.tsx`
- `frontend/src/features/cards/drag-drop-context.tsx`
- `frontend/src/features/cards/drag-over-cache.ts`
- `frontend/src/features/cards/card.tsx`
- `frontend/src/features/cards/card-draggable.tsx`
- `frontend/src/features/cards/card-preview.tsx`
- `frontend/src/features/columns/column.tsx`
- `frontend/src/features/columns/column-droppable.tsx`
- `frontend/src/features/columns/column-card-list.tsx`
- `frontend/src/lib/perf-profiler.tsx`
- `frontend/e2e/perf-drag.spec.ts`
- `frontend/e2e/perf-scroll.spec.ts`
- `frontend/e2e/column-add-card-placement.spec.ts`

## Implementation Steps

1. [x] Capture baseline from failing/slow e2e run and identify perf harness budgets.
2. [x] Fix due-dates e2e race caused by stale card detail cache after update.
3. [x] Reduce DnD render fanout by replacing `useDndContext()` in `ColumnCardList` with `useDndMonitor()` for active drag id pinning.
4. [x] Stabilize BoardView -> Column props: remove `allColumns` array prop, pass `nextColumnId` and memoized `moveTargets`.
5. [x] Add explicit `React.memo` to `Column` and `ColumnCardList`.
6. [x] Mitigate `throttled-4x` perf-drag timeout (test timeout + 1 warmup/2 measured for throttled case; seeding timeout bump).
7. [x] Re-run targeted unit tests, typecheck, lint, perf-drag e2e, and full frontend e2e.
8. [x] Add instrumentation to attribute render/commit causes (Profiler meta, per-column counts, CardFace profiler, e2e per-column diagnostics).
9. [x] Profile drag render fanout and add CardFace memo boundary: confirmed per-column ColumnCardList commits still ~all columns/commit (useSortable/DndContext), but CardFace renders dropped to ~672/run vs prior implied ~36k/card-content rerenders.
10. [~] Profile scroll commits/jank: added `directDomUpdates` to ColumnCardList; maxDt improved 116.8ms -> 90.1ms, commits still 240/240 because range/isScrolling changes per step. BoardView Profiler is subtree-commit count, not necessarily BoardView component renders.
11. [ ] Evaluate remaining scroll optimizations carefully: double scroll container, further virtualizer tuning, card DOM cost, LoAF/longtask sources; keep add-a-card placement contract intact.
12. [ ] Re-run full `npm run test:e2e` after final perf changes; record evidence before tightening budgets.

## Constraints

- Follow `frontend/CONVENTIONS.md` and AGENTS e2e/perf rules.
- Do not weaken perf assertions without justification; current budgets are generous dev-mode tripwires.
- Keep add-a-card placement contract intact (`frontend/e2e/column-add-card-placement.spec.ts`).
- Be mindful of React Compiler being active in Vite dev/build; manual memo should only be used where it provides clear value.
- Avoid `waitForTimeout()` in e2e tests unless there is no deterministic alternative.
- Use targeted cache invalidation and preserve existing structural-sharing behavior in drag-over cache.
- Instrumentation must be DEV-gated/zero-cost in prod where possible; do not add noisy console logs in normal runs.

## Acceptance Criteria

- [x] Original due-dates e2e failure fixed and passing.
- [x] Original throttled-4x perf-drag timeout no longer fails.
- [x] Full frontend e2e suite passes (79 passed, 0 failed, 2026-11-22 run shared by user) - needs rerun after latest perf changes.
- [x] `tsc -b --pretty false` passes.
- [x] `npm run lint` passes (0 errors; 1 pre-existing `useVirtualizer` incompatible-library warning in board-list-view).
- [x] Drag instrumentation attributes rerenders: per-column ColumnCardList + CardFace counts confirm `useSortable`/DndContext wrapper fanout across all columns, while CardFace memo skips card-content rerenders.
- [~] Drag actual work reduced materially (CardFace ~672/run vs implied ~36k content rerenders), but ColumnCardList subtree-commit count remains ~1730/run because sortable wrappers still rerender; metric needs refinement or deeper dnd-kit isolation.
- [~] Scroll perf improved (maxDt 116.8ms -> 90.1ms) with `directDomUpdates`; commits remain 240/240 due range/isScrolling changes; LoAF/longtasks still high (187/113).
- [ ] Perf improvements validated with full `@perf` and full e2e after final changes.

## Plan (2026-11-22, web research + instrumentation)

### Web findings

- **dnd-kit `useSortable` rerenders all sortable items** in v6/8 because it consumes DndContext. Standard fix: memoize expensive child/content inside the sortable wrapper; wrapper may rerender but memo boundary skips when props stable. Sources: [dnd-kit #1379](https://github.com/clauderic/dnd-kit/issues/1379), [SO 78004828](https://stackoverflow.com/questions/78004828/how-to-stop-minimize-sortableitem-re-renders-during-drag-events-with-dndkit). dnd-kit-next/experimental improves this but is an alpha migration.
- **React Profiler `onRender` fires for any commit in the profiled subtree**; `id` identifies the Profiler tree, phases include `mount|update|nested-update`. Sources: [React Profiler](https://react.dev/reference/react/Profiler), [nested-update commit](https://github.com/facebook/react/commit/393c452e3). Implication: BoardView "240 renders" is likely 240 subtree commits, not 240 BoardView component renders.
- **TanStack React Virtual v3.14.x (installed)** includes `useFlushSync` (default true) and `directDomUpdates`/`directDomUpdatesMode`. `directDomUpdates` skips React rerenders for scroll-only updates by writing item positions/container size to DOM; requires virtualizer-owned main-axis style, `containerRef`, no container height style, absolute anchored items. Source: [TanStack Virtual React docs](https://tanstack.com/virtual/latest/docs/framework/react/react-virtual).

### Hypothesis matrix

| Symptom | Likely cause | Confidence | Confirmation |
|---|---|---:|---|
| Drag ColumnCardList renders ~14.4/15 per commit | `useSortable` in `CardDraggable` subscribes to DndContext; all mounted cards rerender on drag context updates | High | Render-budget/vitest probe + per-column Profiler/meta counts + CardDraggable render counter; React DevTools "why did this render" |
| Scroll 240/240 BoardView/ColumnCardList commits | Virtualizer scroll commits + Profiler subtree counting; only scrolled column rerenders | Medium-high | BoardView body render counter vs Profiler commits; per-column profiler ids/meta |
| Scroll 116ms maxDt, 152 LoAF, 106 longtasks | Virtualizer measure/DOM work, card DOM complexity, possible double scroll container/padding | Medium | DevTools Performance trace; experiment with `directDomUpdates` / layout simplification |

### Instrumentation plan

1. **PerfProfiler metadata/events** (`frontend/src/lib/perf-profiler.tsx`): extend entries with optional `meta` (columnId/cardId/reason), add `mark(name,data)` / `recordPerfEvent`, keep DEV-gated.
2. **Per-column attribution**: record ColumnCardList renders with column id (meta or dynamic id) only in dev/perf mode; add `BoardColumns` profiler boundary inside BoardView.
3. **Dev-only counters behind flag** (`window.__PERF_DEBUG__` or `__PERF__` presence): BoardView body render count, Column render+prop diff, CardDraggable render cause, ColumnCardList virtualizer range/isScrolling.
4. **Drag cache marks** (`frontend/src/features/cards/drag-drop-context.tsx`, `drag-over-cache.ts`): mark applied/skipped dragOver, changed column refs, same-column/cross-column outcomes.
5. **Perf e2e diagnostics** (`frontend/e2e/perf-helpers.ts`, `perf-drag.spec.ts`, `perf-scroll.spec.ts`): aggregate per-column counts, optional `PERF_DEBUG=1` JSON artifact to `test-results/perf/`, keep budgets unchanged.
6. **Deterministic render-budget probe** (`frontend/src/features/boards/board-view/render-budget.test.tsx` or new drag fanout test): 3 columns, simulate dragOver `setQueryData`/moveCardToColumn, assert only touched columns rerender.

### Fix sequence

1. Add instrumentation + render-budget probe (no behavior change).
2. Confirm drag root cause with probe/artifact.
3. Fix drag fanout by isolating `CardDraggable`/`useSortable` rerender from card face: extract memoized `CardFace` (or equivalent memo boundary) receiving `isDragging` and stable card/handlers; keep ContextMenu/dialog state in `Card`.
4. Re-measure drag perf; target ColumnCardList renders near touched-columns-per-commit.
5. Investigate scroll: confirm BoardView component vs subtree commits; inspect double scroll container in `column.tsx`/`column-card-list.tsx`; evaluate TanStack Virtual `directDomUpdates` carefully (requires owning transform/height via virtualizer/containerRef, preserving add-a-card placement and keyboard/focus behavior).
6. Re-run targeted tests, perf-drag/scroll, full e2e; document evidence in this file.

## Notes

### Baseline (user run, 2026-11-22, before fixes)

- Full e2e: 77 passed, 2 failed.
- Failures:
  - `e2e/due-dates.spec.ts:18` clear due date button not visible after setting date.
  - `e2e/perf-drag.spec.ts:185` `throttled-4x drag perf` timeout at 180000ms; CDP cleanup failed because page closed.
- Perf logs:
  - Scroll: `median maxDt=236.6ms`, `eventP95=0`, `loafts=167`, `longtasks=105`, `ColumnCardList renders=246`, `BoardView renders=246`.
  - Cross-column: `median maxDt=892.5ms`, `eventP95=15.5`, `loafP95=532.1`, `ColumnCardList renders=1757`, `BoardView renders=120`.
  - Same-column: `median maxDt=779.9ms`, `eventP95=15.8`, `loafP95=453.4`, `ColumnCardList renders=1803`, `BoardView renders=124`.
  - Throttled-4x: no perf summary (timed out).
  - Load/heap: `navToNetworkIdleMs=1005ms`, `heap delta=-0.53MB`.

### After fixes (user full e2e run, 2026-11-22)

- Full e2e: **79 passed, 0 failed (6.6m)**.
- Perf logs:
  - Scroll: `median maxDt=116.8ms`, `eventP95=0`, `loafts=152`, `longtasks=106`, `ColumnCardList renders=240`, `BoardView renders=240`.
  - Cross-column: `median maxDt=374.4ms`, `eventP95=13.2`, `loafP95=303.1`, `ColumnCardList renders=1729`, `BoardView renders=120`.
  - Same-column: `median maxDt=377.1ms`, `eventP95=13.5`, `loafP95=269.3`, `ColumnCardList renders=1803`, `BoardView renders=124`.
  - Throttled-4x: `median maxDt=1215.6ms`, `eventP95=13.4`, `loafP95=1230.5`, `ColumnCardList renders=1734.5`, `BoardView renders=118.5`, `202 events`, `47.0s`.
  - Load/heap: `navToNetworkIdleMs=1086ms`, `heap delta=-0.30MB`.

### Latest perf-drag-only run (after memo/allColumns stabilization attempt)

- `npx playwright test e2e/perf-drag.spec.ts --project=chromium` - 3 passed.
- Cross-column: `maxDt=356.9ms`, `eventP95=13.1`, `loafP95=375.2`, `ColumnCardList renders=1743`, `BoardView renders=120`.
- Same-column: `maxDt=316.2ms`, `eventP95=13.7`, `loafP95=347.7`, `ColumnCardList renders=1804`, `BoardView renders=125`.
- Throttled-4x: `maxDt=1980.4ms`, `eventP95=13.3`, `loafP95=1963.5`, `ColumnCardList renders=1743`, `BoardView renders=119.5`.

### Latest perf run (after CardFace memo + directDomUpdates + instrumentation, 2026-11-22)

- `npx playwright test e2e/perf-drag.spec.ts --project=chromium` - 3 passed.
  - Cross-column: `maxDt=327.1ms`, `eventP95=12.8`, `loafP95=343.2`, `ColumnCardList renders=1730`, `BoardView renders=120`, `CardFace renders=672`.
  - Same-column: `maxDt=395.3ms`, `eventP95=13.2`, `loafP95=338.0`, `ColumnCardList renders=1804`, `BoardView renders=125`, `CardFace renders=672`.
  - Throttled-4x: `maxDt=1870.7ms`, `eventP95=13.4`, `loafP95=1613.6`, `ColumnCardList renders=1727`, `BoardView renders=118`, `CardFace renders=779`.
  - Per-column ColumnCardList counts show all 15 columns rerender almost every BoardView commit (e.g. ~114-118/120), confirming `useSortable`/DndContext fanout at wrapper level.
- `npx playwright test e2e/perf-scroll.spec.ts --project=chromium` - 1 passed.
  - Scroll: `maxDt=90.1ms`, `eventP95=0`, `loafts=187`, `longtasks=113`, `ColumnCardList renders=240`, `BoardView renders=240`, `CardFace renders=2523`.
  - `directDomUpdates` improved maxDt (116.8 -> 90.1ms) but range/isScrolling changes still produce ~240 commits over 120 steps.

### What worked

- **Due-dates fix**: `useUpdateCard` seeds detail cache on success, removing stale-detail race for open card panel.
- **DnD context churn fix**: `ColumnCardList` uses `useDndMonitor` to track only active drag id on start/end/cancel, avoiding high-frequency context rerenders.
- **Prop stabilization**: removed `allColumns` array cascade; BoardView now passes `nextColumnId` and memoized `moveTargets` keyed by column metadata.
- **CardFace memo boundary**: extracted `CardFace` memo inside `CardDraggable` render prop. Confirms dnd-kit `useSortable` rerenders wrappers, but card content now skips when `isDragging`/card props stable. `CardFace` renders dropped to ~672-779/run (from implied ~36k visible-card-content renders).
- **Scroll virtualizer directDomUpdates**: `ColumnCardList` uses TanStack Virtual `directDomUpdates: true` with `containerRef`; scroll maxDt improved 116.8ms -> 90.1ms.
- **Perf instrumentation**: `PerfProfiler` meta/marks, per-column e2e counts, CardFace profiler - gives attributable evidence instead of guessing.

### What didn’t move as expected

- **ColumnCardList subtree commit count remains ~1730-1800/run**. Per-column counts show all 15 columns render almost every BoardView commit. Root cause: `useSortable` subscribes to DndContext; each CardDraggable wrapper rerenders, so ColumnCardList Profiler subtree commits fire even when memoized CardFace skips. The metric measures subtree commits, not card-content work.
- **Scroll commits remain 240/240**. `directDomUpdates` avoids React rerenders for scroll-offset-only changes, but visible range / `isScrolling` changes still trigger rerenders; 120 wheel steps produce ~240 commits. LoAF/longtasks remain high (187/113).
- **Throttled-4x maxDt still high/noisy** (1870ms). Card content work reduced, but wrapper rerenders + dnd-kit internals + dev-mode overhead remain.

### Hypotheses / next debugging steps

1. **Reduce sortable-wrapper rerenders**: evaluate isolating `useSortable` further - e.g. make CardDraggable a memoized component that only rerenders when its own sortable state/props change is hard because hook lives inside; alternatively use `@dnd-kit/core` draggable/droppable with lighter per-item subscriptions, or migrate to dnd-kit-next/experimental (bigger migration). For v6/10, investigate disabling sortables outside active/over columns during drag if dnd-kit semantics allow.
2. **Refine perf metric**: add `CardDraggable`/wrapper profiler or count actual CardFace renders in perf budgets; keep ColumnCardList subtree metric as diagnostic. Consider asserting CardFace count or maxDt/LoAF instead of raw ColumnCardList commit count.
3. **Scroll deep-dive**: profile with DevTools Performance to attribute LoAF/longtasks (virtualizer measurement, layout, card preview paint). Check double scroll container in `column.tsx` vs `column-card-list.tsx` against `frontend/e2e/column-add-card-placement.spec.ts` before changing.
4. **BoardView Profiler semantics**: add body-level render counter/debug flag to distinguish BoardView component renders from subtree commits; avoid optimizing BoardView unless it actually rerenders.
5. **Consider `useFlushSync: false` / virtualizer options** only with careful testing - can affect scroll feel and React 19 behavior.
6. **Re-run full e2e** (`cd frontend && npm run test:e2e`) after any further changes; record before/after evidence.

### Commands

```bash
# targeted tests
cd frontend && npx vitest run src/features/columns src/features/boards/board-view src/features/cards/use-cards.test.tsx src/features/cards/card-detail-panel.test.tsx

# checks
cd frontend && npx tsc -b --pretty false
cd frontend && npm run lint

# perf e2e
cd frontend && npx playwright test e2e/perf-drag.spec.ts --project=chromium
cd frontend && npx playwright test e2e/perf-scroll.spec.ts --project=chromium
cd frontend && npm run test:e2e
```
