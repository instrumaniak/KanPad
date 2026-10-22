# KanPad backend rewrite: NestJS → Rust — plan index

> Target: `backend/` (NestJS + TypeORM + MySQL + cookie session) → `backend-rs/` (Axum 0.8 + **sqlx 0.9** + MySQL + **tower-sessions 0.15** + tower-http 0.7).
> This is a **rewrite, not a port**: Nest is behavior reference only. Every API, schema, and pattern decision follows industry standards and Rust community best practices. All spec files agree with each other — if two ever disagree, fix the disagreement (see `00-overview.md` Amendments for the decision log). No NestJS-isms are carried over; no response byte-parity is owed.
> Decisions locked 2026-10-10: **1) sqlx-only, 2) keep MySQL, 3) force re-login, 4) adopt RFC9457 problem+json + real pagination**.
> Amendment 2026-10-11 (review sweep, see `00-overview.md` "Amendments"): validation status canonicalized to **400** (Nest per-endpoint pipes verified to emit 400), pagination **strict 400, no clamp**, session store owns its own schema (no `session`→`sessions` rename), global throttle re-scoped (120/min global, 5/min auth). sqlx pinned **0.8** — `tower-sessions-sqlx-store` 0.15 (latest) requires `sqlx ^0.8` + `tower-sessions-core ^0.14`; sqlx 0.9 escape hatch = in-repo `SessionStore` impl or the maintained `tower-sessions-ext-sqlx-store` fork. **Superseded 2026-10-22 by Amendment 12 below** — the in-repo `SessionStore` impl was chosen, so sqlx 0.9 + tower-sessions 0.15 are now the targets.
> Amendment 2026-10-22 (`54-rust-idiom-hardening.md`): the last two Node/Nest artifacts are dropped — **`APP_ENV` replaces `NODE_ENV`** (values unchanged; boot fails when `NODE_ENV` is present, so a stale deployment never silently downgrades to `development`) and the session cookie **`connect.sid` → `kanpad.sid`** (`__Host-kanpad.sid` in production). The rewrite's CI workflow moved to the **repo root** `.github/workflows/` — GitHub discovers workflows only there, so the pipeline had **never run** and every gate in it was unverified. Quality baseline folded in: CORS `allow_methods` gains `PUT`, `#[tracing::instrument]` handlers skip the request body + session, an `Origin` check on mutating requests + a CSP on the SPA-serving origin, `/metrics` + JSON production logs, `Role`/`AppEnv` enums, shared-kernel DRY, and the build chain (`[lints]`, `[profile.release]`, `rust-toolchain.toml`, `deny.toml`, edition 2024). Target: **≥ 9.5/10 on all ten review axes**. Every claim was re-verified against primary sources on 2026-10-22 (RFC 6265bis, MySQL 8.4 reference manual, GitHub Actions docs, `docs.rs`, and the vendored `sqlx-core-0.8.6` / `tower-http-0.6.11` sources) — **14 details were corrected**, 3 of them breaking (the CSP's `connect-src`/`script-src`, and the isolation-level statement's position relative to `BEGIN`). The full correction table is in `54-rust-idiom-hardening.md` §"Verification pass" and supersedes the first draft on those points.
> Amendment 2026-10-22 (b) — **dependency hygiene & modernization** (`54` **Part C**): a second, deeper review of `backend-rs/Cargo.toml` found **three dead direct dependencies** (`axum-valid` — which dragged a **second, complete axum 0.7.9** into the binary — plus `utoipa-axum`, `zeroize`) and **two dead `tower-http` features** (`timeout`, `limit`); `secrecy/serde` is **load-bearing** and stays. The **session store is now owned in-tree** (`src/common/session_store.rs`; the `sessions` DDL and the MessagePack payload are unchanged), because `tower-sessions-sqlx-store 0.15` pinned `sqlx ^0.8` **and** `tower-sessions-core ^0.14`, which made both dependency majors unreachable. That unlocks **sqlx 0.9** (feature names change; the `DATABASE_URL` must now carry `collation=utf8mb4_unicode_ci` so the driver version cannot change the session collation), **tower-sessions 0.15**, **tower-http 0.7** (whose `csrf` feature brings the **Go 1.25 `CrossOriginProtection` algorithm** — adopted for the mutating-request cross-origin check, with the allowlist **canonicalized at boot** and the rejection reusing the **existing** `AppError::Forbidden` problem type, so **no new problem-type URI**), **utoipa 6 + swagger-ui 10**, **argon2 0.6 + bcrypt 0.19** (the same `password-hash` 0.6 wave), **sha2 0.11**, **garde 0.23**, **dialoguer 0.12**. The **toolchain moves 1.96.1 → 1.99.1** (the current stable; the Dockerfile base moves in the same PR). **Why now:** the NestJS data has **not** been migrated — the toolchain pin gates `edition = "2024"` → `resolver = "3"` → the entire dependency set, so these are cheap now and expensive after the cutover. Phase 7 grows to **T54–T67**.
> Cleanup 2026-10-11: rewrite deltas folded into base files (3-file layout, Argon2id, 503 timeout + middleware order, set-based positions, `__Host-` cookies, `/api/v1` + `204` + snake_case, blue-green cutover). `11`/`31` are rationale records; `50-tasks.md` Phase 6 keeps only T02b.
> API parity explicitly waived; this is a standards-led rewrite — industry-standard REST shapes and community best practices over byte-compat.

## Reading order for a fresh agent

1. `00-overview.md` — why, scope, locked decisions, DoD.
2. `10-architecture.md` — crate layout, layers, patterns.
3. `11-rewrite-deltas.md` — rationale record for the rewrite divergences (folded into base files; skim, don't implement from it).
4. `20-api-contract.md` — new wire contract (routes, errors, pagination, auth).
5. `21-api-standard-shapes.md` — Nest→rewrite endpoint map (agrees with 20). Read with your resource task.
6. `30-db-schema.md` — tables, baseline migration, sqlx workflow.
7. `31-live-unicode-migration.md` — prod latin1 findings, utf8mb4 conversion `0002`, `backend-rs migrate` CLI + cutover runbook. Read before any migration task.
8. `40-typegen.md` — utoipa → openapi-typescript pipeline + frontend migration.
9. `50-tasks.md` — **the only file executors work from**, one task at a time (~46 tasks: T01–T08 scaffold, T10–T13 auth, T20–T27 resources split, T30–T31 hardening, T40a–T41e typegen+frontend, T50a–T51 cutover, T52 contract-amend process, plus T02b live charset conversion, plus **Phase 7: T54–T67** — the Rust-idiom amendments + the quality baseline + the dependency program).
10. `54-rust-idiom-hardening.md` — the 2026-10-22 pre-cutover pass: Rust-idiom naming (`APP_ENV`, `kanpad.sid`) + the quality baseline (CI relocation, CORS `PUT`, cross-origin/CSRF + CSP, observability, build chain, shared-kernel DRY) + **Part C: dependency hygiene & modernization** (the manifest audit, the dead deps, the in-tree session store, the toolchain 1.99.1, the sqlx 0.9 collation pin), with the ten-axis scorecard and two verification tables. Rationale + task source; the executable entries are in `50-tasks.md` Phase 7 (**T54–T67**).

## Fresh-agent protocol

1. Read `00-overview.md` constitution (global DoD).
2. Read your one task in `50-tasks.md` + its linked contract/schema section.
3. Implement TDD: failing test first, then code. No test deletion/weakening.
4. Every POST/PATCH/DELETE test re-reads state via second GET or direct sqlx query.
5. Run these exact commands and paste outputs in PR:
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test -p backend-rs --test api <your-filter>` (kernel tasks: `cargo test -p backend-rs`)
   - `cargo sqlx prepare --check --workspace --all -- --all-targets` (+ commit `.sqlx/` if queries changed)
   - frontend tasks: `npm run gen:api -- --check`, `tsc --noEmit`, `vitest`/`playwright` vs `:3001`
6. Contract changes only via T52 (spec + utoipa + `openapi.json` + typegen in same PR). Report evidence (commands + outputs) in PR.

## Source of truth

* Current behavior: `KanPad/backend/src/**` (118 files, 52 handlers, 12 entities, 10 migrations).
* Contract artifact after rewrite: `backend-rs/openapi.json` (generated by utoipa, diffed in CI).
* Frontend types artifact: `frontend/src/lib/api/schema.d.ts` (generated by openapi-typescript, never hand-edited).
