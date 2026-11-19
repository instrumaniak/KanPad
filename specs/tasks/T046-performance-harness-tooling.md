# T046: Performance harness & tooling

> **STATUS: in_progress.** Wave 1 (Phase 1). *(Header reconciled with
> `specs/tasks/status.yaml:285`, which is the system of record for status —
> see `00-overview.md:139`.)* Self-contained tooling task — no task depends on
> it, but it *feeds* T026 (consumes the k6 scaffolding built here) and
> coordinates with T024 (borrows `/metrics`), T022 (pagination rework changes
> F2's measured path), T029 (workflow-file lock + SQL observability), T030
> (Sentry — see deferred RUM note), T044 (edits the two files F1 wraps).

## Goal

Close the "no performance tooling/harness anywhere" gap with a committed,
reproducible measurement layer for **both frontend and backend**, plus a single
budget registry so every perf claim has a number, a tier, an enforcing test,
and an explicit NFR mapping (or an explicit "not NFR-enforcing" label).

Research basis (2026-11-20): wall-clock benchmarks swing ~3× on shared CI
runners while callgrind instruction counts hold ±0.13% (hotpath.rs 2026-11
100-run GitHub-Actions experiment; Criterion FAQ for the "don't gate on CI
wall-clock" guidance) — so deterministic gates go in CI and wall-clock stays
local; drag smoothness is measured with frame deltas + Long Animation Frames,
not INP (INP excludes the *continuous* pointermove portion of a drag — drag
start/end are still counted); query plans are guarded by *shape* (index used /
no full scan), never cost; Lighthouse CI is deferred (bundle gate + load
timing + drag harness cover the signal at lower variance).

Caveats carried into the runbook: instruction counts are only comparable on
the **same valgrind build + toolchain + architecture** (runner images update
weekly → re-baseline procedure required), and `Ir` is blind to cache locality
(identical counts for sequential vs random access).

### Frontend (4 measurements + helpers)

| # | Deliverable | Tier | Key details |
|---|---|---|---|
| F1 | **Drag interaction perf harness** | B (default e2e) | `e2e/perf-helpers.ts`: rAF frame sampler, LoAF + longtask observers (`PerformanceObserver.supportedEntryTypes` feature-detect — LoAF is Chromium-only, fine for our chromium-only suite), **input-latency probe with a pinned formula** (primary: LoAF `firstUIEventTimestamp → presentationTime`; fallback: `event.timeStamp → next rAF` — an upper-bound proxy; p95 over pooled pointermove samples across the measured runs, ≥100 moves/drag so p95 is meaningful), CDP 4× CPU throttle (`Emulation.setCPUThrottlingRate` via `newCDPSession`, try/finally restore), **1 discarded warm-up + 3 measured runs → median**; dev-gated React Profiler (`import.meta.env.DEV`, returns children untouched otherwise) wrapping `BoardView` + `ColumnCardList` — store holds **timings/component ids only, never props**; `e2e/perf-drag.spec.ts` — cross-column / same-column / 4×-throttled scenarios, ~600-card seeded board (15 cols × 40), **paced pointer input** (single-step `mouse.move` interleaved with an awaited rAF — a burst `{steps:20}` move would let the sampler record only idle frames), target column `scrollIntoViewIfNeeded()` first with scroll excluded from the measure window, **measure-first budgets** (~2–3× baseline, documented in spec header), all numbers printed even when green; `test.setTimeout(180_000)` (precedent `virtualization.spec.ts:18`); seed **once** in `beforeAll`, reused across warm-up + 3 runs |
| F2 | **Board load timing budget** | B | Promote `virtualization.spec.ts`'s ad-hoc `ttInteractiveMs` console log (`:131,:137`) into an asserted budget, **renamed `navToNetworkIdleMs`** — it is navigation→`networkidle` measured against the **Vite dev server**, so it is a **dev-mode tripwire, explicitly NOT NFR-enforcing** (NFR "board <1 s" is end-to-end/production; prod load timing stays unenforced — recorded as deferred below). Baseline first: known runs are 983 ms / 1189 ms (T021) and ≈1.2 s (T026 header) → budget = measured median ×2–3, **not** an invented 500 ms figure (which would be red on day one); pin one baseline figure + date in the registry |
| F3 | **Heap growth guard** | B | CDP `Performance.enable` + **`HeapProfiler.collectGarbage` before each sample** (JSHeapUsedSize without forced GC is MB-noisy); N× navigate/return over a **fixed board set** (same boards each cycle so legitimate React Query cache growth plateaus instead of tripping the wire), fixed N, delta budget from a CI baseline; bounded tripwire for cache/listener leaks |
| F4 | **Render-count budgets in vitest** | A (per-PR) | React Profiler in jsdom: **`BoardView` + `ColumnCardList` render-*counts*** under synthetic prop churn (name consistent with the wrap targets; `Column` was a typo — `ColumnCardList` owns the virtualizer). Counts are integer-deterministic → asserted. **Durations are print-only** (jsdom wall-clock is not deterministic — asserting them in Tier A would contradict "never flaky"). Document in the spec header: vitest does not run the React Compiler preset (`vite.config.ts:71-73` applies it, `vitest.config.ts:14` does not), so counts guard identity-stability of uncompiled React, not compiled prod renders |

### Backend (3 workstreams)

| # | Deliverable | Tier | Key details |
|---|---|---|---|
| B1 | **Micro-benchmarks: criterion + gungraun instruction gate** | A (gungraun) / manual (criterion) | `backend-rs/benches/` for **pure** hot paths: `order_key::{key_between, keys_between, needs_rebalance}` (+ `clamp_position` folded into a scenario — a 2-line clamp benches as noise). **Not `common/rebalance.rs`**: its four pub items are `async` and take `&mut MySqlConnection` (`rebalance.rs:31-33`) — not micro-benchable; its pure kernel `rebalance_sort_keys` calls `order_key::keys_between`, which *is* benched. criterion = local wall-clock only (never a CI gate); **`gungraun` ≥ 0.20.0 = the CI regression gate** (iai-callgrind was renamed Gungraun at 0.17 and upstream is unmaintained; ≥0.20's test mode matters because CI runs `cargo llvm-cov --all-targets` (`backend-rs.yml:438-440`) which *executes* bench targets — older gungraun would try to invoke valgrind inside the coverage step and fail). **Baseline artifact**: committed baseline files + documented re-baseline procedure (toolchain/valgrind bump → re-record with justification in the same PR) + `valgrind --version` assertion in the job — without a stored baseline the gate cannot go red deterministically. Guards T010's sort-key math (note: T022's keyset open call is fed by B2's plan shapes + T026's load numbers, not by these benches) |
| B2 | **Query plan regression guards** | A (both DB legs) | **Placement: `backend-rs/tests/api/query_plans.rs`** — cargo only auto-discovers `tests/<name>.rs` / `tests/<dir>/main.rs`, and the repo has exactly one integration root (`[[test]] name = "api"`, `Cargo.toml:86-88`); `tests/perf/query_plans.rs` would be dead code, and `tests/README.md` prescribes helper/import rules, not suite placement (the old "(suite placement per tests/README.md)" pointer was wrong). Plain read-only `EXPLAIN` on hot SQL (board columns/cards load, **card-move transaction — post-T010 it is last-key read + neighbour read + single UPDATE, no more position-shift renumbering**, pagination, notes list) asserting **plan shape: index used, no row with `type: ALL`** — never cost/timing. Parsing rules: runtime `sqlx::query` (not `query!` — avoids `.sqlx` regen and the MySQL-only `prepare --check` byte-compare), read `type`/`key` **by column name**, never fetch `partitions` (MySQL 8 has it, MariaDB 11.4 tabular does not), **per-row** assertions for multi-row plans (notes list is a LEFT JOIN), plain EXPLAIN only (`EXPLAIN EXTENDED` is deprecated in MySQL 8). Seeded realistic rows + `ANALYZE TABLE` **inside the per-test ephemeral DB only**. All named queries already have covering indexes (migrations 0001/0003) → starts green; the red-proof proves it |
| B3 | **k6 scaffolding (T026 consumes)** | C (manual) | `scripts/load/seed.mjs` (N boards × M cards, **sessions via the login API** — matches `frontend/e2e/*` pattern; direct DB session minting would need `SESSION_SECRET` + the internal HMAC/rmp-serde cookie format, duplicating audited auth logic — not doing it) + `scripts/load/board-smoke.js` scenario skeleton (ramp to 100+ VU; board loads, card moves, notes list) + `scripts/load/README.md` with exact run commands **and mandatory safety guards** (see Constraints) + **`scripts/load/compose.load.yaml`** — a local override file for the throttle raise. T026 runs it, tunes scenarios, writes `capacity-headroom.md`. **Ownership**: T046 creates `board-smoke.js` (T026's Files table also lists it — T046 owns creation, T026 owns tuning; recorded in both §Notes) |

### Cross-cutting docs

- **`docs/perf-budgets.md`** — single registry of every budget: value, tier,
  enforcing test, last-measured date, **NFR mapping (or explicit "dev-mode
  tripwire — not NFR-enforcing" label)** (bundle ✅ already enforced, drag
  frame stats, input-latency p95 → NFR3 drag <100 ms, board load → NFR2
  caveat, heap delta, render counts, plan shapes, API p95 → T026).
  **Freshness rule**: any threshold change must update `last-measured date`
  in the same PR (loosening a number to go green without re-measuring is the
  classic way perf gates die).
- **`docs/perf-runbook.md`** — how to measure anything: drag profile,
  flamegraph/`tokio-console`, criterion, gungraun/callgrind (incl.
  re-baseline procedure + `Ir` cache-locality caveat), k6, EXPLAIN dump.
- Frontend perf playbook appended to **`docs/lessons/frontend.md`** (dated,
  tagged — the enumerated lessons file per AGENTS.md:87; `frontend-perf-testing.md`
  is not in the list and `frontend-visual-testing.md` is the wrong topic).

### Explicitly deferred (recorded, not forgotten)

- **Lighthouse CI** — variance + CI minutes; bundle gate + F1/F2 cover the
  signal. Revisit only if a page-weight ceiling beyond bytes is needed.
- **Production (dev-server-independent) load timing** — F2 deliberately gates
  the dev-server number only; a `vite preview`-based prod-like TTI gate is
  deferred with Lighthouse CI.
- **Field RUM / web-vitals** — no task currently owns this (T030 is Sentry
  errors + traces; its text does not claim web-vitals). Recorded as an open
  call, not silently assigned to T030.
- **Virtualized-scroll frame-delta gate & notes/long-text render budget** —
  F1 covers drag; virtualization *DOM* bounds are already asserted
  (`virtualization.spec.ts:134-145`). Add scroll/notes budgets if jank is
  reported or T022's pagination rework lands.
- **Slow-query digest / per-query metrics** — T029; T046 only *borrows*
  `/metrics` for a smoke assertion (coordinate with T024, which fixes the
  empty-histogram defect first — ordering noted in Depends On).

### Enforcement philosophy

- **Tier A — per-PR, deterministic, never flaky:** bundle bytes (exists),
  gungraun instruction counts (stored baseline + pinned toolchain), EXPLAIN
  plan shapes, vitest render **counts** (durations print-only).
- **Tier B — default e2e, statistical:** 1 discarded warm-up + 3 measured
  runs → median, deliberately generous budgets (tighten only after observing
  the distribution), numbers printed as test output even when green;
  CI-runner baselines measured separately. **Non-contended CI execution**:
  perf specs run as a separate `playwright test --grep @perf --workers=1
  --retries=0` step inside the docker-e2e job (same job ⇒ still every-PR,
  not optional; workers:4 + retries:2 would both inflate frame timings via
  CPU contention and mask flake via silent retry-passes). **Threshold env
  var: `PERF_BUDGET_SCALE`** (documented in the runbook; CI sets the measured
  value, local defaults to 1). **Quarantine rule**: a perf spec that flakes
  twice in CI is escalated in §Notes the same day — options are re-baseline
  (registry updated) or demote to Tier C; never "retry until green".
- **Tier C — manual/nightly measurement, numbers not gates:** k6 runs,
  criterion trends, flamegraphs.

## Depends On

(None hard — self-contained. Soft ordering recorded here so it is not
aspirational:)
- **T024 before the `/metrics` smoke** (its empty-histogram fix must land
  first; if T024 is still pending at T046 completion, the smoke is recorded
  as skipped-in-§Notes, not silently dropped).
- **T026 consumes B3** — request adding `T046` to T026's `depends` edge in
  `specs/tasks/status.yaml` during hand-off (today T026 depends on
  `[T020, T021, T022]` only, and T026's Files table also claims
  `scripts/load/board-smoke.js` — see B3 ownership row).

## Phase

1

## Critical

No — it is evidence/tooling infrastructure; nothing blocks on it except T026
consuming the k6 scaffolding.

## Spec References

- `specs/planning-artifacts/architecture.md` — NFR1–NFR5 (page <2 s, board
  <1 s, drag-drop <100 ms, API <500 ms).
- `specs/arch-hardening-specs/20-performance.md` §Board rendering (P3/P4 test
  plan names the "perf smoke (500-card board, input latency)" — **assigned to
  T026 at `:223-224`; T046 builds the harness/assertion that T026's baseline
  then consumes** — hand-off step records the transfer), §Capacity smoke
  (→ **T026** at `:320`; T046 scaffolds the scripts), §Metrics fixes (P7 —
  `/metrics` endpoint T046 borrows). Known typo there: `THROTTAL_GLOBAL_PER_MIN`
  at `:339` = `THROTTLE_*` — do not propagate.
- `specs/tasks/T021-…` — shipped virtualization + rAF dragOver coalescing;
  §Notes admits dragOver cost "was NOT re-measured with a profiler" (F1 closes
  that hole); also the source of F2's known baselines (983/1189 ms).
- `specs/tasks/T022-…` — pagination rework (first-N embedded + per-column
  infinite scroll) **changes F2's measured path and F1's board shape** →
  re-baseline after it lands. T022's keyset open call is fed by T026/B2, not
  by B1's benches.
- `specs/tasks/T044-…` — edits `board-view.tsx` + `column-card-list.tsx`, the
  two files F1/F4 wrap → coordinate wrap placement; re-measure after layout
  changes.
- `specs/tasks/T026-capacity-smoke-headroom-doc.md` — consumes B3 + F1/F2
  baselines (the 500-card perf smoke of record belongs to T026 per
  `20-performance.md:224`).
- `specs/tasks/T016-…` — bundle budget gate (exists, referenced by registry).
- CI: `.github/workflows/backend-rs.yml` — backend + backend-mariadb legs
  (the MariaDB leg runs a **curated `cargo test --test api <filter>` list**,
  `:591-601` — B2 must be added there), web job (budget step `:648`, where
  the `__PERF__` dist-grep also goes), docker-e2e job (playwright).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/e2e/perf-helpers.ts` | **NEW** — frame sampler (paced-input aware), LoAF/longtask observers (feature-detected, torn down between runs), input-latency probe (pinned formula above), CDP throttle (try/finally), warm-up+median helper |
| `frontend/e2e/perf-drag.spec.ts` | **NEW** — F1 scenarios (cross-column, same-column, 4× throttle), measure-first budgets, `@perf` tag, 180 s timeout, seed-once |
| `frontend/e2e/perf-load-heap.spec.ts` | **NEW** (or folded into existing specs) — F2 `navToNetworkIdleMs` budget + F3 heap growth guard (GC-forced) |
| `frontend/src/lib/perf-profiler.tsx` | **NEW** — dev-gated `<PerfProfiler>` (React Profiler → `window.__PERF__` store with reset/snapshot; **timings/ids only, no props**) |
| `frontend/src/features/boards/board-view/board-view.tsx` | Wrap with `PerfProfiler` (F1/F4) — coordinate with T044's edits to this file |
| `frontend/src/features/columns/column-card-list.tsx` | Wrap with `PerfProfiler` (F1/F4) — coordinate with T044's edits to this file |
| `frontend/src/lib/perf-profiler.test.tsx` | **NEW** — unit red→green for store record/reset/snapshot + dev gate (note: proves the DEV=true path only — prod absence is proven by the dist grep below) |
| `frontend/src/features/board-view/render-budget.test.tsx` | **NEW** (concrete path; was an ellipsis placeholder) — F4 render-count budgets (vitest/jsdom), durations print-only |
| `frontend/e2e/virtualization.spec.ts` | F2: promote `ttInteractiveMs` log to an asserted budget renamed `navToNetworkIdleMs` (or move to perf spec) |
| `backend-rs/benches/order_key.rs` | **NEW** — criterion wall-clock benches (local/nightly) |
| `backend-rs/benches/order_key_callgrind.rs` | **NEW** — gungraun ≥0.20 instruction-count benches = CI gate, with **committed baselines** |
| `backend-rs/Cargo.toml` | bench targets (`harness = false`) + dev-deps (`criterion`, `gungraun`); must keep `cargo deny`, `sqlx prepare --check --all-targets`, `cargo llvm-cov --all-targets` green |
| `.github/workflows/backend-rs.yml` | callgrind bench step/job (valgrind presence check + install, `valgrind --version` assertion); **`query_plans` added to the MariaDB-leg curated test list (`:591-601`)**; `__PERF__` dist-grep in the `web` job. ⚠️ T029 marks this file "NO CHANGES (locked down)" — sequence/announce the edit (see Constraints) |
| `backend-rs/tests/api/query_plans.rs` | **NEW** — B2 EXPLAIN plan-shape guards (inside the existing `api` root; per-test ephemeral DB) |
| `scripts/load/seed.mjs` | **NEW** — B3 scalable seed; **mechanical prod-guards, no dotenv autoload** |
| `scripts/load/board-smoke.js` | **NEW** — B3 k6 scenario skeleton (created here, tuned by T026) |
| `scripts/load/compose.load.yaml` | **NEW** — local override raising `THROTTLE_*` for load runs (compose.yaml itself is a prod appliance — never edited) |
| `scripts/load/README.md` | **NEW** — exact run commands (T026 entry point) + safety guards |
| `docs/perf-budgets.md` | **NEW** — budget registry (value, tier, test, last-measured, **NFR mapping**) |
| `docs/lessons/frontend.md` | Append dated, tagged Playwright perf playbook lesson |
| `docs/perf-runbook.md` | **NEW** — measurement runbook |
| `docs/lessons/rust-backend.md`, `docs/lessons/ci-quality.md` | Dated, tagged lessons (gungraun baseline + workflow-gate lessons) |
| `specs/arch-hardening-specs/20-performance.md` | Dated amendment: record transfer of perf-smoke harness/assertion from T026 → T046 (baselines remain T026's) |
| `specs/tasks/T026-…` §Notes | Hand-off: scaffolding ready + `board-smoke.js` ownership split + `depends` edge request in `status.yaml` |
| `specs/tasks/T024-…` §Notes | Metrics smoke coordination (skip-if-pending rule) |

## Implementation Steps

1. **F1 drag harness (TDD):**
   a. Red: unit tests for the Profiler store (record/reset/snapshot, dev gate).
   b. Implement `perf-profiler.tsx`, wrap `BoardView` + `ColumnCardList`
      (T044 collision check first); `npm run budget` re-verified **and**
      post-build grep of `dist/` for `__PERF__` must return empty — the byte
      budget alone has ~20% headroom and would let a 1–2 KB leak through.
   c. `perf-helpers.ts` collectors: frame sampler, LoAF + longtask observers
      (feature-detect + teardown), input-latency probe with the pinned
      formula, CDP 4× throttle with try/finally restore, discarded-warm-up +
      median helper.
   d. `perf-drag.spec.ts` red-proof: impossible budget → red → restore;
      real degradation (temporarily disable T021's rAF dragOver coalescing) →
      red → revert. Then baseline (warm-up + 3× locally), fill thresholds
      ~2–3× median, document distribution in the spec header. Verify the
      paced driver actually records non-idle frame deltas before trusting the
      numbers (a burst-input sampler will happily measure 16.7 ms idle frames).
2. **F2/F3/F4:** F2 baseline-first (known ~983 ms–1.2 s → generous budget),
   F3 with `Performance.enable` + `HeapProfiler.collectGarbage` + fixed board
   set (baseline first — dev-mode React inflates numbers), F4 render-count
   red first (durations print-only).
3. **B1 benches:** criterion benches for `order_key` pure fns; gungraun twins
   (≥0.20.0) with committed baselines; CI job green, then red-proof
   (artificial slowdown in a bench branch → count goes up → gate red →
   revert). Confirm `cargo llvm-cov --all-targets` still passes (benches get
   *executed* there — gungraun test mode / criterion `--test` self-detect).
4. **B2 plan guards:** in-test seed + `ANALYZE TABLE` (ephemeral per-test DB
   only) + tabular `EXPLAIN` assertions by column name; red-proof by
   `DROP INDEX` **inside the per-test DB only — never edit
   `backend-rs/migrations/*.sql` (checksum break, `tests/README.md:17`)** →
   guard red → done (DB is discarded); add the MariaDB-leg allowlist line and
   verify on **both** MySQL 8.4 and MariaDB 11.4 legs.
5. **B3 k6 scaffolding:** seed script (login-API sessions, guarded), scenario
   skeleton, compose override, README; smoke-run locally against the compose
   stack via loopback (never prod — guards below); hand off to T026.
6. **Docs:** `docs/perf-budgets.md` skeleton first (fill as each gate lands),
   `docs/perf-runbook.md`, lessons.
7. **Gates:** frontend `npm run test`, `lint`, `tsc -b`, `build` + `npm run
   budget` + dist grep, full `npm run test:e2e` (perf specs join the default
   run; CI perf step green under `--workers=1 --retries=0`); backend
   `cargo fmt --check`, `clippy -D warnings`, `cargo test` on both DB legs,
   `cargo deny`, `sqlx prepare --check`. **Record the docker-e2e job's
   before/after duration delta in §Notes** — a perf harness that doubles CI
   time is itself a regression.
8. **Hand-offs:** T026 §Notes (scaffolding ready, ownership split, depends
   edge), T024 §Notes (metrics smoke), `20-performance.md` amendment
   (perf-smoke transfer), status.yaml depends edge.

## Constraints

- **No commit without approval** (Global DoD); TDD + red-proof every gate
  (a gate that has never gone red is not a gate — ci-quality lesson).
- **No runs against production data/DB** — compose/throwaway schemas only —
  enforced **mechanically, not just prose** (this repo's own convention:
  Playwright forces the `kanpad_e2e` DB name; `#[sqlx::test]` mints ephemeral
  DBs):
  - `seed.mjs`: explicit env vars only — **no `dotenv/config` autoload** (the
    repo-root `.env` holds live prod secrets: `SESSION_SECRET`, `DB_PASSWORD`,
    prod origins); assert target DB name/host matches a throwaway pattern and
    **refuse** anything else.
  - k6 README: default `BASE_URL=http://localhost:5173` (the only published
    compose port), **refuse non-loopback hosts without an explicit
    `K6_ALLOW_REMOTE=1`**; warn never to paste origins from
    `docs/deployment.md` (they are the real prod URLs).
  - Plan-guard red-proofs (index drop, `ANALYZE`) run only inside ephemeral
    per-test DBs — never on a shared/persistent schema (migration-safety rule).
- F1/F2/F3 run in the **default e2e suite** (user decision); in CI they
  execute as the dedicated `@perf` step with `--workers=1 --retries=0`
  (contention + retry-masking, see Enforcement philosophy); budgets stay
  generous enough not to flake; thresholds scale via `PERF_BUDGET_SCALE`.
- Profiler is dev-gated (`import.meta.env.DEV`) — ~zero prod-bundle impact
  (DCE is likely but unproven for small residues: **prove absence with the
  dist grep**, re-verify `npm run budget` after wrapping). Store never
  contains props/user data.
- Callgrind gate: valgrind is absent from ubuntu runner images — verify &
  install (this workflow's own history documents an apt-get cascade failure,
  `backend-rs.yml:320-324` — prefer the cached/known-good path); assert
  `valgrind --version` against the recorded value; toolchain/valgrind bumps
  trigger the documented re-baseline (counts are only comparable on identical
  toolchain + valgrind + arch).
- Plan guards: **shape only** (index used / no row with `type: ALL`), never
  cost or wall-clock; read EXPLAIN columns by name via runtime `sqlx::query`;
  portable tabular EXPLAIN across MySQL 8.4 + MariaDB 11.4 (no JSON diffs, no
  `partitions` column); realistic row counts + `ANALYZE` (tiny fixtures
  produce lying plans).
- k6 / B3: throttle raise via **`compose.load.yaml` override only** — never
  edit `compose.yaml` (it is the production appliance: `APP_ENV: production`,
  no `THROTTLE_*` passthrough) or the repo-root `.env`; compose runs
  production → session cookies are `Secure` and a Go cookiejar drops them over
  plain HTTP — **send `Cookie` explicitly as a request header** (README
  documents this); 100+ VU × Argon2 is CPU-bound by design → README carries
  an expected-footprint warning (AGENTS.md: don't crash the machine);
  Argon2 login cost measured, never "fixed" (HASH_COST floor is a security
  rule).
- `/metrics` smoke: scrape in-process (as `tests/api/metrics.rs` does) or
  loopback — never add `METRICS_PORT` to compose (the endpoint is
  unauthenticated by design and binds `0.0.0.0`; port restriction is the
  control).
- Coordinate, don't collide: T024 (`/metrics`), T026 (k6 + board-smoke
  ownership + depends edge), T029 (**workflow file is "NO CHANGES" locked**
  — announce the two workflow edits; T029's conditional index migration can
  flip B2 plan shapes → note both directions), T030 (Sentry; RUM is an open
  call, not theirs), **T022 (re-baseline F2/F1 after pagination rework),
  T044 (co-edits the two Profiler wrap files)**.
- Benchmarks are measurement, not optimization — fixes belong to their own
  tasks.

## Acceptance Criteria

- [ ] F1: `perf-drag.spec.ts` in default suite; red-proven both ways (budget
      red; dragOver-coalescing degradation red); baseline distribution
      recorded in spec header; **paced input verified (non-idle frame deltas
      observed)**; warm-up+median budgets green locally **and** in the CI
      `@perf` step (`--workers=1 --retries=0`).
- [ ] F2: `navToNetworkIdleMs` asserted (not just logged) with recorded
      baseline (pin one figure/date), labeled in the registry as a dev-mode
      tripwire — **not** claimed as NFR-enforcing.
- [ ] F3: heap-growth guard green with recorded delta budget; GC forced
      before each sample; fixed board set documented.
- [ ] F4: vitest render-**count** budgets red→green, run in default
      `npm run test`; durations printed, not asserted; React-Compiler
      divergence noted in the spec header.
- [ ] B1: criterion benches run manually (`cargo bench` split commands
      documented in the runbook); **gungraun ≥0.20 gate green in
      `backend-rs.yml` with committed baselines + version assertions**;
      red-proven (artificial slowdown flips it); `cargo deny` +
      `llvm-cov --all-targets` still green with the new dev-deps.
- [ ] B2: plan guards green on **both** CI DB legs (MariaDB allowlist line
      added — file row above); red-proven by dropping an index **inside the
      ephemeral test DB only**; no cost/timing assertions anywhere; parsed by
      column name via runtime `sqlx::query`.
- [ ] B3: `scripts/load/` committed with README **including the safety
      guards** (no dotenv autoload, throwaway-DB assert, loopback default +
      `K6_ALLOW_REMOTE` refusal, explicit `Cookie` header, compose-override
      throttle raise, resource warning); local smoke run pasted in §Notes;
      T026 §Notes records hand-off + ownership split.
- [ ] `/metrics` smoke: in-process assertion added, coordinated with T024 —
      **or**, if T024 is still pending, explicitly recorded as skipped in
      §Notes (never silently dropped).
- [ ] `docs/perf-budgets.md` lists every budget with value, tier, enforcing
      test, last-measured date, **NFR mapping / tripwire label**;
      `docs/perf-runbook.md` covers every tool incl. gungraun re-baseline;
      threshold changes require a same-PR `last-measured` update.
- [ ] `npm run budget` still green after profiler wraps **and dist grep for
      `__PERF__` in `dist/` is empty**; full frontend + backend gates green;
      **docker-e2e duration delta recorded in §Notes**; lessons appended
      (dated, tagged) to `docs/lessons/frontend.md` etc.

## Notes

- **2026-11-20 amendment (pre-implementation review, 4-angle):** STATUS
  reconciled with `status.yaml`; F2 redefined (`navToNetworkIdleMs`,
  dev-mode tripwire — the old "client half = 500 ms" figure appears nowhere
  in the specs and was red on day one vs known 983 ms–1.2 s baselines); B1
  tool pinned to **gungraun ≥0.20** (iai-callgrind unmaintained; test mode
  protects `llvm-cov --all-targets`) and `common/rebalance.rs` dropped as a
  target (async/DB-bound — bench `order_key::keys_between` instead), baseline
  artifact added; B2 placement corrected to `tests/api/query_plans.rs` (dead
  path otherwise) + MariaDB allowlist edit surfaced; input-latency formula,
  paced input, warm-up run, CI `--workers=1 --retries=0` step and
  `PERF_BUDGET_SCALE` env var pinned; F4 durations demoted to print-only;
  F3 GC control added; mechanical prod-guards added to seed/k6/throttle
  (compose override; `compose.yaml` is a prod appliance); spec-reference
  misattributions fixed (perf smoke + capacity smoke belong to T026 per
  `20-performance.md:224,320`; T022 keyset claim dropped; RUM/T030 claim
  corrected to an open call; ±0.13% citation corrected to hotpath.rs);
  coordination extended with T022/T044 + T029's workflow lock; lessons file
  corrected to `docs/lessons/frontend.md`.
- (implementation notes appended here during/after implementation — incl. the
  docker-e2e duration delta, CI threshold values, and quarantine decisions)

## Lessons

(pending)
