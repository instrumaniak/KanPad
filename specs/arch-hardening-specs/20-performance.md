# 20-performance — Wave 1 design (P1–P7, R4, S2–S4)

> Per-finding design: current state → target design → edge cases → test plan.
> Executed by tasks T016 (bundle, Wave 0), T020–T026. Global DoD: `00-overview.md`.

## Bundle: route splitting + mermaid (P1, P2) → T016

> **Amendment 2026-11-17 (T016 implementation):** the renderer is no longer mermaid —
> it is **pele** (`obsidianmd/pele`, MIT, Obsidian team; pinned exact `1.13.0`, loaded
> via `pele/lazy` so each diagram-type chunk loads on first use). The budget-gate
> mechanism is unchanged, but the measured numbers were refreshed on 2026-11-17:
> pre-fix dist = 5 JS files; entry 251,476 B raw / 69,824 B gzip · rolldown-runtime
> 1,116 / 658 · vendor-react 451,942 / 138,217 · vendor 925,976 / 278,425 ·
> vendor-mermaid 4,896,991 / 1,355,682; `modulepreload` referenced runtime,
> vendor-react, **vendor-mermaid**, vendor → ≈1.84 MB gzip initial on every cold load.
> Post-fix (catch-all `vendor` and `app` groups removed, `vendor-react` kept) dist =
> 83 JS files; route-level chunks restored; **initial gzip (entry `<script
> type="module" src>` + `modulepreload` hrefs) = 167,385 B** (gate measurement, node
> zlib) — well under the old
> 300 KB aspiration, so the 300 KB figure is retired; the gate is measure-then-set
> (167,385 B measured + ~19.5 % headroom = **200,000 B**; per-chunk limit **80,000 B**,
> covering vendor-react 67,627 B + ~18.3 %). The gate guards regression, not an
> absolute target — a ratchet task in a later wave is suggested.
> Follow-up correction (same day): the first fix measured 221,534 B with an
> over-capturing `vendor-react` regex that hoisted the react-* ecosystem (react-markdown,
> react-router, …) into the initial chunk; boundary-anchoring it to
> `(react|react-dom|scheduler)` dropped vendor-react 451,942 → 218,921 B raw
> (138,453 → 67,627 B gzip) and the numbers above were re-ratcheted.
>
> **Second amendment (same day, 2026-11-17):** user directive — perceive-snappiness
> first; the 83-file lazy split was judged too fragmented (one chunk per lucide icon,
> cold-landing routes paying an extra round trip, and two markdown renderers to boot).
> **All app routes are now static imports; a single `vendor` chunk holds all
> node_modules except the markdown/pele family (kept off the initial path, lazy via
> `LazyMarkdownRenderer`; pele/lazy keeps its per-type split)** — 58 JS files,
> **initial gzip = 257,986 B** (entry 69,500 + runtime 429 + vendor 188,057). Gate
> re-set to **310,000 B** initial / **225,000 B** per-chunk. The 167,385 B / 200,000 B
> / 80,000 B figures above are superseded. Card-details panel Preview tab now uses
> `LazyMarkdownRenderer` (GFM + highlight.js + mermaid — feature parity with notes);
> `MarkdownPreview` (kept, unwired there) now renders the board card face title
> (basic markdown, `inline` mode, memoized, links stopPropagation + target=_blank);
> `markdown-renderer` + the markdown/pele family are excluded from the initial path via
> an explicit LAZY_ONLY_PACKAGES set in vite.config.ts (packages with an out-of-family
> prod consumer — dequal → aria-query, is-alphabetical/is-alphanumerical/is-decimal →
> bcp-47 — deliberately stay in vendor). pele 1.13.0 stays exact-pinned and lazy;
> budget-gate red-proof re-verified at the new numbers (eager pele in the entry → 559,701 B → gate exits 1 → revert → OK).
> Also refreshed: the stale 2026-11-13/2026-11-16 P2 measured dates cited below are
> superseded by the 2026-11-17 re-paste above. For what the second amendment changed
> (all routes static, single vendor), see the block immediately below.

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

> **Superseded by the dated notes above (implementation + second amendment,
> 2026-11-17):** the "Target design" below is the pre-implementation plan — mermaid
> (not pele), 300 KB budget aspiration, 150 KB per-chunk, `chunkSizeWarningLimit`
> 300. Kept for history; the amendment blocks win on every number. The `vendor`
> catch-all removal was added during implementation (same bug class as `app`); the
> second amendment then restored one deliberate catch-all `vendor` group with the
> markdown/pele family excluded — the distinction that matters is what's *in* it.

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

## Transaction retry helper (R4) → T020 — **LANDED 2026-11-18** (T010 gate satisfied; D-5)

> Amendment 2026-11-16 (pass 2) rewrote the current-state list; the work
> landed 2026-11-18 (wave 1). The task's `position-shifting` name predates
> T010's fractional keys and is stale (kept for traceability). The
> pre-implementation text below the landed-state block is kept for design
> rationale — where it contradicts this block, this block wins.

**What landed (2026-11-18):** a shared `with_tx_retry` kernel in
`backend-rs/src/common/tx_retry.rs`, built on `common/retry.rs` (the scoped
`1213 | 1205 | UQ_*_sort_key`-1062 matcher — never duplicated; the matcher is
what the boundary also consults). The six inline retry loops (`create_card`,
`update_card`'s move worker, checklist-item create/update, column update,
bulk `move_cards`) migrated onto it behavior-preserving. The previously
single-shot write transactions now use it: **`create_column`** (the only un-retried position-rewriting tx —
serialized `LIMIT 1 FOR UPDATE` append + full-board rebalance), `delete_card`,
scalars-only `update_card`, `delete_column`, `update_checklist`,
`delete_checklist`, `delete_item`, `patch_note`/`delete_note`,
`patch_board`/`delete_board`, `delete_project`, and `register`. Insert-only transactions (`create_board`,
`create_checklist`, `create_note`) are audited and carry one-line comments
why they are not retry-wrapped; `admin.rs`, `common/session_store.rs`, and
`common/rebalance.rs` each carry their audit note (CLI exit-code contract;
own id-collision loop; helper belongs to the enclosing tx). `labels`/`tags`
have no transaction sites.

Locked decisions honoured: exhaustion → **503 + `Retry-After: 2`** at the
`Db` boundary (`BUSY_RETRY_AFTER = "2"`); scoped `1062` stays **409**;
commit is never re-executed; `READ COMMITTED` is set before `BEGIN` per
attempt (pool connections never leak `SET SESSION`); backoff is jittered
(`rand(5, 20·retry)` ms — decision 6); the backoff sleep holds no pool
connection; Argon2 stays outside the retried closure; the same-card race
`Restart` passes through **unretried** (bounded worker loop in the move
path). See `specs/tasks/T020-tx-retry-helper-for-position-shifting-transactions.md`
§Notes for the as-landed disposition of every implementation-step open item.

**Post-T010 scope note (D-5):** T010 replaced integer positions with fractional
`sort_key`s — single-row writes, no gap open/close, no whole-column locks — and
extends the matcher to `1213 | 1205 | constraint-scoped 1062`. T020 ran **after** T010 and
- extracted the inline retry into the shared kernel helper (`common/tx_retry.rs`,
  built on `common/retry.rs`),
- applied it to the remaining single-shot transactions (create/delete/move/
  scalars rewrites incl. `common/rebalance.rs`-bearing paths via their enclosing
  tx — NOT to `common/rebalance.rs` itself, whose call sites are the retried
  closures),
- did **not** re-add gap-lock logic T010 deleted.

**Target design (as landed):**
- Kernel helper (`src/common/tx_retry.rs`): `with_tx_retry` (wraps the common
  case) + `with_tx_retry_restartable` (the move worker's same-card-race
  `Restart` short-circuit passes through unretried). Both are built on
  `common/retry.rs`'s matcher + `MAX_ATTEMPTS = 5`; explicit rollback on every
  non-success path; optional `READ COMMITTED` before `BEGIN` (default on;
  per-call opt-out recorded at call sites if ever needed).
- A boundary arm in `error.rs` so an *exhausted* retry returns **503 + Retry-After: 2**
  (busy) rather than generic 500 — client-visible honesty. Boundary classification
  in the `Db` arm (`1213`/`1205` → 503; no new `AppError` variant); scoped-`1062`
  exhaustion stays 409 (see T020 Design decisions 1–2).
- The six existing inline retry sites (incl. `move_cards`) migrated onto the
  helper behavior-preserving; `cards_move.rs` + `sort_key_ordering.rs` stay green.

**Edge cases (all honoured as landed):** retries wrap only in-tx effects; the
same-card race keeps the `Restart` short-circuit (a logical conflict never
burns the lock-retry budget — it passes through `with_tx_retry_restartable`
unretried and the move worker bounds its own restarts); backoff holds no
pool connection and never approaches the 30 s `TimeoutLayer` (jittered
≤ 5+20+40+60+80 ms ≈ ≤ 200 ms worst case; a real blocked `1205` still rides
the timeout layer, by design); Argon2 stays outside any retried tx (unchanged).

**Test plan (delivered):** `backend-rs/tests/api/tx_retry.rs` — always-fire
SIGNAL-injector exhaustion for `1213`/`1205` (→ 503 + `Retry-After: 2` +
bounded-attempt count via the MyISAM fire counter), recovery across an
injected burst (→ 204/201 + row persisted), scoped-`1062` exhaustion stays
409 (fires == 5) and unrelated-`1062` never retries (fires == 1), rollback
proven by the survivor row; unit tests pin the backoff bounds, the
max-connections=1 sleep-holds-no-connection invariant, and the kernel's
attempt budget. `cards_move.rs` (incl. the T61 tests) and `sort_key_ordering.rs`
stay green unchanged. Persistence assertions per touched endpoint are in
`tx_retry.rs` (re-read via SQL).

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
