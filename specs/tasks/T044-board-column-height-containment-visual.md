# T044: Board column height containment + visual affordances

> **STATUS: pending.** Wave 1, after T021 + T022. Depends on **T021** (virtualizer,
> dragOver throttle — owns `column-card-list.tsx`, `drag-drop-context.tsx`) and
> **T022** (hydration: first-N embedded, per-column load-more, server totals — owns
> the same files' data layer). Closes the visual half of **P3** (UX of big boards)
> and adds the missing "growing column" signaling. Not a pagination task — T022
> fixes the data path; this task fixes the box the cards live in and what the
> board says about column size.
>
> **Amendment 2026-11-19 (T021):** **Stage A shipped inside T021** (user sign-off —
> virtualization cannot scroll without a bounded height chain; the two unconditional
> breaks went exactly as listed in part 1: `board-view.tsx` root `h-full min-h-0` +
> `.transition-opacity` `flex min-h-0 flex-1 flex-col` + `overflow-x-auto` scrollers
> `h-full min-h-0`, and `column-droppable.tsx` root `flex min-h-0 flex-1 flex-col`;
> plus `flex min-h-0` on the column scroller's child in `column.tsx` and
> `column-card-list.tsx` (the virtualizer's scroll element — chain-necessary), and
> two matching `min-h-0` on the list-view chain (`board-view.tsx` list wrapper,
> `BoardListView` root + `px-6 scroll area`). `app-layout.tsx` untouched, no
> `main` edits. `overscroll-behavior: contain` / `scrollbar-gutter: stable` were NOT
> applied (not required for correctness; deliberately deferred — fold into Stage B2
> with the fade + Load-more pass, or drop). When picking this task up, drop
> part 1 and treat parts 2–5 as the remaining scope.

## Goal

Today (measured 2026-11-18 on a 148-card seeded board, 1600×900): a 70-card column
renders **5996 px** tall, the document never scrolls, and `<main>` degenerates into
one ~6k px vertical strip — board chrome (name, search, filters, view toggle)
scrolls off-screen, all columns are dragged as a single slab, and the per-column
`overflow-y-auto` never engages (`clientHeight == scrollHeight`). The board reads
as broken, not just ugly.

Five parts:

1. **Stage A — Containment fix (the actual bug).** Restore the bounded height
   chain (BoardView-scoped; `app-layout.tsx` untouched; **no** `main` edits). Two
   unconditional breaks must both go: (a) the `.transition-opacity` wrapper in
   `board-view.tsx` has no `flex`/`min-h-0`/`flex-1`, so `h-full` below it collapses
   to content height; (b) `column-droppable.tsx` renders a plain `<div>` with no
   `className` — a block child (of the column's flex-col) with content height,
   sitting between the column root and the scroller and making its `flex-1` a
   no-op. Fix both so columns get their designed internal scroll and the chrome
   stays pinned —
   the classic Trello canvas (SVAR `columnScroll: true` default; hermes-webui's
   `max-height: min(68vh,720px)` + `overscroll-behavior: contain` +
   `scrollbar-gutter: stable`).
2. **Stage B1 — Column header count badge.** Size visible at a glance
   (`Backlog · 70`). After T022, sourced from server totals — hydration means
   `cards.length` no longer equals the column's size. When search/filters are
   active the badge shows the count of *visible* cards (plain number); the board-level
   `aria-live="polite"` text already announces "`shown of total`" — per the 5-1
   decision, do **not** invent `shown of total` dual-count in the column header.
3. **Stage B2 — Clipped-column fade + "N of M — Load more" footer.** A gradient
   above the last visible card + a footer row. `IntersectionObserver` (rooted at
   the column scroller, not `null`) auto-loads the next page whenever it approaches
   the end — **to the end**. The footer row (`N of M — Load more`) is rendered
   **only while more pages remain** and **hidden at full hydration**; clicking it
   force-loads the next page (keyboard-reachable). No explicit "first pages then
   button" threshold — IO is unconditional. (Board search remains the reliable
   find-path; search/filters scope to loaded cards until server-side search is a
   product decision — documented in T022.)
4. **Stage B3 — Compact density toggle.** Board-chrome icon (next to Board/List);
   persists to `localStorage`; `data-density="compact"` on the board root;
   CSS-only (padding, `line-clamp-2`, labels collapse to first + `+n`).
5. **Stage B4 — Advisory per-column limits.** GitHub Projects / Jira / Trello
   List Limits / Azure pattern: nullable `columns.card_limit ≥ 1`; advisory only —
   **never blocks** add/drag/move; **no toast**; per-column only (no board-wide
   default); two breach-tier header badge `count / limit`: muted under → amber at
   (`count >= limit`, at capacity) → red over (`count > limit` — red wins over
   amber, "+N over" tooltip). Set via the
   column "…" menu → "Column limit…" dialog (empty default, inline `≥ 1`
   validation, "Remove limit" clears). This is the one affordance that actively
   discourages runaway columns instead of just rendering them.

## Depends On

T021, T022.

## Phase

1 (after T021/T022).

## Critical

No — P6-class, but it fixes a reproduced layout defect, not just polish.

## Spec References

- `specs/tasks/T022-pagination-u-x-page-ceilings.md` (server totals; hydration
  shape this task's Stage B2 consumes; ordering = `sort_key` + `id`, unchanged).
- `specs/tasks/T021-board-render-perf-virtualization-drag-over-throttle.md`
  (virtualizer over visible items; drag source force-rendered; overscan feeds B2's
  observer).
- `specs/arch-hardening-specs/20-performance.md` §Board rendering;
  `00-overview.md` §P3/P4.
- Industry patterns (2026-11-18 web research): Trello (fixed canvas + independent
  column scroll), SVAR React Kanban (`render.columnScroll` default true,
  `virtualizeCards`), frappe PR #40343 (50/col first page, IO-triggered hydration,
  500-item memory cap), twenty.dev PR #23764 (window + spacers; force-render drag
  source), Syncfusion (`enableVirtualization`, fixed `cardHeight` tradeoff),
  hermes-webui PR #4832 (containment patch + regression test), Smashing Magazine
  infinite-scroll guidelines (footer/Load-more over endless scroll), GitHub
  Projects "Set column limit" (advisory, per view), Jira column min/max
  (red/yellow header), Trello List Limits Power-Up, Azure Boards (soft limits,
  highlighted count) — universal pattern: advisory, never enforced.
- Lessons: `docs/lessons/frontend-visual-testing.md` (the Playwright playbook
  used to reproduce; reuse it for the before/after verification here).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/features/boards/board-view/board-view.tsx` | Stage A: `flex flex-1 min-h-0 flex-col` wrapper (desktop/mobile/list paths); Stage B3: density toggle + `data-density` |
| `frontend/src/features/columns/column-droppable.tsx` | Stage A: `flex flex-1 min-h-0 flex-col` — today a plain `<div>` with no className; second unconditional break in the height chain |
| `frontend/src/features/columns/column.tsx` | Stage A: scroller `overscroll-behavior: contain`, `scrollbar-gutter: stable` |
| `frontend/src/features/columns/column-header.tsx` | Stage B1 badge; Stage B4 limit badge + "Column limit…" menu item/dialog |
| `frontend/src/features/columns/column-card-list.tsx` | Stage B2: fade + footer (hydration data from T022 hooks) |
| `frontend/src/features/columns/` (new) `column-limit-dialog.tsx` | Stage B4 dialog (number input, `≥ 1`, "Remove limit") |
| `frontend/src/features/columns/` (new) `density-toggle.tsx` or inside board chrome | Stage B3 toggle; `localStorage` key `kanpad-board-density` |
| `frontend/src/index.css` | Stage B3 compact-density rules; Stage B2 fade styles (Tailwind utilities where possible) |
| `backend-rs/migrations/0004_columns_card_limit.sql` | Stage B4: `ALTER TABLE \`columns\` ADD COLUMN \`card_limit\` INT NULL` — no backfill (nullable, default null = no limit); dual-engine instant add, column counts as end-of-table; MySQL 8.4 + MariaDB 11.4 compatible; single non-reversible file (repo has no `.up/.down.sql`); next free version `0004`; snake_case filename; header comment noting it is not editable once applied (checksum) |
| `backend-rs/src/features/columns/types.rs`, `store.rs`, `routes.rs` | Stage B4: expose `card_limit`; accept in PATCH (`≥ 1` or explicit `null` clears; absent = unchanged) |
| `backend-rs/openapi.json` + `frontend/src/lib/api/schema.d.ts` | Additive contract — same PR |
| Tests (unit/integration/e2e) | See steps; e2e reuses seeded-board flow (see §Reproduction) |
| `docs/lessons/frontend.md` | Dated, tagged implementation lesson |

**Must NOT appear in the diff:** keyset cursors, server-side search, any change to
T021/T022 owned files' *behavior* beyond what sharing requires, blocking-of-move
semantics for the limit, toasts on limit breach, board-wide limit default,
`app-layout.tsx` edits, Done-column auto-hide (separate product decision).

## Implementation Steps

TDD. Stage A first, then B1→B4. Verify on the real repro (below) after each stage.

1. **Red (Stage A):** e2e/integration check on the big seeded board — every column's
   scroller `clientHeight < scrollHeight` once content exceeds the viewport; the
   board region (not `<main>`) is the growth path; board chrome stays pinned;
   `<main>` `scrollHeight == clientHeight` (not empty because of the bounded chain).
   Fails today (measured 5996 px column, document 900 px, `<main>` the real scroller).
2. Implement Stage A (both breaks: `board-view.tsx` wrapper gains
   `flex flex-1 min-h-0 flex-col`; `column-droppable.tsx` gains
   `flex flex-1 min-h-0 flex-col`; scroller gains `overscroll-behavior: contain` +
   `scrollbar-gutter: stable`; no behavior change).
   Existing unit/e2e suites green; re-measure (see §Verification).
3. **Red (B1):** header renders no count today → test expects `Backlog · 70`.
   B1 is a **post-T022 only** feature — T044 depends on T022; do not write a
   pre-T022 `cards.length` fallback (dead code under the declared dependency); the
   count comes from server totals.
4. B1: badge in `column-header.tsx` (muted, `tabular-nums`). Plain count of the
   *visible* cards when search/filters are active — the board-level
   `aria-live="polite"` text announces the filtered totals (`3 of 70`); do **not**
   invent `shown of total` dual-count in the column header (per the 5-1 header
   guidance, single number under filter).
5. **Red (B2):** clipped column shows no affordance today → test: after hydration
   wiring, fade visible when scrollHeight > clientHeight; footer renders
   `50 of 137 — Load more` **only while more pages remain**; footer **hidden** at
   full hydration; IO (rooted at the column scroller, not `document`) triggers the
   next fetch as the bottom sentinel approaches — unconditional, all the way to the
   end; clicking the footer force-loads the next page; drag across the loaded
   boundary stays guarded.
6. B2: implement fade + footer (uses T022's hydration hook; loading affordance, no
   layout shift; IO observer `root` = the column's actual `overflow-y-auto`
   scroller — the inner one, not `document` and not `null` — with the fade/footer as
   its trigger element). `data-testid="column-load-more"`.
7. **Red (B3):** toggle absent → test: click toggles `data-density="compact"`,
   persists across remount (localStorage), restored preference on load.
8. B3: toggle + CSS.
9. **Red (B4) — backend:** PATCH `/columns/{id}` with `card_limit = 0` and `-1`
   → 400 (garde `range(min = 1)`, same shape as `MoveCardsInput.target_column_id`;
   `card_limit = 15` round-trips; explicit `card_limit: null` clears (use the
   tri-state `Option<Option<i32>>` + `deserialize_some` pattern proven by
   boards `project_id` / cards `description`/`due_date` — plain
   `Option<i32>` cannot tell "absent" from "explicit null"); absent key leaves the
   column untouched; unknown `card_limit` on create is ignored (201, per
   additive-optional contract); column payload carries `card_limit`.
   (Fails today: column doesn't exist.)
10. B4 backend: migration + validation + PATCH + openapi/typegen same PR.
11. **Red (B4) — frontend:** badge states (`< limit` muted; `== limit` — i.e.
    `count >= limit` — amber at capacity; `count > limit` — red "+N over" wins over
    amber); menu opens dialog; `0` rejected inline; "Remove limit" clears; drag
    into an over-limit column **is allowed**.
12. B4 frontend: badge + dialog + menu item. Tooltip copy: "15-card limit —
    advisory only; moves aren't blocked."
13. Full verification pass (see §Verification) on the 148-card seeded board;
    screenshots before/after at desktop + mobile; numbers into §Notes.
14. Lessons; `sqlx prepare --check` if SQL touched; typegen gates green;
    **no commit without approval.**

## Constraints

- **BoardView-scoped layout fix.** Do not edit `app-layout.tsx`; `<main>` keeps its
  `overflow-y-auto` but must never grow because the chain is bounded inside
  BoardView.
- Advisory limits never block add/drag/move/dnd; no toast on breach; per-column
  config only; no board-wide default; no Done/Backlog exclusions.
- Additive contract only (`card_limit` optional + nullable). `openapi.json` +
  `schema.d.ts` regenerate in the same PR.
- Ordering stays `sort_key` + `id` (T010/T022). The badge count uses server totals
  post-T022; never derive size from the hydrated window alone.
- T021's virtualizer + T022's observer cooperate with the B2 footer/fade — verify
  in the final pass, do not fight overscan with spacer CSS.
- Client-side board search remains loaded-cards-only until server-side search is
  scoped (documented in T022); B1 badge shows plain visible-card count under
  filters (single number, per the 5-1 header rule) and never `shown of total`.
- DnD-over highlight stays teal; the limit badge never tints the column body.
- Global DoD; tests green on both engines for the migration; no commit without
  approval.

## Acceptance Criteria

- [ ] `<main>` no longer the growth path: `main.scrollHeight == main.clientHeight`
      (measured gap 844 px client vs 6162 px scroll pre-fix), board chrome pinned at
      1600×900 and 420×880, and every column internally scrolls
      (`scroller.clientHeight < scroller.scrollHeight`). (document scrollHeight is
      NOT discriminating — it was already equal to `innerHeight` pre-fix.)
- [ ] Column header shows a count badge (plain count of visible cards —
      `70` or the filtered count; never dual-count `shown of total` in the header);
      after T022, sourced from server totals.
- [ ] Clipped column shows bottom fade + "50 of 137 — Load more" footer; IO
      (root = column scroller) auto-loads all remaining pages; footer visible only
      while more pages remain and hidden at full hydration; drag boundary behavior
      unchanged (loaded cards only).
- [ ] Density toggle flips `data-density="compact"`, persists via localStorage,
      visible difference in screenshot pair.
- [ ] Advisory limits: 0/negative rejected (400); set/clear round-trip; nullable
      column added without touching existing rows (dual-engine migration green);
      badge renders muted/amber/red by state; dialog validates inline; drag into
      over-limit column allowed; no toast fired.
- [ ] `openapi.json` + `schema.d.ts` regenerated; `gen:api --check` green;
      existing sanitizer/pagination/e2e suites green; typegen gates green.
- [ ] Before/after screenshots + `measurements.json` archived in the task notes;
      lesson appended.

## Notes

**Design locked 2026-11-18 via planning Q&A** (all options taken: count badge +
fade/footer, compact toggle, advisory limits; BoardView-scoped fix; new task after
T021/T022 — see chat log).

### Reproduction & verification (2026-11-18)

Seeded a scratch board on the throwaway `kanpad_e2e` schema (never the dev DB):
user `e2e-colheight@test.local` (`Test1234!`), board #24 "Repro Big Board mwncmhjl",
**148 cards** across 5 columns (To Do 70 / In Progress 45 / Done 15 / Pending 12 /
In Progress 6 — deliberately including a duplicated column name) via direct API
calls, titles varied for height diversity, label links attached per the seed script
(`repro-columns.mjs`, first two label slots — **`R&D`, `Feature`**; "Enhancement"
was never actually attached). Measured at 1600×900 and 420×880:

| Metric | Before (this board, Chrome) |
|---|---|
| Column "To Do" bounding height | **5996 px** (cards unclipped, content height) |
| `document.documentElement.scrollHeight` | 900 == `innerHeight` → `windowScrolls` **false** |
| Column scroller `clientHeight` vs `scrollHeight` | 5904 / 5904 — internal scroll never engages |
| Failure path | `<main>` (`app-layout.tsx:166`, `overflow-y-auto`) is the only scroller — `scrollHeight` ≈ 6162 vs `clientHeight` 844 (console-reported, not archived; `scroll-main.mjs` output, lost on session end — re-measure with output teed if cited again); toolbar scrolls off-screen; all columns dragged as one slab |
| Empty canvas below short columns | entire lower half of viewport empty once scrolled |
| Bottom | cards #62–#70 + "+ Add a card" ~6k px below chrome |

Techniques (full playbook: **`docs/lessons/frontend-visual-testing.md`**): API-seeded
fixtures via a Playwright `request` context (not the UI); `.mjs` repro scripts in
`/tmp/opencode/` using `createRequire` against `frontend/package.json` (repo
untouched); geometry measured via `page.evaluate` (`getBoundingClientRect`,
`scrollHeight`/`clientHeight`, `window.scrollTo`, `main.scrollTop`), not eyeballed;
screenshot matrix viewport | `fullPage` | scrolled real scroller | mobile;
note: `measurements.json`'s `cardCount` (71/46/16/13/7) is an off-by-one
`querySelectorAll('[class*="card"]')` artifact — the `cardCountAttrs` values
(70/45/15/12/6, matching the seed counts) are the counts of record.
browser-driving login through the UI only for real-path proof. Gotcha 1 — **CORS**:
dotenvy loads `.env` from the *process CWD* (`config.rs:486`); the prod origin
(`CORS_ORIGINS=https://kanpad.raziur.com`) lives in the **repo-root** `.env`, and the
API was booted from the repo root → browser calls CORS-blocked with a healthy
`/health`, preflight without `access-control-allow-origin` (reproduced via curl).
`backend-rs/.env` has **no** `CORS_ORIGINS` (its keys: APP_ENV/DB_*/SESSION_SECRET/
CARGO_*); the built-in default (`DEFAULT_CORS_ORIGIN`, `config.rs:52`) is
`http://localhost:5173`, which is why the stock Playwright suite works. Fix: serve
with `CORS_ORIGINS=http://localhost:5173` (boot log confirms
`cors_origins=["http://localhost:5173"]`). Gotcha 2 — **`waitForURL('/')`**:
Playwright waits for the navigation URL to match; its default is `waitUntil:'load'`,
and for a document that already fired `load` a completed same-document navigation
resolves immediately — the actual cause of the observed timeout was that the
CORS-blocked login never navigated at all. Diagnose network/console first
(`debug-login.mjs`); `waitForFunction(location.pathname === '/')` or
`waitUntil:'commit'` are both valid fallbacks.

After the fix, re-run the same script against a fresh 148-card board and diff
`measurements.json` (expect: per-column `scrollHeight > clientHeight`, columns
bounded by `clientHeight`, board chrome pinned, `<main>` `scrollHeight ==
clientHeight`).

**Running environment (dev):** API on :3001 (`APP_ENV=test`, built binary,
`CORS_ORIGINS=http://localhost:5173`), Vite on :5173 with
`VITE_API_URL=http://localhost:3001`, seeded board id **24**. Tear down:
`pkill -f 'backend-rs serve'` and find the Vite node process with
`pgrep -af "node.*vite"` (do **not** `pkill -f vite` — it matches the invoking
shell's own command line and SIGTERMs it, exit 143; see lesson), plus
`DROP DATABASE kanpad_e2e` if the seed should not persist.

### Research conclusion (why these four affordances)

- **Containment is the fix for the report, not a perf optimization.** Every
  serious kanban (Trello, SVAR) gives columns an internal scroll; the badge/fade
  patterns are signal, not cure.
- **Advisory limits are the only runaway-column remedy shipped widely** (GitHub
  "Set column limit" / Jira min-max / Trello List Limits / Azure soft limits). All
  are advisory — hard blocks corrupt reordering flows, so none are proposed here.
- **Explicit control beats endless scroll** (Smashing): we auto-load to the end
  and keep the footer as a real, keyboard-reachable control that is visible only
  while more pages remain (it is the manual path when IO hasn't fired yet);
  search/filters remain the find path.
- **Done-column hygiene (Jira's 14-day auto-hide) deliberately rejected** as a
  bundled item — product decision; would live behind its own task if wanted.

**Open calls:**
1. Fade placement (scroll sentinel vs before footer) — pick during B2; footer must
   remain reachable by keyboard.
2. `card_limit` validation bound beyond `≥ 1` (e.g. sane upper cap 1000?) — default:
   any `≥ 1`, revisit if abused.
3. Epic placement/wave — confirm when scheduled alongside T021/T022.

Resolved at planning (2026-11-18): filter-active badge = plain filtered count
(5-1 semantics); B2 footer visible only while more pages remain (disappears at full
hydration); IO rooted at the column scroller with unconditional auto-load to the
end; Stage A includes the `column-droppable.tsx` break; `status.yaml` records
`depends: T021, T022`.

## Lessons

- **2026-11-20 — button-placement follow-up (shipped):** "+ Add a card" was
  pinned to the column's full height because the droppable root *and* the column
  scroller both carried `flex-1`. Fix = content-sized scroller (`flex: 0 1 auto`
  + `min-h-0`) + footer moved **inside** the droppable (keeps the drop zone
  spanning header→column bottom; the droppable-only variant collapsed it to a
  16 px stub on empty columns). Geometry regression test:
  `frontend/e2e/column-add-card-placement.spec.ts`; full story in
  `docs/lessons/frontend.md` ("Column footer placement…", 2026-11-20).

(pending — Stage A lesson likely lands in `docs/lessons/frontend.md`; the Playwright
verification playbook already lives at `docs/lessons/frontend-visual-testing.md`,
2026-11-18)
