# T021: Board render perf — virtualization + dragOver throttle

> **STATUS: done (2026-11-19).** Wave 1. Closes **P4** (MEDIUM-HIGH) and the **rendering half**
> of **P3** (HIGH). P3's hydration half closes with T022; P3's visual half with T044.

## Goal

A board with hundreds of cards must stay smooth (NFR: drag < 100 ms input latency):

1. **Virtualize card lists** — `@tanstack/react-virtual` on `column-card-list` and
   `board-list-view` (today: every card mapped, O(N) `CardDetailPanel` instances, no
   virtualization anywhere).
2. **Throttle dragOver cache writes** — `handleDragOver` writes
   `setQueryData(['columns', boardId])` on **every dragOver event** during cross-column
   drags; rAF-coalesce to ≤ 1 write/frame. The dragOver write is what moves the card
   visually during a cross-column drag (the canonical dnd-kit multi-container pattern)
   — **keep live rearrangement; do not delete it without a product decision.** Write
   cheaply during dragOver: only re-spread the columns whose arrays actually changed
   and avoid re-creating every card object in the target column (reindex only the
   cards whose `position` actually changed; if no order changed, skip the write).
   `dragEnd`'s mutation + invalidation remains the real sync.
3. **Stabilize identities** — stop recreating `allColumns`/`cards` arrays per write
   (defeats React Compiler); memoize the double-flatMap board counts.

## Depends On

(None. T022 depends on this — both own `board-view/board-view.tsx`/column list files.
T044 (column containment + visual affordances) and T026 (capacity smoke) also build on
this task — avoid out-of-scope layout or perf changes that collide with them.)

## Phase

1

## Critical

No — but P3 is HIGH and it's the core UX at 100+ users with real board sizes.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Board rendering (design: virtualization
  + dnd-kit interplay, rAF coalescing, identity stabilization, edge cases, test plan).
- `specs/arch-hardening-specs/00-overview.md` — findings P3, P4; global DoD (gates are
  pasted as evidence; `gen:api -- --check` likely N/A — no wire change — say so;
  coverage floor stays green).
- `frontend/src/features/cards/drag-drop-context.tsx:100-186` (dragOver defect),
  `frontend/src/features/boards/board-view/board-view.tsx:119-120,296,309` (counts +
  `allColumns` identity; NB the flat `…/boards/board-view.tsx` paths cited in older
  specs are stale — the real tree lives under `board-view/`),
  `frontend/src/features/columns/column-card-list.tsx:22-29`,
  `frontend/src/features/boards/board-view/board-list-view.tsx:258-350`,
  `frontend/src/features/cards/card.tsx:32-43` (per-card cost; `CardDetailPanel`
  mounted at card.tsx:159).
- dnd-kit + virtualization integration notes (verified against current docs 2026-11-19):
  - Official docs: `SortableContext` `items` = the **full sorted list of unique ids, in
    render order** — never the virtualized/visible slice; `verticalListSortingStrategy`
    is the only sortable strategy that supports virtualized lists (`rectSortingStrategy`
    does not). `useSortable` computes indexes via `items.indexOf(id)` and
    `getSortedRects` iterates the full `items` array — a visible-only slice shifts every
    index and desyncs drag transforms, animations, and `sortableKeyboardCoordinates`.
  - Official virtualized story (`stories/2 - Presets/Sortable/5-Virtualized.story.tsx`,
    react-tiny-virtual-list + `overscanCount` + DragOverlay) and its Cypress virtualized
    keyboard/mouse suites (`sortable_keyboard_virtualized.ts`).
  - `DragOverlay` is the recommended pattern for scrollable lists (official sortable
    README) — already in use here (`drag-drop-context.tsx:346`), so the lifted preview
    is unaffected by virtualization.
  - Production corroboration: Elementor PR #35659 (2026-05-03, `@tanstack/react-virtual`
    + `@dnd-kit`, hundreds-item reorder list): rows absolutely positioned at
    `top: virtualRow.start`, custom `rangeExtractor` pinning the dragged item,
    overscan ≈ 6, drag placeholder `visibility:'hidden'`.
  - `@tanstack/react-virtual@^3.14.26` (2026-11-03, MIT, peer react ^16.8–^19, 0 known
    advisories) — actively maintained; correct choice (react-window: fixed-height rows
    only; react-tiny-virtual-list: effectively unmaintained).
  - Legacy dnd-kit (`@dnd-kit/core@6.3.1` / `@dnd-kit/sortable@10.0.0`) is **frozen
    since Dec 2024**; the actively developed line is `@dnd-kit/react@0.5.3` (2026-10),
    which the author recommends for all new work. **Migration is out of scope here** —
    see open call below.

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/package.json` | + `@tanstack/react-virtual` (**runtime `dependencies`** entry, not devDependency — T018 hygiene lesson; re-check T016 bundle budget afterwards: 310 KB initial / 225 KB per chunk) |
| `frontend/src/features/columns/column-card-list.tsx` | Virtualizer (see Integration below); `useVirtualizer` before the early return — hook calls must be unconditional (the file currently early-returns `null` for empty columns) |
| `frontend/src/features/boards/board-view/board-list-view.tsx` | Virtualize flat card rows; `aria-rowcount`/`aria-rowindex` (or equivalent role bookkeeping) — the list uses `role="table"`; keep the inline-edit row (`editingCardId`) mounted via `rangeExtractor`/overscan so it doesn't lose focus on scroll |
| `frontend/src/features/cards/drag-drop-context.tsx` | rAF-coalesced dragOver updates (one pending frame max); keep live cross-column rearrangement; structural sharing — skip the write when no card order changed, reindex only cards whose `position` changed (keeps card-object identity stable); keep optimistic rollback + `dragSource`-at-start |
| `frontend/src/features/boards/board-view/board-view.tsx` | Stabilize `allColumns` identity (`columns ?? EMPTY_ARRAY` module-level constant or `useMemo`); `useMemo` counts (visibleCount/totalCount) |
| `frontend/src/features/cards/card.tsx` | Mount `CardDetailPanel` on **first open only** (`hasOpenedOnce` state), keep mounted afterwards (exit animation preserved, zero cost until first open) — implements the design's "gate mount on open" edge case |
| Tests | Red-first: dragOver write-count ≤ rAF ticks (spy `setQueryData` over a synthetic ≥30-event cross-column dragOver sequence, assert writes ≤ raf-tick count + 1); identity stability across dragOver writes (array **and** card-object references); DOM row count bound; keyboard drag across virtual boundary; existing drag suites green |
| `docs/lessons/frontend.md` | Dated, tagged lesson |
| `specs/tasks/status.yaml` + `specs/arch-hardening-specs/00-overview.md` | Same-PR: flip T021 to `done` at implementation; annotate the P3 cell ("rendering half closed by T021; hydration by T022; visual by T044") and the P4 cell; note P3's "Closed by T021" register line is stale w.r.t. the split |

**Integration (column-card-list + board-list-view):**

- `useVirtualizer({ count, getScrollElement, estimateSize, overscan: ≥3 (5 suggested), getItemKey: card id })`.
  `estimateSize` must be an **upper bound** of measured card height (dynamic rows via
  `measureElement` + `data-index`); underestimating causes scroll jumpiness.
- `SortableContext items={cards.map(c => \`card-${c.id}\`)}` — the **full** ordered id
  list, never the virtual slice; `verticalListSortingStrategy` (existing).
- Row = absolute-positioned cell (`position:absolute; top:0; transform: translateY(virtualRow.start px)`;
  container sized `height: getTotalSize()`). **Layer separation:** the virtualizer's
  `translateY` and `useSortable`'s `transform` must not fight — outer virtualizer cell +
  inner sortable element (or split `translate:` vs `transform:` on one node). Merge refs
  (`setNodeRef` + `measureElement`).
- **Custom `rangeExtractor`** unioning `defaultRangeExtractor` with the active drag
  index (react-virtual has no `stickyIndices`) so the dragged row stays mounted even
  when scrolled out of view — otherwise dnd-kit loses its measured node mid-drag.
- Keep the source row mounted but dimmed (`opacity: 0.35/0.5`) while dragging; don't
  unmount it. `DragOverlay` stays the preview (already used) — no change.
- Screen-reader positions reference the **full list** (`indexOf+1 of items.length`),
  not the window; empty column renders as today; single-card column works.
- `columns ?? []`-style inline arrays are forbidden here too — hoist/memo.

**Must NOT appear in the diff:** pagination changes (T022), optimistic-rollback
semantics, sensor distances (documented touch/context-menu interplay stays),
`@dnd-kit/react` migration + any `DragOverlay`/portal structure change, unrelated
`board-view` layout changes (T044 owns).

## Implementation Steps

TDD.

1. **Red proof (P4):** synthetic cross-column drag sequence → spy `setQueryData` calls
   → count ≫ frames (fails today). rAF mocked or awaited under vitest. Note that
   `drag-drop-context.test.tsx` currently has no dragOver coverage — this is new
   infrastructure, not an extension of existing tests. Also profile marker:
   full-board re-render per write (React Profiler paste optional but encouraged).
2. **Fix dragOver:** rAF schedule (one pending frame max), apply the structural-sharing
   skip (no order change → no cache write). Keep same-column fast path. Keep
   cross-column live rearrangement — do **not** switch to over-id-only hover in this
   task (see open call).
3. **Red test (identity):** `allColumns`/`cards` array references **and card object
   references** stable across dragOver writes when no order changed; counts memoized.
4. **Virtualize:** add `@tanstack/react-virtual@^3.14.26`; wire per the Integration
   block above; verify keyboard sensor behavior across a ≥100-card column: arrow-key
   drag progresses stepwise (auto-scroll mounts rows — unmounted rows are
   unreachable by construction), focus + announcement at the **full-list index**
   preserved after each move, `scrollBehavior: 'auto'` in E2E; touch 750 ms delay
   unaffected; empty/single-card columns; editing row stays mounted in list view.
5. Perf smoke: 500-card board (seeded — **throwaway tmpfs/RAM-disk DB container only,
   never the dev DB; scratch e2e-style DB with explicit teardown**, AGENTS-local.md
   rule; T044 is the model — measured 148-card board on `kanpad_e2e`). Paste before/after
   render counts or latency note; record as T026's baseline (precise numbers belong to
   T026).
6. Same-PR bookkeeping: flip `status.yaml` T021 → done; annotate the P3/P4 cells in
   `00-overview.md` per the Files table. Full vitest + `tsc -b`; existing drag-drop
   unit + e2e suites green; bundle-budget gate re-checked after adding the dep;
   `gen:api -- --check` (expect no-op — no wire change; state so); coverage floor
   green; persistence-verification note (no POST/PATCH/DELETE endpoints touched;
   dragEnd's PATCH is exercised by existing suites — state it); lesson.

## Constraints

- dnd-kit + virtualization conflict is the main risk — if drag preview/keyboard breaks,
  fall back to **conditional virtualization (>50 cards)** and document (open call).
- SortableContext always gets the **full** ordered id list. Visible-range lists are
  forbidden (index desync, broken transforms/keyboard).
- Optimistic update + rollback + `dragSource` conventions untouched (AGENTS.md).
- Counts must stay correct (server totals arrive with T022 — until then, memoized
  flatMap is acceptable).
- React Compiler stays enabled (don't hand-memo everything — memoize the *inputs*).
- Hook rules: `useVirtualizer` unconditionally, never below an early return.
- Global DoD (gates pasted as evidence); no commit without approval.

## Acceptance Criteria

- [x] dragOver cache writes ≤ 1 per animation frame; red-first test pasted (synthetic
      ≥30-event cross-column sequence; writes ≤ rAF ticks + 1) (closes **P4**).
      Red proof (pre-fix, `drag-drop-context.test.tsx`): `AssertionError: expected "setQueryData" to be called +0 times, but got 30 times` at the "No cache write may happen synchronously during the burst" assertion — 30 events → 30 writes synchronously. Post-fix: burst → exactly 1 write; writes never exceed rAF ticks + 1 (tests: `drag-drop-context.test.tsx` → "dragOver cache writes").
- [x] Identities stable across dragOver: `allColumns`/`cards` arrays **and unchanged-order
      card objects** (test green, references asserted).
      `drag-over-cache.test.ts` (9 tests): same-column/not-found/missing-target → SAME array reference; unchanged cards keep object identity (`next[1].cards[0]).toBe(cardC)` etc.); uninvolved columns keep reference; only position-changed cards re-created. Context-level: cache reference `toBe` across a steady-hover burst (0 writes). `allColumns` is now a module-level `EMPTY_COLUMNS` (identity constant) in `board-view.tsx`; counts are `useMemo`.
- [x] Card lists virtualized — DOM row count for a 500-card board pasted (≤ 2×overscan+1
      per column; report the measured number) (closes **P3 rendering half**).
      Measured in a real Chromium against a throwaway tmpfs MariaDB: **13 rows mounted of
      500** in the board view (`[T021] 500-card board: mounted rows=13, time-to-interactive=1189ms`) and **13 of 500 in list view** (`aria-rowcount="501"` = 500 data rows + sr-only header row). The literal `≤ 2×overscan+1` bound assumes a 1-row viewport; at ~100px rows in a 900px viewport the window is ~6 visible + 2×5 overscan ≈ 16 max — 13 measured is consistent. jsdom-bound unit tests assert ≤ 16 rows for both views.
- [x] Keyboard sensor verified across a ≥100-card column (stepwise progress, focus +
      full-list index preserved — e2e; full-list index announcement sub-clause not
      applicable: the app ships no custom announcements, so no announcement carries an
      index either way — stated honestly); touch
      delay, empty column, drag preview all verified.
      e2e `virtualization.spec.ts` (120-card column, seeded via API): pick-up → ArrowDown ×24 → drop persisted at index 24 (asserted via a pre-created `page.waitForResponse(PATCH /cards/:id)` + `GET /columns` + reload-with-first-row assertion); `document.activeElement` stays the dragged sortable node across the 24 steps (source row pinned by `rangeExtractor`). Full-list indexes: `SortableContext items` = full id list (unit-asserted, 500/500), `index = items.indexOf(id)` unchanged. Announcement: app DndContext uses **defaultAnnouncements (no index)** — unchanged by this task; sortable `data.sortable.index` carries the full-list position for any announcement/coordinate getter (documented, no custom announcement added — out of scope).
      Touch 750 ms delay untouched (unit: "configures TouchSensor so tap-and-hold opens context menu instead of drag" green; sensors otherwise untouched). Empty column (unit), single-card column, and drag preview (existing e2e "drag overlay shows the same labels and description as the source card") all green. `scrollBehavior: 'auto'` added to KeyboardSensor (spec: deterministic keyboard steps; harmless for keyboard UX).
- [x] `tsc -b` + full vitest clean; existing drag-drop unit + e2e suites green; bundle
      gate re-checked (output pasted); `gen:api --check` no-op stated; coverage floor
      green; persistence-verification note stated.
      `tsc -b` clean; **871 unit tests green**; drag-drop e2e suite green (5/5 incl. cross-column move, drag overlay, toggle with exact-name Title selector fix); virtualization e2e green (2/2). Bundle gate: `initial gzip = 267,673 B` (entry 69,594 + vendor 197,650 + runtime 429) vs 310,000 B cap — **gate green** (`npm run budget` → "Bundle budget OK"). `gen:api -- --check`: `schema.d.ts is up to date` — **no wire change, stated**. Coverage floor: N/A for the frontend — the 85% floor is the backend `cargo llvm-cov` gate (untouched); frontend has no coverage gate configured. Persistence: **no POST/PATCH/DELETE endpoints touched**; dragEnd's PATCH is exercised by the existing drag-drop e2e ("between columns" verifies server state via API + reload) and the new keyboard e2e (PATCH-wait then API + reload). Full-suite e2e evidence (2026-11-19, throwaway tmpfs MariaDB):
      ```
      ✓ virtualization.spec.ts › keyboard drag progresses stepwise (120-card window, exact index 24)  (19.4s)
      [T021] 500-card board: mounted rows=13, time-to-interactive=983ms
      [T021] list view: mounted rows=13
      ✓ virtualization.spec.ts › 500-card board renders a bounded DOM window (P3 rendering half)  (15.4s)
      ✓ drag-drop.spec.ts › within a column / between columns / drag overlay (16.3s, 8.9s, 6.7s)
      ✓ board-view-toggle.spec.ts  (14.0s)   ✓ card-search.spec.ts  (13.9s)
      ```
- [x] `status.yaml` flipped + `00-overview.md` P3/P4 cells annotated same-PR.
- [x] Lesson appended (`docs/lessons/frontend.md` — "Virtualization + dnd-kit + dragOver throttling (2026-11-19, T021)").

## Notes

(implemented 2026-11-19)

- **Status:** pending → done (status.yaml). P4 closed; P3 rendering half closed (hydration = T022, visual = T044).
- **Stage A containment shipped inside T021** (user sign-off): virtualization cannot scroll without a bounded height chain. Fix was BoardView-scoped — `board-view.tsx` root `h-full min-h-0`, `.transition-opacity` wrapper `flex min-h-0 flex-1 flex-col`, `overflow-x-auto` scrollers `h-full min-h-0` (mobile + desktop), `column-droppable.tsx` root `flex min-h-0 flex-1 flex-col`, matching `flex min-h-0` on the column scroller's child in `column.tsx` and on the virtualizer's scroll element in `column-card-list.tsx`, and `min-h-0` on the list-view chain (`board-view.tsx` list wrapper, `BoardListView` root + scroll area). `app-layout.tsx` untouched, no `main` edits. Not applied: `overscroll-behavior: contain` / `scrollbar-gutter: stable` (deferred to T044 Stage B2, deliberately). T044 amended inline: it drops Stage A; B1–B4 remain.
- **dragOver:** `handleDragOver` now stores the latest event in a ref and schedules at most one rAF frame (`rafIdRef`); `flushPendingDragOver` applies it through `computeCrossColumnMove` (new pure helper, `drag-over-cache.ts`) which returns the SAME array reference when no order changed → the caller skips `setQueryData` entirely. rAF frame canceled on `dragStart`/`dragEnd`/`dragCancel`/unmount. Same-column fast path preserved; live cross-column migration kept (rAF-coalesced); `dragEnd` keeps its mutation + invalidation untouched (only `cancelPendingDragOver()` added at the top — stale pending frames must never land after the drop's writes).
- **Structural sharing:** only the two changed columns get new identities; unchanged-position cards keep object references; reindex `position` only when it actually changed; uninvolved columns return the same reference; moved card is re-created with clamped insert index (NaN/out-of-range clamped; full-suite green).
- **Virtualization:** `@tanstack/react-virtual@^3.14.26` (runtime `dependencies`, per T018 hygiene). `column-card-list.tsx` and `board-list-view.tsx` both: unconditional `useVirtualizer` (`count`, `getScrollElement`, `estimateSize` upper bound — 128px cards / 96px rows — `overscan: 5`, `gap: 8` (cards only)), `measureElement` + `data-index`, custom `rangeExtractor` unioning `defaultRangeExtractor` with the dragged (column list) / editing (list view) index so neither unmounts mid-interaction, outer virtualizer cell owns `translateY`, inner sortable node keeps its `transform` (layer separation), `SortableContext` receives the FULL id list.
- **board-list-view:** `role="table"` gets `aria-rowcount`; each row `role="row"` + `aria-rowindex` (full-list position), rows `data-testid="board-list-row"` preserved; inline-edit row pinned via `rangeExtractor` (editing input cannot unmount mid-focus).
- **card.tsx:** `CardDetailPanel` mounts only after the card's first open (`hasOpenedOnce`), then stays mounted (exit animation preserved). Unit-tested (`card-panel-gate.test.tsx`).
- **board-view.tsx:** `columns ?? EMPTY_COLUMNS` (module-level immutable) for both `Column allColumns` call sites; `visibleCount`/`totalCount` now `useMemo` over reduce (no double flatMap).
- **Measured:** 500-card board → **13 DOM rows mounted** (board + list view; `aria-rowcount=501`); TTI ≈ 0.98 s; 120-card keyboard drag stepwise e2e-green with focus preserved (exact index 24 persisted); 871 unit tests green; bundle gate 267,673 B initial gzip (cap 310,000 B).
- **dragOver benchmark honesty:** the pre-fix defect was 30 writes/30 events (red-first test pasted above); post-fix = 1 write/frame via coalescing. Whether dragOver cost is "still measurable" post-fix was NOT re-measured with a profiler — no observable jank in the 500-card board; if T026's capacity run shows it, elevate to a product decision per open call 2.
- **jsdom note:** virtualization tests must mock `clientHeight/clientWidth` (rect source) AND `offsetHeight/offsetWidth` (measurement source) or zero/!∞ rows render; encoded in both virtualization test files.
- **Announcements:** no custom DndContext announcements exist (app default); full-list indexes verified in sortable data. Not in scope to add.
- **`scrollBehavior: 'auto'`** added to KeyboardSensor (spec: deterministic keyboard steps; no behavior risk for keyboard users).
- **Persistence verification:** no POST/PATCH/DELETE endpoints touched; dragEnd's PATCH exercised by existing e2e (server state re-read via API + reload) and the new keyboard e2e (PATCH response awaited).

### Review findings & fixes (2026-11-19, after independent multi-angle review)

Five independent reviewers (dnd-kit integration, React/identity/rAF, a11y, test quality, spec/conventions compliance) found and we fixed:

1. **`handleDragEnd` same-column stale index (blocker/major):** after a cross-column round trip A→B→A during one gesture, `dragSource.index` (captured at dragStart) could point at an absent/relocated card — `arrayMove` moved the wrong card and the local cache diverged from the server (`useReorderCard` never invalidates `['columns']`). Fixed: live `findIndex` lookup in the current cache; on `-1`, invalidate `['columns', boardId]`. Round-trip regression test added (A→B→A→same-column drop reorders the right card, mutation called with `position: 0`).
2. **`handleDragEnd` dropped the optimistic cache without rolling back (major):** `!over` / missing `activeData` / missing `dragSource` / missing source or target column each returned without touching the cache — but `applyDragOver` may already have moved the card optimistically. All early-return paths now `invalidateQueries(['columns', boardId])` (same semantics as `handleDragCancel`).
3. **`handleDragEnd` cloned every card on every drop (major):** same-column branch reindexed all cards; cross-column branch cloned every target card AND left position gaps in the source column. Fixed: reorder path now reindexes only cards whose position changed; the cross-column branch routes through `moveCardToColumn` (same structural sharing as dragOver — moved card re-created, changed cards re-created, unchanged cards keep identity).
4. **Keyboard dead-end/teleport theory did not hold up** — `sortableKeyboardCoordinates` moves the card exactly one row per ArrowDown (verified: 24/24 presses → exact index 24 on a 120-card virtualized column, persisted, reload-verified). An attempted custom coordinate getter keyed off `active.data.current.sortable.index` was built on **stale** data (same-column drags never write the cache, so that index stays at the drag-start value) and scrolled the window ~20 rows past the drag, making the drop compute `insertIndex` from a distant `over` (card landed at index 36). The custom getter was deleted; stock `sortableKeyboardCoordinates` is in place; keyboard sensor behavior verified end-to-end (persisted position exact, focus preserved, e2e green).
4b. **Duplicate React keys in the virtual window (major, found in re-verification):** `rangeExtractor`'s union produced the row twice whenever the dragged/editing row was also the *focused* row (which is every keyboard drag, and every `startTitleEdit`) → `key={virtualRow.key}` collided → React "two children with the same key" console error + duplicated rows (broke the keyboard e2e). Fixed with `new Set([...])` dedupe in both views; regression tests assert the row renders exactly once; e2e green.
5. **List view ARIA required-ownership violations (major):** body rows owned no `cell` children → every row violated `aria-required-children`. Fixed: content cell + actions cell wrapped in `role="cell"`.
6. **`aria-rowcount` off-by-one + `aria-rowindex` collision (major):** table had N+1 rows (sr-only header row + N data rows) but `aria-rowcount=N`. Fixed: `aria-rowcount=N+1`, header row `aria-rowindex={1}`, data rows `aria-rowindex=index+2`.
7. **Non-pinned focused rows unmounted under re-windowing (major, WCAG 2.4.3):** focus dropped to `<body>` on scroll/refetch/sort/delete/edit-commit. Fixed: focused row tracked (`onFocusCapture`/`onBlurCapture` with `relatedTarget` containment check) and unioned into both `rangeExtractor`s; after inline-edit commit/cancel, focus is restored to the rename button via a ref map + pending-restore ref (`restoreFocusAfterEdit`).
8. **Rename affordance keyboard-unreachable (major, WCAG 2.1.1 A):** the title span was mouse-only inside the panel-open button. Fixed: restructured into sibling buttons — "Edit title of card: X" (rename) and "Open card details: X" (panel) — no nested interactives.
9. **Inline `getItemKey` defeated the virtualizer's measurement memo (minor):** inline arrow forced an O(n) rebuild per render (every drag frame). Fixed: `useCallback` + string-typed `__i_${index}` fallback in both views.
10. **Test-suite fixes (major/minor):** the "cancels pending dragOver frame" test was near-vacuous (manually wiped the rAF queue, targeted the same column, tautological assertions) → rewritten with a differently-targeted stale frame + function-vs-concrete payload assertion + queue-length check; editing-row and dragged-row pins got real jsdom coverage (scroll to pushed-out window; focused/dragged row stays mounted; drag-ends unpin); `flushAllRaf` re-scheduling tripwire; burst pin tightened to exactly 1 write; jsdom `defineProperty` mocks consistently restored; e2e keyboard drag extended to 24 ArrowDowns (truly crosses the virtual window) and `aria-rowcount` expectations updated (501).
11. **Announcements:** none existed (app default); left as-is (scope) and AC reworded honestly. **DndLiveRegion unthrottled assertive announcements** noted as a known minor (no conformance issue).
12. **Focus-trap caveat documented:** inline-edit blur during scroll (jsdom artifact) — tests assert row presence; real browsers keep focus across relocations.
13. **Source-column position gaps** from un-reindexed filter are accepted as cosmetic (server recomputes at read; frontend reads positions nowhere) — documented, no code change (re-indexing would defeat memoization).
14. **Documentation fixes:** T021 Notes/Lessons filled; T026 task file records the T021 baseline; T044 amendment enumerates the column-chain `min-h-0` spots and notes `overscroll-behavior`/`scrollbar-gutter` were NOT applied; duplicated stale "Open calls" block removed; AC announcements sub-clause reworded honestly; raw gate outputs pasted below.

**Open calls:**

1. *Always-virtualize vs >50-card threshold* — **resolved: always virtualize, no fallback needed.** Keyboard/drag/preview all green on a 120-card column; nothing structural regressed.
2. *Live-migration vs over-id-only hover* — **kept live migration (rAF-coalesced)** per default. Not elevated yet; T026's capacity run may revisit.
3. *`@dnd-kit/react` (v0.5.x) migration* — out of scope, unchanged. Legacy packages frozen since Dec 2024; record as a spike if structural regressions appear (none observed).

### Second review round — a11y hardening fixes (2026-11-19, from the focused re-verification)

15. **Regression tests for the duplicate-row fix strengthened:** both "renders exactly once" tests now scroll far down the virtual window before asserting (asserting while the pinned index is inside the base window was vacuous — pre-fix code passes it too).
16. **Column mapping fixed:** sr-only header row was 4 `columnheader`s vs 2 cells per row (actions cell implicitly bound to the "Updated" column; Created/Updated were sort-only scopes with no cells). Now 2 `columnheader`s matching the 2 data cells — "Cards sorted by X" (with `aria-sort` on the Cards header) + "Actions"; `aria-rowcount` unchanged at N+1.
17. **ProgressBar exposes a real `role="progressbar"`** with `aria-valuemin/max/now` (was `aria-label` on a nameless generic div — value/name prohibited).
18. **Focused-row unpin path now unit-tested** in both views (blur to an outside element releases the pin; ensuing scroll virtualizes the row away).
19. **Dead code removed:** unread `titleButtonRefs` map deleted; pending-restore ref is nulled on unmount (`useEffect` cleanup) so a deleted-then-remounted card can never receive a stale focus restore.
20. Known-minor, unchanged: focus strands on `<body>` after a delete that unmounts the row's trigger (Radix AlertDialog fallback, pre-existing); `useReorderCard.onSettled` does not invalidate `['columns']` (pre-existing, divergence unlikely — noted, not in scope); `useVirtualizer` "incompatible library" lint warnings are benign (compiler skips the component, no stale closures).

