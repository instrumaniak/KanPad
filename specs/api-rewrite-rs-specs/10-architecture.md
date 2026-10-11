# 10 — Architecture (Axum 0.8 + sqlx 0.8 + MySQL, single crate)

Consensus 2026 for ~6–8 resources: **one crate + module-per-feature (vertical slices) + small shared kernel**. No workspace until >20k LOC. No NestJS DI container — replacement is `AppState{pool,settings,sessions} + FromRef + constructors`. No repository traits: feature stores are free `async fn`s generic over `MySqlExecutor` (see `11-rewrite-deltas.md A1`).

## Pinned versions (review-locked)

| Crate | Version | Note |
|---|---|---|
| `axum` | `0.8` | Path syntax is `{param}` (was `:param` in 0.7). All routes use `/api/boards/{id}` style. |
| `sqlx` | `0.8` | **NOT 0.9.** Pinned until `tower-sessions-sqlx-store` supports 0.9 (0.15.0, latest, depends on `sqlx ^0.8` + `tower-sessions-core ^0.14`). Alternatives (opt-in): implement `tower_sessions::SessionStore` in-repo (~80 lines over `MySqlPool`) to unblock sqlx 0.9, or the maintained `tower-sessions-ext-sqlx-store` fork — documented but not default. |
| `tower-sessions` | `0.14` | Cookie sessions. **0.15 exists but is incompatible**: the sqlx store depends on `tower-sessions-core ^0.14`. |
| `tower-sessions-sqlx-store` | `0.15`, feature `mysql` | Compiles against sqlx 0.8 + tower-sessions-core 0.14 only — this is what pins both. Default schema/table = `tower_sessions.session`; custom names supported (MySQL). |
| `tower` | `0.5` | `Timeout`, `ServiceBuilder`. |
| `tower-http` | `0.6` | Features: `trace,cors,timeout,request-id,compression,set-header,limit`. |
| `tower-governor` | pinned at T03 (exact version recorded in `Cargo.toml` + `Cargo.lock` AND written back into this table as a spec amendment in the same PR; must match `tower 0.5` / `axum 0.8` — no floating `current`. CI builds `--locked --offline`) | Rate limiting (see below). Needs `connect_info` + trusted-proxy-gated `X-Forwarded-For`. |
| `utoipa` | `5` | + `utoipa-axum 0.2` (`OpenApiRouter::routes(routes!(handler))`) + `utoipa-swagger-ui` (matching utoipa 5 line). |
| `tokio` | `1` | **Trimmed features:** `macros,rt-multi-thread,signal,time` (+ transitively `net` via axum). NOT `full`. |
| `argon2` | current | Argon2id for **new** hashes (`m=19MB, t=2, p=1`). |
| `bcrypt` | current | Legacy-only: verifies pre-cutover cost-10 `$2a/$2b$` hashes; successful bcrypt login rehashes to Argon2id in the same request. Never mints new bcrypt hashes. Password byte cap `<= 72` enforced by a custom validator (garde `length` counts chars; bcrypt truncates bytes). |
| `garde` | current | `Validate` derive on all input DTOs (via `axum-valid`). |
| `axum-valid` | current | `Valid(Json<T>)` / `Valid(Query<T>)` extractors (garde feature) — replaces hand-rolled `ValidatedJson`/`ValidatedQuery`. Only `PaginationParams` + `AuthSession` stay custom. |
| `secrecy` + `zeroize` | current | `SecretString` for `SESSION_SECRET`/DB password; byte-length checks. |
| `clap 4` + `dialoguer`/`rpassword` | current | `serve/migrate/create-admin/export-openapi` subcommands (see below). |

Offline cache: `.sqlx/` checked in; refresh with `cargo sqlx prepare --workspace --all -- --all-targets` (single crate — the flags are no-ops but this form is canonical everywhere, per `00-overview.md` Amendments). CI checks with `cargo sqlx prepare --check --workspace --all -- --all-targets` (T03 proves the flag form runs). `sqlx.toml` sets `database-url-var`.

## Target tree

```
backend-rs/
├── Cargo.toml              # axum 0.8, tokio{macros,rt-multi-thread,signal,time}, sqlx 0.8{mysql,runtime-tokio-rustls,chrono,migrate,macros}, tower 0.5, tower-http 0.6{trace,cors,timeout,request-id,compression,set-header}, tower-sessions 0.14 + tower-sessions-sqlx-store 0.15{mysql}, tower-governor, argon2, bcrypt (legacy-verify only), garde, axum-valid (garde feature), secrecy, zeroize, utoipa 5 + utoipa-axum 0.2 + utoipa-swagger-ui, serde/json, chrono, time 0.3 (only for tower-sessions `Expiry`/`Duration` construction — app time stays chrono/NaiveDateTime), dotenvy, envy, tracing, thiserror, anyhow(main boundary only), clap 4 + dialoguer/rpassword
├── sqlx.toml               # database-url-var
├── .gitattributes          # `*.sql text eol=lf` FIRST (before any migration is committed — CRLF changes sqlx SHA-384 checksums)
├── .sqlx/                  # checked-in offline query cache (cargo sqlx prepare -- --all-targets)
├── migrations/             # 0001_baseline.sql (squash of 10 TypeORM + tower-sessions DDL) + 0002_*.sql forward-only
├── src/
│   ├── lib.rs              # pub mod …; create_app(state)->Router; build_state() — enables in-process tests. NEVER spawns the session sweeper (only main.rs does).
│   ├── main.rs             # #[tokio::main]: clap `serve/migrate/create-admin/export-openapi`; dotenvy, Settings::load, tracing, MySqlPool, optional migrate, serve + graceful shutdown + sweeper AbortHandle
│   ├── config.rs           # Settings struct (dotenvy + envy, KANPAD__… nesting), validated at boot
│   ├── state.rs            # Clone AppState{pool: MySqlPool, settings: Arc<Settings>} (+ session store handle)
│   ├── error.rs            # AppError(thiserror-only) -> IntoResponse problem+json; RowNotFound→404, 1062→409, pool acquire→503, timeout Elapsed→503+Retry-After
│   ├── extract.rs          # PaginationParams (strict) + AuthSession extractors; PathRejection+QueryRejection→400 mapping (validation itself is axum-valid Valid<T>)
│   ├── db.rs               # MySqlPoolOptions{max 10-20,min 1,acquire 5s,idle 600s,lifetime 30m,test_before_acquire true}, Migrator, health
│   ├── docs.rs             # ApiDoc (utoipa) + SwaggerUi mount, openapi.json export
│   ├── admin.rs            # create-admin clap subcommand (dialoguer/rpassword, guards, exit codes)
│   ├── common/{ids.rs,ownership.rs,pagination.rs}  # newtype IDs, single owner of user-scoping helpers, Page<T> (rewrite of card-summary.helpers.ts lives in the cards feature + common fragments, no copy-paste per service)
│   └── features/{auth,projects,boards,columns,cards,labels,checklists,notes,tags}/
│       └── {routes.rs,store.rs,types.rs}
├── tests/api.rs            # single integration-test target (`[[test]] name="api" path="tests/api.rs"` declares `mod` children below; helpers.rs is a module, not its own binary)
├── tests/{api/{helpers.rs,auth.rs,boards.rs,columns_cards.rs,…},fixtures/{users.sql,projects.sql,boards.sql,columns.sql,cards.sql,…}}
└── frontend/src/lib/api/{schema.d.ts (generated),client.ts (hand)}  # typegen side, see 40-typegen.md
```

Per-feature file roles (3 files per feature; the 6-file Nest translation is retired):

| File | Rule |
|---|---|
| `routes.rs` | Handlers only: extract (`State,Path,Query,Valid,AuthSession`) → store → `Result<Json<…>,AppError>`. No SQL. `pub fn router()->Router<AppState>`; no `with_state` inside (caller applies once). |
| `store.rs` | Free `async fn`s, concrete, no traits. Generic over executor: `async fn f<'e, E: MySqlExecutor<'e>>(ex: E, …) -> Result<_, AppError>` so the same fn accepts `&MySqlPool` and `&mut Transaction`. No `async-trait` needed. Only SQL + ownership + position logic + transactions live here. |
| `types.rs` | Domain newtypes + DTOs together. In: `Deserialize+garde::Validate+ToSchema`. Out: `Serialize+ToSchema`. Newtype IDs (`UserId/BoardId/…`, `ToSchema(value_type = i32)`); `skip_serializing_if` mirrors Nest null-vs-omitted (`description:null` emitted, `checklist_progress` absent when empty). |

## Key patterns

* **State/DI:** `#[derive(Clone)] AppState`; `MySqlPool` already `Arc`-interned. Feature routers generic over `Router<AppState>`. Composition root = `build_state()`. Cross-feature rule: feature stores call `common::ownership` + `common::card_summary` + `common::label_seed` + `common::validation` helpers; features never import each other's stores (kills the `Boards↔Columns↔Cards` triangle). FKs are `Option<i32>` / newtype IDs (`Option<UserId>` etc. — never `i64`; schema is `INT`↔`i32`). Store fns are generic over executor: `E: Executor<'e, Database = MySql>` if `MySqlExecutor` is unavailable in sqlx 0.8 — verify the bound name at T03 and use one form everywhere. Newtypes derive transparent sqlx `Type` (`Encode`/`Decode`) + serde transparent + `ToSchema(value_type = i32)`; bind via the newtype (no `.0`-at-every-site fallback to raw `i32`).
* **Hexagonal, pragmatic:** `routes → store → MySQL`. Web-error rendering (`AppError → IntoResponse`) lives in `error.rs`; the error enum core carries no `axum` state beyond the `IntoResponse` impl (pragmatic coupling — do not claim "domain has zero axum dep" unless the core is split). Domain has no `axum` extractors. One ownership-check helper per aggregate. Mocks: pure position-math only (`clamp_position(requested, len)` + gap-computation helpers — the named unit subjects after the A3 set-based retirement); all DB behavior via `#[sqlx::test]` real DB.
* **Error:** `enum AppError{NotFound,Unauthorized,Forbidden,Conflict,Validation(garde::Report),Json(JsonRejection),PathRejection,QueryRejection,Db(sqlx::Error),Timeout,Internal}` (`thiserror`-only; `anyhow` only at the `main()` boundary; `Internal` is a unit struct) + `IntoResponse` rendering `application/problem+json {type,title,status,detail,instance,errors?}` where `type` is a stable URI per error from the registry below (never `about:blank`) and `instance` is always set (`"{METHOD} {path}#{x-request-id}"`). One shared `pub(crate) fn problem_response(status, type_uri, title, detail, instance, errors)` constructor in `error.rs` — `AppError::IntoResponse`, the governor 429 handler, and `AuthSession` rejection all call it (identical shape). `Db(sqlx::Error)` maps by matching the *inner* error (not one-status-per-variant). `handle_timeout` returns `AppError::Timeout.into_response()` (single 503 renderer). Log full `sqlx::Error` with `tracing::error!` server-side (`IntoResponse` logs before rendering), expose generic 500. Follow Axum `examples/error-handling`. `JsonRejection` is `#[non_exhaustive]` — keep a catch-all arm. Type-URI registry: `validation-error→400`, `invalid-json→400`, `invalid-path→400`, `invalid-query→400`, `unauthorized→401`, `forbidden→403`, `not-found→404`, `conflict→409`, `rate-limited→429`, `service-unavailable→503`, `internal→500` (all `https://kanpad.dev/problems/<slug>`). Mapping table:

  | Source | Status | Note |
  |---|---|---|
  | `JsonRejection::MissingJsonContentType` | 415 | Wrong/missing content-type. |
  | `JsonRejection::JsonSyntax(_) \| JsonDataError(_)` | 400 | Malformed JSON / serde type error. |
  | `JsonRejection` catch-all (`_`) | 400 | Forward-compat with new variants. Body-size-limit (Axum `DefaultBodyLimit` 2MB) routes through the same arm — confirm in T04, record decision. |
  | garde `Report` | 400 | **Parity** (verified 2026-10-11): Nest's per-endpoint `ValidationPipe` emits 400 today on mutations except auth register/login (no pipes there); never 422. The `errors[]` field→message extension is new. |
  | `PathRejection` / `QueryRejection` (Axum 0.8 splits them) | 400 | Both mapped to problem+json (Axum's default 400 body is plain-text — wrap it). |
  | `sqlx::Error::RowNotFound` | 404 | Foreign IDs → 404 (no oracle). |
  | MySQL `1062` duplicate | 409 | Email / unique-label collisions. |
  | Pool acquire/timeout, connection refused | 503 | Readiness-gated; implies `/ready` also red. |
  | `tower::timeout::error::Elapsed` via `HandleErrorLayer` (above `TimeoutLayer`) | 503 + `Retry-After` | Origin timeout is 503 (504 is proxy semantics). `TimeoutLayer` alone never reaches `IntoResponse` — the handle layer converts to problem+json. |
  | Everything else | 500 | Generic detail only. |
* **Validation:** `garde` + `axum-valid` (`Valid(Json<T>)` / `Valid(Query<T>)` extractors; `validate()` → `AppError::Validation` → **400** problem+json with `errors[]`). Only `PaginationParams` + `AuthSession` stay custom extractors: `PaginationParams` does manual strict checks (`page≥1`, `1≤limit≤100`; out-of-range → 400 with `errors[]`, NO clamp — same shape, not a garde derive; `page`/`limit` ranges live here, not in the garde catalogue) + `AuthSession` custom. Rationale (verified 2026-10-11): the source has no global pipe, but per-endpoint `@UsePipes(new ValidationPipe({transform:true}))` exists on ~20 POST/PATCH handlers (NOT on `POST /api/auth/register|login`) → mutations except auth are already validated with **400** responses; garde is near-parity for those mutations and intentional hardening for auth inputs + GET query DTOs (mostly unvalidated in Nest — a new strictness the frontend migration must audit). Serde keeps **ignore-unknown-fields** (no `deny_unknown_fields`) to match Nest behavior (unknown keys dropped, never rejected — the per-endpoint pipes lack `whitelist`, so Nest passes them through too). **400-vs-422:** all validation failures return `400` problem+json with an `errors[]` field→message extension — matches Nest's 400 status; recorded in `20-api-contract.md` + `CONTRACT-CHANGES.md`. Field mappings: password strength `≥8 + digit/symbol` is **register/create-admin only** + custom **byte-length** cap `password.as_bytes().len() <= 72` on all password inputs (garde `length` counts chars; bcrypt truncates bytes — multibyte would silently collide; never bare `MaxLength(72)`); **login = non-empty + byte-cap `≤72` only** (never strength — legacy weak passwords must still reach 401, not 400); email via garde's email validator (approximates validator.js `isEmail` — borderline addresses may differ, note in `CONTRACT-CHANGES.md`).
* **AuthN/Z:** `tower-sessions` cookie + `tower-sessions-sqlx-store` MySQL store (see Sessions below). `AuthSession: FromRequestParts` → `{user_id,email,role}` or 401 problem via the shared `problem_response` constructor — mirrors the stricter `CurrentUser` shape (`userId+email+role`). Session payload trusted without per-request DB lookup (same as Nest's `@CurrentUser()`), **except role-sensitive paths (`GET /api/v1/auth/me` + any admin route) MUST re-lookup the user row** (deleted → 401, demoted → 403) — this closes the stale-admin window; sliding-only renewal otherwise is accepted and recorded in `CONTRACT-CHANGES.md`. Ownership baked into SQL (`…WHERE board.user_id=?`); 404 for foreign IDs (no oracle), 403 only for role denial, 409 for state conflicts. Hashing: Argon2id (`argon2` crate, `m=19MB,t=2,p=1`, cost gated by `Settings.hash_cost`) for new passwords; **hash BEFORE tx.begin()** (tx holds only INSERT user + single multi-row INSERT labels); login tries Argon2 verify → on miss tries bcrypt verify (old cost-10 `$2a/$2b$` from `auth.service.ts:41-42`) → on bcrypt success rehashes to Argon2 + updates the row in the same request. 72-byte cap is new-password validation only; bcrypt fallback truncates to 72 bytes before verify (Nest/bcryptjs semantics). Unknown-email path runs a dummy Argon2 verify against a static PHC at current params (bcrypt timing delta accepted one-release). Keep `email.trim().to_lowercase()` + `REGISTRATION_ENABLED` gate semantics. Registration (`POST /api/v1/auth/register`) and `create-admin` both call the single prod helper `common::label_seed::ensure_default_labels(tx, user_id)` (`INSERT ... ON DUPLICATE KEY UPDATE` / `INSERT IGNORE`, MySqlExecutor-generic) + shared validators in `common::validation` (single email + password fns) — labels exist for every new user regardless of entry point.
* **Config/obs:** `dotenvy + envy` once at boot (see Config below); `tracing+EnvFilter`; middleware stack in fixed order (see Middleware below); `#[tracing::instrument(skip(pool, password, secret))]` — never log secrets. Secrets in `secrecy::SecretString` + `zeroize`; length checks on **bytes**.
* **OpenAPI:** `utoipa 5 + utoipa-axum 0.2 (OpenApiRouter::routes(routes!(handler))) + utoipa-swagger-ui`; `#[utoipa::path]` per handler, `ToSchema` per DTO, one `ApiDoc` in `docs.rs`. Paths: `/swagger-ui` + `/api-docs/openapi.json` (the typegen source). **Path note:** Nest served Swagger at `api/docs`; Rust serves at `/swagger-ui` + `/api-docs/openapi.json` and issues a `302` redirect from `/api/docs` → `/swagger-ui` for familiarity. `Page<T>` uses utoipa `compose_name` (e.g. `Page_BoardResponse`) with an explicit `T: ToSchema` bound — T40a adds alias coverage or export fails to compile. `export-openapi` and `create-admin` are separate clap subcommands; export boots no pool/sessions.
* **Tests/lint/CI:** unit (pure helpers `clamp_position` + gap-computation only; all DB behavior via `#[sqlx::test]` real DB) + integration (`tests/api/*.rs` via the single `tests/api.rs` target + `[[test]] name="api" path="tests/api.rs"` + `lib::create_app`, `axum-test`/`oneshot`; testcontainers only for hermetic e2e). DB strategy: `#[sqlx::test(migrations,fixtures)]` per-test DBs for migration/unicode tests; resource suites share one DB + transaction rollback (per-test `CREATE DATABASE` on MySQL is minutes at ~150 tests — see T07). Throttle tests isolate rate state per test (separate router with tiny limits + test source-IP, or `#[serial]` — see T30). CI: `fmt → clippy --workspace --all-targets -D warnings → migrate run+info → SHOW CREATE TABLE assert → sqlx prepare --check --workspace --all -- --all-targets → test → docker build(SQLX_OFFLINE=true, --locked --offline, Cargo.lock committed)` + `cargo audit` + `cargo deny check licenses/advisories` + grep gates (no `unwrap()/expect()` in `src/` outside main boundary, no `format!`-built SQL values in `store.rs`).

## Sessions (tower-sessions 0.14, fresh store-owned schema)

* **Store-owned schema, not Nest's table.** `tower-sessions-sqlx-store` (MySQL) defaults to schema `tower_sessions`, table `session` (columns `id VARCHAR(255) PK, data BLOB, expiry_date BIGINT NOT NULL` + expiry index; MessagePack codec). Baseline migration starts with `CREATE SCHEMA tower_sessions` then creates `tower_sessions.session` with DDL **verbatim** from the store (including the store's `IF NOT EXISTS` — documented exception to the baseline fail-fast rule; the store DDL is the source of truth here) — T02 verifies byte-equality by creating a scratch DB, running `MySqlStore::new(pool).migrate().await?` (idempotent per store README), and diffing `SHOW CREATE TABLE` against the baseline. The app does NOT call `migrate()` at boot — the baseline owns the schema. **Nest's singular `session` table (public schema) is NOT renamed and NOT in the baseline** — no collision (different schema + shape + codec); it is abandoned at cutover (rows are ephemeral; drop or leave). Old rows are unreadable by the new codec + new `SESSION_SECRET` → **force re-login**: a `connect.sid` cookie signed with the pre-cutover secret fails HMAC verification → treated as anonymous → 401 → login screen (this is the T11 proof test). Do not write a row-migration; document the break.
* **Lifecycle:** `Expiry::OnInactivity(time::Duration::hours(24))` (sliding 24h, matches Nest TTL — note: tower-sessions uses the `time` crate, not `std::time`/`chrono`). The expired-row sweeper is **`store.continuously_delete_expired(tokio::time::Duration::from_secs(60))`** — a method on the *store* (ExpiredDeletion trait), spawned as a `tokio::spawn` task **only in `main.rs`** (never in `lib::create_app` — tests would leak a task + conn per test) with an `AbortHandle` wired into graceful shutdown (per the store README pattern; NOT on `SessionManagerLayer`). `session.cycle_id()` on register + every login **and** privilege change (fixation defense; register establishes an authenticated session too). No absolute expiry cap by design (sliding 24h renews under activity) — stale-privilege risk is closed by mandatory re-lookup on me/admin routes (see AuthN/Z).
* **Cookie:** `name = "connect.sid"` in dev/test, `__Host-connect.sid` in production (`Secure + Path=/`, no `Domain`), `http_only(true)`, `same_site(Lax)`, `secure(NODE_ENV == "production")`, `path("/")`. `SESSION_SECRET` as `secrecy::SecretString`, ≥ 32 **bytes**, validated at boot; rotation = restart with new secret (all sessions invalidate — accepted). T11 asserts the cookie-attribute matrix per env (prod `__Host-` + Secure + Path=/ + HttpOnly + SameSite=Lax + no Domain; dev/test plain name, no Secure).

## Middleware order (outermost → innermost)

Order is load-bearing. Canonical order (first listed = outermost; this list wins over any shorthand elsewhere):

1. Secure-headers `SetHeaderLayer`s (replaces `helmet(csp:false)`): `x-content-type-options: nosniff`, `x-frame-options: DENY`, `referrer-policy: strict-origin-when-cross-origin`, `served-by: rust`. No CSP header (matches `csp:false`). Outermost so 429/503/404 error responses carry them.
2. `SetRequestIdLayer(x-request-id)` — so even early rejections carry an ID. Use an `instance` field (`uuid7`/counter) per replica for log correlation (distinct from problem+json `instance` = `"{METHOD} {path}#{x-request-id}"`).
3. `TraceLayer::new_for_http()` with request-ID + method/uri/status latency; **redact sensitive headers** (`authorization`, `cookie`, `set-cookie`) via a custom `MakeSpan`/`on_request` filter — never log session cookies.
4. `PropagateRequestIdLayer(x-request-id)` — echoes the ID back to the client.
5. Restrictive `CorsLayer` (see below).
6. `CompressionLayer` — after Governor (never compress 429s). Note parity delta: tower-http compresses all response sizes (Nest's `@fastify/compress` used a 1024-byte threshold); accepted, no threshold config needed.
7. `HandleErrorLayer(handle_timeout)` + `TimeoutLayer(30s)` — bounds handlers; `handle_timeout` returns `AppError::Timeout.into_response()` → **503** problem+json with `Retry-After` (origin uses 503/408; 504 is proxy semantics).
8. `SessionManagerLayer` (innermost of the shared stack, closest to handlers).

`CatchPanic` (if added) sits just inside `Timeout`. Static-file serving (`ServeDir`) is outside `/api` routes (see SPA fallback).

Governor is NOT a global layer: `api_router.route_layer(GovernorLayer)` merged into the root router; `/health`, `/ready`, swagger/openapi, static/SPA live outside it and are never throttled. Governor: default **120 req/min per IP global** + **5 req/min per IP on auth login/register**, config-driven (see Config). Rationale (2026-10-11 amendment): Nest global 10/min was never enforced (no `APP_GUARD`) — mirroring it would 429 frontend bursts + shared-NAT users. Key extractor = peer `ConnectInfo` by default; `X-Forwarded-For` honored ONLY when peer is in `TRUSTED_PROXIES` (default empty = never trust; compose sets it only if a proxy is added). 429s render via the shared `problem_response` constructor (same problem+json + `Retry-After` as AppError). Per-process memory: correct for single `:3001`; N× limit with 2+ replicas — Redis-backed governor is the scale-out prerequisite.

## CORS (restrictive, dev-only open)

* `CorsLayer`: **explicit allowlist, no `Any`**. Dev default origin `http://localhost:5173` only (config-driven for prod); `allow_credentials(true)`; allowed methods `GET,POST,PATCH,DELETE,OPTIONS`; allowed headers `content-type,x-request-id` (auth is cookie-only — no `Authorization` header exists; do not allowlist it). Expose `x-request-id`.
* **Forbid `Any` + `credentials(true)` combination** (browsers reject it; fail boot if configured). Prod origins come from settings; empty prod allowlist = deny-all (fail closed).

## Graceful shutdown

`main.rs`: `tokio::signal` listening for **both `SIGINT` (Ctrl-C) and `SIGTERM`** (docker/compose stop) → `axum::serve(listener, app).with_graceful_shutdown(shutdown_future)` → after HTTP drain, `pool.close().await` so idle MySQL connections terminate cleanly. Log shutdown start/finish with the instance ID.

## SPA fallback + health

* **SPA fallback:** `ServeDir::new("public/")` for the built frontend (`public/` is populated from the frontend `dist/` build at image build time — see T06 Dockerfile; dev serves the frontend separately via Vite) + a fallback service: requests under `/api/*` that miss → JSON 404 problem (never `index.html`); `/swagger-ui*` and `/api-docs/*` excluded (never rewritten); everything else → `public/index.html`. Order: `/api` routes → swagger/openapi → static → fallback. Owned by T05 (with a fallback-order test).
* **Health (outside governor, no auth):** `GET /health` = **liveness, stable shape `{status:"ok"}`**, no DB touch (safe for orchestrator probes). `GET /ready` = readiness: `SELECT 1` via pool → `200 {status:"ok",db:"up"}` or `503` problem when MySQL is down. Both live outside `/api/*` so governor scope never touches them.

## CLI (`main.rs`, clap 4)

```
backend-rs serve [--auto-migrate]   # = node dist/main (NO auto-migrate by default; flag is dev/compose only)
backend-rs migrate run              # = node migrate.js (manual, SSH, same step)
backend-rs migrate info             # = migration:show (pending/applied + checksums)
backend-rs migrate revert           # dev-only down-migration (never prod; prod rollback = restore backup)
backend-rs create-admin             # = node release/create-admin.js
backend-rs export-openapi           # writes openapi.json to stdout; boots NO pool/sessions
```

`serve` does **not** migrate by default (multi-replica boot race, slow boots on large alters, surprise schema change on rollback). Migration locking stays on (`set_locking(true)`, MySQL `GET_LOCK`; never disable in prod). See `31-live-unicode-migration.md` for the full runbook.

## create-admin CLI

`backend-rs create-admin` (clap subcommand; also `cargo run -- create-admin`):

* Prompts via `dialoguer::Password` (hidden; `rpassword` only if dialoguer unavailable); flags `--email/--password` for non-interactive use (print shell-history warning as Nest does; prefer `KANPAD_ADMIN_PASSWORD` env/stdin for automation). Password held as `SecretString` + zeroized after hashing.
* Validates with **the same `common::validation` email + password fns as registration** (single shared helper — no drift; source drift was register.dto MinLength+Matches vs create-admin EMAIL/PASSWORD_REGEX).
* **Single-admin guard:** if an `admin` user already exists, refuse with a clear error unless `--force` (reset password path); exit codes: `0` ok, `1` validation/input error, `2` admin-already-exists, `3` DB/config error.
* Runs `common::label_seed::ensure_default_labels` in the same transaction as the insert (hash BEFORE tx.begin).

## Config

| Key | Source | Default | Note |
|---|---|---|---|
| `DB_HOST` / `DB_PORT` / `DB_USERNAME` / `DB_PASSWORD` / `DB_NAME` | env / `.env` | `localhost` / `3306` / `kanpad` / `` / `kanpad` | Individual parts (mirror `backend/.env.example`). |
| `DATABASE_URL` | derived | `mysql://{user}:{pass}@{host}:{port}/{name}?charset=utf8mb4` | Built from parts unless a full `DATABASE_URL` is explicitly set (explicit wins). `?charset=utf8mb4` always (see `31-live-unicode-migration.md`); smoke test asserts `@@character_set_client/collation_connection`. |
| `SESSION_SECRET` | env (required, `secrecy::SecretString`) | — | **≥ 32 bytes**, boot-validated. Rotation invalidates all sessions. |
| `PORT` | env | `3001` | `u16`, reject `0`; parse-fail = boot error. |
| `NODE_ENV` | env | `development` | Must be one of `development\|test\|production` (anything else = boot error; no silent dev-default typo). `production` ⇒ `secure` cookies (+ `__Host-` cookie prefix) + deny-all CORS default + prod HSTS header (or record why omitted in T05). |
| `REGISTRATION_ENABLED` | env | `true` | Gates `POST /api/v1/auth/register` (same semantics as Nest). |
| `HASH_COST` | env | `2` (Argon2 `t` cost; tests pin `t=1`) | Argon2id cost gate so the auth suite stays fast. `Settings::load()` fails boot when `NODE_ENV=production && HASH_COST<2`. T10 asserts stored PHC params `m=19456,t=HASH_COST,p=1,v=19`. |
| `THROTTLE_GLOBAL_PER_MIN` / `THROTTLE_AUTH_PER_MIN` | env | `120` / `5` | Governor limits (per IP, `/api/*` only via route_layer, see Middleware order); tests set tiny values for fast deterministic throttle tests (T30). |
| `CORS_ORIGINS` | env (prod required) | `http://localhost:5173` in dev | Comma-separated allowlist; empty in production = deny-all (fail boot, see CORS). |
| `TRUSTED_PROXIES` | env | empty (never trust XFF) | CIDR list of proxies allowed to supply `X-Forwarded-For`; direct-to-`:3001` default ignores XFF (spoof defense). |

**Precedence: real environment > `.env` > defaults.** Loaded once at boot via `dotenvy` + `envy` (`KANPAD__` nesting where needed); `Settings::load()` validates (secret length in bytes, port range incl. reject 0, NODE_ENV whitelist, HASH_COST prod floor, DB parts present, Any+credentials CORS combo) and fails fast with an actionable message. (`config` crate intentionally not used — ~30 transitive deps for ~10 keys.) `Settings` implements a redacting `Debug` (never logs `DATABASE_URL`/password/secret); the URL is built in one function at pool-creation time only. T05 asserts boot with bad DB password returns 503 problem+json with no password substring in body or captured logs.

## Code policy

* `///` docs on every exported store fn + safety-critical SQL (lock order, ownership WHERE); no redundant narration. Kernel tasks (T04/T05/T11) are exempt from the 5-files rule. `src/common/*` Touch is per-file (ids/ownership/pagination/card_summary/label_seed/validation) — new shared modules need T52-style justification. `unwrap()/expect()` allowed in tests only.
* `AppState.sessions` type is the cloned `MySqlStore`; `FromRef` subsets only if handlers take subsets — all-`State<AppState>` needs no FromRef claim. Boot connection-refused = fatal; 503 is the runtime/readiness mapping only.

## DB pool

`MySqlPoolOptions`: `max_connections 10–20`, `min_connections 1`, `acquire_timeout 5s`, `idle_timeout 600s`, `max_lifetime 30m`, **`test_before_acquire(true)`** (stale-connection defense after idle/shutdown). Health = `SELECT 1` (readiness only; liveness stays DB-free).

## ADRs (short)

* Axum over Loco/Actix/Rocket/Poem: standard hiring/docs, Tower transfers, no dead-weight mailers/jobs; Loco would force SeaORM + generated layout.
* sqlx 0.8 over 0.9/SeaORM/Diesel: 0.9 blocked on `tower-sessions-sqlx-store 0.15`; team SQL-fluent, explicit queries, no compile-DB for Diesel DSL debt; SeaORM's 10–20% convenience not worth new ORM; `diesel-async+deadpool` most setup.
* tower-sessions over JWT Bearer: preserves `credentials:include` frontend transport (no token-storage rewrite); force re-login covers rotation.
* tower-governor over hand-rolled/Fastify-Throttler replacement: per-IP + per-route layers, trusted-proxy-gated `X-Forwarded-For` (default peer IP only); auth 5/min hardens brute force, global 120/min is an abuse ceiling only (Nest's global 10/min was configured but never enforced — mirroring it would break frontend bursts and shared-NAT users; see Middleware order).
* Argon2id (new) + bcrypt (legacy-verify + upgrade) over bcrypt-12-everywhere: OWASP 2024–2026 primary is Argon2id; bcrypt stays only to verify pre-cutover cost-10 hashes without forcing password resets.
* `dotenvy + envy` over the `config` crate: ~10 keys don't justify ~30 transitive deps; same precedence, fail-fast `Settings::load()`.
* openapi-typescript (types-only) over orval/hey-api: keep hand fetch+React Query hooks, zero runtime, `erasableSyntaxOnly`-safe (see `40-typegen.md`).
