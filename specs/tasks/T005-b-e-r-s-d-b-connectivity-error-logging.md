# T005: BE-RS: DB connectivity & error logging

## Goal

Make every database connectivity outcome in `backend-rs/` **visible, accurate and actionable**, and
make `serve` **fail fast** when it cannot reach MySQL.

Concretely, four things are broken today:

1. **`migrate info` reports a database outage as an unmigrated database and exits `0`.**
   A wrong password, a refused port and an unknown database all print
   `journal: no _sqlx_migrations table yet (fresh database)` + `pending: 1/2` — indistinguishable
   from a genuinely empty database.
2. **`serve` never checks the DB at boot.** It logs `backend-rs listening`, and the only signal is a
   `/ready` 503 whose log line says `readiness probe failed: database unreachable` — **no host, no
   port, no `sqlx::Error`, no MySQL error number**. An operator cannot tell `ECONNREFUSED` from
   `1045 Access denied` from `1049 Unknown database`.
3. **`/ready` blocks for the full 5 s pool `acquire_timeout`** while a k8s probe's default
   `timeoutSeconds` is 1 s, so the probe is failed by the infrastructure's clock rather than by the
   database's actual state.
4. **Two silent config traps turn a config mistake into a misleading runtime symptom:**
   a `DATABASE_URL` that does not parse is not rejected at boot (it silently becomes a pool dialling
   `127.0.0.1:1`), and the `DB_*`-parts URL interpolates the password **verbatim**, so a password
   containing `/`, `?` or `#` produces an unparseable URL → the same silent fallback.

The intended end state is one sentence: **report the intended target (already implemented, uncommitted),
actually dial it, then report the result — or exit non-zero with the cause and a hint.**

## Depends On

None.

**But: the working tree contains prerequisite uncommitted work that MUST be kept, not reverted or
"cleaned up":**

- `backend-rs/src/config.rs` — `Settings::connect_options()` (the single URL-parse site),
  `Settings::log_boot_config()` (the boot-config `INFO` line), and the accompanying tests.
- `backend-rs/src/lib.rs` — `#[cfg(test)] pub(crate) mod test_support { capture_logs, capture_json_logs }`.
- `backend-rs/src/state.rs` — `AppState::from_settings` now builds the pool from `connect_options()`.
- `docs/data-migration-live.md` — boot-line expectation text in the runbook.

Verify with `git status --short` before starting. `git diff` is your starting point, not noise.

## Phase

0

## Critical

No

## Spec References

Read first (in this order):

- `specs/api-rewrite-rs-specs/00-overview.md` — rewrite constitution / conventions.
- `specs/api-rewrite-rs-specs/10-architecture.md`
  - §Config (line ~202) — `Settings::load()` fail-fast contract, redacting `Debug`.
  - §DB pool (line ~211) — `MySqlPoolOptions` values, `health = SELECT 1 (readiness only)`.
  - §SPA fallback + health (line ~158-161) — liveness DB-free, `/ready` = `SELECT 1` → 503.
  - §Observability (lines ~146-156) — pool gauges, separate `METRICS_PORT`, "no scrape feedback loop".
- `specs/api-rewrite-rs-specs/50-tasks.md` — T05 entry (lines 51-60): "boot fails fast on
  missing/invalid `DATABASE_URL`" + "`/health` 200 while `/ready` 503 … bad-DB-password boot → 503".
  **T05's AC wording needs the amendment described in Step 7** (its test builds a router, so the test
  itself does not change).
- `specs/api-rewrite-rs-specs/30-db-schema.md` — collation pin, `_sqlx_migrations`, migration journal.
- `specs/api-rewrite-rs-specs/54-rust-idiom-hardening.md` — §E2 / B2 gate rules (no `unwrap`/`expect`
  in `src/`; `panic!`/`todo!`/`unimplemented!` banned outside `main.rs` + `#[cfg(test)]`).
- `AGENTS.md` (repo root) — Rust test-harness ownership rules, Cargo cache paths, lessons.
- `BACKEND-RS-ORCHESTRATION.md` — required env vars for every cargo command.

External references used to design this task (researched 2026-10-31; **re-verified against upstream
2026-11-01** — do not re-litigate):

- sqlx 0.9 error surface: `sqlx::Error::{Database, Io, Tls, PoolTimedOut, PoolClosed, Configuration,
  InvalidArgument, Protocol, Migrate}` (`.cargo-home/registry/src/*/sqlx-core-0.9.0/src/error.rs`).
- `sqlx::mysql::MySqlDatabaseError::{code(), number(), message()}` (SQLSTATE vs MySQL error number) —
  `sqlx-mysql-0.9.0/src/error.rs:11-32`.
- `sqlx::MigrateError` — **`Execute` is `#[from] sqlx::Error`**: the variant HOLDS a real
  `sqlx::Error`, so `diagnose_migrate` unwraps it directly (no downcast).
  `ExecuteMigration(Error, i64)`, `Dirty(i64)`, `VersionMissing/VersionMismatch(i64)` also present
  (`sqlx-core-0.9.0/src/migrate/error.rs`, verified).
- **THE POOL HIDES REFUSED CONNECTIONS — the reason `probe()` must NOT acquire through the pool.**
  Verified in the installed `sqlx-core-0.9.0/src/pool/inner.rs:325-385`: the pool's internal connect
  loop retries `Io(ConnectionRefused)` and `is_transient_in_connect_phase()` with backoff until the
  `acquire_timeout` deadline, then returns `Error::PoolTimedOut` **with the inner cause dropped**
  (`Err(_) => return Err(Error::PoolTimedOut)` on the per-attempt timeout; the deadline path is
  `timeout(...).ok_or(Error::PoolTimedOut)`). Upstream corroborates: launchbadge/sqlx **#3808**
  ("Unexpected PoolTimedOut Error When Connecting to Downed MySQL Server" — exact symptom) and PR
  **#3929** (maintainer, 2025-07: *"`Pool::connect` … should definitely return the connection error
  and not just eat it then eventually return `PoolTimedOut`. That's a huge UX miss on our part."*).
  The real fix — PR **#3582** (separate `connect_timeout`, a `PoolConnector` short-circuit hook,
  and *"`PoolTimedOut` will actually include the last connection error"*) targets a **future
  release: sqlx 0.9.0 still ships the old `inner.rs`** (verified against the installed source).
  Consequences for this task: (a) every classification probe dials **directly** via
  `ConnectOptions::connect()` — the exact call the pool itself makes at `inner.rs:350`;
  (b) **the MySQL driver does not implement `is_transient_in_connect_phase()`** (as of #2848 only
  Postgres did), so 1040/1203 "too many connections" would never be retried by the pool either —
  the transient column of the classification table below IS the retry policy, owned entirely by
  `wait_until_ready`. When #3582 ships (`connect_timeout` + last-error-in-`PoolTimedOut`),
  `probe()` can be revisited to go through the pool again — tracked upstream, **not required** for
  this task.
- MySQL numbers verified against the MySQL 8.4 error reference + `sqlx-mysql-0.9.0`: 1045, 1044,
  1049, **1129** `ER_HOST_IS_BLOCKED` ("unblock with 'mysqladmin flush hosts'"), **1130**
  `ER_HOST_NOT_PRIVILEGED` ("Host … is not allowed to connect" — a grant-table problem, NOT the
  blocked-host case: both belong in `HostBlocked` with distinct hints), 1040, 1203, 1251, 1146.
- PostHog probe hardening (**posthog#46457**, verified 2025): their `/_readiness` blocked ≤20 s on
  `pool.acquire()` vs a 2 s K8s probe and **pulled pods from rotation during load spikes**; the fix
  defaults the fallback probe to **500 ms** (`READINESS_PROBE_TIMEOUT_MS` — "must fit within K8s 2s
  limit") plus a 30 s reachability cache. Their `/_startup` is best-effort-200, but another PR keeps
  the WRITE pool eager-fail ("failing startup when the write database is unreachable is correct for
  a service whose whole job is writing") — the same `require`-vs-`warn` split this task makes.
- Kubernetes docs (verified): probe `timeoutSeconds` **defaults to 1 s**; liveness must not depend
  on external dependencies (AWS prescriptive guidance + kube-score + learnk8s consensus; readiness
  MAY check the DB when it is the critical request path — this task follows the mainline reading).
- NestJS parity (`backend/src/app.module.ts` + `@nestjs/typeorm`, verified in source:
  `lib/common/typeorm.utils.ts` `handleRetry(retryAttempts = 9, retryDelay = 3000)`):
  TypeORM initialises **eagerly at boot**, retries **9 × 3000 ms** logging
  `Unable to connect to the database. Retrying (n)...`, then throws → process exits non-zero. The
  Rust service is currently *less* fail-fast than the service it replaces. Do NOT copy the
  retries-everything behaviour: a wrong password must fail immediately. Additional evidence FOR
  classification-first logging: nestjs/typeorm **#1151** — the same "unable to connect" message was
  logged for entity-metadata errors that had nothing to do with connectivity.
- `MySql::list_applied_migrations` runs a bare `SELECT … FROM {table}` with **no table-existence
  check** (`sqlx-mysql-0.9.0/src/migrate.rs:130-152`) — that is *why* a missing journal surfaces as
  MySQL **1146** and why the current catch-all exists.

## Files to Create/Modify

### Create

- `backend-rs/tests/api/migrate_cli.rs` — binary-level regression suite (spawns
  `env!("CARGO_BIN_EXE_backend-rs")`; the only place a `migrate info` misreport can be caught).

### Modify

- `backend-rs/src/db.rs` — diagnostics kernel: `Target`, `DbClass`, `Diagnostics`, `diagnose`,
  `diagnose_migrate`, `probe`, `wait_until_ready` (+ unit tests).
- `backend-rs/src/main.rs` — `migrate info` classifier; diagnostics in `run`/`revert`; startup probe
  in `serve`, `migrate *`, `create-admin`.
- `backend-rs/src/config.rs` — `DB_STARTUP_CHECK` + `DB_STARTUP_TIMEOUT_SECS` +
  `DB_READY_PROBE_TIMEOUT_MS` (the `/ready` probe budget knob); direct
  `MySqlConnectOptions` construction from the `DB_*` parts; reject an unparseable `DATABASE_URL`.
- `backend-rs/src/state.rs` — tri-state `db_state` transition flag (`Unknown/Up/Down`; the first
  observed failure is logged, not swallowed by an `AtomicBool` initialised to `false`); module-doc
  correction ("boot never fails on a DB outage" is no longer unconditional).
- `backend-rs/src/lib.rs` — `/ready` diagnostics + bounded probe + transition logging; doc comment.
- `backend-rs/src/metrics.rs` — `kanpad_db_up`, `kanpad_db_connect_failures_total{class}`.
- `backend-rs/tests/api.rs` — `#[path = "api/migrate_cli.rs"] mod migrate_cli;` (alphabetical slot).
- `backend-rs/tests/api/health.rs` — `/ready` regression assertions.
- `backend-rs/tests/api/metrics.rs` — new series assertions.
- `docs/data-migration-live.md` — one extra expectation line + troubleshooting rows.
- `specs/api-rewrite-rs-specs/10-architecture.md` — Config table + Health section amendment.
- `specs/api-rewrite-rs-specs/50-tasks.md` — T05 AC wording amendment (see Step 7).
- `AGENTS.md` — lessons learned (see Notes §7).

## Implementation Steps

Implement **P0 → P1 → P2 → P3** in that order, TDD per step (red test first, then the code).
Each step is independently mergeable; do not interleave.

### Step 0 — Reproduce (before writing code)

Use the prebuilt binary or `cargo run`:

```bash
cd backend-rs
BIN=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-target-kanpad/debug/backend-rs
DB_PASSWORD=definitely-wrong $BIN migrate info; echo "exit=$?"     # expect: all-pending, exit=0  (BUG)
DB_PORT=1                  $BIN migrate info; echo "exit=$?"     # expect: all-pending, exit=0  (BUG)
DB_PORT=1 PORT=13311       $BIN serve &                          # boots; /ready 503 with a causeless log
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:13311/health   # 200
curl -s -w '\n%{http_code}\n' http://127.0.0.1:13311/ready               # 503 after ~5000 ms
```

Record the outputs in the final report as the "before" evidence.

### Step 1 — P0.1: the diagnostics kernel in `src/db.rs`

`db.rs` is documented as **macro-free** (`//! … No query! here, so no .sqlx/ entries`) — keep it that
way. Use runtime `sqlx::query_as`, never `query!`; **`.sqlx/` must stay byte-identical** in this task.

Add (public, documented — `missing_docs` is deny-level):

```rust
/// Where the process will actually dial, derived from the SAME parsed
/// `MySqlConnectOptions` the pool is built from. Never holds a password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub socket: Option<std::path::PathBuf>,
    pub database: Option<String>,
    pub username: String,
    pub source: DbSource,   // DatabaseUrl | Parts
}
impl Target { pub fn from_options(options: &MySqlConnectOptions, source: DbSource) -> Self }

/// Coarse, human-meaningful classification of a connection failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbClass {
    AccessDenied, DbAccessDenied, UnknownDatabase, HostBlocked,
    TooManyConnections, AuthPlugin, MissingTable,
    Refused, Timeout, Dns, Tls, Config, Other,
}

#[derive(Debug, Clone)]
pub struct Diagnostics {
    pub class: DbClass,
    pub mysql_number: Option<u16>,
    pub sqlstate: Option<String>,
    pub message: String,        // server text; never contains the password
    pub transient: bool,
    pub hint: &'static str,
}

pub fn diagnose(err: &sqlx::Error) -> Diagnostics;
pub fn diagnose_migrate(err: &sqlx::migrate::MigrateError) -> Diagnostics; // unwraps Execute/ExecuteMigration

pub struct ProbeReport { pub server_version: String, pub selected_database: Option<String>,
                         pub elapsed: std::time::Duration }
/// Dials DIRECTLY via `ConnectOptions::connect()` — see the rationale below; NEVER
/// `pool.acquire()`.
pub async fn probe(options: &MySqlConnectOptions, target: &Target) -> Result<ProbeReport, Diagnostics>;
pub async fn wait_until_ready(options: &MySqlConnectOptions, target: &Target,
                              budget: Duration, dial_timeout: Duration)
    -> Result<(ProbeReport, u32), (Diagnostics, u32)>;   // (report, attempts) / (last error, attempts)
```

**`probe` must not go through `pool.acquire()`.** Verified in the installed
`sqlx-core-0.9.0/src/pool/inner.rs:325-385` and corroborated upstream (#3808/#3929): the pool's
internal connect loop retries `Io(ConnectionRefused)` until `acquire_timeout` and then returns a
**causeless** `PoolTimedOut` (the fix that surfaces the last connect error — PR #3582 — is NOT in
0.9.0). A pooled probe therefore leaves the refused case exactly as blind as today, and out-sources
retry policy to `acquire_timeout`. `probe` dials **directly** — `options.connect()`, the same call
the pool makes at `inner.rs:350` — so the raw error class is available on attempt 1 and
`wait_until_ready` is the SINGLE owner of retry policy. It runs `SELECT VERSION(), DATABASE()` on
the fresh connection (the same handshake the pool uses, including `caching_sha2_password` +
`mysql-rsa`), timed by `Instant`, then closes the connection. `wait_until_ready`:

- each attempt is `tokio::time::timeout(dial_timeout, options.connect())` — `dial_timeout`
  (callers pass ~2 s) must be INDEPENDENT of `budget`: a SYN-hanging unroutable host would
  otherwise burn the whole budget inside attempt 1. The wrapper's `Elapsed` maps to
  `DbClass::Timeout`/`transient`; a refused port maps to `DbClass::Refused` on iteration one;
- attempt 1 immediately; retry **only** when `diagnostics.transient`;
- backoff 250 ms → ×2, capped at 2 s, until `budget` is exhausted;
- a **permanent** error returns after exactly **one** attempt (a wrong password answers in
  ~ms and must not burn the budget — no pool retries hide it or pad it).

Classification (MySQL numbers verified against `sqlx-mysql-0.9.0` + the MySQL 8 error reference):

| Signal | `DbClass` | `transient` | Hint text (short, actionable) |
|---|---|---|---|
| `Database` 1045 | `AccessDenied` | no | "MySQL rejected the credentials: check DB_USERNAME / DB_PASSWORD (the password is never logged)." |
| `Database` 1044 | `DbAccessDenied` | no | "the user is not granted access to this database: GRANT … ON <db>.* TO <user>." |
| `Database` 1049 | `UnknownDatabase` | no | "the database does not exist: CREATE DATABASE … CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci." |
| `Database` 1129 | `HostBlocked` | no | "the server blocked this host after too many failed attempts: wait, or FLUSH HOSTS on the server." |
| `Database` 1130 | `HostBlocked` | no | "the server does not allow this host: add a matching account (CREATE USER 'user'@'host' + GRANT …)." |
| `Database` 1040 / 1203 | `TooManyConnections` | yes | "the server is out of connections: raise MySQL max_connections or lower DB_MAX_CONNECTIONS." |
| `Database` 1251 | `AuthPlugin` | no | "the account uses an auth plugin this client cannot negotiate." |
| `Database` 1146 | `MissingTable` | no | "table missing — for the migration journal this means the database has never been migrated." |
| `Io(ConnectionRefused)` | `Refused` | yes | "nothing is listening on host:port — check DB_HOST/DB_PORT against the compose file / the runbook (the shared test container is :3337)." |
| `Io(TimedOut / NetworkUnreachable / HostUnreachable / ConnectionReset / ConnectionAborted)` | `Timeout` | yes | "the server did not answer — firewall, wrong host, or MySQL still starting." |
| `Io(AddrNotAvailable)` / other `Io` | `Dns` | yes | "the host name did not resolve — check DB_HOST." |
| `Tls(_)` | `Tls` | no | "TLS negotiation failed: check the CA/ssl-mode of DATABASE_URL." |
| `Protocol(_)` | `AuthPlugin` | no | "the auth/handshake negotiation failed — sqlx surfaces `caching_sha2_password`/RSA handshake failures here, NOT as `Database` 1251." |
| `Configuration(_)` | `Config` | no | "the connection options are invalid — see the boot configuration line." |
| `PoolTimedOut` | `Timeout` | yes | "the connection pool timed out waiting for a connection." |
| everything else | `Other` | no | "unexpected driver error: see `class`/`mysql_number` and the message." |

Two table footnotes, also verified against the installed 0.9.0 source:

- `Error::InvalidArgument(String)` exists in 0.9's error enum → classify as `Config` with the same
  hint as `Configuration(_)`.
- `diagnose_migrate` must map the NON-connection `MigrateError` variants explicitly — they must
  NEVER render as "fresh database": `Dirty(v)` → `Other` with hint "the migration journal is dirty
  at version v: `DELETE FROM _sqlx_migrations WHERE version = v AND success = 0` (or drop and
  re-migrate dev data) — NEVER hand-edit a `success = 1` row (rows carry SHA-384 checksums)";
  `VersionMismatch(v)` / `VersionMissing(v)` → `Other` with hint "a migration file was edited in
  place after it was applied — checksum break; the applied DB must be dropped and re-migrated".
  All three are non-transient. Only the journal-absent 1146 path classifies as `MissingTable`.

Also add `impl Diagnostics { pub fn log(&self, target: &Target, elapsed: Duration) }` (or a free
`log_connect_failure`) so **every** call site logs the identical field set:

```
host, port, socket?, database, username, source,
elapsed_ms, class, mysql_number?, sqlstate?, transient, message, hint
```

**Never log**: the password, `DB_PASSWORD`, `SESSION_SECRET`, or `Settings::database_url()` (the
derived URL embeds the password verbatim).

Tests (`#[cfg(test)] mod tests` in `db.rs`): classify each code above using the stub-`DatabaseError`
pattern already in `src/error.rs` (`StubDatabaseError` implements `sqlx::DatabaseError`); assert the
`transient` matrix; assert `Target::from_options` + the rendered diagnostics never contain a password;
`is_valid_db_name` tests stay untouched.

### Step 2 — P0.2: `migrate info` must not swallow failures

`src/main.rs`, `MigrateOp::Info` (currently lines ~242-262):

```rust
let applied = match applied_versions(&state.pool, &migrator).await {
    Ok(versions) => versions,
    // ONLY 1146 means "the journal was never created" (sqlx runs a bare SELECT
    // against _sqlx_migrations with no existence check — sqlx-mysql-0.9.0
    // src/migrate.rs:130-152). EVERY other error is a real failure.
    Err(err) if db::diagnose_migrate(&err).class == db::DbClass::MissingTable => {
        println!("journal: no _sqlx_migrations table yet (fresh database)");
        Vec::new()
    }
    Err(err) => {
        let diagnostics = db::diagnose_migrate(&err);
        diagnostics.log(&target, elapsed);           // full structured cause
        return Err(anyhow!(
            "cannot read the migration journal: {} ({}) — {}",
            diagnostics.class_label(), diagnostics.code_label(), diagnostics.hint
        ));
    }
};
```

- **Compile reality (verified in the tree):** `applied_versions` (main.rs:209-225) wraps EVERY error
  in anyhow `.context(...)`, so in the match arm above `err` is `anyhow::Error`, NOT
  `sqlx::migrate::MigrateError` — `diagnose_migrate(&err)` as literally sketched will not compile.
  Reshape the helper: a private `journal_versions(pool, migrator) -> Result<Vec<i64>,
  sqlx::migrate::MigrateError>` that returns the RAW error (no `.context`), and keep the anyhow
  contexting at the `migrate run`/`revert` call sites that still want it. The classification must
  happen on the raw error.
- Keep the **last** line exactly `applied: {n} pending: {p}` (humans + runbook rely on it).
- Print the target **before** the status lines so a `RUST_LOG=error` operator still sees which
  database was inspected: e.g. `database: kanpad_rs_dev at 127.0.0.1:3306 (user "user", source DB_* parts)`.
  Put it after the boot line and before `pending:`/`applied:`; last line stays `applied: …`.
- `migrate run` / `migrate revert`: add the same `diagnose` to the `context(...)` and flatten the
  current 3× duplicated `anyhow` chain into one line
  (`Error: failed to apply migrations: access denied (1045) — check DB_USERNAME / DB_PASSWORD`).
  Do not change the exit code (still `1`).

Tests: `tests/api/migrate_cli.rs` (new). Spawn the binary with an explicit env; note that
`Settings::load()` calls `dotenvy::dotenv()` which only FILLS missing vars, so an explicit env wins
over `backend-rs/.env`:

```rust
let bin = env!("CARGO_BIN_EXE_backend-rs");
// bad password → exit != 0; stderr mentions "access denied"/1045 + the hint;
//                 stdout must NOT contain "pending:" (the old bogus output).
// refused port → exit != 0; stderr mentions "connection refused" + DB_PORT hint.
// unknown db   → exit != 0; stderr mentions 1049 + CREATE DATABASE hint.
// healthy DB   → exit 0; last stdout line == "applied: 2 pending: 0"
//   — against a SUITE-OWNED database, never the shared `kanpad`: the 3337 tmpfs container
//   restarts to "fresh database" at any moment, so asserting `applied: 2 pending: 0` against
//   the shared DB is order-dependent flake. The suite derives its URL from the env-provided
//   `DATABASE_URL` (path-segment swap → `kanpad_cli_<pid>`, a valid identifier per
//   `db::is_valid_db_name`), lets `migrate run` create it, then asserts `applied: 2 pending: 0`
//   there. skip+pass when `DATABASE_URL` is absent or unparsable.
```

Run the binary from a temp `current_dir` and set `SESSION_SECRET` (≥32 bytes) + `APP_ENV=test` +
`DATABASE_URL` (derived as above) explicitly so the test never depends on a developer `.env`.

### Step 3 — P0.3 + P2: `/ready` diagnostics, bounded probe, metrics

`src/lib.rs::ready` (currently lines ~843-875):

- Keep the **response** byte-identical (`{"type":"…/service-unavailable", …}`; the existing
  `health::ready_503_when_db_down_no_password_leak` asserts no password and no dial address leak —
  do not add `class`/`host` to the body).
- Swap the pooled `SELECT 1` for the **same `db::probe` the boot path uses** (direct dial, from
  `state.settings.connect_options()` — after Step 5 `validate()` guarantees `Some`): a pooled
  acquire retries refused errors internally until `acquire_timeout` (5 s) and then returns a
  **causeless `PoolTimedOut`** (`sqlx-core-0.9.0/pool/inner.rs:325-385` + upstream #3808/#3929),
  which would leave `/ready` as blind as today for the most common failure. Cost is one direct
  handshake per probe (probes fire every ~10 s; a healthy tmpfs/LAN MySQL answers in ~10-30 ms).
  Confirmed precedent: PostHog removed `pool.acquire()` from `/_readiness` for exactly this
  reason (posthog#46457).
- **Bounded at ~500 ms — calibrated to the kubelet clock.** K8s `timeoutSeconds` defaults to
  **1 s**: a 5 s budget (or even a 2 s one) is still cut by the infrastructure, not by the
  database's actual state. `const READINESS_PROBE_TIMEOUT_DEFAULT: Duration =
  Duration::from_millis(500);` with an env override in `Settings`:
  `DB_READY_PROBE_TIMEOUT_MS` (validated `50..=3000`, else `ConfigError::Invalid`) so a manifest
  with a raised `timeoutSeconds` can raise it without a rebuild. Document the pairing in
  `10-architecture.md`: probe `timeoutSeconds ≥ 2×` the budget. PostHog's calibration is the same
  shape (500 ms fallback under a 2 s probe limit).
- Log the full `Diagnostics` **on transition**, via a **tri-state** flag on the state
  (`AppState.db_state: AtomicU8`, `0 = Unknown / 1 = Up / 2 = Down`): `error!` on Up→down (with
  the cause), `info!` on down→Up, and a `warn!` **bootstrap log on the FIRST observed failure**
  (`Unknown→Down`). A plain `AtomicBool` initialised `false` would make the very first failure a
  non-transition — the log-free case `warn` mode (server up, DB down from t=0) specifically cares
  about. No per-probe spam afterwards.
- Set `metrics.db_up` (1/0) and increment `metrics.db_connect_failures_total{class}` (new fields in
  `src/metrics.rs`, rendered as a gauge + a labelled counter; follow the existing
  `kanpad_db_pool_size` / `kanpad_throttle_rejections_total` shapes). The gauge's HELP text states
  its refresh points (`/ready` scrapes + the boot probe — there is NO background monitor), so an
  operator does not expect `/metrics` alone to flip it.
- Structural constraint: `READINESS` must stay outside `/api/*`, unauthenticated and unthrottled
  (`create_app` route table — do not move it).

Tests: `tests/api/health.rs` (body unchanged, still 503 — now in < ~1 s on an unroutable target;
the 5 s pool delay is gone by construction since the probe no longer acquires),
`tests/api/metrics.rs` (`kanpad_db_up 0` after a failed probe; counter increments),
`src/lib.rs` unit test for the transition-log helper using `test_support::capture_logs` (asserting
the FIRST-observation log fires, the cause is present, and the password is not).

### Step 4 — P1: fail fast at boot

`src/config.rs` — two new `Settings` fields (also add to the redacting `Debug` and `Default`):

| Key | Type | Default | Validation |
|---|---|---|---|
| `DB_STARTUP_CHECK` | `StartupCheck { Require, Warn }` | `Require` | unknown value → `ConfigError` naming `DB_STARTUP_CHECK` (mirror `AppEnv`'s `FromStr`/`Deserialize` pattern) |
| `DB_STARTUP_TIMEOUT_SECS` | `u64` | `15` | `1..=300` → else `ConfigError::Invalid` |
| `DB_READY_PROBE_TIMEOUT_MS` | `u64` | `500` | `50..=3000` → else `ConfigError::Invalid` (landed with Step 3; listed here because it lives in `config.rs` too) |

`src/main.rs::serve` — **before** `tokio::spawn(sweeper…)` and **before** `TcpListener::bind`:

```rust
let started = Instant::now();
// `state.pool.connect_options()` is the SAME options object the pool holds (never `None` —
// `from_settings` guarantees a concrete options object; the pool never acquired in this path,
// so the 5 s `acquire_timeout` is irrelevant here). The per-attempt dial timeout lives inside
// `wait_until_ready` (Step 1).
match db::wait_until_ready(state.pool.connect_options(), &target,
                           settings.db_startup_timeout(), Duration::from_secs(2)).await {
    Ok((report, attempts)) => tracing::info!(
        host = …, port = …, database = …, username = …, source = …,
        elapsed_ms = started.elapsed().as_millis(), attempts,
        server_version = %report.server_version,
        selected_database = report.selected_database.as_deref().unwrap_or("<none>"),
        "database connected"
    ),
    Err((diagnostics, attempts)) => {
        diagnostics.log(&target, started.elapsed());
        match settings.db_startup_check {
            StartupCheck::Require => bail!(
                "cannot connect to the database ({} attempts in {}s): {} — {}",
                attempts, started.elapsed().as_secs(), diagnostics.class_label(), diagnostics.hint
            ),
            StartupCheck::Warn => tracing::warn!(
                "continuing without a database: /health stays 200, /ready reports 503 (DB_STARTUP_CHECK=warn)"
            ),
        }
    }
}
```

- If `report.selected_database != target.database` → `tracing::warn!` (catches "connected to the
  wrong database", e.g. a leftover `DB_NAME`).
- `serve --auto-migrate`: probe first, then migrate (never migrate against an unreachable DB).
  `DB_STARTUP_CHECK=warn` does NOT exempt the migration: after a failed probe, `--auto-migrate`
  still aborts with exit 1 from the migration step itself — `warn` buys a DB-*less* server,
  never a silently-skipped migration. Say so in the warn log line.
- `create-admin` and every `migrate *` op run the same probe (with the same `Require`/`Warn` policy);
  `create-admin` maps a failure to its existing `AdminExit::Db` (code 3) after printing the hint.
- `export-openapi` stays DB-free and must not gain the probe.

Tests: binary-level in `tests/api/migrate_cli.rs` (or a sibling `serve_boot.rs` if cleaner):
`serve` + bad password + `DB_STARTUP_CHECK=require` → exit 1, stderr has `access denied` + hint,
and no `listening` line / no port bound; `DB_STARTUP_CHECK=warn` → `/health` 200 and `/ready` 503.
Lib test: `wait_until_ready` attempts exactly once on a permanent error and >1 on a refused port
(use `127.0.0.1:1` with a ~1 s budget to keep it fast).

### Step 5 — P3: config-parse correctness

1. **Build the options directly from the parts** (no URL string round-trip) inside
   `Settings::connect_options()`:

   ```rust
   match self.database_url.as_deref() {
       Some(explicit) => explicit.parse().ok(),
       None => Some(
           MySqlConnectOptions::new()
               .host(&self.db_host).port(self.db_port)
               .username(&self.db_username).password(self.db_password.expose_secret())
               .database(&self.db_name)
               .charset("utf8mb4").collation("utf8mb4_unicode_ci"),   // the T66 pin, preserved
       ),
   }
   ```

   Rationale: `database_url()` interpolates the password verbatim, so `pa/ss` yields
   `authority=user:pa` → parse error → the silent `127.0.0.1:1` fallback. Keep `database_url()` for
   tooling/docs, and percent-encode user/password there.

2. **`validate()` rejects an unparseable `DATABASE_URL`** (`ConfigError::Invalid`) — a malformed URL is
   a config error, not an outage. The `state.rs` unroutable fallback stays (it is the seam for
   hand-built `Settings` in tests: `state::tests::unparseable_url_stays_total`).

Tests (`config.rs`): parts with `DB_PASSWORD="pa/ss#1"` → `connect_options()` is `Some`, and the
password round-trips exactly; `DATABASE_URL="mysql://u:p@h:notaport/db?charset=utf8mb4&collation=utf8mb4_unicode_ci"`
→ `Settings::from_pairs` returns `Err`. The existing collation-pin test
(`tests/api/collation.rs`) must stay green — the direct options set the same `collation`.

### Step 6 — Regression sweep

- `migrate info` output contract: last line still `applied: N pending: M`.
- `/ready` response body unchanged; `openapi.json` and `frontend/src/lib/schema.d.ts` **unchanged**
  (no route/DTO change) — if they move, something is wrong.
- `.sqlx/` **byte-identical** (no `query!` added/changed).
- Do not regress the four CI grep gates (no `unwrap`/`expect` in `src/`, no `panic!`/`todo!`/
  `unimplemented!` outside `main.rs` + `#[cfg(test)]`, no `format!`-built SQL in `store.rs`).
- Gate-compat note (verified against `.github/workflows/backend-rs.yml`): the unwrap/expect gate
  greps `\.(unwrap|expect)\(` — the new code's `.unwrap_or(...)` / `as_deref().unwrap_or(...)`
  do NOT match (`_or` intervenes), and the `#[tracing::instrument(skip(state))]` gate greps the
  exact `skip(state))` spelling — the `ready` handler keeps `skip(state, headers)`. Do not contort
  code into `match` gymnastics to satisfy a gate that was never aimed at those constructs.

### Step 7 — Docs + spec amendments (same PR)

- `10-architecture.md`: Config table gains `DB_STARTUP_CHECK` / `DB_STARTUP_TIMEOUT_SECS` /
  `DB_READY_PROBE_TIMEOUT_MS`; the §Health paragraph gains "boot probes the DB once within a bounded
  retry budget and exits non-zero on a permanent failure (`DB_STARTUP_CHECK=require`, default);
  `warn` keeps the degraded mode" **and** "the readiness probe dials MySQL directly (NOT via
  `pool.acquire()` — the pool swallows `ConnectionRefused` into a causeless `PoolTimedOut`,
  sqlx #3808/#3929) under a ~500 ms default budget (`DB_READY_PROBE_TIMEOUT_MS`); manifests should
  set the probe's `timeoutSeconds ≥ 2×` the budget (K8s default `timeoutSeconds` is 1 s)". Note in
  the amendment that the §Code policy line "Boot connection-refused = fatal" ALREADY states this
  contract — this task fulfills it; the §DB pool's "`health = SELECT 1 (readiness only)`" line
  becomes "readiness = a bounded direct dial (`SELECT VERSION(), DATABASE()`), diagnostics in the
  transition log".
- `50-tasks.md` T05 AC (lines ~56 and ~59): clarify that "boot → 503" means **the router built by
  `create_app`**, and that the *binary's* default is now fail-fast. No test changes.
- `docs/data-migration-live.md`: the runbook's expected output gains one line
  (`INFO … database connected …`) before `migrations applied: 2`, plus troubleshooting rows for
  `1045`, `1049`, **`1130`**, `ECONNREFUSED`, and "the boot line says `DATABASE_URL` while `.env`
  only has `DB_*`".
- `AGENTS.md` → Lessons: (a) a catch-all `Err(_)` on a DB read is a *lie detector inverted* — it
  converts outages into "empty database"; classify by MySQL number (1146) instead; (b) an eager
  startup probe must retry only *transient* classes — and must NOT borrow the pool's retry policy:
  sqlx's own `inner.rs` retries `ConnectionRefused` into a causeless `PoolTimedOut` (#3808/#3929),
  so probes/exit-fast paths own the dial directly; (c) never interpolate a password into a URL;
  (d) **operator-facing hint text must only carry verified facts** — the first draft of the
  `Refused` hint cited "compose publishes 3307→3306", a number that appears nowhere in the tree
  (compose actually publishes 3306:3306); a wrong hint compiled into the binary is worse than a
  generic one, so treat hint constants like the grep gates: *prove them* before landing.

## Constraints

- **Scope:** `backend-rs/src/{db,main,config,state,lib,metrics}.rs`, the three test files listed,
  and the four doc files. No feature module (`src/features/**`) changes. No migration files.
- **No new dependency.** Everything is reachable from `sqlx`, `tokio`, `std`, `tracing`.
- **No `.sqlx/` change.** `db.rs` stays macro-free; use runtime queries only.
- **No HTTP contract change.** `/ready` body, problem+json shapes, `openapi.json`, `schema.d.ts`.
- **No credential in any log or body.** Never log the password, `SESSION_SECRET`, or
  `Settings::database_url()`. Server messages (`Access denied for user 'x'@'y' …`) are allowed — they
  are the diagnostic value and are already sanctioned by T05 ("`tracing::error!` server-side").
- **Secrets screen:** the existing config tests assert the boot line never contains the password —
  keep them green.
- **TDD:** red test first per step; no test deletion or weakening. Every behaviour that previously
  "worked" (e.g. `/health` 200 with a broken pool) must still work.
- **Rust idioms:** `#![forbid(unsafe_code)]`, edition 2024, `missing_docs = "deny"` — every new `pub`
  item needs a doc comment. Follow the workspace `[lints]` (pedantic is advisory).
- **Commands** (per `BACKEND-RS-ORCHESTRATION.md`; root disk is tight — do not substitute paths):

  ```bash
  export CARGO_HOME=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-home
  export CARGO_TARGET_DIR=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-target-kanpad
  export DATABASE_URL='mysql://root:dev_root_pw@127.0.0.1:3337/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci'
  export SESSION_SECRET='forty-two-bytes-of-test-session-secret!!'
  ```

  The shared test DB is `docker-compose.test.yml` → container `kanpad-rs-testdb`, host `:3337`,
  database `kanpad`; it is **tmpfs**, so a restart wipes the schema — `docker start kanpad-rs-testdb`
  then `SQLX_OFFLINE=true cargo run -- migrate run`.
- **One database per test binary:** `#[sqlx::test]` suites own their DBs; the new `migrate_cli` suite
  spawns the binary and must never mutate the shared `kanpad` schema.

## Acceptance Criteria

- [ ] `DB_PASSWORD=wrong cargo run -- migrate info` exits **non-zero**, names `1045` / "access denied",
      and prints the hint; it no longer prints `pending:` lines as if the DB were empty.
- [ ] `DB_PORT=1 cargo run -- migrate info` exits **non-zero** with "connection refused" + the
      `DB_PORT`/`DB_HOST` hint.
- [ ] `DB_NAME=does_not_exist cargo run -- migrate info` exits **non-zero** with `1049` + the
      `CREATE DATABASE` hint.
- [ ] On a migrated DB, `migrate info` prints the target line, the two `applied:` rows, and the last
      line is exactly `applied: 2 pending: 0`.
- [ ] `migrate info` on a genuinely fresh database still reports the journal-absent message and all
      migrations pending (the 1146 path), exit 0.
- [ ] `serve` with an unreachable DB and `DB_STARTUP_CHECK=require` (default) exits **1 within the
      budget**, logs `class`, `mysql_number`, `sqlstate`, `message`, `hint`, `host`, `port`,
      `database`, `username`, `elapsed_ms`, and **never binds the port**. With the direct-dial probe
      the class is exact: a refused port exits as `refused` — never as a causeless `PoolTimedOut`.
- [ ] A **wrong password fails in one attempt** (no retry storm); a **refused port retries** until the
      budget and then exits 1. Both are proven by an attempts-count assertion (counted by
      `wait_until_ready`'s own loop — the pool's internal connect/retry path is never involved).
- [ ] `serve` with `DB_STARTUP_CHECK=warn` and an unreachable DB stays up: `/health` 200, `/ready` 503
      (the pre-existing degraded mode is preserved and documented).
- [ ] `DB_STARTUP_CHECK=warn` + `--auto-migrate` with an unreachable DB still exits **1 from the
      migration step** — `warn` permits a DB-less server, never a silently-skipped migration.
- [ ] `diagnose_migrate` maps `Dirty`/`VersionMismatch`/`VersionMissing` to journal-guard hints that
      never say "fresh database" and never suggest hand-editing a `success = 1` journal row.
- [ ] Healthy boot logs `database connected` with `server_version` and `selected_database`, and warns
      when `selected_database` differs from the configured `db_name`.
- [ ] `/ready` against an unreachable DB returns the **byte-identical 503 problem+json body in
      ≈0.5 s** (the ~500 ms default budget sits well under the kubelet's 1 s `timeoutSeconds`
      default; the pool is never in the probe path) and logs the cause on **every first
      observation and transition** of the tri-state (`Unknown→Down` bootstrap, `Up→Down`,
      `Down→Up`) — not per probe.
- [ ] `kanpad_db_up` and `kanpad_db_connect_failures_total{class}` appear on `/metrics` and change
      state across a success→failure→success sequence.
- [ ] `DB_PASSWORD='pa/ss#1'` (parts only, no `DATABASE_URL`) → `connect_options()` is `Some` and the
      password round-trips; the pool no longer falls back to `127.0.0.1:1`.
- [ ] An unparseable `DATABASE_URL` fails `Settings::load()` with a message naming `DATABASE_URL`
      (no silent fallback), and `create-admin` exits 3 with the hint.
- [ ] No password, `SESSION_SECRET`, or raw `DATABASE_URL` appears in any log, problem body, or
      `Diagnostics` rendering (asserted by tests).
- [ ] `.sqlx/` **unchanged** (`git status --short backend-rs/.sqlx` empty), `openapi.json` unchanged,
      `frontend/src/lib/schema.d.ts` unchanged.
- [ ] Gates: `cargo fmt --check` 0 · `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic` 0 ·
      `cargo test -p backend-rs` (lib + `--test api`) green · `cargo sqlx prepare --check --workspace --all -- --all-targets` 0 ·
      the four CI grep gates pass · `cargo deny check advisories bans licenses` ok.
- [ ] Docs updated in the same change: `10-architecture.md` (config keys + health), `50-tasks.md`
      (T05 wording), `docs/data-migration-live.md` (expected output + troubleshooting), `AGENTS.md`
      (four lessons).

## Notes

### 1. Reproduced evidence (2026-10-31, current tree)

```
$ DB_PASSWORD=definitely-wrong backend-rs migrate info
INFO backend_rs::config: boot configuration … db_name="kanpad_rs_dev" db_source="DB_* parts"
journal: no _sqlx_migrations table yet (fresh database)
pending: 1 baseline
pending: 2 convert to utf8mb4
applied: 0 pending: 2
exit=0

$ DB_PORT=1 backend-rs serve   # same all-pending output for `migrate info`; for serve:
INFO backend-rs listening port=13311
GET /health -> 200
GET /ready  -> 503 (after 5002 ms)  and the ONLY log line is:
ERROR ready: readiness probe failed: database unreachable

$ DB_PASSWORD=definitely-wrong backend-rs migrate run
Error: failed to apply migrations
Caused by:
    0: while executing migrations: error returned from database: 1045 (28000): Access denied for user 'user'@'localhost' (using password: YES)
    1: error returned from database: 1045 (28000): Access denied …
    2: 1045 (28000): Access denied …
exit=1
```

The cause is *already available* in `migrate run`'s path; P0 is mostly about propagating it instead of
discarding it.

### 2. Root-cause map

| # | Cause | Location (as of 2026-10-31) |
|---|---|---|
| 1 | `MigrateOp::Info` catch-all: `Err(_) => println!("journal: no _sqlx_migrations table yet")` | `src/main.rs:243-249` |
| 2 | `connect_lazy_with` — boot never dials, nothing can fail fast | `src/state.rs:38-46` |
| 3 | `/ready` logs a fixed string; the error is dropped | `src/lib.rs:857-868` |
| 4 | No classifier: 1045 / 1049 / 1146 / `Io` are never distinguished | does not exist |
| 5 | An unparseable `DATABASE_URL` silently becomes a pool dialling `127.0.0.1:1`, and `validate()` lets it boot | `src/config.rs::validate`, `src/state.rs::unroutable_options` |
| 6 | `database_url()` interpolates the password verbatim → `/`, `?`, `#` make the URL unparseable (verified: `pa/ss` → authority `user:pa` → port parse error) | `src/config.rs:414-427` |
| 7 | **The pool swallows `ConnectionRefused`:** its internal connect loop retries refused/transient connect errors with backoff until `acquire_timeout`, then returns `Error::PoolTimedOut` **with the inner cause dropped** — so every pooled error surface (today's `/ready`, and any pooled probe) is blind to the most common failure | installed `sqlx-core-0.9.0/src/pool/inner.rs:325-385` + upstream launchbadge/sqlx #3808 / PR #3929 (fix #3582 not in 0.9.0) |

5 + 6 compound: a special-character password produces "database unreachable" with **zero** indication
that the URL itself is broken. 7 means the classifier can never see the real cause through
`pool.acquire()` — which is why `probe` dials directly (Step 1).

### 3. Why this matters in this repo specifically

- `backend-rs/.env` points at `localhost:3306` / db `kanpad_rs_dev`; the compose topology publishes
  MySQL on **3306:3306** (`compose.yaml`, mysql service — verified 2026-11-01; an earlier draft of
  this spec wrongly claimed 3307, a number that appears nowhere in the tree), and the shared test
  container listens on **3337** (`backend-rs/docker-compose.test.yml`, the only compose file with
  that port). A port/db mismatch still produces exactly the reproduced symptom (a system MySQL on
  3306 answers with an *auth* error, which is then rendered as "fresh database"). **There is no
  `.env.example` in `backend-rs/`** — do not promise one in the PR; the boot-config log line
  (prerequisite work) is the discoverability fix for port/db confusion.
- The cutover runbook (`docs/data-migration-live.md`) is executed by a human against staging/prod;
  a misleading `migrate info` there is a correctness risk, not just an annoyance (§11's gates assume
  `pending: 0` means "migrated").

### 4. NestJS parity (why `require` is the right default)

`@nestjs/typeorm` initialises the DataSource eagerly during module init and retries **9 × 3000 ms**
(`typeorm.utils.js:102-119`), logging each attempt, then throws → the process exits non-zero. So the
service being replaced is *more* fail-fast than the current Rust binary. The improvement over Nest:
retry **only transient** classes, so a wrong password fails in milliseconds instead of ~27 s.

### 5. Ecosystem consensus (2026) — used to pick the design

- Liveness must stay DB-free (already correct); readiness is the only place a dependency check
  belongs, and it must be **bounded well under the probe timeout**. Verified precedent —
  **posthog#46457**: their `/_readiness` blocked up to **20 s** on `pool.acquire()` against a **2 s**
  K8s probe and pulled pods from rotation during load spikes; the fix defaults their fallback probe
  to **500 ms** (`READINESS_PROBE_TIMEOUT_MS` — "must fit within K8s 2s limit"). This task
  calibrates the same shape: `DB_READY_PROBE_TIMEOUT_MS` default 500 ms, safely under the kubelet's
  1 s `timeoutSeconds` default. A companion PostHog PR keeps the WRITE pool **eager-fail** at
  startup ("failing startup when the write database is unreachable is correct for a service whose
  whole job is writing") — the same `require`-vs-`warn` split this task makes.
- Startup: fail fast after a **bounded** retry budget, retrying availability failures only (Nairo:
  `startup_timeout` then exit non-zero; Multica: jittered backoff, permanent SQL errors fail
  immediately).
- Never defer the handshake to the first user request — eager warm-up at boot (the `min_connections(1)`
  pool keeps the connection warm afterwards).
- Never make liveness depend on the DB (restart-stampede anti-pattern).

### 6. Risks / things that will bite

- **T05's AC wording** ("bad-DB-password boot → 503") reads as "boot succeeds". The *test* builds a
  router, so it does not change; the wording is amended in Step 7 and the old semantics survive as
  `DB_STARTUP_CHECK=warn`.
- **CI docker smoke** (`Serve :3001 + smoke test`) passes a live `DATABASE_URL` → unaffected. Compose
  uses `depends_on: mysql: condition: service_healthy` → unaffected. Any *other* deployment that
  starts the API before MySQL must either set `warn` or rely on the 15 s retry budget.
- **`Dockerfile` HEALTHCHECK** hits `/health` (DB-free) — do not change it to `/ready`, or a DB blip
  will restart the container.
- **`acquire_timeout` is 5 s** and is shared with request handling. Keep it (T03 canonical value) —
  but note that the readiness/boot probes NO LONGER pass through it: both dial directly via
  `wait_until_ready`, which owns its per-attempt `dial_timeout` (~2 s) independent of the budget.
  An instrumentation listener can therefore no longer alter handler behaviour through the pool, and
  the pool's internal acquire retry (the `PoolTimedOut` path) is likewise out of the probe path.
- **K8s pairing risk:** the ~500 ms default budget is only safe while the manifest promises
  `timeoutSeconds ≥ 2×` the budget (kubelet default `timeoutSeconds` = 1 s). Document the pairing in
  `10-architecture.md` (Step 7) and keep `DB_READY_PROBE_TIMEOUT_MS` validated `50..=3000` — an
  unvalidated knob would let an operator raise the budget past the probe timeout and reintroduce
  the exact infrastructure-clock failure this task removes.
- **The metrics tests** assert exact series text; adding two series is a deliberate diff — update the
  assertions rather than relaxing them.
- **`migrate_cli` tests spawn the built binary**: they need `env!("CARGO_BIN_EXE_backend-rs")`, an
  explicit env (`SESSION_SECRET`, `APP_ENV=test`, `DATABASE_URL`), and a `current_dir` outside
  `backend-rs/` so a developer `.env` cannot silently supply values. The healthy-path test owns its
  database (`kanpad_cli_<pid>`, path-segment swap out of `DATABASE_URL` — see Step 2): asserting a
  fixed `applied: 2 pending: 0` against the shared tmpfs `kanpad` would be order-dependent flake
  (a container restart wipes the schema). skip+pass when `DATABASE_URL` is absent or unparsable,
  mirroring `health.rs::ready_200_when_db_up`.

### 7. Suggested AGENTS.md lessons (write these when the work lands)

1. **A catch-all `Err(_)` on a database read is an inverted lie detector** — it converted an outage
   into "empty database" for two different failure modes. Classify by MySQL error number (1146 =
   missing journal) and propagate everything else with its diagnostics.
2. **An eager startup probe must retry only transient classes — and must own its dial, not borrow
   the pool's.** Retrying 1045/1049/1251 wastes the startup budget and delays the real error;
   retrying `Io` absorbs a MySQL that is still booting. And the pool is the wrong vehicle for
   classification: sqlx's connect loop retries `ConnectionRefused` until `acquire_timeout`, then
   returns a causeless `PoolTimedOut` (`sqlx-core-0.9.0/pool/inner.rs:325-385`, upstream #3808/#3929)
   — a pooled probe cannot even see the failure it is probing for.
3. **Never interpolate a password into a connection URL.** Build `MySqlConnectOptions` from the parts;
   keep the URL for tooling and percent-encode there. `pa/ss` in a password currently turns into a
   silent `127.0.0.1:1` pool.
4. **Operator-facing hint text must only carry verified facts.** The first draft of the `Refused`
   hint claimed "compose publishes 3307→3306"; a grep proved the number appears nowhere in the tree
   (compose publishes 3306:3306; only the test container is 3337). A wrong hint compiled into the
   binary is worse than a generic one — treat hint constants like the CI grep gates: prove each one
   against the tree before it lands, and re-prove on every edit.
