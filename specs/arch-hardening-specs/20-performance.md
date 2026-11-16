# 20-performance — Wave 1 design (P1–P7, R4, S2–S4)

> Per-finding design: current state → target design → edge cases → test plan.
> Executed by tasks T016 (bundle, Wave 0), T020–T026. Global DoD: `00-overview.md`.

## Bundle: route splitting + mermaid (P1, P2) → T016

**Current state:** source is correctly lazy — 9 `React.lazy` routes (`App.tsx:11-37`),
lazy mermaid (`mermaid-diagram.tsx:9-11`), lazy markdown renderers. But the **production
build defeats it**: `dist/` has 5 JS files, the entry chunk contains exactly one dynamic
import (mermaid), and `dist/index.html` `modulepreload`s the
**mermaid chunk (measured 2026-11-13: 4.9 MB raw / ~1.36 MB gzip)** alongside
rolldown-runtime (1.1 KB), vendor-react (~138 KB gzip), vendor (~278 KB gzip) and the
entry (~68 KB gzip) → **≈1.85 MB gzip modulepreloaded on a cold `/login` load**. Suspected
cause: the rolldown `codeSplitting.groups` in
`vite.config.ts:32-60` — the catch-all `app` group (`minSize: 100000`, priority 1)
merges entry-containing modules and keeps `entryFileNames`, collapsing route chunks;
the mermaid group (priority 30) gets statically linked from the merged entry. (Numbers
re-measured 2026-11-16; the original "3.1 MB / ~1.29 MB" figures predated the T010 rebuild.)

**Target design:**
1. **Diagnose first (spike step):** `npx vite build --debug` / rolldown chunk report
   (`rollup-plugin-visualizer`-equivalent or `--sourcemap` inspection) to pin the exact
   module edges that statically import mermaid into the entry. Red-proof: current build
   must show `modulepreload vendor-mermaid` before the fix; after, it must not.
2. **Fix the groups config:** drop or narrow the catch-all `app` group so entry modules
   stay in the entry and `React.lazy` boundaries survive (set `entriesAware` semantics
   correctly per current rolldown docs — verify against docs, don't guess), keep the
   `vendor-mermaid` group but ensure it's only reachable via dynamic import.
3. **Mermaid stays lazy:** confirm no static edge `src → mermaid` (grep already says
   none — the edge is a bundler artifact). If a bundler shim (e.g. rehype/remark plugin
   import) creates it, isolate it behind the existing `import('mermaid')`.
4. **CI budget gate:** script `frontend/scripts/check-bundle-budget.mjs` parsing
   `dist/` → fail if entry+vendors initial gzip > **300 KB** or if any `modulepreload`
   points at a chunk > 150 KB gzip. Wire into the CI `web` job after `vite build`.
   Lower `chunkSizeWarningLimit` back to a sane value (e.g. 300) once mermaid is out of
   the initial path.
5. Also (C2, with T018): move `shadcn`/`tailwindcss`/`@tailwindcss/vite`/`tw-animate-css`
   to `devDependencies` — separate commit-sized step or left to T018 (T018 owns deps).

**Edge cases:** mermaid must still render in notes (lazy boundary +
`LazyLoadBoundary` pattern already exists); `vite preview` smoke on `/login` and
`/notes/:id` with a diagram; hashed asset names change → `.htaccess`/nginx cache rules
already key on `/assets/*` (T012 adds `immutable`); dev mode unaffected (no chunking in
dev); the budget must not fight legitimate growth — 300 KB gzip initial is ~4× headroom
over a lean React app.

**Test plan:** red-proof both directions (see above) + budget gate red-proof (temporarily
import mermaid eagerly → CI fails); `vite build` output pasted in evidence; visual smoke.

## Transaction retry helper (R4) → T020 — T010 gate **satisfied** (T010 done; D-5)

> Amendment 2026-11-16 (pass 2): the current-state list is rewritten — the
> pass-1 amendment had verified only the card paths. As of today the retry
> envelope covers **six** write sites, including `move_cards`; the helper
> extraction below is still un-started. This section is the post-T010 target,
> not a description of untried current code.

**Current state:** the hardened path (`SET TRANSACTION ISOLATION LEVEL READ
COMMITTED` + stable lock order + `is_transient_lock_error` (1213|1205|scoped
1062) + linear backoff) now exists inline in **six** sites post-T010:
`create_card` and `update_card`'s move worker (`cards/routes.rs:112,325`),
checklist-item create/update (`checklists/routes.rs:260,359`), column update
(`columns/routes.rs:192`), and bulk `move_cards` (`columns/routes.rs:348`).
`move_card` remains the reference implementation. Sites that are still
single-shot and fall into the generic-500 arm (`error.rs`): `delete_card`,
`delete_checklist`/`delete_item`, `create_column`/`delete_column`,
notes/boards/labels/tags transactions, `common/rebalance.rs` (position
rewrites), and anything else the `FOR UPDATE` re-enumeration finds (32
`FOR UPDATE` sites across 7 files — audit, don't assume). Exhausted 500s at
these single-shot sites remain client-visible.

**Post-T010 scope note (D-5):** T010 replaces integer positions with fractional
`sort_key`s — single-row writes, no gap open/close, no whole-column locks — and extends
the matcher to `1213 | 1205 | 1062`. T020 therefore runs **after** T010 and:
- extracts the inline retry into a shared kernel helper,
- applies it to whatever single-shot transactions remain post-T010, as found
  by a fresh `FOR UPDATE` enumeration (create/delete/move conditional updates,
  `1062` unique-key races on `(parent, sort_key)`, notes/boards/labels/tags,
  `common/rebalance.rs`) — **not** `move_cards`, which already has it,
- does **not** re-add gap-lock logic T010 deleted.

**Target design:**
- Kernel helper (e.g. `src/common/tx_retry.rs`): `async fn with_tx_retry<F, T>(pool,
  opts, f) -> Result<T, AppError>` where `f` receives a `Tx`, retries on
  `is_transient_lock_error` (1213|1205|1062 post-T010) up to N=5 with linear backoff,
  explicit rollback between attempts, optional `READ COMMITTED` set before `BEGIN`
  (keep the move_card semantics; make it an option so non-positioning txs can skip it).
- A matching arm in `error.rs` so an *exhausted* retry returns **503 + Retry-After**
  (busy) rather than generic 500 — client-visible honesty.
- Apply to: `delete_card` and the remaining single-shot sites found by the
  fresh `FOR UPDATE` enumeration (checklist delete sites, `create_column`/
  `delete_column`, notes/boards/labels/tags, `common/rebalance.rs`) — each
  either gets the helper or gets a comment why it doesn't (single-row, no
  race). Do not re-touch the six sites that already retry.

**Edge cases:** retries must not wrap non-idempotent *effects outside the tx* (all
effects are in-tx here — assert by review); same-card race must keep the `Restart`
short-circuit (don't retry a logical conflict as a lock conflict); backoff must not hold
a pool connection beyond the 30 s request timeout (5 × backoff ≪ 30 s — compute worst
case); Argon2 hashing must stay *outside* any retried tx (already true — keep it).

**Test plan:** the existing 500-move sequential-exact-order + concurrent invariant fuzz
(`cards_move`) must stay green; add an injected-lock-error test (failpoint or a test
that holds `FOR UPDATE` from a second connection) proving retry recovers; exhausted-retry
→ 503 assertion; persistence assertion per touched endpoint (AGENTS.md).

## Board rendering (P3, P4) → T021

**Current state:** `GET /boards/{id}/columns` returns **every card** (contract:
nested unpaginated, `columns/routes.rs:69-70`); `column-card-list.tsx` maps all; no
virtualization anywhere; every `Card` mounts `CardDetailPanel` + ContextMenu + AlertDialog
+ hooks (O(N) instances); `BoardView` flatMaps all cards twice for counts. During
cross-column drags, `handleDragOver` (`drag-drop-context.tsx:100-186`) writes
`setQueryData(['columns', boardId])` on **every dragOver event**, and `allColumns`
(`board-view.tsx:256,269`) is a fresh array each write → full-board re-render at
pointer-move rate. Mitigant today: React Compiler (enabled) — but prop identity changes
defeat it.

**Target design:**
1. **Virtualize the card lists:** `@tanstack/react-virtual` on `column-card-list` and
   `board-list-view` (fixed/estimated row height; dnd-kit integration needs care —
   see edge cases). Threshold: virtualize always (simpler than conditional) *or* when >
   50 cards — decide at implementation after measuring; default = always, with
   `overscan` tuned so drag placeholders stay visible.
2. **Throttle dragOver cache writes:** rAF-coalesce `setQueryData` (at most one write
   per frame), and derive the "hover column" visual state from a lightweight local
   state (or dnd-kit's own over-id) instead of rewriting the whole columns cache —
   the cache write on dragOver exists only to move the card visually; on `dragEnd` the
   real mutation + invalidation already happen. Keep same-column fast path.
3. **Stabilize identities:** memoize `allColumns` (or pass `columns` ids + a getter),
   avoid recreating `cards` arrays per write (structural sharing: React Query does this
   if we patch immutably — ensure the dragOver write doesn't rebuild every column's
   array).
4. **Count derivations:** precompute board counts in `useMemo` keyed on columns (or
   server-provided counts) instead of double flatMap per render.
5. Keep optimistic update + rollback semantics and the `dragSource`-at-start convention
   (AGENTS.md) untouched.

**Edge cases:** dnd-kit + virtualization is the classic conflict — sortable items must
exist in the DOM to be dragged; use dnd-kit's `SortableContext` with the virtualizer's
items and render only visible rows (verify drag preview/overlay still works — the
`DragOverlay` pattern already used helps); touch sensors (750 ms delay) unaffected;
keyboard sensor must still reach off-screen rows (dnd-kit keyboard drag scrolls — test);
empty column, single card, 500-card column; drag over a column whose cards are
virtualized away (overlay must not jump); `CardDetailPanel` per-card cost — gate mount
on `open` (already fetch-gated; also make the component mount lazy if cheap).

**Test plan:** existing drag-drop suites (unit + e2e) green; new test: dragOver writes
≤ 1 per rAF (spy on setQueryData count during a synthetic drag sequence); perf smoke in
T026 (500-card board, input latency); no behavior change to optimistic rollback.

## Pagination (P5 ceilings, P6 UX) → T022

**Current state:** top-level lists are `COUNT` + `LIMIT/OFFSET` with `limit ≤ 100` but
**no page ceiling** (`extract.rs:115-135`; test literally asserts "page has no ceiling");
frontend board/archived/notes lists fetch defaults (limit 50) with **no pagination UI**
→ items beyond page 1 invisible; cards have a paginated endpoint + `useCards(page,limit)`
that **no component uses** (board loads all via columns embed).

**Target design:**
- **Server:** cap `page` (e.g. `page * limit ≤ 10_000` → 400 outside, matching the
  strict-no-clamp convention) — cheap guard against deep-offset scans. Keyset cursors
  are the *correct* fix but a contract change (new `cursor` param, `Link`-style
  envelope) — **defer** unless T026 measurements show offset pain; record as open call.
- **Board cards** *(decision pinned 2026-11-16, was an open call)*: switch the board
  view's card source to the existing paginated cards API with **per-column infinite
  scroll** (`useInfiniteQuery` per column, keyed e.g. `['column-cards', columnId]`) past
  the first **N=50** cards, which stay embedded in the columns response. Per-column,
  not board-level, because: (a) the UI scrolls per column; (b) per-column cache keys
  keep writes/invalidation local to the column involved in a drag; (c) a board-level
  query re-pages every column on any drag and complicates optimistic rollback. The
  columns embed keeps powering structure/counts until counts come from the server
  (T021 §4 / below). Optimistic drag source must be loaded (first page) — see edge cases.
- **Lists (boards/archived/notes/projects):** add load-more / page controls using the
  existing `Page<T>` envelope (`total` is already returned).
- **Counts:** `BoardView` double-flatMap counts must come from server totals once cards
  are paged (ties into T021 §4).

**Edge cases:** optimistic drag across a not-yet-loaded page (drag source must be
loaded; drop target loading state); `total` vs loaded mismatch during concurrent adds;
search/filter interplay (filter resets pagination — existing `board-view` search is
client-side over loaded cards — must move server-side or document the limitation:
**open call**); archived toggle preserving page state (project-context: view toggle
preserves scroll/filters).

**Test plan:** page-ceiling 400 test; infinite-scroll loads page 2 with dedupe (no
duplicate keys); drag an item from page 1 into a column while page 2 loads; lists show
load-more and stop at `total`; existing pagination sanitizer tests stay green.

## Search FTS (P5) → T023 *(conditional)*

**Current state:** `notes.title LIKE '%q%'` and `projects.name LIKE '%q%'` (escaped,
bound — injection-safe) with `IDX_notes_title` B-tree unusable for leading wildcards →
table/index scan + row filter. No `FULLTEXT` anywhere.

**Target design (execute only when data volume warrants — T026 informs):** migration
adding `FULLTEXT INDEX` on `notes.title` (+ `content`? — open call: content search is a
product decision) and `projects.name`; query switches to `MATCH … AGAINST (? IN BOOLEAN
MODE)` with the same escaping/threshold semantics; keep `LIKE` as fallback when the
term has no FTS-usable tokens (short/stopword-only). MySQL 8.4 **and** MariaDB 11.4 both
support InnoDB FULLTEXT (10.6 also does; the migration must be verified on the CI legs —
and on 10.6 as well if T011 confirms prod is 10.6, see `40-operations.md` §6).

**Edge cases:** min word length (`innodb_ft_min_token_size` default 3 — we can't tune
server config portably → document); boolean-mode operators in user input (`+ - * ( )`
must be neutralized — strip or escape); emoji/CJK (utf8mb4, tokenization differs — the
repo has an emoji gate test pattern to reuse); ranking vs `ORDER BY updated_at` (decide:
relevance or recency — default recency to match current behavior); MariaDB/MySQL
behavior parity (test both).

**Test plan:** migration green on both engines; parity tests: same results FTS vs LIKE
on a seeded corpus (modulo ranking); operator-injection input doesn't 500; existing
notes/projects search tests stay green or are updated with rationale.

## Metrics fixes (P7) → T024

**Current state:** `AppState::acquire_timed` records into
`kanpad_db_pool_acquire_duration_seconds` but its **only caller is a test**
(`state.rs:177-184`, `tests/api/metrics.rs:157`) → histogram always empty in prod;
`kanpad_db_up` refreshed only by `/ready` hits + boot (stale when probe traffic stops);
latency histogram is global, not per-route; no process metrics.

**Target design:**
- Route real pool acquires through `acquire_timed` (make it the default acquire path in
  `state.rs` / wherever `pool.acquire()` is called from stores — or wrap at the pool
  level so call sites don't change; pick the single-chokepoint option).
- Background probe task: `tokio::spawn` every 30–60 s calling the existing
  `wait_until_ready` with a small budget, feeding `record_db_probe` (reuses the
  transition-logger CAS) → `kanpad_db_up` always fresh. Must not fight `/ready`'s
  probe (both write the same state — the CAS transition logger already makes this safe;
  verify gauge writes are atomic).
- Per-route latency: add `route` label to the duration histogram using the same
  `MatchedPath` cardinality guard as `requests_total` (never raw URI).
- Keep the fail-closed `METRICS_PORT` model and no-self-scrape rule untouched.

**Edge cases:** background probe on a lazy pool must not force connections during a DB
outage storm (budget-bounded, same as `/ready`); probe task abort on shutdown (join the
sweeper's abort pattern); clock/backoff interactions none; metrics endpoint stays
unauthenticated → document port restriction (already in `.env.example`).

**Test plan:** scrape asserts non-empty acquire histogram after N requests (red today);
`db_up` freshness test (probe task updates without `/ready` hit); cardinality test
(`/api/v1/cards/{id}` variants don't mint series — mirror the existing guard test);
shutdown aborts the task (no leaked task warnings in test output).

## Capacity smoke + headroom (S3, P7 interplay) → T026

**Current state:** no capacity model anywhere; pool=10, 30 s global timeout, 120/min/IP
throttle — ~10 concurrent slow queries starve the pool (an 11th waits on pool checkout up
to the 30 s timeout → 503).

**Target design:** a reproducible load profile doc + run:
- Tool: **k6** (single binary, script committed as `scripts/load/board-smoke.js`) or
  `wrk`/`oha` if k6 unavailable — pick at implementation; goal is repeatability, not
  tooling. Scenario: seeded DB (T07 fixtures pattern) with N boards × M cards; ramp to
  target **100+ concurrent users** (D-2): login storm (respect throttle — use session
  cookies pre-seeded), board loads, card drags (move endpoint), notes list.
- Measure: p50/p95/p99 latency vs NFRs (page <2 s, board <1 s, API <500 ms, drag
  <100 ms), pool acquire wait, 429/503 counts, DB CPU.
- Output: `docs/capacity-headroom.md` — measured ceiling on the target box, the pool
  sizing rule (`replicas × DB_MAX_CONNECTIONS ≤ max_connections − margin`, one
  connection held by the sweeper per instance), and the tuning knobs that moved the
  needle. This doc is what future "should we scale?" questions answer.

**Edge cases:** throttle skews results (raise `THROTTAL_GLOBAL_PER_MIN` for the test
env only, document it); Argon2 login storm is CPU-bound by design (measure, don't
"fix"); run against the *compose* stack (T012) so results approximate the appliance;
never run against production data.

**Test plan:** the run itself + committed script + recorded results (this is a
measurement task; AC = reproducible numbers + doc).

## Deferred / out of scope (recorded, not forgotten)

- **S2 — shared rate-limit state (Redis-backed governor)** before any horizontal
  scale-out; until then document N× limits (`lib.rs:356-358` comment already says this).
  Not needed while single-box (T026 validates the box first).
- **S4 — queue/worker infra** for future async (export/email): revisit when a feature
  needs it; sessions-in-MySQL already makes replicas stateless.
- **Keyset pagination** (open call under T022) — adopt only if T026 shows offset pain.
- **Session read caching** (F9 from the review: 1 SELECT/request on `sessions`) — hot PK
  table, fine at this scale; revisit with capacity data.
