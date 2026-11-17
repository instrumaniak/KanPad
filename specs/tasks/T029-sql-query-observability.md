# T029: SQL query observability

> **STATUS: pending.** Wave 1 (Phase 1). **Depends on T024** (metrics registry, fixed
> `BUCKETS`, pool-acquire histogram pattern; registry now records the edge) —
> coordinate, do not collide. Compose substrate assumed from **T012**; measurement
> methodology borrows from **T026**.
>
> **Post-review (2026-11-16):** corrected dialect facts (MariaDB `ANALYZE`, bundled
> `sys`), fixed `RUST_LOG` expectations, dropped the CI MariaDB `--performance_schema`
> route (verification is **local-only**; see §Decisions), fixed the `Arc<Metrics>` seam,
> pinned `sqlx 0.9.0`, pinned runbook to `kanpad_ops`, noted the `render()` rebase
> hotspot with T024. See §Notes.

## Goal

Replace "we have no window into query behavior" with a three-layer setup that surfaces
slow and pathological queries, and an optimization loop that acts on them:

1. **Application-side visibility** — sqlx slow-statement logging made explicit and
   always on (today the default `EnvFilter` `backend_rs=info,tower_http=info` never
   enables the `sqlx::query` target, so even sqlx's ≥1 s slow-statement warning is
   silent), plus per-query Prometheus metrics (`kanpad_db_queries_total`,
   `kanpad_db_query_duration_seconds`, `kanpad_db_slow_queries_total`) via a tracing
   Layer, no new dependencies.
2. **DB-side ground truth** — `performance_schema.events_statements_summary_by_digest`
   and the slow query log, portable across **MySQL 8.4** and **MariaDB 11.4**, wired
   into compose (dev + local test DBs) with portable digest SQL and before/after
   snapshot tooling. **Verification is local-only** (docker-compose.test.yml legs);
   CI is explicitly out of scope — GitHub Actions service containers accept no
   command args (see §Decisions).
3. **Optimization loop** — seeded dev DB at realistic scale, digest-driven ranking,
   dialect-appropriate `EXPLAIN`, fixes (index migration and/or N+1 rewrites),
   measured delta, lesson recorded.

Out of scope: OpenTelemetry per-query spans (**not available** in sqlx 0.9
[events only]; repo-op spans are wired in **T030** via `#[tracing::instrument]`;
`sqlx-tracing` rejected — its `0.2.1` pins `sqlx ^0.8`, a second major in the tree),
`log_queries_not_using_indexes` (too noisy), production DB changes (operator decision;
documented in the runbook only).

## Why now / research summary

sqlx **0.9.0** defaults (`sqlx-core-0.9.0/src/connection.rs` `LogSettings::default`):
statements log at `debug`, statements ≥ **1 s** log at `warn` with structured fields
`elapsed_secs`, `elapsed`, `slow_threshold`, `rows_affected`,
`rows_returned` and message `slow statement: execution time exceeded alert threshold`
(`sqlx-core-0.9.0/src/logger.rs`). The slow path emits at target `sqlx::query`.
**PII nuance**: *bound parameters* are never logged, but **inline literals written
directly into SQL text do appear** in the warn line — do not inline secrets/PII; the
summary itself is only the first 4 words of the statement.

MariaDB 11.4 vs MySQL 8.4 (both must stay supported):

- MariaDB ships **`performance_schema = OFF`** by default (needs restart);
  MySQL 8.4 defaults ON. MariaDB's digest table **lacks `QUANTILE_95/99/999` and
  `QUERY_SAMPLE_TEXT`** (MySQL 8.0+ only). All monitoring SQL must use only portable
  columns: `count_star`, `sum/avg_timer_wait`, `sum_rows_examined/sent`,
  `sum_created_tmp_disk_tables`, `schema_name`.
- **MariaDB ≥ 10.6 bundles the `sys` schema** (needs `performance_schema=ON`; partial
  port vs MySQL 8's) — so portable tooling stays **raw performance_schema**; `sys`
  views are docs-only extras. (Was incorrectly "no sys schema" — corrected
  post-review.)
- MariaDB variable aliases (`long_query_time`, `slow_query_log`,
  `slow_query_log_file`, `min_examined_row_limit`) still work, so compose/runbook can
  use the portable names; `log_slow_extra` is MySQL-only (MariaDB:
  `log_slow_verbosity`, e.g. `query_plan`).
- "`EXPLAIN ANALYZE`" is **MySQL 8.4 syntax; MariaDB rejects it as outdated** — use
  MariaDB's **`ANALYZE <statement>`** (`ANALYZE FORMAT=JSON` for detail).
  `EXPLAIN ANALYZE … FORMAT=TREE` is MySQL-only (8.0.16+). Restrict both to SELECT
  statements (`EXPLAIN ANALYZE` executes the statement).
- digest tables are bounded by `performance_schema_digests_size`; when full, new
  digest stats aggregate in the catch-all row (`DIGEST = NULL`) and (MySQL) a
  `Performance_schema_digest_lost` counter increments — the tooling below reports
  `digest lost` when non-zero; snapshot diffs are untrusted until it's addressed.
- Slow-log and performance_schema settings via `SET GLOBAL` are not persisted (MySQL
  8.0+ also has `SET PERSIST`); compose `command:` flags make dev/test deterministic;
  prod stays an operator decision.

## Depends On

T024 (metrics registry + BUCKETS; acquire-time histogram pattern to mirror; registry
updated by `taskpad edit`). Practically T012 (compose substrate). T026 (methodology) —
informational.

## Phase

1 — performance & resilience wave.

## Critical

No.

## Decisions / shared ownership

- **CI MariaDB stays out of scope** (option b, 2026-11-16): GitHub Actions service
  containers take no `command:`; a bind-mounted `my.cnf` was rejected for complexity.
  Digest verification runs on the **local** `docker-compose.test.yml` legs; CI is
  unchanged.
- **Ownership of shared files** (T029 lands first; T030 merges after):
  this task authors the `init_tracing()` builder chain in `lib.rs` (registers
  `SqlxSlowQueryLayer`, adds default `sqlx::query=warn`) and the new render block in
  `metrics.rs`; T030 appends its OpenTelemetry layer **after** this Layer in the same
  builder chain, keeps the `sqlx::query=warn` leg, and does not touch `metrics.rs`.
  Both keep `init_tracing` swallow-reinit semantics for tests. `render()` is a
  known rebase-merge hotspot vs T024 (both append blocks) — see §Notes.

## Spec References

- `backend-rs/src/config.rs` — `connect_options()` builds `MySqlConnectOptions`
  without `log_slow_statements()` today (verified :587-601); new env
  `DB_SLOW_STATEMENT_MS` (default 250 ms, validated > 0).
- `backend-rs/src/lib.rs` — `init_tracing()` default `EnvFilter` exactly
  `"backend_rs=info,tower_http=info"` (:515); add `sqlx::query=warn`. `log_json`
  (:507) for prod JSON.
- `backend-rs/src/metrics.rs` — per-AppState `Metrics` (`state.rs:39/94`), fixed
  `BUCKETS` (`pub const BUCKETS: [f64; 12]`), `render()` (:180), `observe_request`
  pattern (:120), `record_acquire` (:147); T60 opt-in `metrics_router` (fail closed).
- `backend-rs/src/db.rs` — `acquire_time_level(log::LevelFilter::Info)` already set
  (:58); keep.
- `backend-rs/src/features/cards/routes.rs:612-719` — `SpanFieldCapture` test pattern
  for a tracing Layer unit test (span/event capture) — was "620-700".
- `compose.yaml` — dev `mysql` service has NO `command:` today; volume `mysql-data` at
  `/var/lib/mysql`; `rust` service passthrough for `METRICS_PORT` added (commented).
- `backend-rs/docker-compose.test.yml` — mysql leg has `command:` with
  `--default-authentication-plugin=mysql_native_password --max-connections=1000` (append,
  don't replace); mariadb leg has no `command:` today (add one).
- `backend-rs/tests/README.md` — integration test harness rules.
- Project lessons home: `docs/lessons/rust-backend.md`; runbook target:
  `docs/query-performance.md` (NEW).
- External: MySQL 8.4 Reference Manual §7.4.5 (Slow Query Log), §29 (Performance
  Schema) — `events_statements_summary_by_digest`; MariaDB docs "Performance Schema
  Digests" and "Incompatibilities … MariaDB 11.4 and MySQL 8.0". Pins: **sqlx 0.9.0**.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/config.rs` | NEW env `DB_SLOW_STATEMENT_MS` (default 250 ms; parse u64 millis; `>0`; reject garbage; Rustdoc + unit tests); `.log_slow_statements(log::LevelFilter::Warn, Duration::from_millis(threshold))` in `connect_options()` |
| `backend-rs/.env.example` | `DB_SLOW_STATEMENT_MS` documented (default 250 ms, `>0`) |
| `backend-rs/src/lib.rs` | default `EnvFilter` becomes `backend_rs=info,tower_http=info,sqlx::query=warn`; `init_tracing` gains layer registration — **`Arc<Metrics>` must be pre-built at boot** and passed to `init_tracing` (layer) *and* to the `AppState` build (clone) — today `init_tracing` takes no state and `AppState` builds its own `Arc<Metrics>` internally; change signatures accordingly (pre-built clone handed to both; tests construct independently as today) |
| `backend-rs/src/metrics.rs` | NEW series: `kanpad_db_queries_total`, `kanpad_db_query_duration_seconds` (histogram reusing `BUCKETS`), `kanpad_db_slow_queries_total`; render + unit tests |
| `backend-rs/src/common/db_metrics_layer.rs` | NEW — `SqlxSlowQueryLayer`: tracing Layer matching events with `target == "sqlx::query"`; increments counters/observes `elapsed_secs`; slow events identified by presence of the `slow_threshold` field; **no SQL-text/param labels** (cardinality + leak). Note: `sqlx 0.9.0` emits events, never spans |
| `compose.yaml` | mysql `command:` — `--slow_query_log=1 --long_query_time=0.5 --min_examined_row_limit=100 --log_slow_extra=ON` (file under `/var/lib/mysql`, persisted by volume); `rust` service: commented-out `METRICS_PORT=${METRICS_PORT:-}` passthrough suggestion (dev convenience, default fail-closed) |
| `backend-rs/docker-compose.test.yml` | mysql leg: **append** `--slow_query_log=1 --long_query_time=0.5 --min_examined_row_limit=100 --log_slow_extra=ON` (keep existing plugin/max-connections flags); mariadb leg: `command: --performance_schema=ON --slow_query_log=1 --long_query_time=0.5 --min_examined_row_limit=100 --log_slow_verbosity=query_plan`; **never copy the mysql command into the mariadb leg** (`log_slow_extra` is unknown there → container boot failure); boot-verification step (run one forced slow query on both legs) |
| `.github/workflows/backend-rs.yml` | **NO CHANGES** (locked down, see §Decisions) |
| `scripts/db/digest-top.sql` | NEW — top-20 by `sum_timer_wait`, rows_examined vs rows_sent, tmp-disk tables; **portable columns only**; includes `digest lost` diagnostic row (`Performance_schema_digest_lost` equivalent — `events_statements_digest_lost`-class counter; runbook: if non-zero, deltas untrusted until `performance_schema_digests_size` raised / restart) |
| `scripts/db/digest-snapshot.sql` | NEW — `ops` schema selected as `kanpad_ops` (**outside the seeded DB** — seed must never touch it; verified once on dev compose): create `kanpad_ops.digest_snapshots` + re-seed-safe diff query (per-digest `post − pre`); runbook states the `TRUNCATE performance_schema.events_statements_summary_by_digest` reset option (allowed on both) as the preferred pre-snapshot step |
| `scripts/seed/dev-seed.sql` | NEW — pure SQL, reproducible: ~50 boards × ~2000 cards × ~500 notes with fractional `sort_key`s; preserves `kanpad_ops`; documented run command; dev compose only |
| `docs/query-performance.md` | NEW runbook: enable/read slow log (`SET GLOBAL` — no restart; MySQL 8.0.4 supports `SET PERSIST`), digests (`P_S` ON — no restart MySQL, restart MariaDB), snapshot workflow (`TRUNCATE` method vs `post−pre`; cumulative-counter semantics), `EXPLAIN ANALYZE` vs MariaDB `ANALYZE <stmt>` (SELECT-only), `pt-query-digest` (3.7.x, both dialects), app warn-line ↔ `/metrics` correlation via request-id, 250 ms app warn vs 0.5 s slow-log threshold (deliberate), inline-literal caveat, **MySQL-vs-MariaDB extras** (`log_slow_extra`, `log_slow_verbosity`, quantiles with P_S ON, `sys` port notes), prod = operator decision |
| `backend-rs/migrations/0004_*.sql` | **Conditional** — only if Phase 3 finds missing/composite indexes; CI describe gate; both dialects |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson appended on completion |

**Must NOT appear in the diff:** parameter values in logs/metrics (bound params never
logged — but **inline literals do appear**; do not inline secrets/PII), `log_queries_not_using_indexes=ON` anywhere, prod env changes, hand-edited `_sqlx_migrations`, query-text metric labels, `sys`-schema-only columns in shared scripts.

## Implementation Steps

**Phase 1 — app-side (TDD; mirror existing unit-test patterns)**

1. `Settings`: env parsing + validation for `DB_SLOW_STATEMENT_MS` (u64 millis, default
   250, reject 0/NaN/garbage); unit tests.
2. `connect_options()`: apply `.log_slow_statements(Warn, Duration::from_millis(ms))`;
   boot log line unchanged. Threshold not publicly readable (`log_settings` is
   `pub(crate)` in `sqlx-mysql-0.9.0`) — do **not** assert from the options object;
   primary proof is the forced slow query in step 6; optional `format!("{options:?}")
   .contains("slow_statements_duration")` Debug-smoke clearly marked brittle.
3. `lib.rs`: default filter becomes `backend_rs=info,tower_http=info,sqlx::query=warn`;
   `init_tracing` re-structured to accept/pre-build `Arc<Metrics>` (see Files row);
   keep `try_init` swallow-reinit semantics.
4. `Metrics`: three new series + render + unit tests. (`render()` rebase hotspot with
   T024 — coordinate merge order; see §Notes.)
5. `SqlxSlowQueryLayer`: unit tests from a synthetic `sqlx::query` event (slow and
   non-slow), asserting counter increments and histogram observation; re-init safety.
6. Integration check: run dev compose; force a slow query (e.g. `SELECT SLEEP(0.3)`
   with threshold 250 ms or a failing-adjacent large join); assert the warn line and
   `kanpad_db_slow_queries_total ≥ 1` on `/metrics` scrape (series present **and
   non-zero**).

**Phase 2 — DB-side substrate (local-only)**

7. Compose edits (dev mysql; **both** test legs with the flag sets above — no CI).
   Boot verification: `docker compose -f docker-compose.test.yml up -d --wait`; run one
   forced slow query on **both** legs; confirm the slow log line appears (mysql log
   path / container log for mariadb); tear down.
8. `scripts/db/digest-top.sql`, `digest-snapshot.sql` — execute cleanly on **both**
   MySQL 8.4 and MariaDB 11.4 clients; snapshot schema `kanpad_ops` deliberately
   outside the seed; include digest-lost diagnostic.
9. `docs/query-performance.md` runbook (portable-first; MySQL-only extras flagged;
   methods documented: `TRUNCATE performance_schema.events_statements_summary_by_digest`
   vs compute post−pre; zero interleaving traffic).

**Phase 3 — measure → fix → measure**

10. Seed dev compose (`~50/2000/500`); `kanpad_ops` untouched; run command in runbook.
11. Baseline: **`pre` means pre-fix, with instrumentation in place** (Phases 1–2 are
    behavior-neutral). `TRUNCATE` digest tables before the pre-snapshot (preferred) or
    compute `post − pre` per digest; snapshot into `kanpad_ops.digest_snapshots`;
    no interleaved traffic; exercise: board load, card move, notes list, search.
12. Digest ranking → top offenders → `EXPLAIN ANALYZE` (MySQL 8.4) / `ANALYZE <stmt>`
    (MariaDB 11.4) each, **SELECT only**; classify:
    missing index → new migration `0004_*`; N+1 in `src/features/*/…*repo*.rs` → batch
    (`WHERE … IN (…)`) or join; pathological order/filter (fractional `sort_key`
    range scans likely first suspect — verify, don't assume).
13. Apply fixes **with tests**; re-seed (baseline rows survive); re-run;
    `post-<change>` snapshot; diff per digest; record before/after table in runbook.
14. Lesson appended to `docs/lessons/rust-backend.md`.

## Constraints

- Parameter **values** never logged or labeled — bound params never appear; **inline
  literals DO** (keep statements parameterized) — summary/duration only either way.
- Slow log stays **rate-limited by `min_examined_row_limit=100`**;
  `log_queries_not_using_indexes` stays OFF everywhere (noise); MariaDB
  `log_slow_rate_limit` left at default.
- No new crates in Phase 1 (Layer over existing tracing stack). Any new dependency
  (e.g. `hotpath` for dev-only digest reporting) is **dev-only, opt-in, justified in
  §Notes** before adding — default answer is no.
- `sys` views only in clearly-marked optional doc sections (both dialects, MariaDB
  needs P_S ON; partial port); shared scripts use portable digest columns.
- Slow-log settings in compose/tests are dev/CI determinism, not prod policy; prod
  thresholds are operator-tuned (documented).
- App adds no hot-path latency beyond the two atomic counter bumps + one histogram
  observation.
- **No commit without asking**; TDD; verify on both dialects; follow
  `backend-rs/tests/README.md`.

## Acceptance Criteria

- [ ] With defaults, a query ≥ `DB_SLOW_STATEMENT_MS` emits exactly one `sqlx::query`
      warn event containing `elapsed_secs` and `slow_threshold`; prod JSON logs carry it.
- [ ] `RUST_LOG=backend_rs=info,sqlx::query=debug` enables full statement logging with
      no code changes; when `RUST_LOG` is set it **replaces** the built-in default —
      documented in the runbook (full logging impossible with the plain
      `backend_rs=debug` pair).
- [ ] `/metrics` exposes `kanpad_db_queries_total`, `kanpad_db_query_duration_seconds_*`,
      `kanpad_db_slow_queries_total`; after ≥1 authenticated requests the query counter
      is ≥ 1 (not just HELP/TYPE lines); scrape requires `METRICS_PORT` opt-in.
- [ ] `compose.yaml` dev mysql logs slow queries with `long_query_time=0.5`; **both**
      `docker-compose.test.yml` legs run with slow log enabled; MariaDB test leg has
      `performance_schema=ON` (verified by boot + one forced slow query); CI workflow
      unchanged (local-only).
- [ ] `scripts/db/digest-top.sql` + `digest-snapshot.sql` execute cleanly on **both**
      MySQL 8.4 and MariaDB 11.4 and contain no MySQL-only columns; digest-lost
      diagnostic included.
- [ ] Snapshot survives a full re-seed (rows in `kanpad_ops`); demonstrated once;
      diff produces the before/after table.
- [ ] `docs/query-performance.md` covers: enabling/reading slow log & digests on both
      dialects, snapshot workflow (`TRUNCATE` vs post−pre), `EXPLAIN ANALYZE` vs
      `ANALYZE <stmt>`, `pt-query-digest`, request-id correlation, prod-vs-dev
      differences, inline-literal caveat, slower 0.5 s slow-log vs 250 ms app-warn
      rationale; runbook commands copy-pasteable and executed at least once; one full
      transcript pasted into §Notes.
- [ ] `backend-rs/.env.example` documents `DB_SLOW_STATEMENT_MS`.
- [ ] Seeded dev DB reproducible from the committed seed (documented command).
- [ ] Measured delta recorded: before/after digest snapshots for the fixes applied
      (or an explicit "no actionable offenders at target scale" finding with the
      ranking tables attached).
- [ ] Any new migration applies + describes clean on MySQL 8.4 and MariaDB 11.4 (CI
      describe gate + local legs).
- [ ] Lesson appended; no commit without approval.

## Notes

Post-review (2026-11-16, author: spec review ×3 — codebase/spec/external):
- Fact fixes: MariaDB uses `ANALYZE <stmt>` (`EXPLAIN ANALYZE` rejected); MariaDB ≥
  10.6 bundles `sys` (partial, needs P_S ON) — raw P_S stays the portability rule;
  `RUST_LOG=backend_rs=debug` never enables `sqlx::query` (filter default = ERROR);
  `log_settings` not publicly readable (`pub(crate)`) — assertion switched to
  integration-primary; digest bound by `performance_schema_digests_size` (catch-all
  row) — diagnostic added.
- Structure: MariaDB CI verification moved locally (option b, §Decisions);
  `kanpad_ops` splits snapshots from the seed; snapshot diff spells out `TRUNCATE`
  vs `post − pre`; `.env.example` row added; `Arc<Metrics>` seam specified (pre-built).
- Coordination: T029 edits `init_tracing`'s builder chain first; T030 appends its OTel
  layer after in the same `try_init` chain (keeps `sqlx::query=warn`); `metrics.rs`
  `render()` is the known T024 rebase hotspot — merge T029 → T030 order assumed; both
  keep swallow-reinit semantics for tests.
- Proposed (user decision pending): split Phase 3 into a follow-up task
  (data-dependent outcome; "no actionable offenders" is shippable) — surface to caller
  before execution.
- Pin: **sqlx 0.9.0** (verified `sqlx-core-0.9.0` sources for the LogSettings claim).

## Lessons

(pending)
