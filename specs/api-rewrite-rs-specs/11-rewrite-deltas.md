# 11 — Rewrite deltas (rationale record; applied to 10/20/21/30/50 on 2026-10-11 cleanup)

Status: rewrite, not line-port. This file records *why* the rewrite
diverges from Nest. The decisions below are already folded into
`10-architecture.md` / `20-api-contract.md` / `21-api-standard-shapes.md` /
`30-db-schema.md` / `50-tasks.md` — those files are self-consistent and
executors follow them directly. If any sentence here still conflicts with
those files, fix the disagreement — neither silently wins (README rule); this file must be corrected. Do NOT implement from this file.
Rationale sources: verified Axum 0.8 docs (`HandleErrorLayer` +
`TimeoutLayer` pattern, `FromRequestParts` extractor pattern), sqlx
`MySqlExecutor` docs, utoipa `compose_name` generics, OWASP
password-storage guidance (Argon2id primary, bcrypt legacy-only).

## A1. Crate layout: 3 files per feature, no repository traits

> Past-tense record: folded into `10-architecture.md` target tree + T03. Do not implement from the MUSTs below.

The pre-cleanup `10-architecture.md` described 6 files per feature
(`handlers/service/repository/model/dto/mod`) — a Nest translation that
causes `Boards↔Columns↔Cards` triangular imports. Folded into
`10-architecture.md` target tree; normative layout:

```
src/features/<name>/{routes.rs, store.rs, types.rs}
src/common/{ids.rs, ownership.rs, pagination.rs}
```

* `routes.rs` = handlers only (extract → store → `Result<Json<_>, AppError>`). No SQL.
* `store.rs` = free `async fn`s, concrete, no traits. Generic over executor:
  `async fn f<'e, E: MySqlExecutor<'e>>(ex: E, ...) -> Result<_, AppError>`
  so the same fn accepts `&MySqlPool` and `&mut Transaction`. Both implement
  the executor bound (verify `MySqlExecutor` vs `Executor<Database = MySql>` name at T03; use one form everywhere); no `async-trait` crate needed (edition 2021, RPITIT stable).
* `types.rs` = domain newtypes + DTOs together (see A2).
* `ownership.rs` = single owner of user-scoping helpers. Feature stores call
  it; features never import each other's stores.
* Mocks: pure position-math only. All DB behavior via `#[sqlx::test]` real DB.
  No `repository trait/impl` layer.

## A2. Newtypes (wire-compatible, compile-time mixing protection)

Plan uses raw `i32` for every id (`20-api-contract.md`). Rewrite:

```rust
struct UserId(i32); struct BoardId(i32); struct ColumnId(i32); struct CardId(i32);
// Email validation lives in `common::validation` + garde DTOs (no separate Email newtype — retired to avoid dual layers)
```

* `Path<BoardId>`, serde transparent, utoipa `ToSchema(value_type = i32)`.
* Wire names are `snake_case` per `21-api-standard-shapes.md:13-17`
  (`board_count`, `tag_ids`, `{label_id}`, `{target_column_id}`,
  `?project_id=`). Rust fields are already snake_case so no serde `rename`
  is needed except where noted in task DTOs.
* `last_insert_id()`: `u64 → i32::try_from`, overflow → `AppError::Internal`
  (never silent `as` cast).

## A3. Position logic: set-based SQL + row locking (not row-by-row)

Nest `cards.service.ts:120-193` loads all cards in a column and `Promise.all(save)`
(N+1 roundtrips, no locking, races on concurrent drags). Rewrite mandates
(set-based SQL; the old shift-range clamp port is retired):

* Same-column move = one `UPDATE ... SET position = position ± 1 WHERE
  column_id=? AND position BETWEEN ? AND ?`, then one point `UPDATE` for the
  moved row. Cross-column = close-gap range + open-gap range + point update.
* `SELECT ... FOR UPDATE` on the affected range first, inside a short tx
  (begin → 2–5 stmts → commit). Stable `(column_id, position)` lock order, locking `min(column)` first on cross-column moves. Without `UNIQUE(column_id, position)` gap-locks do not fully serialize inserts — fuzz proves single-node dense-order + no-loss invariants, not strict serializability.
* `T24b` fuzz extends to concurrent `tokio::join` fuzz (invariant oracle: dense 0..N + unique + no lost rows; exact-order assertion sequential-only), not only sequential 500.

## A4. Ownership in SQL (no find-then-check)

Nest `cards.service.ts:320-352` does `findOne + relations` then
`if board.user_id != userId → 403` (extra join, TOCTOU, 403-vs-404 oracle).
Normative: every store fn takes `UserId` and bakes it into `WHERE`:

```sql
... FROM cards JOIN `columns` ON ... JOIN boards ON ...
WHERE cards.id=? AND boards.user_id=?
-- miss → 404, never 403. No second query, no oracle.
```

`403` only for role denial (`REGISTRATION_ENABLED=false`, non-admin on admin
route) and `409` for state conflicts (e.g. deleting an active board where
the rewrite requires archived-first). Foreign/not-owned IDs are always `404`.
See `21-api-standard-shapes.md` (verb routes `archive/restore/permanent`
retired; `permanent` gone) and the `50-tasks.md` global 403-vs-404-vs-409 rule.

## E1. Timeout + middleware order (Axum-docs verified)

* `TimeoutLayer` alone never reaches `AppError::IntoResponse`. Normative:
  `HandleErrorLayer(handle_timeout)` **above** `TimeoutLayer`; handler checks
  `err.is::<tower::timeout::error::Elapsed>()` and returns **503** problem+json
  with `Retry-After` (not 504 — 504 is proxy semantics; origin uses 503/408).
  `T04` AC must assert `Content-Type: application/problem+json` on timeout.
* Order outermost→innermost is canonical in `10-architecture.md` Middleware order (secure-headers + RequestId outermost so 429/503 carry them; PropagateRequestId + served-by included; Governor is `route_layer` on `/api/*` only, not global — this shorthand omits those details, defer to 10). Rationale: `RequestId/Trace` outermost so early rejections are traced with
  an ID; `Compression` after `Governor` so 429s are not compressed.
* Problem shape: `type` = stable URI per error
  (`https://kanpad.dev/problems/validation-error`), not `about:blank`;
  `instance` always set (`"{METHOD} {path}#{x-request-id}"`).
* Deletes uniform `204 No Content`. Creates return `201` + `Location` header.
* `Page<T>` generics work with utoipa (`compose_name` → `Page_BoardResponse`)
  but require explicit `T: ToSchema` bound — `T40a` must add alias coverage or
  export fails to compile. `export-openapi` and `create-admin` are separate
  clap subcommands; export must not boot pool/sessions.

## E2. Errors + validation + secrets

* `AppError`: `thiserror` only. `anyhow` only at `main()` boundary.
  `Internal` is a unit struct; server logs `tracing::error!(?err)`, client gets
  generic 500. Map `PathRejection`/`QueryRejection` (Axum 0.8 splits them) to
  400 problem+json, not just `PathRejection`.
* Validation: depend on `axum-valid` (garde feature) — `Valid(Json<T>)` /
  `Valid(Query<T>)` — instead of hand-rolled `ValidatedJson`. Keep only
  `PaginationParams` (strict 400, no clamp) + `AuthSession` custom extractors.
  `AuthSession: FromRequestParts`, rejection = `Response` (problem+json 401).
* Secrets: `secrecy::SecretString` + `zeroize` for `SESSION_SECRET`/DB password.
  Length checks on **bytes** (`secret.as_bytes().len() >= 32`;
  bcrypt-verify path `password.as_bytes().len() <= 72` — garde `length` counts
  chars, bcrypt truncates bytes; multibyte would silently collide).
* `#[tracing::instrument(skip(password, secret))]`; redact `authorization`,
  `cookie`, `set-cookie` from spans.

## S1. Passwords: Argon2id with bcrypt upgrade path (no resets)

OWASP 2024–2026: Argon2id primary, bcrypt legacy-compatible only (72-byte
cap, no memory hardness). Folded into `10-architecture.md` pins and
`T10/T11` (custom byte-length password cap, not garde `length`):

* New hashes: `argon2` crate, Argon2id `m=19MB, t=2, p=1`.
* Login: try Argon2 verify → on miss try bcrypt verify (old cost-10
  `$2a$/$2b$` from `auth.service.ts:41-42`) → on bcrypt success rehash to
  Argon2 + update row in the same request. Document in `CONTRACT-CHANGES.md`.
* Gate hashing cost via `Settings { hash_cost }`; tests use minimal cost
  (default Argon2 cost × auth suite = minutes otherwise).

## A4. Naming: `APP_ENV` + `kanpad.sid` (no Node artifacts)

> Rationale record. **PLANNED (2026-10-22) — not yet implemented.** To be
> folded into `10-architecture.md` (Config + Sessions) by T54. Do not
> implement from the MUSTs below.

The rewrite's own constitution (`00-overview.md` Amendment 10) forbids
carrying NestJS-isms into `backend-rs/`. Two survived the first pass because
they look like "parity" but never were on the wire contract:

* **`NODE_ENV`** — a **Node.js/npm** convention (`npm` sets it,
  `react-scripts` branches on it). It has no meaning in a Rust binary. The
  Rust-web convention is `APP_ENV` (`actix-web`/`axum` community templates:
  `APP_HOST`/`APP_PORT`/`APP_ENV`; `sqlx` uses `DATABASE_URL`; `tracing` uses
  `RUST_LOG`). Rejected `RUST_ENV` (reads as "the compiler's env", collides
  conceptually with `RUST_LOG`), `KANPAD_ENV` (the config table is
  unprefixed-by-design, `DB_*`/`DATABASE_URL` aside) and `PROFILE` (cargo's
  vocabulary). The **Nest** service keeps `NODE_ENV` — separate container,
  separate runtime.
  * **The rename must not fail open.** `APP_ENV` defaults to `development`,
    so a deployment that still exports `NODE_ENV=production` and has not
    added `APP_ENV` would silently boot as development — losing `Secure`
    cookies, the `__Host-` prefix, prod HSTS and the deny-all CORS default.
    `Settings::load()` therefore **fails** when `NODE_ENV` is present, naming
    `APP_ENV` explicitly. Same discipline as the DB-name screen, the charset
    requirement and the `HASH_COST` prod floor: never degrade silently.
* **`connect.sid`** — **express-session's default**, copied verbatim from the
  Nest source (`backend/src/main.ts:39`). A Rust service must not name its
  session cookie after an Express middleware; the name misidentifies the
  implementation to every operator, browser extension and log scraper. New
  name: `kanpad.sid` (dev/test) / `__Host-kanpad.sid` (production).
  * The **`__Host-` prefix is kept** (host-only binding ⇒ no subdomain
    fixation; mandates `Secure`; mandates `Path=/`) and stays
    **environment-conditional** because `__Host-` mandates `Secure`, which
    browsers only transmit over HTTPS — the plain-HTTP staging box in
    `docs/data-migration-live.md` needs the un-prefixed name.
  * The `__Host-` invariant (`Secure` + `Path=/` + no `Domain`) becomes
    **test-locked**: a future `.with_domain(...)` must break a test, not
    production. A no-`connect.sid` regression guard is added alongside.

Neither change touches the wire contract (the cookie name is not in
`openapi.json`; `APP_ENV` is not on the wire at all), so neither needs an
`openapi.json` / `schema.d.ts` regeneration.

## E4. CSRF defence-in-depth + CSP on the SPA-serving origin

> Rationale record. **PLANNED (2026-10-22) — not yet implemented.** To be
> folded into `10-architecture.md` (CORS + Middleware order) by T56. Do not
> implement from the MUSTs below.

* **Cross-origin check on mutating requests — the Go 1.25
  `CrossOriginProtection` algorithm.** The API authenticates with an ambient
  cookie, so a browser attaches the session cookie to cross-site requests.
  `SameSite=Lax` blocks it for cross-site `POST`/`PUT`/`PATCH`/`DELETE` —
  but that is a **browser** control: it does nothing for a non-browser client
  and nothing at all if a deployment relaxes `SameSite` to `None`. So a
  server-side check is required.
  **Decision (re-decided 2026-10-22): adopt the algorithm, not a bespoke
  rule.** The reference is Filippo Valsorda's research behind Go 1.25's
  `net/http.CrossOriginProtection`; `tower-http` 0.7 ships it verbatim as
  `CsrfLayer` (feature `csrf`): safe methods always allowed;
  `Sec-Fetch-Site: same-origin|none` allowed; `Sec-Fetch-Site: cross-site|
  same-site` allowed only if the `Origin` is trusted; with no
  `Sec-Fetch-Site`, no `Origin` means allowed (non-browser), and an `Origin`
  whose authority equals the request's effective host means allowed.
  **Why the algorithm and not the rule this section originally proposed**
  ("an `Origin` that is present must be in the CORS allowlist"): browsers
  send `Origin` on **every** mutating request, same-origin included, and the
  Rust service serves the SPA itself. Under the original rule an operator
  who forgets `CORS_ORIGINS` in production gets **every write 403'd** — the
  whole mutating API, silently broken, with a CORS-shaped error that points
  nowhere near the cause. The algorithm's `Origin`/`Host` fallback removes
  that cliff: a same-origin SPA works with an **empty** allowlist.
  **Why not a CORS feature** (verified against `tower-http` 0.6.11 and
  0.7.x): `CorsLayer` does **not** reject a request from a non-allowlisted
  origin — for a non-preflight request it forwards the request to the inner
  service regardless and merely omits the `Access-Control-Allow-Origin`
  response header (the *browser* then blocks the response client-side), and
  0.6.11 has **no `enforce_origin` option** at all. Server-side enforcement
  needs its own layer; 0.7 finally ships one. It must sit **inside**
  `CorsLayer`: `CorsLayer` short-circuits every `OPTIONS` request
  (preflight), so a preflight never reaches the check.
  **Three guardrails** (why this is a task and not a one-line change):
  (i) **the allowlist is canonicalized at boot** — `CsrfLayer` matches
  origins byte-for-byte and its own docs warn that a non-canonical entry
  *"will silently never match"*, so each entry is parsed to canonical form
  (lowercase scheme+host, the scheme's default port stripped, no trailing
  slash, no path/query/fragment, no opaque origin) and a non-canonical entry
  is a **boot error naming the offending entry**; one canonical list feeds
  `CorsLayer::allow_origin`, `CsrfLayer::add_trusted_origin` and the CSP's
  `connect-src`; (ii) the rejection is rendered through the shared
  `problem_response` + the **existing** `AppError::Forbidden` mapping, so
  **no new problem-type URI** is introduced — the change is contract-neutral;
  (iii) the `Host`-rewrite proxy caveat is recorded in the runbook.
  `SameSite=Lax` + the `__Host-` cookie remain the first line of defence.
* **CSP.** The Nest-era "no CSP (`csp:false`)" decision was correct *for
  Nest* — the Nest service never served the SPA. The **Rust** service does
  (`ServeDir public/` + SPA fallback), so it is the origin that loads the
  frontend's scripts and styles. Shipping the SPA with no CSP would be a
  regression in the rewrite's security posture. Normative CSP:
  `default-src 'self'; script-src 'self' 'sha256-<hash>'; style-src 'self'
  'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:;
  connect-src 'self' <CORS allowlist origins>; frame-ancestors 'none';
  base-uri 'self'; form-action 'self'`.
  **Two corrections, verified against the frontend on 2026-10-22:**
  1. **`connect-src` MUST include the CORS allowlist origins.** The dev
     topology is cross-origin (SPA on `:5173`, API on `:3001` —
     `frontend/src/lib/api/client.ts:67`, `frontend/vite.config.ts:27`), so
     `connect-src 'self'` alone would have the browser block every API call.
     The allowlist is the right source: it is exactly "the origins allowed
     to call this API".
  2. **`script-src` MUST allow the inline theme bootstrap.** `frontend/
     index.html:12-21` contains an inline `<script>`; `script-src 'self'`
     alone blocks it (the page renders, but the stored theme is ignored and
     there is a flash of the wrong theme). It is allowed by **CSP hash**
     (`'sha256-<base64>'` over the exact inline script bytes) — a hash is
     appropriate because the inline script is a **fixed literal**, not a
     per-request value. The hash is **computed at boot from the served
     `public/index.html`** and covered by a test, so it cannot drift
     silently. A per-request **nonce** stays out of scope (the static SPA
     fallback does not template the HTML) — the earlier "no hash" rationale
     was wrong because it assumed the SPA had no inline script at all.
  `style-src 'unsafe-inline'` is required (12 inline `style={{…}}` React
  props); `script-src` keeps neither `unsafe-inline` nor `unsafe-eval`.

## E5. Observability, build chain, and the "gate that never ran"

> Rationale record. **PLANNED (2026-10-22) — not yet implemented.** To be
> folded into `10-architecture.md` (Observability, Config, DB pool) and
> `54-rust-idiom-hardening.md` (B5, B8, B10, B11) by T55/T57/T60/T61/T62.
> Do not implement from the MUSTs below.

* **The CI workflow never executed.** It lived at
  `backend-rs/.github/workflows/ci.yml`, and GitHub discovers workflows
  **only** at the repo root's `.github/workflows/`. Every gate in it (fmt,
  clippy, the `unwrap` grep gate, `prepare --check`, `cargo audit`/`deny`,
  the openapi diff, the docker build) was therefore unverified — and when the
  gate was finally replayed by hand, the repo's own `unwrap`/`expect` grep
  gate **failed on three sites**. Lesson: *a gate that has never executed is
  not a gate*; a green pipeline badge is worth more than a perfect gate
  definition, and the definition must be validated by a red run.
* **Observability.** `tracing-subscriber` gains `json` (production formatter);
  a `/metrics` endpoint (Prometheus text) exposes request rate/duration, pool
  saturation, throttle rejections and process metrics. The `route` label is
  the **matched pattern**, never the raw path (cardinality) — via axum's
  `MatchedPath` extension, which needs the `matched-path` feature.
  **Three corrections (verified 2026-10-22):** (1) sqlx 0.8.6 has **no
  `PoolMetrics` type and no `metrics` feature** (and sqlx 0.9 adds none
  either) — the pool gauges come from
  `Pool::size()` / `Pool::num_idle()`, and the acquire-duration series is
  **our own** `acquire_timed()` helper; (2) `/metrics` must **not** be on the
  public API port (axum's own Prometheus example uses a second server for
  exactly this reason) — it gets its own listener, enabled by an optional
  `METRICS_PORT` that is **unset = no endpoint at all**; (3) a scrape must
  not record itself (no feedback loop).
* **Build chain.** `[lints]` (with `missing_docs = "deny"` — the doc
  discipline becomes a build gate, not a convention), `[profile.release]`
  (LTO + `strip`; **no** `panic = "abort"` — axum/tower catch panics per task
  and aborting would kill the process instead of returning a 500),
  `rust-toolchain.toml` (local == CI == Docker), a committed `deny.toml`, and
  **edition 2024** (verified free of blockers: no `unsafe`, no `gen`, no
  `if let … else` temporaries).
  **Four corrections (verified 2026-10-22):** (1) `edition = "2024"`
  implies **`resolver = "3"`** → cargo's Rust-version-aware resolver, so the
  dependency set can change (crates may be *downgraded* to versions that
  still support our MSRV) — the `Cargo.lock` diff must be **reviewed line by
  line**; (2) the `deny.toml` policy is **derived from a first failing run**,
  not guessed (a naive `multiple-versions = "deny"` trips over the duplicate
  versions every real tree has, and the license allow-list must enumerate the
  tree's actual SPDX set); (3) the toolchain pin moves to **`1.99.1`** — the
  current stable (2026-10-13), **not** the `1.99.0` the first draft named —
  and it clears the MSRV ceiling of the whole target dependency set (sqlx
  0.9's `1.94.0` is the highest; everything else is <= 1.88). The Dockerfile
  base moves to `rust:1.99-bookworm` **in the same PR**: the Dockerfile and
  the toolchain move together, never one alone. The pin is justified as
  "match the Dockerfile, and match the dependency set's MSRV ceiling" — not
  "latest" — and the *reason it changes now* is the window: the toolchain
  pin gates `edition = "2024"` → `resolver = "3"` → the entire dependency
  set, which is cheap before the data migration and expensive after it;
  (4) `secrecy`'s `serde` feature LOOKS removable and is **not** —
  `Settings` is `Deserialize`d by `envy` and holds two `SecretString`
  fields, and `SecretString: Deserialize` only exists with the feature.
* **Dependency hygiene (2026-10-22, `54` Part C).** The manifest carries
  **three dead direct dependencies** — `axum-valid`, `utoipa-axum`, `zeroize`
  (a fourth, `time`, was a **false positive** of the audit's first pass:
  `src/lib.rs:155` uses `Expiry::OnInactivity(time::Duration::hours(24))` and
  that `time::` is the `time` **crate**, not `std::time` — `cargo tree -i time`
  lists `backend-rs` as a direct dependent, and `tower_sessions::Expiry`
  *requires* it) — and **two dead `tower-http` features** (`timeout`, `limit`). The
  expensive one is `axum-valid`: it is unused (the code hand-rolls
  `ValidJson`/`ValidQuery` because `axum-valid 0.20` binds garde 0.20 while
  the DTOs derive garde 0.22) yet it drags a **second, complete axum 0.7.9**
  into the binary — plus `validator` (a second validation framework, pulled
  by `axum-valid 0.20`'s default features) and a second `garde`. The
  decision is **remove, not upgrade**: `axum-valid 0.24+` now supports
  garde 0.22 + axum 0.8, but its `Garde<E>` extractor still carries the
  `Context: FromRef<State>` bound with no blanket `FromRef<S> for ()`, so it
  would not be a drop-in — and the *stale comment* that justifies the
  dependency is corrected so the next contributor does not re-add it.
* **Session-store ownership (2026-10-22, `54` Part C).** The session store
  is the **hinge** of the dependency program.
  `tower-sessions-sqlx-store 0.15.0` requires `sqlx ^0.8.0` AND
  `tower-sessions-core ^0.14.0`; `tower-sessions 0.15.0` requires
  `tower-sessions-core =0.15.0`. So today **neither** dependency major is
  reachable — a second sqlx or a second `tower-sessions-core` is a compile
  error — and upstream's dependabot bump of the store to sqlx 0.9 has been
  open since 2026-05-21 against a crate whose last release was 2025-01-01.
  **Decision: own the store in-tree** (~200 LOC implementing
  `tower_sessions_core::SessionStore` over the app pool), keeping the
  on-disk format IDENTICAL (the existing `sessions` DDL + MessagePack) so
  the change is a code-ownership change with no data implication and the
  runbook's schema-drift gate stays green. Every alternative was searched:
  `tower-sessions-ext` and `tower-sessions-sqlx-store-chrono` both still pin
  sqlx 0.8; `axum_session` + `axum_session_sqlx` supports sqlx 0.9 but
  replaces the whole session middleware; a vendored `[patch.crates-io]` fork
  trades a version pin for permanent supply-chain surface. The in-tree store
  also removes the *reason* for `sqlx/time` being enabled — which is why
  every query carries `AS "col: chrono::NaiveDateTime"` — so those
  overrides delete with it. A precedent exists: `darksailor.dev/kobo-shelf`
  (2026-07-27) replaced the same crate for the same reason and moved to
  sqlx 0.9 in the same change.
* **Concurrency.** The move transaction's bounded retry extends to MySQL
  **deadlock** (`1213`) and **lock-wait timeout** (`1205`), and the move tx
  runs at `READ COMMITTED` (gap-lock deadlock reduction — the spec already
  noted this as a candidate). Pool sizing becomes env-driven.
  **Three corrections (verified 2026-10-22 against the MySQL 8.4 manual):**
  (1) match the retry on the error **NUMBER**, not the SQLSTATE — `1213` is
  `40001` but `1205` is `HY000`, so a `40001`-only matcher misses every
  lock-wait timeout; (2) `1205` rolls back **only the current statement** by
  default (the transaction survives half-applied), so the retry must
  explicitly `ROLLBACK` the whole transaction; (3) `SET TRANSACTION
  ISOLATION LEVEL READ COMMITTED` must be issued **before** `BEGIN` —
  unscoped it applies to the next transaction only and MySQL rejects it
  inside one with `ERROR 1568 (25001)`. And the benefit must not be
  overstated: `READ COMMITTED` *"greatly reduces the probability of
  deadlocks, but they can still happen"* — the retry remains required.
* **Image.** `.dockerignore` (the build context currently ships `target/`),
  a `HEALTHCHECK` (the app already exposes `/health`), and digest-pinned
  base images.

## S2. Sessions, config, throttle, tests

* Sessions: keep `tower-sessions` (**0.15** as of 2026-10-22; the store is now
  **in-tree** — `src/common/session_store.rs` — see `54` Part C §C4) +
  store-owned `sessions` **in the app
  database** (`.with_schema_name(<current DB>)` + `.with_table_name("sessions")`),
  `cycle_id()` on login **and** privilege change. Prod cookie uses `__Host-`
  prefix (`__Host-kanpad.sid` — the Express-derived `connect.sid` name is
  retired; see A4, `Secure + Path=/`, no `Domain`). Sweeper
  `continuously_delete_expired` spawns **only in `main.rs`**, never in
  `lib::create_app` (tests would leak a task + conn per test). The baseline
  creates the `sessions` table before the app tables' consumers, in the same
  database (no separate `CREATE SCHEMA`).
* Deleted/demoted users keep a valid session until expiry under
  trust-without-DB-lookup (`10-architecture.md:79`). Accepted risk — record in
  `CONTRACT-CHANGES.md`, or re-lookup on `me`/admin routes only.
* Config: `dotenvy + envy` (or manual `std::env`) instead of `config` crate
  (~30 transitive deps for ~10 keys). Same precedence
  (real env > `.env` > defaults), `Settings::load()` fails fast.
* Throttle: `tower-governor` is per-process memory — correct for single `:3001`,
  N× limit with 2+ replicas. Note Redis-backed governor as the scale-out
  prerequisite; `X-Forwarded-For` honored only from known proxies.
* Tests: `[[test]] name = "api" path = "tests/api.rs"` lives in `T03` (without it
  `--test api <filter>` fails). `#[sqlx::test]` per-test DB for migration
  tests; resource suites share one DB + tx rollback (per-test `CREATE DATABASE` on
  MySQL is minutes at ~150 tests).
