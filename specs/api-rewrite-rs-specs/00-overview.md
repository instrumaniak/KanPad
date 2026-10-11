# 00 — Overview, scope, locked decisions

## Intent

Rewrite `KanPad/backend/` (NestJS 11, TypeORM 0.3, `mysql2`, Fastify + `@fastify/session` + `connect-typeorm`) as idiomatic Rust (`backend-rs/`). Nest is behavior reference only — not a template. Every API, schema, and pattern decision follows industry standards and Rust community best practices. All spec files agree with each other; the Amendments below are the decision log. Optimize for small-team maintainability, explicitness, and testability — not for response byte-parity.

## Facts about the source (verified read-only)

* `backend/src/app.module.ts:1` — 10 feature modules + global `ConfigModule` + `ThrottlerModule(10/min)`.
* `backend/src/main.ts:30` — Fastify + `fastifyCookie` + `fastifySession(TypeormStore, connect.sid, 24h)` + `helmet(csp:false)` + `compress` + Swagger `api/docs` + SPA fallback. No global `ValidationPipe`.
* Auth is **cookie session, not JWT**: `backend/src/auth/auth.controller.ts:1` (`register/login/logout/me` mutate `session{userId,email,role}`), `backend/src/auth/guards/session.guard.ts:1` gates all domain controllers. No `passport`/`jsonwebtoken` anywhere.
* DB is **MySQL** (`backend/src/database/data-source-options.ts:5` `type:'mysql'`), 10 TypeORM migrations in `backend/src/migrations/`, journal `typeorm_migrations`. Entities use `INT AUTO_INCREMENT` PKs, `DATETIME` timestamps, no UUID/JSONB/soft-delete (`is_archived` bool on boards instead).
* Envelopes: single `{data,message?}`, lists `{data,total=data.length}` (projects/boards only, no real pagination). Errors: Nest `{statusCode,message:string|string[],error}`. Frontend `frontend/src/features/*/*.api.ts` copy-pastes `handleResponse` + `FETCH_OPTIONS{credentials:include}` per file; centralized `@/lib/api` from AGENTS.md not implemented.
* No uploads, no websockets/queues, no `activities` entity (specs-roadmap only).

## Locked decisions (2026-10-10)

1. **sqlx-only 0.8, no SeaORM.** Team is SQL-fluent; wants minimal deps, explicit queries, fastest builds. Pinned to **0.8 until `tower-sessions-sqlx-store` supports 0.9** (in-repo `SessionStore` impl is the documented escape hatch). Relations/pagination hand-rolled via `query!`/`query_as!` + `QueryBuilder`. See `10-architecture.md`.
2. **Keep MySQL 8 (pin `8.4`).** No engine switch. Placeholders `?`, `NaiveDateTime↔DATETIME`, `last_insert_id()` for creates. Dev/test/prod same minor.
3. **Force re-login with a fresh session table.** Keep cookie-session mechanism (`tower-sessions 0.14` + `tower-sessions-sqlx-store 0.15`, `connect.sid` in dev/test + `__Host-` prefix in prod, sliding 24h TTL) but issue fresh sessions with a new `SESSION_SECRET` in a **new store-owned schema** (`CREATE SCHEMA tower_sessions`; Nest's singular `session{id/json/expiredAt/destroyedAt}` table is not reused). Do NOT chase byte-compat with `@fastify/session` + `connect-typeorm` row codec. Old sessions 401 → login screen. Passwords upgrade without resets (Argon2id new, bcrypt cost-10 verify + same-request rehash).
4. **Adopt `application/problem+json` (RFC9457) + real `?page&limit` pagination.** Declared breakage vs Nest envelope; documented in `20-api-contract.md` + `CONTRACT-CHANGES.md` (created in T01). Frontend migrates via centralized `apiFetch` + generated types (`40-typegen.md`).

## Amendments (2026-10-11 decision log — all folded into the base files on the 2026-10-11 cleanup; no "override" layers remain)

1. **Validation status is canonical 400** (with `errors[]` field→message extension) — NOT 422. Verified 2026-10-11: Nest has no global pipe, but per-endpoint `@UsePipes(new ValidationPipe({transform:true}))` exists on ~20 POST/PATCH handlers, so validation failures are **400 today for mutations except `POST /api/auth/register|login` (no pipes there — `RegisterDto`/`LoginDto` decorators are dead code, weak inputs reach the service today)**; 400 is parity for those mutations, **new strictness for auth register/login + GET query DTOs** (mostly unvalidated in Nest).
2. **Pagination is strict, no silent clamp**: out-of-range `page`/`limit` → **400** problem+json with `errors[]` (not clamped, not 422).
3. **Session store owns its own schema**: baseline starts with `CREATE SCHEMA tower_sessions` and ships the `tower-sessions-sqlx-store` DDL verbatim. Nest's singular `session` table is NOT renamed and NOT in the baseline — no collision (different schema), abandoned at cutover. (Reverses the `session`→`sessions` rename plan.)
4. **Global throttle re-scoped**: governor covers `/api/*` only (`/health` + `/ready` live outside `/api`), limits config-driven — 120 req/min per IP global + 5/min on auth (Nest's global 10/min was never enforced; blind mirroring would break frontend bursts and shared-NAT users).
5. **`sqlx` pinned to 0.8 everywhere**: `tower-sessions-sqlx-store` 0.15.0 (latest) depends on `sqlx ^0.8` + `tower-sessions-core ^0.14`, which also pins `tower-sessions` to 0.14 (0.15 exists but is incompatible with the store).
6. **Canonical `sqlx prepare` form**: `cargo sqlx prepare --check --workspace --all -- --all-targets` everywhere (harmless no-op flags on a single crate; one form, no drift).
7. **Rewrite deltas (2026-10-11, folded into `10/20/50`):** 3-file feature layout (`routes/store/types`, `MySqlExecutor` generics, no repository traits), newtype IDs + `snake_case` wire names, set-based position `UPDATE`s with `SELECT ... FOR UPDATE`, ownership in SQL (`404` foreign, `403` role-only, `409` state conflicts), `HandleErrorLayer→503` + middleware order (`RequestId→Trace→Cors→Governor→Compression→Timeout→Session`), `thiserror`-only `AppError` + `axum-valid` + `secrecy` (byte-length checks), Argon2id with bcrypt upgrade path, `__Host-` cookies in prod, `[[test]] api` target, CLI `serve/migrate/create-admin/export-openapi` (`serve` never auto-migrates by default). Rationale record: `11-rewrite-deltas.md`.
8. **Live unicode + migration (2026-10-11, folded into `10/30/50`):** prod is MariaDB 10.6.28, all tables `latin1` via bare `ENGINE=InnoDB`, 1 non-ASCII char `’` so conversion is still lossless; `utf8mb4_unicode_ci` portability rule; `0002_convert_to_utf8mb4.sql` (`CONVERT TO`, `ROW_FORMAT=DYNAMIC`, emoji gate test, MariaDB-10.6 staging proof); cutover is blue-green ONLY (transformed restore with `--ignore-table` + ghost-column strip; no stamp; prod rollback = restore backup). Runbook: `31-live-unicode-migration.md §31.5`.
9. **API standard shapes (2026-10-11, folded into `20/50`):** `/api/v1` versioning (deprecated `/api` dual-mount one release), uniform `204` deletes, no verb URLs (`archive/restore/permanent/sort/move-all` replaced), no duplicate routes (`/boards/archived`, `/boards/:id/notes` deleted), full `snake_case`, added `GET cards/:id/checklists` + `PUT cards/:id/labels/:labelId`, `/health` + `/ready` outside `/api`. Map: `21-api-standard-shapes.md`.
10. **Rewrite framing (2026-10-11):** this plan is a **Rust rewrite, not a port**. Nest is behavior reference only. No NestJS-isms (DI-container layouts, repository traits, verb URLs, `message` envelopes, `latin1` defaults) are carried into `backend-rs/`.

## Scope

In: `auth/users/sessions`, `projects`, `boards`, `columns`, `cards(+labels M2M)`, `labels`, `checklists/items`, `notes/tags`, health, Swagger→utoipa, `create-admin` CLI→`clap` subcommand, compose setup (none exists today).
Out: engine switch, UUID PKs, file uploads, activity feed (confirm absent, keep out), real-time/collab, zero-downtime strangler proxy (blue-green DB promote + port-level switch `:3000` vs `:3001` + app one-line rollback, DB rollback via restore).

## Constitution (global DoD, applies to every task)

* `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test` green; `cargo sqlx prepare --check --workspace --all -- --all-targets` green (single crate — flags are no-ops but the form is canonical everywhere); `.sqlx/` committed.
* TDD: failing test written first. No existing test deleted, skipped, or weakened to pass.
* Every data-mutating test asserts persistence via a **second read** (GET or direct sqlx `SELECT`), not just the mutation response.
* Contract changes only via spec task (update `20-api-contract.md` + `21-api-standard-shapes.md` + utoipa + `openapi.json` + typegen in same PR).
* Unknown JSON fields ignored (serde default, no `deny_unknown_fields`) to match Nest behavior — no global `ValidationPipe`, and the per-endpoint pipes use `transform:true` without `whitelist`, so unknown keys pass through and are dropped without rejecting. `axum-valid` (garde) hardening returns `400` (never 422 — parity with Nest's per-endpoint pipes on mutations except auth register/login, which had no pipes; see Amendments + `10-architecture.md`). garde extends the same 400 to auth inputs + GET query DTOs, which Nest mostly left unvalidated — a new strictness the frontend migration (T41*) must audit (pre-sanitize `?project_id=abc`, `?page=0`, undefined/garbage query params or accept the 400 as a surfaced bug).
* No `unwrap()/expect()` in request paths; no `sqlx::Error` text leaked to clients; no `format!`-interpolated SQL values.
