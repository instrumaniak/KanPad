# T006: BE-RS: MySQL & MariaDB SQL portability

## Goal

**Production runs MariaDB 10.6.28; every test and every dev box runs MySQL 8.4.** The Rust backend must
run on **both**, and today it does not:

1. **`POST /api/auth/login` returns a bodyless `500` in production** for every *correct* password
   (wrong passwords, unknown emails, `/health` and `/ready` are all fine). Root cause proven in §Notes 1:
   the session store's upsert uses the **MySQL 8.0.19+ row alias** (`... VALUES (?, ?, ?) AS new
   ON DUPLICATE KEY UPDATE data = new.data, ...`), which MariaDB 10.6 rejects with
   `ERROR 1064 (42000) ... near 'AS new ON DUPLICATE KEY UPDATE ...'`. `tower-sessions`' middleware turns
   that save failure into a 500 (`tower-sessions-0.15.0/src/service.rs:252-258`), which is why only the
   "successful login" branch fails.
2. **Nothing in the repo could have caught it.** `backend-rs/docker-compose.test.yml` and the CI workflow
   both pin `mysql:8.4`; `#[sqlx::test]` runs against that server only. There is no MariaDB leg anywhere,
   and the `AGENTS.md` note "MariaDB 10.6 staging proof (T02b AC-b)" was never closed.
3. **The portability rule was never written down**, so the next MySQL-8-only construct (another row-alias
   upsert, `utf8mb4_0900_*`, `INSERT ... RETURNING`) ships the same way.

End state, in one sentence: **every statement in `backend-rs/` is valid on both MySQL 8.0.19+ and
MariaDB 10.6+, that claim is enforced by a test leg that actually runs on MariaDB, and the rule is
documented where the next author will read it.**

## Depends On

None. T005 is done; its diagnostics kernel (`db.rs`) is reused by the verification steps but is not
modified by this task.

## Phase

0

## Critical

Yes — production login is broken until the binary is rebuilt with the fix.

## Spec References

Read first (in this order):

- `specs/api-rewrite-rs-specs/00-overview.md` — rewrite constitution / conventions.
- `specs/api-rewrite-rs-specs/30-db-schema.md` — schema source of truth: collation pin,
  `utf8mb4_unicode_ci`, `_sqlx_migrations`, deviation 11 (the `sessions` table lives in the app DB).
  **This task adds the supported-server matrix + the portable-SQL rule here.**
- `specs/api-rewrite-rs-specs/10-architecture.md` — §DB pool, §Sessions (store format, sweeper),
  §Config (`DATABASE_URL` charset + collation pin).
- `specs/api-rewrite-rs-specs/50-tasks.md` — T02/T03 (baseline + schema) and Phase 7 (T54–T67).
- `AGENTS.md` (repo root) — Rust test-harness ownership rules (`tests/api/helpers.rs` /
  `wire.rs`, explicit imports), Cargo cache paths, "operator-facing text must only carry verified facts",
  "a gate that has never executed is not a gate".
- `BACKEND-RS-ORCHESTRATION.md` — required `CARGO_HOME` / `CARGO_TARGET_DIR` / `DATABASE_URL` for every
  cargo command.
- `docs/data-migration-live.md` — the prod import runbook (its gates were run on MariaDB 10.6 staging;
  the runtime was still MySQL-8-only).

External references (verified 2026-11-02 — do not re-litigate):

- MariaDB KB, `INSERT ... ON DUPLICATE KEY UPDATE`: MariaDB supports the `VALUES(col)` function form; it
  does not implement MySQL 8.0.19's `AS new` row alias in any released version (targeted for 13.2 —
  MDEV-29919, still In Review). Reproduced locally (see §Notes 1).
- MySQL 8.0.20+ **deprecates** `VALUES(col)` in favour of the row alias (MariaDB does NOT deprecate it,
  so the rule is cross-server, not MySQL-driven) — so the portable form for this codebase is **neither
  alias nor `VALUES()`**: re-bind the post-insert values as extra placeholders (one statement, valid on
  both, no deprecation warning on either).
- `tower-sessions 0.15` behaviour (installed source, `.cargo-home/registry/src/*/tower-sessions-0.15.0`):
  - `Session::insert` is **in-memory** (`tower-sessions-core-0.15.0/src/session.rs:171, 108-137`).
  - `Session::cycle_id` hits the store (`store.delete`, `session.rs:845-870`).
  - the middleware's `save()` failure is converted to a **bodyless 500**
    (`service.rs:252-258`) — a save failure is NOT swallowed.
- sqlx 0.9 MySQL driver: `charset`/`collation` are sent as `SET NAMES utf8mb4 COLLATE ...`; MariaDB accepts
  `utf8mb4_unicode_ci`. Auth-plugin reality: MariaDB 10.6's default is `mysql_native_password` and it has NO
  `caching_sha2_password` plugin at all (added only in 11.4.9/11.8.4/12.1); MySQL 8.4's default is
  `caching_sha2_password` and `mysql_native_password` is **disabled by default** (removed in 9.0). The app
  must not depend on either plugin. Per-leg account setup: MariaDB 10.6 uses its default; MySQL 8.4 uses
  `caching_sha2_password` (or add `--mysql-native-password=ON` to the server only if a fixture truly needs
  it). Do NOT mandate `mysql_native_password` globally — that breaks the MySQL leg.

## Files to Create/Modify

### Create

- `backend-rs/tests/api/sql_portability.rs` — a suite that runs the store statements (session upsert,
  sweeper delete, a representative sample of feature writes) against the configured server and asserts
  they parse **and** round-trip. Runs on whatever `DATABASE_URL` points at, so the CI MariaDB job
  exercises the same code the MySQL job does.
- `backend-rs/tests/api/mariadb_login.rs` — **only if** the login e2e does not already cover the session
  save on the MariaDB leg (prefer extending the existing auth login test with a
  "login -> `/auth/me` 200 -> `sessions` row exists -> logout -> row gone" flow; add the file only when the
  existing suite cannot be parameterised).
- `docs/mariadb-compat.md` — short operator note: prod is MariaDB 10.6, what the supported matrix is,
  and the two-minute verification (version, login smoke, `sessions` row).

### Modify

- `backend-rs/src/common/session_store.rs` — **the fix**: replace the row-alias upsert in
  `save_with_conn` with the portable re-bind form; rewrite the now-false comment ("`AS new` is the
  modern form ..."), which is exactly the reasoning that caused the outage.
- `backend-rs/docker-compose.test.yml` — add a `mariadb` service (`mariadb:10.6`) beside `mysql:8.4`
  (profile or second service on `:3338`), so a developer can run the suite against production's flavour.
- `.github/workflows/backend-rs.yml` (repo root — GitHub ignores anything under `backend-rs/.github/`;
  T55 already relocated it) — add a `test-mariadb` job (service `mariadb:10.6`) running the session-store +
  auth/login + `sql_portability` suites at minimum. **Set `SQLX_OFFLINE=true` on the `prepare --check` step**
  (it is currently unset there) so the cache is validated against the repo, not the job's DB name.
- `backend-rs/.sqlx/` — **regenerated** (`cargo sqlx prepare`) because the session upsert text changes.
- `backend-rs/tests/api.rs` — `#[path = "api/sql_portability.rs"] mod sql_portability;` (+ `mariadb_login`
  if created), explicit, alphabetical.
- `specs/api-rewrite-rs-specs/30-db-schema.md` — new "Supported servers" section: MySQL **8.0.19+** and
  MariaDB **10.6+**, the portable-SQL rules (no row-alias upserts, no `utf8mb4_0900_*`, no
  `INSERT ... RETURNING`, no MySQL-only `JSON` operators, `VALUES(col)` only where both accept it or — as
  this task does — re-bind instead), collation/charset expectations on both, and the required test legs.
- `specs/api-rewrite-rs-specs/10-architecture.md` — §DB pool / §Sessions: note that prod is MariaDB 10.6
  and that the session store's SQL must stay portable.
- `BACKEND-RS-ORCHESTRATION.md` — the MariaDB test-container command + `DATABASE_URL` for the second leg.
- `docs/data-migration-live.md` — one line in the appendix: production is MariaDB 10.6.28 (not MySQL 8),
  and the login smoke (`POST /api/auth/login` -> 200 + `Set-Cookie`) is a post-deploy gate.
- `AGENTS.md` — lessons (see §Notes 5).

## Implementation Steps

TDD per step: make the failure reproduce first, then fix, then guard. Each step is independently
mergeable; do not interleave.

### Step 0 — Reproduce (before touching code)

```bash
docker run -d --rm --name kanpad-mariadb-probe -e MARIADB_ALLOW_EMPTY_ROOT_PASSWORD=1 mariadb:10.6
# wait for readiness, then create the EXACT baseline table (0001_baseline.sql:46-52)
```

Run, in order, against that server:

```sql
-- (a) what the binary ships today -> must FAIL on MariaDB
INSERT INTO `sessions` (`id`,`data`,`expiry_date`) VALUES ('abc','x',UTC_TIMESTAMP(6)) AS new
  ON DUPLICATE KEY UPDATE `data` = new.`data`, `expiry_date` = new.`expiry_date`;
-- expect: ERROR 1064 (42000) ... near 'AS new ON DUPLICATE KEY UPDATE ...'

-- (b) the portable replacement -> must succeed, twice (insert then update)
INSERT INTO `sessions` (`id`,`data`,`expiry_date`) VALUES ('abc','y',UTC_TIMESTAMP(6))
  ON DUPLICATE KEY UPDATE `data` = 'y', `expiry_date` = UTC_TIMESTAMP(6);
```

Record both outputs in the final report as the "before" evidence, then `docker rm -f kanpad-mariadb-probe`.

Also reproduce end-to-end on the live deployment (read-only, no credentials needed):

```bash
curl -sS -o /dev/null -w '%{http_code}\n' -X POST https://kanpad.raziur.com/api/auth/login \
  -H 'content-type: application/json' -d '{"email":"nobody@example.com","password":"x"}'   # 401
curl -sS -o /dev/null -w '%{http_code}\n' -X POST https://kanpad.raziur.com/api/auth/login \
  -H 'content-type: application/json' -d '{"email":"REAL","password":"wrong"}'               # 401
# the frontend's correct-password login -> 500, EMPTY body (not problem+json)
```

### Step 1 — P0: the portable session upsert

`src/common/session_store.rs::save_with_conn` (currently lines 104-126; the upsert is lines 114-126 and the
comment recommending the MySQL-only form is lines 111-113). Replace the statement and that comment:

```rust
        // Portable upsert. The 8.0.19+ row alias (`... VALUES (...) AS new ...
        // new.col`) does NOT parse on MariaDB 10.6 (production), and MySQL
        // deprecates `VALUES(col)` since 8.0.20 — so re-bind the post-insert
        // values as extra placeholders: ONE statement valid on both servers,
        // with no alias and no deprecated function.
        let session_id = record.id.to_string();
        sqlx::query!(
            "INSERT INTO `sessions` (`id`, `data`, `expiry_date`) VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE `data` = ?, `expiry_date` = ?",
            session_id,
            &payload,
            expiry_date,
            &payload,
            expiry_date,
        )
```

- `payload` is a `Vec<u8>` (non-`Copy`) -> bind `&payload` twice; `expiry_date` is `Copy`.
- Keep the `create()` id-collision handling, `delete`, `load`, and `delete_expired` untouched — verified
  portable (`DELETE ... WHERE id = ?`, `DELETE ... WHERE expiry_date < UTC_TIMESTAMP(6)`,
  `SELECT ... WHERE id = ? AND expiry_date > ?`).
- **Do not** switch to `REPLACE INTO` (delete+insert semantics) and **do not** add server-flavour
  detection — one portable statement is the whole point.
- Binding note: `payload` is `Vec<u8>` (non-`Copy`), so the new statement binds `&payload` **twice** (refs
  are `Copy` and sqlx encodes `&[u8]`); `expiry_date` is `Copy` and binds twice by value. The offline cache
  (`.sqlx/`) must be regenerated, otherwise `prepare --check` goes red.

Tests: extend the existing `save_upserts_the_same_row` (`#[sqlx::test]`) with an assertion that the
statement text contains neither `AS new` nor `new.` (a cheap tripwire that runs on every leg), and keep
the "same id -> one row, payload replaced" behaviour assertion.

### Step 2 — Portability audit of the whole SQL surface

Grep and read **every** statement, not just the session store:

```bash
grep -rn "sqlx::query" backend-rs/src | wc -l        # inventory
grep -rnE "AS new|\bnew\.|0900|RETURNING|JSON_TABLE|JSON_CONTAINS|->>|MEMBER OF|NOWAIT|SKIP LOCKED" \
  backend-rs/src backend-rs/migrations
```

Rules to confirm per statement:

| Construct | MySQL 8.0.19+ | MariaDB 10.6 | Verdict |
|---|---|---|---|
| `INSERT ... VALUES (...) ON DUPLICATE KEY UPDATE` (plain) | yes | yes | allowed |
| `... VALUES (...) AS new ... new.col` | yes | **no — 1064** | **forbidden** |
| `... ON DUPLICATE KEY UPDATE col = VALUES(col)` | yes (deprecated 8.0.20+) | yes | allowed only with a comment; prefer re-binding |
| `INSERT ... RETURNING` | no | yes | forbidden (breaks MySQL) |
| `CHECK` constraints, `ALTER TABLE ... RENAME INDEX` | yes / yes | yes (10.2+ / 10.5.2+) | allowed |
| CTEs, window functions | yes | yes (10.2+) | allowed |
| `utf8mb4_0900_*` collations | yes | no (≤11.4.4); accepted as an alias from 11.4.5 | forbidden on the 10.6 leg; scope the rule so it can be relaxed when the MariaDB leg moves past 11.4.4 |
| `JSON_TABLE` | yes | yes (10.6+) | allowed but avoid `JSON` columns entirely |
| `->` / `->>` | yes | **no** (13.1+) | forbidden on the MariaDB leg — do not use JSON operators at all |
| `INSERT IGNORE`, multi-row `INSERT`, `UTC_TIMESTAMP(6)` | yes | yes | allowed |

Deliverables: the table above in `30-db-schema.md`, plus a note on any statement kept for a reason
(e.g. `INSERT IGNORE` in `cards/store.rs:459`, the multi-row label/column seeds).

Also verify the **connection** side on MariaDB:

- `SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci` (the T66 pin) is accepted -> assert
  `@@collation_connection = 'utf8mb4_unicode_ci'` on both servers in `tests/api/collation.rs` (parameterise
  the existing assertion; never weaken it).
- the app account's auth plugin is a *per-leg* fact, not a universal rule (see External references): MariaDB
  10.6 default = `mysql_native_password` (no `caching_sha2_password` at all); MySQL 8.4 default =
  `caching_sha2_password` (`mysql_native_password` disabled by default). Document the per-leg setup and add
  `--mysql-native-password=ON` to the MySQL server only if a fixture truly needs it.
- migrations `0001_baseline.sql` / `0002_convert_to_utf8mb4.sql` already applied cleanly on MariaDB 10.6
  (`_sqlx_migrations` = 2 in prod) — keep them byte-identical (editing an applied migration is a
  checksum break, see `AGENTS.md`).
- Timestamp parity: `UTC_TIMESTAMP(6)` / `TIMESTAMP(6)` give microsecond precision on both legs and
  `TIMESTAMP` is stored UTC on both. Note the 2038 ceiling holds on both pinned legs (MySQL 8.4 and
  MariaDB 10.6); MariaDB 11.5+ raises it to 2106 — a future divergence if the leg ever moves.

### Step 3 — P1: a real MariaDB test leg (the guard that would have caught this)

1. `backend-rs/docker-compose.test.yml`: add

   ```yaml
     mariadb:
       image: mariadb:10.6
       environment:
         MARIADB_ALLOW_EMPTY_ROOT_PASSWORD: "1"
       ports: ["3338:3306"]
       tmpfs: [/var/lib/mysql]
       healthcheck:
         test: ["CMD", "healthcheck.sh", "--connect", "--innodb_initialized"]
         interval: 5s
         retries: 20
   ```

   (`#[sqlx::test]` creates a database per test -> the account must be able to `CREATE DATABASE`; root with
   an empty password is fine for a tmpfs container, mirroring the `mysql:8.4` service.)

2. CI: a `test-mariadb` job with the same steps as the existing test job, `DATABASE_URL` pointing at
   `mariadb:10.6`. Run at minimum: `cargo test -p backend-rs --lib common::session_store`,
   `cargo test -p backend-rs --test api` (the auth/login e2e included), and the new portability suite.
   Prefer a **matrix** (`mysql:8.4`, `mariadb:10.6`) over a second hand-written job so the two cannot
   drift.

3. `tests/api/sql_portability.rs`: for each representative statement (session upsert/load/delete/sweep,
   a multi-row `INSERT` (board default columns), `INSERT IGNORE` card-labels, a `CHECK`-constrained insert
   that must fail), execute it against the live `DATABASE_URL` and assert it parses. Skip+pass when
   `DATABASE_URL` is absent, mirroring `health.rs::ready_200_when_db_up`.

4. Prove the guard goes red: re-apply the `AS new` statement in the suite (temporarily) and confirm the
   MariaDB job fails, then revert. A guard that has never been red is not a guard.

### Step 4 — P1: the offline cache + CI checks

- `cargo sqlx prepare` after Step 1 (pinned sqlx 0.9 CLI) and commit `backend-rs/.sqlx/` changes.
- Set `SQLX_OFFLINE=true` on the `prepare --check` step (it is currently unset there — see the audit).
  sqlx 0.9 binds cache filenames to DB-qualified names (`kanpad.checklists` vs `kanpad_ci.checklists`); the
  cache must be validated against the repo, not against whichever server the job happens to run.
- Confirm the workflow is at the **repo root** `.github/workflows/` (a workflow under `backend-rs/.github/`
  is never discovered — that mistake already happened once) and that the new job actually starts.

### Step 5 — Docs + rule (same PR)

- `30-db-schema.md`: "Supported servers" + the portable-SQL table from Step 2 + "the session store is the
  reference implementation of a portable upsert".
- `10-architecture.md`: DB/session sections — prod flavour, portability rule, per-leg auth-plugin note.
- `BACKEND-RS-ORCHESTRATION.md`: the MariaDB container + `DATABASE_URL` for leg 2.
- `docs/data-migration-live.md`: prod is MariaDB 10.6.28; add the login smoke as a post-deploy gate.
- `docs/mariadb-compat.md`: operator-facing two-minute check (version, login, `sessions` row).
- `AGENTS.md`: lessons in §Notes 5.

### Step 6 — Deploy + verify (production recovery)

1. Rebuild the prod binary from `HEAD` into `release/backend-rs` (`release/` is **git-ignored**: it is
   the prod preparation/verification area, not a committed artifact. The deployed binary is dated
   2026-11-01 15:43 and predates the T005 commits — rebuild rather than patch. The proxy wrapper that
   spawns it is the committed `scripts/app-proxy.js`.)
2. Redeploy, restart the Node wrapper.
3. Verify, in this order:
   - log has **no** `failed to save session`;
   - `POST /api/auth/login` (real credentials) -> `200` + `Set-Cookie: __Host-kanpad.sid=...`;
   - `SELECT id, expiry_date FROM sessions;` -> one row per login;
   - `GET /api/auth/me` with that cookie -> `200`;
   - `POST /api/auth/logout` -> `204` and the row is gone.

## Constraints

- **Scope:** the session store fix, the portability suite, the test/CI plumbing, and docs. No feature-module
  behaviour changes. No migration file edits (checksum break).
- **No HTTP contract change:** route table, problem+json shapes, `openapi.json`,
  `frontend/src/lib/api/schema.d.ts` unchanged. (The 500's empty body is *not* part of the contract — it is the
  bug.)
- **No new dependency.** `sqlx`, `tower-sessions`, `time`/`chrono`, `tracing` are enough.
- **No credential in any log or body.** Never log the password, `SESSION_SECRET`, or
  `Settings::database_url()`.
- **The store's on-disk format is unchanged:** `sessions` = `id CHAR(22)` / `data BLOB` (MessagePack) /
  `expiry_date TIMESTAMP(6)` + `IDX_session_expiry_date`. Only the upsert *text* changes.
- **Do not weaken MySQL coverage to satisfy MariaDB**: both legs run the same suites.
- **Rust idioms:** edition 2024, `#![forbid(unsafe_code)]`, `missing_docs = "deny"`. Follow the workspace
  lints; run `cargo fmt` AFTER clippy fixes (a lint fix can introduce a format diff).
- **Commands** (per `BACKEND-RS-ORCHESTRATION.md`; root disk is tight — do not substitute paths):

  ```bash
  export CARGO_HOME=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-home
  export CARGO_TARGET_DIR=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-target-kanpad
  # leg 1 (MySQL 8.4, compose.test.yml -> :3337)        # leg 2 (MariaDB 10.6, compose.test.yml -> :3338, tmpfs)
  # MySQL leg:  mysql://root:dev_root_pw@127.0.0.1:3337/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci
  # MariaDB leg: mysql://root:@127.0.0.1:3338/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci
  ```

- **`#[sqlx::test]` owns a database per test**; never point the suite at the shared `kanpad` schema, and
  never blanket `DELETE`/`UPDATE` the `sessions` table (scope by `WHERE id = ?`).

## Acceptance Criteria

- [ ] On `mariadb:10.6`, the statement shipped before this task fails with `1064`; the statement shipped
      after it succeeds twice (insert -> update) with one row and a replaced payload.
- [ ] `POST /api/auth/login` against production returns `200` + a `Set-Cookie: __Host-kanpad.sid=...`
      header, and `sessions` gains exactly one row per login.
- [ ] The response is **not** a bodyless 500 anymore; a session-save failure is impossible on either server
      because every store statement is portable (asserted by the new suite).
- [ ] `tests/api/sql_portability.rs` passes on **both** legs, and the session-store tripwire asserts the
      upsert text carries neither `AS new` nor `new.`.
- [ ] A MariaDB 10.6 CI job exists and runs the session-store + auth + portability suites; it has been
      proven red by re-introducing the `AS new` upsert and green after reverting.
- [ ] `@@collation_connection` is `utf8mb4_unicode_ci` on both servers (the T66 pin still holds).
- [ ] The full suite is green on `mysql:8.4` **and** `mariadb:10.6`.
- [ ] `.sqlx/` is regenerated and `cargo sqlx prepare --check` (with `SQLX_OFFLINE=true`) is green.
- [ ] `cargo fmt --check` 0 · `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic` 0
      · `cargo deny check advisories bans licenses` ok · every CI grep gate passes.
- [ ] `openapi.json` and `frontend/src/lib/api/schema.d.ts` are unchanged.
- [ ] Docs landed: `30-db-schema.md` (supported servers + portable-SQL table), `10-architecture.md`,
      `BACKEND-RS-ORCHESTRATION.md` (MariaDB leg), `docs/data-migration-live.md` (prod flavour + login
      smoke), `docs/mariadb-compat.md`, `AGENTS.md` (lessons).
- [ ] The prod binary in `release/` (git-ignored build area) is rebuilt from `HEAD`, and the post-deploy
      checklist (login -> `/auth/me` -> logout) passes.

## Notes

### 1. Proven root cause (2026-11-02)

Production facts, read from phpMyAdmin on `raziurco_kanpad_rs_prod`:

- `SELECT version()` -> **`10.6.28-MariaDB`**.
- `SHOW TABLES` -> `sessions` **exists**; `_sqlx_migrations` = **2** (built by `migrate run`).
- `SELECT id, email, role FROM users` -> all three roles are `user` (so the `role_from_db` -> 500 theory is
  dead).
- `POST /api/auth/login` with a real email and a **wrong** password -> `401` (so the `SELECT` + row mapping
  work; the failure is strictly post-verify).
- The frontend's correct-password login -> `500` with an **empty body** (not problem+json — the fingerprint
  of a middleware-generated response).

Reproduced on a throwaway `mariadb:10.6` container with the exact baseline `sessions` DDL:

```
ERROR 1064 (42000) at line 1: You have an error in your SQL syntax;
check the manual that corresponds to your MariaDB server version for the right syntax to use near
'AS new ON DUPLICATE KEY UPDATE `data` = new.`data`, `expiry_date` = new.`expiry_date`' at line 1
```

The shipped statement is `src/common/session_store.rs:114-126`. The middleware that converts the failure
into a 500 is `tower-sessions-0.15.0/src/service.rs:252-258`:

```rust
if let Err(err) = session.save().await {
    tracing::error!(err = %err, "failed to save session");
    let mut res = Response::default();
    *res.status_mut() = INTERNAL_SERVER_ERROR;   // <- bodyless 500, replaces the handler's 200
    return Ok(res);
}
```

Why every other request looked healthy:

| Request | Handler result | `session.save()` called? | Observed |
|---|---|---|---|
| `/health`, `/ready` | — | no | 200 |
| login, unknown email | 401 | no (`is_modified()` false) | 401 |
| login, real user, wrong password | 401 | no | 401 |
| login, real user, correct password | 200 | **yes -> MariaDB 1064** | **500, empty body** |

`insert()` is in-memory and `cycle_id()` only issues `DELETE ... WHERE id = ?` (both portable), so the
first MariaDB-incompatible statement in a login is the final `save()` -> `create()` -> `save_with_conn()`.

Why it worked locally: `backend-rs/docker-compose.test.yml` and CI pin **`mysql:8.4`**, which accepts the
row alias. The data is identical in both environments; only the server flavour differs.

### 2. Why this is bigger than one statement

This is the first MySQL-8-only construct found, not necessarily the last. The general failure mode is:
**dev/CI and production ran different RDBMS flavours, and no test could observe the difference.** The fix
that matters long term is the MariaDB leg (Step 3), not the one-line SQL edit.

### 3. `VALUES(col)` vs re-binding

- `VALUES(col)` works on both, but MySQL 8.0.20+ deprecates it (server-side warning on every session save)
  and it is scheduled for removal.
- The row alias is MySQL-only (this outage).
- Re-binding the post-insert values as extra placeholders is valid on both, emits no warning on either,
  and keeps one round-trip. Prefer it for every future upsert; document it in `30-db-schema.md`.

### 4. `.sqlx/` cache caveat

sqlx 0.9 binds offline-cache filenames to **DB-qualified** query names (`kanpad.checklists` vs
`kanpad_ci.checklists`). The committed cache was generated against the dev/test DB. CI must validate with
`SQLX_OFFLINE=true` so the check compares "committed cache vs committed SQL", not "committed cache vs the
job's database name" (this exact mismatch already produced a false red once — see `AGENTS.md`).

### 5. Suggested `AGENTS.md` lessons (write these when the work lands)

1. **A test suite that only runs one RDBMS flavour cannot prove portability.** Dev/CI ran MySQL 8.4;
   production runs MariaDB 10.6. Every SQL statement must be exercised on **both** in CI, or a
   server-specific syntax error ships green.
2. **An ORM/driver's "modern" syntax may be server-specific.** `AS new` is the *recommended* MySQL 8.0.19+
   form — and a hard `1064` on MariaDB. A comment that justifies a construct by one vendor's docs is not
   portability proof; run it on the other server.
3. **A middleware can replace a 200 with a 500.** `tower-sessions` converts a store-write failure into a
   bodyless 500 (`service.rs:252-258`), so "the endpoint's own error handling" is not the whole error
   surface. When a 500 has **no body**, look above the handler — our problem+json kernel never emits that.
4. **Split the symptom before theorising.** "Unknown email -> 401, wrong password -> 401, correct password
   -> 500" localises the failure to post-verify in one probe. The first theory (missing `sessions` table)
   was disproven by `SHOW TABLES` in 30 seconds; the version string (`10.6.28-MariaDB`) was the real clue.
5. **Rebuild the release binary from `HEAD` before diagnosing prod.** The deployed binary predated the
   T005 commits, so "prod behaves differently from the tree" had two candidate causes at once.

### 6. Related but out of scope

The three prod-side issues found while probing (localhost `CORS_ORIGINS` leaking into the CSP, the
`scripts/app-proxy.js` header rewriting producing a single throttle bucket, and `422 text/plain`
extractor rejections bypassing problem+json) are tracked as **T007 — BE-RS: production proxy, CORS &
error-shape hardening**. Do not fold them into this task: T006 is the MariaDB portability fix and must
stay independently deployable.

## Lessons migrated from AGENTS.md (2026-11-10)

### T006 lessons (2026-11-03) — MySQL & MariaDB SQL portability

1. **A test suite that only runs one RDBMS flavour cannot prove portability.** Dev/CI ran MySQL 8.4; production runs MariaDB 10.6. Every SQL statement must be exercised on both in CI, or a server-specific syntax error ships green.
2. **An ORM/driver's "modern" syntax may be server-specific.** `AS new` is the recommended MySQL 8.0.19+ form — and a hard `1064` on MariaDB. A comment that justifies a construct by one vendor's docs is not portability proof; run it on the other server.
3. **A middleware can replace a 200 with a 500.** `tower-sessions` converts a store-write failure into a bodyless 500 (`service.rs:252-258`), so "the endpoint's own error handling" is not the whole error surface. When a 500 has no body, look above the handler.
4. **Split the symptom before theorising.** "Unknown email -> 401, wrong password -> 401, correct password -> 500" localises the failure to post-verify in one probe.
5. **Rebuild the release binary from HEAD before diagnosing prod.** The deployed binary predated the T005 commits, so "prod behaves differently from the tree" had two candidate causes at once.

