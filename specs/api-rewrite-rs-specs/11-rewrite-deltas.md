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

## S2. Sessions, config, throttle, tests

* Sessions: keep `tower-sessions 0.14` + store-owned `tower_sessions.session`,
  `cycle_id()` on login **and** privilege change. Prod cookie uses `__Host-`
  prefix (`__Host-connect.sid`, `Secure + Path=/`, no `Domain`). Sweeper
  `continuously_delete_expired` spawns **only in `main.rs`**, never in
  `lib::create_app` (tests would leak a task + conn per test). Baseline must
  `CREATE SCHEMA tower_sessions` before `CREATE TABLE`.
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
