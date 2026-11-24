# T052: Backend: multi-db support

## Goal

Add first-class backend support for **SQLite, PostgreSQL, MySQL, and MariaDB** (3 sqlx drivers / 4 supported targets) so self-hosters can choose a DB via `DATABASE_URL`, while preserving existing API behavior and MySQL/MariaDB compatibility.

## Depends On

- None (coordinate with any in-flight backend DB/store work; this is cross-cutting).

## Phase

- Phase 1: portability foundation (no behavior change for MySQL/MariaDB)
- Phase 2: SQLite support
- Phase 3: PostgreSQL support
- Phase 4: full parity, CI matrix, docs/hardening

## Critical

- Yes. Cross-cutting DB, migration, session, auth, tx/retry, and test-harness behavior; data safety and semantic parity are release-critical.

## Spec References

- `AGENTS.md` (DB migration safety, MySQL/MariaDB compatibility, backend e2e DB verification, Rust test harness, git/commit rules)
- `BACKEND-RS-ORCHESTRATION.md`
- `CONTRACT-CHANGES.md` (update only if wire contract changes; target is no contract change)
- `specs/api-rewrite-rs-specs/30-db-schema.md`
- `specs/api-rewrite-rs-specs/31-live-unicode-migration.md`
- `specs/api-rewrite-rs-specs/45-ci-gates-justfile.md`
- `backend-rs/tests/README.md`
- `docs/data-migration-live.md`
- Research refs: [sqlx migrate macro](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html), [sqlx Migrator](https://docs.rs/sqlx/latest/sqlx/migrate/struct.Migrator.html), [sqlx migrate config](https://docs.rs/sqlx-core/latest/sqlx_core/config/migrate/struct.Config.html), [sqlx-cli README](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md)

## Files to Create/Modify

### Create (expected)

- `backend-rs/migrations/mysql/0001_baseline.sql` (moved, byte-identical)
- `backend-rs/migrations/mysql/0002_convert_to_utf8mb4.sql` (moved, byte-identical)
- `backend-rs/migrations/mysql/0003_sort_key_fractional_ordering.sql` (moved, byte-identical)
- `backend-rs/migrations/postgres/0001_baseline.sql`
- `backend-rs/migrations/postgres/0002_convert_to_utf8mb4.sql` (or equivalent no-op/compat migration; preserve version intent)
- `backend-rs/migrations/postgres/0003_sort_key_fractional_ordering.sql`
- `backend-rs/migrations/sqlite/0001_baseline.sql`
- `backend-rs/migrations/sqlite/0002_convert_to_utf8mb4.sql` (or equivalent no-op/compat migration; preserve version intent)
- `backend-rs/migrations/sqlite/0003_sort_key_fractional_ordering.sql`
- `backend-rs/src/db/mod.rs` (replace/refactor current `db.rs` into module)
- `backend-rs/src/db/mysql.rs`
- `backend-rs/src/db/postgres.rs`
- `backend-rs/src/db/sqlite.rs`
- `backend-rs/src/db/dialect.rs` (small trusted SQL fragment helpers)
- `backend-rs/src/common/session_store/mod.rs`
- `backend-rs/src/common/session_store/mysql.rs`
- `backend-rs/src/common/session_store/postgres.rs`
- `backend-rs/src/common/session_store/sqlite.rs`
- Backend test compose/CI wiring for Postgres tmpfs (and SQLite job config as needed)

### Modify (expected)

- `backend-rs/Cargo.toml` - driver features (`db-mysql`, `db-postgres`, `db-sqlite`), sqlx `postgres`/`sqlite` features, feature matrix/build defaults.
- `backend-rs/src/config.rs` - backend-aware `DATABASE_URL` parsing/validation; MySQL-only charset/collation rules; SQLite path/pragmas; Postgres options.
- `backend-rs/src/state.rs` - runtime-selected `DbPool` enum among enabled drivers; backend-aware session store construction; probe/metrics integration.
- `backend-rs/src/main.rs` - backend-aware startup probe, migrate run/info/revert, auto-migrate, create-admin DB checks.
- `backend-rs/src/lib.rs` - app wiring unchanged at API level; DB-state/metrics/probe plumbing backend-aware.
- `backend-rs/src/error.rs` - backend-neutral DB error mapping; preserve problem+json contract.
- `backend-rs/src/common/retry.rs`, `backend-rs/src/common/tx_retry.rs` - map MySQL 1213/1205/1062, Postgres 40001/40P01/23505, SQLite BUSY/LOCKED/constraint to existing retry/conflict/503 policy.
- `backend-rs/src/common/rebalance.rs`, `backend-rs/src/common/label_seed.rs`, `backend-rs/src/common/ownership.rs`, `backend-rs/src/common/card_summary.rs` - remove MySQL-only SQL assumptions where shared.
- `backend-rs/src/features/*/store.rs`, `backend-rs/src/admin.rs` - port MySQL-specific idioms (`LAST_INSERT_ID`, `INSERT IGNORE`, `ON DUPLICATE KEY`, `UTC_TIMESTAMP(6)`, `FOR UPDATE` semantics, backticks/types/casts) via backend-neutral or backend-specific implementations.
- `backend-rs/tests/README.md`, `backend-rs/tests/api/helpers.rs`, `backend-rs/tests/api/*` - backend matrix harness, same migration workflow, DB-state assertions across supported backends.
- `backend-rs/docker-compose.test.yml` and/or new compose files - MySQL/MariaDB remain; add Postgres tmpfs service.
- `.env.example`, `AGENTS.md`, deployment/docs/runbooks - 4-target self-host support and migration matrix policy.

## Implementation Steps

1. **Lock decisions & guardrails**
   - Confirm supported targets: SQLite, PostgreSQL, MySQL 8.4, MariaDB 11.4 (MariaDB via mysql driver, separately tested).
   - Architecture: **runtime-selectable concrete drivers** (`DbPool` enum over enabled drivers), with separate migration dirs, small dialect helper, backend-specific db/session/error modules, and runtime queries for shared code.
   - Do **not** use `Any` as primary abstraction. Do **not** make driver features mutually exclusive unless also providing a runtime-selectable multi-driver build path; self-host UX requires `DATABASE_URL` scheme selection.
   - Migration workflow must be identical in dev, testing, and prod: same backend migration source, same commands/runner semantics, same checksum/immutability rules, same fresh-DB bootstrap path. Env differences limited to connection/config only.

2. **Phase 1 - portability foundation (MySQL/MariaDB no-regression)**
   - Add Cargo features and sqlx `mysql/postgres/sqlite` dependencies; define build matrix and default feature policy.
   - Refactor `db.rs` into `db/` module: `DbPool` enum, connect options, probe, migrate, diagnostics, error classification hooks.
   - Move existing migrations to `backend-rs/migrations/mysql/` **byte-identically**; update migrator to backend-specific dir. Add guard test/script to assert byte identity/checksums for moved MySQL files.
   - Add `.gitattributes` rule for SQL migrations LF endings (`*.sql text eol=lf`) to prevent checksum drift.
   - Make config backend-aware: MySQL keeps `charset=utf8mb4` and `collation=utf8mb4_unicode_ci` requirements; SQLite/Postgres get appropriate validation.
   - Keep MySQL/MariaDB CI green before enabling new backends.

3. **Phase 1b - shared query/macro strategy**
   - Inventory `query!`/`query_as!`/`query_scalar!` and MySQL-specific SQL idioms.
   - For shared paths, prefer runtime `sqlx::query/query_as/query_scalar` + explicit row mapping or `QueryBuilder` binds. Keep SQL parameterized; no user-input interpolation.
   - Allow backend-specific modules to use driver-specific queries where clarity/safety is better than forced abstraction.

4. **Phase 2 - SQLite vertical slice**
   - Implement `db/sqlite.rs`: pool options, `foreign_keys=ON`, busy timeout, WAL/journal policy for file DBs, in-memory test behavior.
   - Create SQLite migration set with logical schema parity (tables, PK/FK/unique/check/index/default/nullability semantics). Handle 0003 `ELT/DIV`/MySQL DDL with SQLite-compatible SQL.
   - Split session store: shared MessagePack record + expiry semantics; SQLite SQL for upsert (`ON CONFLICT(id) DO UPDATE`), timestamp/expiry queries, id/data/expiry column mapping.
   - Map SQLite busy/locked/constraint errors into existing retry/conflict/503 behavior; no fake row-level `FOR UPDATE`.
   - Add SQLite smoke e2e and DB-state verification tests.

5. **Phase 3 - PostgreSQL support**
   - Implement `db/postgres.rs`: pool options, connect/probe/migrate, Postgres error classification.
   - Create Postgres migration set with logical schema parity (`TIMESTAMPTZ`/`BOOLEAN`/`TEXT`/`BYTEA`/identity or serial as appropriate; explicit constraints/indexes/FKs).
   - Port dialect hotspots: placeholders/RETURNING/last insert id strategy, upserts (`ON CONFLICT`), timestamp functions, boolean/type casts, lock/tx semantics.
   - Add Postgres tmpfs compose service and CI job.

6. **Phase 4 - full store port + parity hardening**
   - Port remaining feature stores/common helpers: auth/users, boards/columns/cards, checklists/labels, notes/tags/projects, ownership/rebalance/card_summary/admin.
   - Replace MySQL-only idioms: `INSERT IGNORE`, `ON DUPLICATE KEY UPDATE`, `LAST_INSERT_ID`, `UTC_TIMESTAMP(6)`, `ROW_NUMBER()`/dialect differences as needed, backtick quoting assumptions, MySQL-specific error code downcasts.
   - Ensure tx retry kernel is backend-aware but policy-consistent (same max attempts/backoff shape unless backend requires documented difference).
   - Add schema parity/invariant tests and core API smoke matrix across all 4 targets.

7. **CI, docs, and rollout gates**
   - CI matrix: MySQL 8.4 tmpfs, MariaDB 11.4 tmpfs, Postgres tmpfs, SQLite job. Keep required MySQL/MariaDB checks.
   - Run same migration workflow in CI as dev/prod: `migrate info/run` on fresh DB and upgrade path where applicable.
   - Update AGENTS, `.env.example`, deployment/self-host docs, and migration runbook for 4-target support and same-workflow rule.
   - Verify before completion: `cargo fmt`, `cargo clippy -D warnings` for relevant feature sets, backend test matrix, and DB-state assertions for data-modifying endpoints.

## Constraints

- **Same migration workflow in dev, testing, and prod.** Same backend migration source, commands/runner semantics, checksum immutability, and fresh-DB bootstrap. Env differences are connection/config only.
- **DB migration safety first.** Never edit applied migrations; never hand-edit `_sqlx_migrations`. Moving MySQL migrations must preserve bytes/checksums. Use forward-only fixes in production.
- **MySQL/MariaDB no-regression.** Existing MySQL 8.4/MariaDB 11.4 support and CI remain required.
- **Backend E2E DB-state verification.** For every data-modifying endpoint test, assert persisted state via a second API call or DB query.
- **API contract stability.** No wire contract changes unless explicitly documented in `CONTRACT-CHANGES.md`.
- **No user-input SQL interpolation.** Dialect helpers return trusted static fragments only; dynamic values are bound.
- **SQLite semantics explicit.** No fake row locking; use busy timeout/retry policy; document single-writer behavior and durability pragmas.
- **No primary `Any` abstraction.** Use concrete driver modules + thin runtime-selected enum.

## Acceptance Criteria

- [ ] `DATABASE_URL` schemes for supported/enabled backends (`sqlite`, `postgres`, `mysql`/`mariadb` as applicable) select the correct backend; unsupported or disabled schemes fail with clear boot config errors.
- [ ] Same migration workflow works in dev, testing, and prod: same backend migration dirs/files, same runner/commands, same checksum/immutability rules; fresh DB bootstrap and `migrate info` clean state behave consistently.
- [ ] Existing MySQL migrations moved to `migrations/mysql/` are byte-identical; MySQL/MariaDB upgrade path remains valid.
- [ ] Per-backend migrations create logically equivalent schema invariants (tables, columns, nullability, defaults, PK/FK/unique/check/indexes, ordering-critical keys).
- [ ] Core API smoke/e2e flows pass on SQLite, PostgreSQL, MySQL 8.4, and MariaDB 11.4 with unchanged wire contract.
- [ ] Session store, auth, CRUD, ordering/rebalance, tx conflict handling, and DB error mapping have backend parity coverage.
- [ ] Data-modifying endpoint tests verify persisted DB state via a second read/query across supported backends.
- [ ] CI includes MySQL 8.4 tmpfs, MariaDB 11.4 tmpfs, PostgreSQL tmpfs, and SQLite jobs; required MySQL/MariaDB gates remain green.
- [ ] AGENTS, `.env.example`, deployment/self-host docs, and migration runbook document 4-target support, backend selection, and migration safety/same-workflow policy.

## Notes

- Biggest risk is **semantic parity**, not connection plumbing: collation/case folding, timestamp precision, lock/busy behavior, upsert/conflict semantics, and migration parity.
- `query!` macros are a major portability cost; prefer runtime queries for shared code. If macros remain, define a deliberate per-backend offline/prepare strategy.
- Consider splitting execution into subtasks after acceptance: T052a foundation, T052b SQLite, T052c PostgreSQL, T052d parity/CI/docs.
