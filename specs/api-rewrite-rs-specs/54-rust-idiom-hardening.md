# 54 — Rust-idiom amendments + quality baseline (target: 9.5/10 on every axis)

> **Status: PARTIALLY IMPLEMENTED (2026-10-22).** This file + `50-tasks.md`
> Phase 7 are the **work order**; executors implement T54–T68 and flip each AC
> as it lands. **Six tasks have landed** (the table below); everything else
> is still PLAN ONLY.
>
> **The tree no longer has** `NODE_ENV` + `connect.sid` (T54), **a workflow
> that has never run** (T55 moved it to the repo root — and T68 repaired the
> two steps that were red when it finally became verifiable), **a failing
> `unwrap`/`expect` grep gate** (T55), **an `allow_methods` list without
> `PUT`** (T55), **no `[lints]` / `[profile.release]` / toolchain pin /
> `deny.toml`** (T57), and **three dead direct dependencies** (T63).
>
> ### Implementation status (2026-10-22)
>
> | Task | Status | Commit | What it landed |
> |---|---|---|---|
> | **T54** | **DONE** | `8b0cef64` | `APP_ENV` replaces `NODE_ENV` (boot **fails** when `NODE_ENV` is present); `kanpad.sid` / `__Host-kanpad.sid` replaces `connect.sid`; the `__Host-` invariant is test-locked |
> | **T55** | **DONE** | `689dc895` | the workflow **moved to the repo root** (it had never run); the 3 lint-gate sites fixed in code (`to_response_one`); a second grep gate (the panic spellings) + the third gate **repaired**; `allow_methods` gained **`PUT`** + the preflight guard |
> | **T57** | **DONE** | `98fb1ce2` | `edition 2024` + `rust-toolchain.toml` @ **1.99.1** + the Dockerfile base bump; the `[lints]` + `[profile.release]` tables; the committed **`deny.toml`**, derived from its first failing run |
> | **T63** | **DONE** | `85f433f3` | `axum-valid` / `utoipa-axum` / `zeroize` + 2 dead `tower-http` features removed; **2 advisory ignores deleted** with their crates |
> | **T68** | **DONE** | `0b03ea58` | the CI **first-run defect repair**: the `cargo audit` step deleted (it contradicted the committed `deny.toml`) + the SHOW CREATE TABLE assert fixed (it queried the pre-2026-10-19 `tower_sessions` schema) + `cargo-deny` pinned |
> | **T65** | **DONE** | *(this PR)* | the **in-tree `SessionStore`** — the hinge of the phase: `tower-sessions-sqlx-store` is gone (the lock **shrank by one package**), `sqlx/time` is off, the `AS "…chrono::NaiveDateTime"` overrides are gone from all **eight** feature store files, and the `sqlx 0.9` + `tower-sessions 0.15` majors are now reachable |
> | T67, T66, T56, T58, T59, T60, T61, T62 | pending | — | see `50-tasks.md` Phase 7 |
>
> **The three corrections this file's own text still needs** (recorded in
> `50-tasks.md` + `AGENTS.md` as the 2026-10-22 (c)/(d)/(e) passes): the
> verification matrix's clippy command, the "five store files" that are
> actually **eight**, and the claim that no task touches `openapi.json`
> (T57 changed its `info` block).
>
> **Relationship to the other spec files:** this file records *why* the naming
> amendments (Part A) and the quality-baseline tasks (Part B) exist. The
> **executable** task entries live in `50-tasks.md` (Phase 7, T54–T67) — that
> file remains the only file executors work from. Where this file and
> `10/11/20/21/30/50` disagree, the disagreement must be fixed, not silently
> resolved (README rule). The 2026-10-22 edits to `10-architecture.md` and
> `11-rewrite-deltas.md` are **target-state descriptions** and carry their own
> "planned, not implemented" markers.
>
> **Origin:** a code review of `backend-rs/` on 2026-10-22 (organization,
> architecture, idiomatic Rust, industry best practices) + the project owner's
> instruction to drop the Node/Nest-isms that leaked into a rewrite whose own
> constitution forbids them.
>
> **Companion artifacts (the same-PR rule from `50-tasks.md` T52 still applies
> to every file touched when a task lands):** `00-overview.md` (amendment log) ·
> `10-architecture.md` (Config table + Sessions/CORS/Middleware/observability) ·
> `11-rewrite-deltas.md` (rationale record) · `50-tasks.md` (Phase 7) ·
> `README.md` (index) · `CONTRACT-CHANGES.md` (cookie-name row) · root
> `AGENTS.md` (lessons) · `docs/data-migration-live.md` (operator runbook) ·
> `compose.yaml` · `.github/workflows/backend-rs.yml` (B1 — the workflow moved to the repo root in T55).

---

## Intent

Two classes of change, one PR series:

* **A — Rust-idiom naming.** `NODE_ENV` and `connect.sid` are **Node.js and
  express-session artifacts** that survived into a rewrite whose own
  constitution (`00-overview.md` Amendment 10) says *"This is a **rewrite, not
  a port** … No NestJS-isms are carried over into `backend-rs/`."* They are
  not "parity" — the cookie name and the env-var name were never on the wire
  contract, and the rewrite is explicitly allowed (and required) to drop them.
* **B — Quality baseline.** Findings from the 2026-10-22 review, in severity
  order: **CI has never run** (B1), the repo's **own lint gate fails on the
  current tree** (B2), **CORS is missing `PUT`** so label-attach is blocked
  by the browser preflight (B3), **request bodies are recorded in tracing
  spans** (B4), **no release profile / toolchain pin / lint table / license
  metadata** (B5), the shared kernel has **9 duplicated copies of
  `format_timestamp` + an empty `common::ownership` module** (B6), there is
  **no CSRF/Origin check and no CSP on an origin that now serves the SPA
  itself** (B7), there is **no metrics endpoint and no structured JSON
  logging** (B8), the **public Rust API leaks `garde`/`axum` types and
  stringly-typed enums** (B9), and the **Docker image has no `.dockerignore`,
  no `HEALTHCHECK`, no digest pin** (B10).

Both classes are *safe* changes: B1–B10 fix latent defects or close
quality gaps; A renames configuration that no client depends on. **None of
them change a JSON response body, a status code, or a route** (the only
`openapi.json` change in the whole series is adding `/ready` — T59).

---

## Scorecard — current → target, and the task that closes each gap

The 2026-10-22 review scored the crate on ten axes. This series exists to
lift **every axis to ≥ 9.5/10**. Each row names the tasks that move it.

| # | Axis | Now | Target | Closed by |
|---|---|:--:|:--:|---|
| 1 | **Idiomatic Rust** | 8.5 | **9.6** | T57 (edition 2024, `[lints]`), T58 (`Role`/`AppEnv` enums, `AppResult`, `#[non_exhaustive]`, `impl Into<String>`), T59 (delete `db::with_executor`, collapse the duplicate `JsonRejection` arm), **T63** (drop **3** dead direct deps + 2 dead features), **T65** (drop the `sqlx/time` feature unification) |
| 2 | **Architecture / layering** | 9.0 | **9.6** | T59 (`common::time` single owner for `format_timestamp`; implement `common::ownership` as the real cross-feature owner; delete `db::with_executor`) |
| 3 | **Error handling & API contract** | 9.5 | **9.8** | T59 (`/ready` into `ApiDoc`; collapse the byte-identical `JsonRejection` arms; `detail: impl Into<String>`) |
| 4 | **Concurrency & data correctness** | 9.5 | **9.7** | T61 (bounded **deadlock/lock-wait retry** on the move tx; `READ COMMITTED` for the move tx; env-driven pool sizing) |
| 5 | **Security** | 8.5 | **9.7** | T54 (`APP_ENV` fail-fast — no silent downgrade; `kanpad.sid` + `__Host-` invariant), T56 (**Origin check** on mutating requests = CSRF defence-in-depth; **CSP** on the SPA-serving origin; CORS `PUT`; span redaction) |
| 6 | **Testing** | 9.0 | **9.7** | T55 (live CI + the two new grep gates), T56 (CORS preflight + CSP + Origin + span-redaction tests), T62 (cookie-matrix test, `APP_ENV` boot-failure test, **`cargo-llvm-cov` coverage gate**) |
| 7 | **Documentation** | 10.0 | **10.0** | maintained — `missing_docs` becomes a **deny-level lint** (T57) so the discipline stops being convention |
| 8 | **Build / CI / supply chain** | **5.0** | **9.7** | T55 (**CI relocated to the repo root**), T57 (`[lints]`, `[profile.release]`, `rust-toolchain.toml` @ **1.99.1**, `deny.toml`, `license`, `rust-version`), T62 (**action SHA pins**, `dependabot.yml`, `--locked` everywhere, `cargo doc` gate), **T63**/**T65**/**T66**/**T67** (the dependency set itself: dead deps removed, the stale session store replaced, the two dependency majors taken) |
| 9 | **Production readiness** | 7.0 | **9.6** | T54/T57 (`APP_ENV` + release profile + toolchain pin), T60 (**`/metrics` + Prometheus**, JSON logs, pool metrics, graceful-shutdown timeout), T62 (`.dockerignore`, `HEALTHCHECK`, digest pins, distroless), **T66** (sqlx 0.9 + the pinned collation) |
| 10 | **API ergonomics (Rust-facing)** | 7.5 | **9.6** | T58 (`Role`/`AppEnv` enums replace `String`; `pub type AppResult<T>`; `#[non_exhaustive]` on `AppError`; `AppError::Validation` stops leaking `garde::Report`), T59 (`Page<T>` + `common::ids` stay the single public vocabulary) |

**The 8.0 → 9.5 gap on axis 8 is the whole point of this file.** Everything
else was already in the high band; the CI pipeline being undiscoverable meant
*none* of the quality the code already had was ever verified.

---

## Verification pass (2026-10-22) — what was checked, and what it changed

Every claim in this file was re-checked against **primary sources** (not
memory): the RFC/MDN text for the cookie prefix, GitHub's own Actions docs
for workflow discovery, the Fetch spec for the CORS safelist, the MySQL 8.4
reference manual for the error codes and isolation semantics, `docs.rs` for
`tracing`/`tower-sessions`/`axum`/`cargo-llvm-cov`, and — decisively — the
**vendored crate sources** for the pinned versions (`sqlx-core-0.8.6`,
`tower-http-0.6.11`, `envy-0.4.2`) plus the **actual frontend** and the
**actual `backend-rs` tree**.

The verification confirmed the plan's core findings and **corrected 14
details**. Three of the corrections were *breaking* — they would have shipped
a plan that does not work:

| # | Correction | Severity |
|---|---|:--:|
| D1 | CSP `connect-src` must include the CORS allowlist origins, not `'self'` alone — the dev topology is cross-origin, so `'self'` alone blocks every API call | **critical** |
| D2 | CSP `script-src 'self'` alone breaks the SPA — `frontend/index.html:12-21` has an inline theme bootstrap; allow it by **CSP hash** computed from the served `public/index.html` | **critical** |
| D3 | `SET TRANSACTION ISOLATION LEVEL …` must be issued **before** `BEGIN` — inside a transaction MySQL rejects it with `ERROR 1568 (25001)` | **critical** |
| D4 | sqlx 0.8.6 has **no `PoolMetrics` and no `metrics` feature** — the pool gauges come from `Pool::size()`/`Pool::num_idle()`; the acquire-duration series must be self-instrumented | high |
| D5 | "READ COMMITTED removes most gap-lock deadlocks" overstates it — MySQL §17.7.5 says the *possibility* of deadlocks is not affected by isolation level; §17.7.2.1 says it *"greatly reduces the probability … but they can still happen"* | medium |
| D6 | The retry must match the error **number** (`1213` = SQLSTATE `40001`, `1205` = `HY000`), must `ROLLBACK` the whole tx (a `1205` rolls back only the statement), and `1205` is rarely reached end-to-end (50 s lock wait vs a 30 s request timeout) | medium |
| D7 | `cargo llvm-cov --fail-under-lines` needs an export format — `--summary-only`/`--output-path` require `--json`/`--lcov`/`--cobertura` | medium |
| D8 | A guessed `deny.toml` fails on the first run — the `skip`/`skip-tree` + license allow-list must be **derived from that run** | medium |
| D9 | `edition = "2024"` implies `resolver = "3"` → the Rust-version-aware resolver can **change the dependency set**; the `Cargo.lock` diff needs line-by-line review. **Corrected 2026-10-22 (b):** the current stable is **`1.99.1`** (2026-10-13), not `1.99.0`; the local rustup has `1.96.1` installed and `rustup check` reports the update available | low |
| D10 | `tower-http`'s own `Any`+credentials guard is a **panic on the first request** (`ensure_usable_cors_rules` runs in `poll_ready`), not a boot error — so the app-level boot check is load-bearing | low |
| D11 | `tower-sessions`' own default cookie name is `"id"` (and its docs say "OWASP recommends not using the default name") — the rename steps away from **both** framework defaults | low |
| D12 | `/metrics` must not be on the public API port (axum's own example uses a second server) and a scrape must not record itself | low |
| D13 | axum's `MatchedPath` is feature-gated (`matched-path`) — **but it is in axum 0.8's DEFAULT features** (verified in the vendored `axum-0.8.9/Cargo.toml`: `default = ["form", "http1", "json", "matched-path", …]`), so T60 needs **no** feature change | low |
| D14 | GitHub now ships workflow dependency **lockfiles** (`gh actions-lock`) + policy-enforced SHA pinning; dependabot does not raise alerts for SHA-pinned actions | low |
| D15 | **The session store is the blocker for both dependency majors.** `tower-sessions-sqlx-store 0.15.0` requires `sqlx ^0.8.0` (its dependabot bump to 0.9 is **open since 2026-05-21**, and the crate's last release was 2025-01-01) and `tower-sessions-core ^0.14.0` — while `tower-sessions 0.15.0` requires `tower-sessions-core = 0.15.0`. Taking either major today pulls a **second** sqlx / a **second** `tower-sessions-core` and does not compile | **critical** |
| D16 | The sqlx 0.9 collation change is **smaller than the changelog implies**: `sqlx-mysql-0.8.6/src/collation.rs:139` maps `CharSet::utf8mb4 => Collation::utf8mb4_unicode_ci`, so 0.8.6 already sends `SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci` — **matching this project's schema**. 0.9 sends only `SET NAMES utf8mb4` and lets the server pick. Pin `collation=utf8mb4_unicode_ci` in the connection URL (0.8.6's URL parser accepts `collation=` — `sqlx-mysql-0.8.6/src/options/parse.rs:52`) and the two versions become **byte-identical** on this axis | medium |
| D17 | The spec'd hand-rolled Origin check has a **production cliff**: browsers send `Origin` on *every* POST/PUT/PATCH/DELETE (same-origin included), so with an empty prod `CORS_ORIGINS` every mutating request would 403. The Go 1.25 `CrossOriginProtection` algorithm (ported to tower-http 0.7 as `CsrfLayer`) has an `Origin`/`Host` fallback that makes a same-origin SPA work with **zero** origin configuration | **critical** |

### Second verification pass (2026-10-22 (c)) — a reviewer replayed the plan against the tree

A **third** review pass (this one) re-executed the plan's gates against the
live tree instead of reading them. It produced **6 further corrections**,
none of which change a decision — they fix *self-contradictions* and *hidden
scope* that would have surfaced as failures mid-execution:

| # | Correction | Severity |
|---|---|:--:|
| **E1** | **B2's new "second grep gate" contradicted B2's own prescribed fix.** The draft gate matched `panic!`/`unwrap_or_else`/`todo!`/`unimplemented!` — but (a) `unwrap_or_else` is used **benignly 6×** on the tree (`state.rs:76`, `notes/routes.rs:225/228/231`, `error.rs:162`, `config.rs:315`) and is **not** a panic spelling, and (b) `state.rs`'s documented fail-fast **is** `unwrap_or_else(\|_\| panic!(…))` — the exact line B2 tells the executor to write. The gate is now `panic!`/`todo!`/`unimplemented!` with the **one boot-time** `state.rs` fail-fast explicitly allow-listed (see B2's third decision) | **blocking** |
| **E2** | **T63's AC contradicted itself and failed on the current tree.** The AC's grep `axum_valid\|utoipa_axum\|zeroize::\|\btime::` → 0 hits cannot pass: `src/lib.rs:155` uses `time::Duration::hours(24)`, which is the **`time` crate** and is **required** by `tower_sessions::Expiry::OnInactivity` (and T65 removes the *store*, not the `Expiry` call). The same task's later bullet correctly says `time` stays. → `\btime::` removed from that grep | **blocking** |
| **E3** | **T57 would not actually pin the toolchain in CI.** `dtolnay/rust-toolchain@stable` exports `RUSTUP_TOOLCHAIN`, which **overrides `rust-toolchain.toml`** — so a 1.99.1 pin is silently ignored by CI and "local == CI == Docker" is false. T57 must now edit the workflow (and the workflow is added to its Touch-only) | high |
| **E4** | **`clippy::pedantic` is ~370 warnings, not 0.** Measured `cargo clippy --workspace --all-targets -- -W clippy::pedantic` → **~370** (173 `missing_errors_doc`, 45 `must_use_candidate`, 22 `doc_markdown`, ~40 `cast_*`, …). `pedantic = "warn"` + CI's `-D warnings` would make T57 a 370-warning refactor whose `cast_*` fixes change runtime semantics. → `pedantic` is **advisory** (CI adds `-A clippy::pedantic`), the debt is recorded, a follow-up drains it | high |
| **E5** | **`missing_docs = "deny"` is free.** Measured `cargo rustc --lib -- -W missing_docs -W unreachable_pub` → **0 warnings each** (and the gate was proved live by appending an undocumented `pub fn` and watching it fire). The spec's "currently produce 0 warnings" was true but vacuous; the tree is genuinely documented | low |
| **E6** | **T66 must bump `sqlx-cli`.** CI installs `sqlx-cli --version 0.8.6` and `30-db-schema.md:24` pins 0.8.6 — a 0.8.6 CLI cannot regenerate a 0.9 `.sqlx/` cache. The CLI and the driver move together | high |

Additionally: the "**4 dead direct deps**" wording in this file's scorecard,
A1 blast-radius table and task table was corrected to **3** (the count
`time` was **not** dead — see C1); `11-rewrite-deltas.md` had **two**
`## A4.` headings (the new "Naming" section collided with the existing
"Ownership in SQL" section → renumbered **A5**); and `D13`'s `matched-path`
note was corrected (`matched-path` **is** in axum 0.8's default features → no
feature change is needed in T60).

The **repo-side** claims were re-verified against the tree and are exact:
3 lint-gate hits (`src/state.rs:113`, `src/features/columns/routes.rs:137`
and `:191`), 43 `instrument(skip(state))` sites across 8 feature route files
(`auth/routes.rs` has 0 — it already skips correctly), `format_timestamp`
duplicated 9×, `src/common/ownership.rs` a one-line stub, and
`allow_methods = [GET, POST, PATCH, DELETE, OPTIONS]` — **`PUT` missing**.
`cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D
warnings` both exit 0 on the current tree.

**Re-verified after the five landed tasks (2026-10-22 (e)) — the claims above
are a SNAPSHOT of the tree as it was when this file was written.** Two of them
have since changed (both **fixed**, which is the point): the 3 lint-gate hits
are gone (T55) and `allow_methods` now carries `PUT` (T55). The rest still
hold, re-measured against the tree today: **43** `instrument(skip(state))`
sites across 8 route files, **`format_timestamp` duplicated 9×**,
`common/ownership.rs` still a one-line stub, the **`sqlx-cli` pin still
0.8.6**, and **`deny.toml` still carrying its 4 task-bound advisory
ignores** (the two T63 owned were deleted with their crates). **One claim
needs its command updated:** `cargo clippy … -- -D warnings` now **fails**
(~290 pedantic errors) because T57 made `clippy::pedantic` advisory — the
canonical form is `-D warnings -A clippy::pedantic`.

---

## Part A — Rust-idiom naming amendments

### A1 — `APP_ENV` replaces `NODE_ENV`

**Decision.** The runtime-mode env var is `APP_ENV`, with the **value set
unchanged**: `development | test | production`.

**Rationale.**

* `NODE_ENV` is a **Node.js/npm convention** (`npm` sets it; `react-scripts`
  branches on it). It has no meaning in a Rust binary and imports a whole
  ecosystem's baggage into a service that shares no runtime with Node. The
  Nest service keeps `NODE_ENV` — it *is* a Node service. The two services are
  separate containers in `compose.yaml` and legitimately use different names.
* `APP_ENV` is the Rust-web convention (`actix-web`/`axum` community
  templates use `APP_HOST`/`APP_PORT`/`APP_ENV`; `sqlx` uses `DATABASE_URL`;
  `tracing` uses `RUST_LOG`). It is namespaced enough to not collide and
  generic enough to not rebrand the project.
* Rejected alternatives:
  * `RUST_ENV` — reads as "the Rust compiler's env" and collides conceptually
    with `RUST_LOG`/`RUST_BACKTRACE` (which mean something else entirely).
  * `KANPAD_ENV` / `KANPAD__ENV` — app-namespaced, but the config table
    already uses **unprefixed descriptive names** (`PORT`, `HASH_COST`,
    `CORS_ORIGINS`, `TRUSTED_PROXIES`, `SESSION_SECRET`) with `DB_*` /
    `DATABASE_URL` for the DB parts. `APP_ENV` fits that table; a `KANPAD__`
    prefix would be the only key in it with one.
  * `PROFILE` — collides with `cargo`'s build profile vocabulary.
* **Zero plumbing cost:** `envy` derives `SCREAMING_SNAKE_CASE` from the field
  name, so `Settings.node_env: String` → `Settings.app_env: String` makes
  envy read `APP_ENV` with no prefix/rename configuration.

**Fail-fast rule (load-bearing).** `APP_ENV` defaults to `development` for
developer convenience — **but** if `NODE_ENV` is present in the environment,
boot **fails** with an actionable error:

```
configuration error: NODE_ENV is not used by backend-rs (this is a Rust
service, not a Node one) — set APP_ENV=development|test|production instead
```

Without this, a deployment that still exports `NODE_ENV=production` and has
not yet added `APP_ENV` would **silently boot as `development`** — losing
`Secure` cookies, the `__Host-` prefix, prod HSTS, and the deny-all CORS
default. A silent security downgrade is the one outcome the config layer
already refuses to allow elsewhere (DB-name screening, charset requirement,
`HASH_COST` prod floor, `Any`+credentials CORS). This rename must not be the
first change in the series that fails *open*.

**Blast radius.**

| File | Change |
|---|---|
| `backend-rs/src/config.rs` | field `node_env` → `app_env`; whitelist error text; the `SETTINGS_KEYS`/`BASE_VARS` env matrices in the unit tests; **+ the new `NODE_ENV`-present → boot-error rule**; `Settings::default()` sets `app_env: "test"` |
| `backend-rs/src/config.rs` | `is_production()` reads `app_env` (the method name is already env-agnostic) — it lives in `config`, not `state` |
| `backend-rs/src/lib.rs` | `state.settings.is_production()` call sites unchanged |
| `backend-rs/tests/api/helpers.rs` | `test_settings()` sets `app_env: "test"`; doc comment `NODE_ENV=test` → `APP_ENV=test` |
| `backend-rs/tests/api/*.rs` | any suite that builds `Settings { node_env: … }` |
| `backend-rs/.env` (untracked) | `NODE_ENV=development` → `APP_ENV=development` |
| `backend-rs/.env.test` (untracked) | add `APP_ENV=test` |
| `compose.yaml` | the **rust** service only: `NODE_ENV: ${NODE_ENV:-development}` → `APP_ENV: ${APP_ENV:-development}`. The **nest** service keeps `NODE_ENV` unchanged. |
| `.github/workflows/backend-rs.yml` | `-e NODE_ENV=test` → `-e APP_ENV=test` (the file was at `backend-rs/.github/workflows/ci.yml` when T54 edited it, and moved to the repo root in T55) |
| `specs/api-rewrite-rs-specs/54-rust-idiom-hardening.md` | **Part C** (dependency hygiene & modernization) + the D9/D15/D16/D17 corrections + the rewritten B7 Origin decision — this file (T63/T65/T66/T67 land here) |
| `specs/api-rewrite-rs-specs/10-architecture.md` | **Pinned versions** table rewritten (sqlx 0.9 / tower-http 0.7 / tower-sessions 0.15 / utoipa 6 / the removal of `axum-valid`, `utoipa-axum`, `zeroize`, `time`); **Sessions** section -> the in-tree store; **CORS** section -> the canonicalized allowlist + `CsrfLayer`; **Middleware order** -> the cross-origin layer's position; **Config** -> the `CORS_ORIGINS` canonical-form rules + the `DATABASE_URL` collation pin |
| `specs/api-rewrite-rs-specs/11-rewrite-deltas.md` | **E4** rewritten around the Go 1.25 `CrossOriginProtection` algorithm; **E5** gains the session-store rationale + the toolchain decision |
| `specs/api-rewrite-rs-specs/50-tasks.md` | Phase 7 gains **T63 / T65 / T66 / T67**; T56 re-specified around `CsrfLayer`; T57's toolchain pin becomes **1.99.1**; the ordering paragraph rewritten |
| `specs/api-rewrite-rs-specs/00-overview.md` | **Amendment 12** — the dependency-hygiene decision log; **Amendment 13** — the implementation status (2026-10-22 (e)); **Amendment 14** — the (c) verification pass |
| `specs/api-rewrite-rs-specs/README.md` | reading order + the amendment header |
| `backend-rs/Cargo.toml` + `backend-rs/Cargo.lock` | T63 (the **three** removals + the two feature removals), T65 (the store swap), T67 (the library majors), T66 (the driver major) — **one owner per task**, never two branches at once |
| `backend-rs/Dockerfile` | T57 — `rust:1.96-bookworm` -> `rust:1.99-bookworm`, in the **same PR** as the toolchain pin |
| `backend-rs/.env` (untracked) | T66 — `collation=utf8mb4_unicode_ci` added to the `DATABASE_URL` |
| `compose.yaml` | T66 — the same `DATABASE_URL` pin; T54 — the `APP_ENV` rename (the **nest** service keeps `NODE_ENV`) |
| `.github/workflows/backend-rs.yml` | T55 (the move) + T56/T62 (the gates) + T66 (the `DATABASE_URL` pin) |
| `docs/data-migration-live.md` | T54 (the env block + the `set-cookie:` expectation), T66 (the `DATABASE_URL` pin), **and the new collation invariant** (every Rust database is created with an explicit `COLLATE utf8mb4_unicode_ci`, never the server default) |
| `BACKEND-RS-ORCHESTRATION.md` | T66 — the shared test DB's `DATABASE_URL` carries `collation=utf8mb4_unicode_ci`; plus the current-state pointer for the new tasks |
| `backend-rs/src/extract.rs` | T63 — the **stale comment** at `:12-18` (the `axum-valid` justification) is corrected so the next contributor does not re-add the dependency |
| `docs/data-migration-live.md` | the operator runbook's env block + the `__Host-` note (see A2) |
| root `AGENTS.md` | the "session cookie name is env-dependent" lesson |

**Not a wire-contract change.** `APP_ENV` never appears in a response body, a
header, or a route. → **no `openapi.json` regeneration, no `schema.d.ts`
regeneration, no `npm run gen:api`.** (This is why T54 is *not* a T52
contract-amend task; only the cookie name in A2 touches
`CONTRACT-CHANGES.md`.)

### A2 — Session cookie: `kanpad.sid` (dev/test) and `__Host-kanpad.sid` (prod)

**Decision.** The session cookie name is **app-scoped**, not framework-scoped:

| `APP_ENV` | Cookie name | `Secure` | Other attributes |
|---|---|---|---|
| `production` | `__Host-kanpad.sid` | yes | `Path=/`, `HttpOnly`, `SameSite=Lax`, **no `Domain`** |
| `development` / `test` | `kanpad.sid` | no | `Path=/`, `HttpOnly`, `SameSite=Lax`, **no `Domain`** |

**Rationale.**

* `connect.sid` is **express-session's default**, copied verbatim from Nest's
  `fastifySession({ cookieName: 'connect.sid' })` (`backend/src/main.ts:39`).
  It is the archetypal NestJS-ism that `00-overview.md` Amendment 10 forbids.
  A Rust service that names its session cookie after an Express middleware
  misidentifies itself to every operator reading a `Set-Cookie` header, to
  every browser extension, and to every log-scraper that keys on cookie names.
  *(2026-10-22: `tower-sessions`' own default cookie name is `"id"`, and its
  own docs say "OWASP recommends not using the default name" — so the rename
  also steps away from **both** framework defaults, not just Express's.)*
* `kanpad.sid` is app-scoped, dot-separated (same shape as `connect.sid`, so
  it is still a valid RFC 6265 `token` — `.` is a token character), and cannot
  collide with any other session cookie on the same host.
* The `__Host-` prefix **stays**, because it is load-bearing hardening: it
  makes the cookie **host-only** (no `Domain` ⇒ no subdomain fixation), it
  requires `Secure` (⇒ no plaintext transmission), and it requires `Path=/`.
  Renaming the *base* keeps that hardening intact.
* **Why the prefix stays environment-conditional:** `__Host-` mandates
  `Secure`, and `Secure` cookies are only transmitted over HTTPS. The staging
  box in `docs/data-migration-live.md` serves **plain HTTP** on a non-localhost
  host, so an unconditional `__Host-` cookie there would make login
  impossible. The dev/test name is therefore required, not a convenience.
* Rejected alternatives:
  * *Always `__Host-kanpad.sid` + always `Secure`* — modern Chrome/Firefox
    accept `Secure` cookies on `http://localhost` (potentially trustworthy
    origin), but plain-HTTP **staging** and **curl** flows do not. It would
    also make the in-process test suites and the
    `docs/data-migration-live.md` verification steps depend on browser-specific
    leniency. Rejected as fragile, not as wrong.
  * *Configurable `SESSION_COOKIE_NAME`* — a config key whose only effect is
    to let an operator accidentally drop the `__Host-` prefix in production.
    Rejected: one canonical name, derived from `APP_ENV`.
  * *No prefix at all* — loses host-only binding and the `Secure` mandate.
    Rejected.

**New invariant (must be asserted, not assumed).** The `__Host-` prefix is
only *legal* under RFC 6265bis when the cookie carries **`Secure` +
`Path=/` + no `Domain`**. tower-sessions satisfies that today only because
nobody calls `.with_domain(...)`. The invariant must be **test-locked**, so a
future `.with_domain()` call breaks a test instead of production:

* production: `Set-Cookie` MUST start with `__Host-kanpad.sid=` and MUST
  carry `Secure`, `Path=/`, `HttpOnly`, `SameSite=Lax`, and MUST NOT carry
  `Domain`;
* dev/test: `Set-Cookie` MUST start with `kanpad.sid=` and MUST NOT carry
  `Secure`;
* **regression guard:** no env may emit a cookie named `connect.sid` (the
  Express name must never come back).

**Consequence worth stating explicitly:** in production the `__Host-` prefix
makes a *plain-HTTP* production deployment fail **visibly** (the browser
rejects the cookie ⇒ every request is anonymous ⇒ 401 on login) rather than
silently serving an insecure session cookie. Production therefore **requires
HTTPS**. That is the correct direction to fail.

**Blast radius.**

| File | Change |
|---|---|
| `backend-rs/src/config.rs` | `session_cookie_name()` → `__Host-kanpad.sid` / `kanpad.sid`; the base name becomes the named const `config::SESSION_COOKIE_NAME` and the prefix `config::HOST_COOKIE_PREFIX` (the existing `SESSION_TABLE` const lives in `state.rs`, so the cookie consts sit with the other settings constants in `config`) |
| `backend-rs/src/lib.rs` | `SessionManagerLayer::…with_name(settings.session_cookie_name())` — unchanged call shape |
| `backend-rs/tests/api/auth_session.rs` | the cookie-attribute matrix assertions (the T11 env-matrix test) — update expected names + add the `connect.sid` regression guard |
| `backend-rs/tests/api/*.rs` | the black-box suites' cookie-jar helpers — the **name is opaque to them** (they parse `Set-Cookie` generically), so no change unless a suite hard-codes the name |
| `CONTRACT-CHANGES.md` | §1 "Transport" row + §10 "Auth notes" — the cookie-name row is the one observable change |
| `docs/data-migration-live.md` | the operator's `set-cookie:` expectation line + the `__Host-` note |
| root `AGENTS.md` | the "session cookie name is env-dependent" lesson |

**Not a wire-contract change either** — `openapi.json` mentions "cookie" only
in one prose description (the logout handler's doc comment) and never the
name; the frontend uses `credentials: 'include'` and never names the cookie.
→ **no `openapi.json` / `schema.d.ts` regeneration.** (Verified:
`grep -c 'connect.sid' backend-rs/openapi.json` → 0.)

**`CONTRACT-CHANGES.md` update (T52 shape).** The Transport row becomes:

```
| Transport | cookie session (`connect.sid`, 24h, `credentials:include`) |
same mechanism (`tower-sessions` 0.14 + sqlx store, sliding 24h,
`credentials:include`), cookie renamed to `kanpad.sid` (`__Host-kanpad.sid`
in production) — the Express-derived `connect.sid` name is not carried over
| **keep mechanism, break name** |
```

---

## Part B — Quality baseline (findings)

Each finding below is **verified against the tree**, not inferred. The
evidence lines are reproducible from `backend-rs/`.

### B1 — CI has never run (P0)

A workflow file existed at `backend-rs/.github/workflows/ci.yml` — thorough (fmt → clippy → grep
gates → migrate → `SHOW CREATE TABLE` assert → `sqlx prepare --check` → test →
`cargo deny` → openapi diff → docker build → `:3001` smoke →
vitest/playwright), and **does not execute**: GitHub Actions discovers
workflows **only** at the repository root's `.github/workflows/`. The file's
own header says so:

> "GitHub Actions discovers workflows ONLY at the repo-root .github/workflows/,
> so this workflow does NOT auto-run until a root-level caller exists
> (orchestrator / T52 decision: move it, or add a root workflow that reuses
> it)."

The consequence is that every gate in that file is unverified — including the
two in B2 and B3.

**Decision.** **Move** the file to `<repo-root>/.github/workflows/backend-rs.yml`
(a pure path rename — the workflow already uses repo-root-relative paths and
`working-directory: backend-rs`, which becomes correct the moment the file
lives at the root). Do **not** add a root wrapper that calls the nested file:
GitHub has no "include" mechanism, so a wrapper would have to duplicate the
whole job graph, and the two copies would drift.

**Second decision.** The move is a **rename, not a copy** — the nested file is
deleted in the same commit. Leaving it behind would create the exact
"two sources of truth" failure this series exists to remove.

**Third decision — the first run will surface latent failures.** The workflow
has never executed, so T55 must budget for fixing whatever it finds (the
`docker build` step was never actually run to completion — the orchestration
log records *"docker build substituted w/ review — disk infeasible"*). T55's
AC therefore includes *"the workflow runs green on the PR"*, not *"the
workflow file is correct"*.

**What the first run actually surfaced (T68, 2026-10-22).** Moving the
workflow to the root made it **discoverable** — and therefore **verifiable**
— for the first time. Replaying every step by hand (no `gh` on the dev box,
so the run itself still needs a push) found **two steps that were RED on
every run**, both latent since the workflow was authored:

1. **The `cargo audit` step contradicted the committed `deny.toml`.** The
   workflow ran **two** advisory gates with **different** policies:
   `cargo-deny` reads `deny.toml` (the four task-bound ignores), while
   `cargo-audit` reads `.cargo/audit.toml` — which does not exist — so it
   never saw the ignores and failed on exactly the advisories the project had
   deliberately, and with a reason, accepted (`error: 4 vulnerabilities
   found!`, exit 1). **Two advisory gates with two policies guarantee one is
   red.** The step is removed: `cargo deny check advisories` reads the same
   RustSec DB, additionally checks yanked crates, honours the task-bound
   ignores, and *is* the committed policy.
2. **The SHOW CREATE TABLE assert queried a schema that no longer exists.**
   It asserted the store-owned session table at `TABLE_SCHEMA =
   'tower_sessions' AND TABLE_NAME = 'session'`, but the 2026-10-19 store
   change moved the store into the **app database** (`sessions`, created by
   the baseline migration). The query returned an empty row and the `case`
   treated that as a failure → the step was red on every run. The special
   case is deleted: the baseline creates `sessions` with the same explicit
   `ENGINE=InnoDB … COLLATE=utf8mb4_unicode_ci` as every other app table, so
   the general query already covers it.

**The lesson generalises past this workflow:** *a pipeline that has never run
is not a pipeline.* Moving it to a discoverable location is what made the
defects visible — and the first thing it revealed was that two of its own
steps had been broken since the day they were written. **Replay every step,
not just the ones you changed.**

### B2 — The repo's own lint gate fails on the current tree (P0)

The CI grep gate *"no `unwrap()/expect()` outside `main.rs` and `#[cfg(test)]`"*
(replaying the CI's exact `awk` brace-aware filter over `src/`) reports:

```
src/state.rs:113:                  .expect("SESSION_TABLE is a valid identifier")
src/features/columns/routes.rs:137: .expect("created column renders"),
src/features/columns/routes.rs:191: .expect("updated column renders"),
```

This is why B1 matters: **the gate has never been able to fail**, because the
gate has never run.

**Third gate — REPAIRED in T55 (2026-10-22 (c)).** The `no format! in
store.rs` gate had also never run, and it failed on the current tree for two
reasons that are the GATE's fault, not the code's: (a) a **doc comment** in
`columns/store.rs` merely mentioning `format!` was matched as code; (b)
`notes::escape_like_pattern` builds a **bound value** (`format!("%{escaped}%")`
→ `push_bind`) — the sanctioned LIKE-escape pattern (`30-db-schema.md`), not
SQL text. The gate now skips comment lines and accumulates each `format!(...)`
call, flagging it only when the built text carries a SQL keyword — i.e. it
enforces the gate's real intent ("no `format!` may BUILD SQL"). **Lesson: a
gate that has never executed fails on false positives first — fix the GATE
when its intent is right and its matcher is wrong.**

**Decision — fix the code, not the gate.** The gate's intent is right
(`00-overview.md` Constitution: *"No `unwrap()/expect()` in request paths"*).
Both sites have a strictly better form:

1. **`columns/routes.rs` — make the invariant structural.** The current shape
   builds a one-element `Vec`, pops it, and asserts it was there:

   ```rust
   data: to_response(Vec::from([row]), &summaries)
       .pop()
       .expect("created column renders"),
   ```

   The cardinality is a compile-time fact, so it must not be a runtime
   assertion. Add a single-row constructor next to `to_response` in
   `src/features/columns/store.rs` and use it:

   ```rust
   /// Render ONE column for the wire (the caller knows there is exactly one
   /// row — the `Vec` round-trip in `to_response` is for the list case only).
   pub fn to_response_one(
       row: ColumnRow,
       summaries: &std::collections::HashMap<i32, Vec<crate::common::card_summary::CardSummary>>,
   ) -> ColumnResponse {
       let cards = summaries.get(&row.id.get()).cloned().unwrap_or_default();
       ColumnResponse {
           id: row.id,
           name: row.name,
           position: row.position,
           board_id: row.board_id,
           cards,
           created_at: format_timestamp(&row.created_at),
           updated_at: format_timestamp(&row.updated_at),
       }
   }
   ```

   Call sites become `data: to_response_one(row, &summaries)` — no `Vec`, no
   `pop`, no `expect`, and the compiler now enforces the cardinality.

2. **`state.rs` — use the file's existing fail-fast idiom.** The
   `with_schema_name` call immediately above already panics via
   `unwrap_or_else(|_| panic!(…))` with a documented contract (and
   `AppState::with_pool` carries a `# Panics` section). The table-name call
   should use the **same** idiom rather than `expect`:

   ```rust
   .with_table_name(SESSION_TABLE)
   .unwrap_or_else(|_| {
       panic!(
           "session store rejected table name {SESSION_TABLE:?}: it must satisfy the \
            store's identifier rule (see db::is_valid_db_name)."
       )
   })
   ```

   `SESSION_TABLE` is a `const` literal (`"sessions"`) that a unit test
   already pins against `db::is_valid_db_name`
   (`state::tests::session_table_lives_in_the_app_database`), so the panic is
   unreachable — but it is *explicitly* unreachable, which is what the gate
   wants.

**Third decision — close the gate's blind spot.** The gate greps only
`src/`; `tests/` is unconstrained (correct — tests may unwrap). But the gate
also **cannot see a future `unwrap` added inside a `#[cfg(test)]` module's
brace range**, and it cannot see `panic!`/`todo!`/`unimplemented!` at all. Add
a **second** gate to the CI that matches the *other* panic spellings in request
paths, so the class is covered rather than the three known instances:

```bash
# no panic!/todo!/unimplemented! in src/ outside main.rs + #[cfg(test)]
```

**CORRECTED 2026-10-22 (c) — the first draft of this gate also matched
`unwrap_or_else`, which was wrong twice over.** (1) **`unwrap_or_else` is not
a panic spelling.** It is used **benignly 6×** on the current tree
(`state.rs:76`, `features/notes/routes.rs:225/228/231`, `error.rs:162`,
`config.rs:315`) — e.g. `.unwrap_or_else(|| self.db_name.clone())` — so
matching it would fail the gate on code that cannot panic at all. (2) **The
gate would have failed on the fix B2 itself prescribes**: `state.rs`'s
documented fail-fast is *precisely* `unwrap_or_else(|_| panic!(…))`, so a
gate matching either `unwrap_or_else` or `panic!` flags the very line B2
tells the executor to write.

**The gate therefore matches `panic!`/`todo!`/`unimplemented!` — and the ONE
documented fail-fast is allow-listed explicitly** (an allow-list, not a
silent exemption, so the exemption is visible in the workflow):

```bash
# no panic!/todo!/unimplemented! in src/ outside main.rs + #[cfg(test)]
# ALLOWLIST (documented fail-fast, not a request-path panic):
#   src/state.rs  — the session-store identifier guard (see the # Panics
#   contract on AppState::with_pool). It runs at BOOT, never per request.
```

**Why the allow-list is a single boot-time site, not a request path.**
`state.rs`'s panic is in `session_store()`, called once while building the
router — it cannot be triggered by a request. The gate's purpose is
"no panic in a request path", and boot-time invariants are a different
category. **Note (sequencing):** T65 replaces the store in-tree, so
`state.rs`'s `with_schema_name`/`with_table_name` chain disappears entirely —
this allow-list entry becomes obsolete in T65. T65 must then drop the
allow-list entry (or the gate fails for a reason that no longer exists).

### B3 — CORS `allow_methods` is missing `PUT` (P0)

`src/lib.rs` allows `GET, POST, PATCH, DELETE, OPTIONS`. But
`src/features/cards/routes.rs` registers:

```rust
.route("/cards/{id}/labels/{label_id}",
       put(attach_card_label).delete(detach_card_label))
```

and the frontend calls it — `frontend/src/features/cards/cards.api.ts:113`
(`method: 'PUT'`). `PUT` is **not** a CORS-safelisted method, so the browser
sends a **preflight**; the server answers `Access-Control-Allow-Methods`
**without `PUT`**, and the browser blocks the request.

**This bites in the shipped topology.** `compose.yaml:70` sets
`VITE_API_URL=http://localhost:3001` and `frontend/src/lib/api/client.ts:67`
does `BASE = import.meta.env.VITE_API_URL ?? ''` — so in compose the browser
calls `:3001` **cross-origin** from `:5173` (the Vite proxy is bypassed
whenever `VITE_API_URL` is set). The origin `http://localhost:5173` **is** in
the default allowlist, so every other verb works; **only label-attach fails.**
That is the worst possible failure shape: one endpoint, one verb, one feature,
with a browser-console CORS error instead of an application error.

**Decision.**

1. Add `Method::PUT` to `allow_methods`. **LANDED IN T55** (2026-10-22 (c)) —
   the fix moved here from T56, because the preflight guard below has to be
   GREEN when it arrives: with `PUT` missing, the guard fails, and a guard
   that lands red in the freshly-activated CI blocks every later task.
2. Fix the same omission in `10-architecture.md` (CORS section lists the same
   five methods).
3. **Add a preflight test** so the route surface and the CORS policy cannot
   drift again — the class of bug is "the policy is hand-maintained and
   drifts", so the guard must be automated:

   ```rust
   // tests/api/cors.rs — new suite (LANDED IN T55, 2026-10-22 (c))
   // Every verb the API actually serves must survive a CORS preflight.
   // NOTE: the PRE-EXISTING `health::cors_preflight_matrix` asserted the CORS
   // *mechanism* (ACAO + credentials) for POST only — it never enumerated the
   // verbs, which is exactly why the PUT drift survived. The new suite
   // enumerates all five served verbs.
   #[sqlx::test(migrations = "./migrations")]
   async fn preflight_allows_every_registered_verb(pool: MySqlPool) {
       let app = app_on_pool(&pool);
       for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
           let response = app.clone().oneshot(
               Request::builder()
                   .method("OPTIONS")
                   .uri("/api/v1/cards/1/labels/2")
                   .header("origin", "http://localhost:5173")
                   .header("access-control-request-method", method)
                   .body(Body::empty()).unwrap(),
           ).await.unwrap();
           let allow = response.headers()
               .get("access-control-allow-methods")
               .and_then(|v| v.to_str().ok())
               .unwrap_or("");
           assert!(allow.contains(method), "CORS must allow {method}, got {allow:?}");
       }
   }
   ```

4. **Non-decision:** the allowed-method list stays hand-written. axum does not
   expose the registered route table, so a derived list is not available
   without a custom route-introspection layer — which is more machinery than
   the risk warrants. The preflight test above is the guard.

### B4 — Request bodies are recorded in tracing spans (P1)

43 handlers across the 8 feature `routes.rs` files use
`#[tracing::instrument(skip(state))]`. `#[instrument]` records **every
argument it is not told to skip**, via that argument's `Debug` impl:

* `auth: AuthSession` → derives `Debug`, holds **`email`** (PII);
* `input: ValidJson<CreateCardInput>` → `ValidJson` derives `Debug`;
  `CreateCardInput` derives `Debug` and holds **`title`** and
  **`description`** (up to 10 000 chars).

So every card create/board create/note create records the **full request
body** into the span, which lands in the log sink. The **auth** handlers get
this right (`skip(state, session, input)`) — which is exactly what makes the
resource handlers look like an oversight rather than a decision.

**Decision.**

1. Every handler skips its body and session: `skip(state, auth, input)`
   (adjust per signature — `session` instead of `input` where a `Session` is
   taken; `path`/`id` are fine to record).
2. **Add a CI gate** so this cannot regress — the same shape as the existing
   `unwrap` gate:

   ```bash
   # Handlers must skip the request body + session from their span:
   # `instrument(skip(state))` records `auth` (email) and `input` (the DTO
   # body) via their Debug impls.
   if grep -rn 'instrument(skip(state))' src/; then
     echo 'FAIL: handlers must skip(auth, input) — the span would log the request body'
     exit 1
   fi
   ```

3. `ValidJson`/`ValidQuery` could also implement `Debug` manually to redact
   the payload — **non-decision**: skipping is enough and is what the auth
   handlers already do; a manual `Debug` would be a second mechanism for one
   problem.

### B5 — Build & supply-chain gaps (P1)

| Gap | Fix |
|---|---|
| No `[lints]` table and no crate-level lint attributes (`lib.rs`/`main.rs` have **zero** `#![warn]`/`#![deny]`) | Add the `Cargo.toml` `[lints]` table (Rust 1.74+). `unsafe_code = "forbid"` makes the (currently true) zero-`unsafe` property enforced. `missing_docs = "deny"` promotes axis 7's 10.0 from convention to a build gate. **MEASURED 2026-10-22 (c):** the tree is **already `missing_docs`-clean and `unreachable_pub`-clean — 0 warnings each** (verified by `cargo rustc --lib -- -W missing_docs -W unreachable_pub`, and the gate was proved live by appending an undocumented `pub fn` and watching it fire). So `missing_docs = "deny"` costs **zero** doc-writing today — it only forbids NEW gaps. |
| No `[profile.release]` — the Docker image ships a stock release build (no LTO, no `codegen-units = 1`, **debug symbols shipped**) | `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`. **`panic = "abort"` is deliberately NOT set** — axum/tower catch panics per task; aborting would kill the process instead of returning a 500. Say so in a comment. |
| No `rust-toolchain.toml` — the Dockerfile pins `rust:1.96-bookworm` while CI uses `dtolnay/rust-toolchain@stable` (floating) | Add `rust-toolchain.toml` pinning **`1.99.1`** + `components = ["clippy", "rustfmt"]`, and bump the Dockerfile base to `rust:1.99-bookworm` **in the same PR** (the Dockerfile and the toolchain move together, never one alone). `rust-version = "1.99"` in `Cargo.toml` documents the MSRV floor. **Corrected 2026-10-22 (b):** the current stable is **`1.99.1`** (released **2026-10-15** — the channel manifest's `date`; the first draft of this spec named the superseded `1.99.0` and mis-dated the release). `1.99.1` clears the MSRV ceiling of the whole target dependency set (sqlx 0.9's `1.94.0` is the highest; everything else is <= 1.88) — see `54` Part C §C5. The pin is justified as *"match the Dockerfile, and match the dependency set's MSRV ceiling"* — not "latest". |
| No `deny.toml` — the CI runs `cargo deny init` and checks against a **generated default** | Commit a real `deny.toml` (advisories + licenses + bans). Supply-chain policy must be a committed artifact, not a generated one. **Expect the FIRST run to fail** (2026-10-22): a naive `[bans] multiple-versions = "deny"` + `wildcards = "deny"` trips over the duplicate crate versions every real tree has, and `[licenses] allow` must enumerate the SPDX set the tree actually uses. The task must therefore *derive* the policy from a first failing run (add the `skip`/`skip-tree` entries the output names, and paste the license set cargo-deny reports) — not ship a guessed allow-list. **WHAT THE FIRST RUN ACTUALLY REPORTED (T57, 2026-10-22):** (a) *licenses* rejected **BSD-3-Clause** (zstd) and every non-MIT/Apache licence → the allow-list is derived from `cargo metadata` over the resolved graph (**minimal** set: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, ISC, Unicode-3.0, Zlib, CDLA-Permissive-2.0 — OR-expressions are satisfied by ONE alternative, so LGPL/0BSD/Unlicense/BSL are NOT needed); (b) *bans* rejected the tree on `multiple-versions = "deny"` (20+ legitimate duplicates) → duplicates are a **warning**, not an error (a hard deny blocks every dependency bump); (c) *advisories* reported **6 real advisories**, and **every one of them is removed by a LATER task in this same phase** — so they are recorded as **task-bound ignores** with their removal condition, not as a blanket allow: `idna 0.5` (RUSTSEC-2024-0421) and `paste` (RUSTSEC-2024-0436) arrive only via the dead `axum-valid`/`utoipa-axum` (**T63**); `zip 3.0` ×3 (RUSTSEC-2026-0367/0368/0369) is a **build-time** dep of utoipa-swagger-ui 9 (**T67** → swagger-ui 10 requires `zip ^9`); `rsa 0.9` (Marvin, RUSTSEC-2023-0071) arrives only via sqlx-mysql 0.8's `caching_sha2_password` auth (**T66** → sqlx 0.9 uses `rsa ^0.10.0-rc.18`). **A stale ignore is a silent hole in the gate: the named task MUST delete its entry and the check must then pass without it.** |
| `Cargo.toml` has no `license` / `repository` / `description` / `rust-version` | Add them (`cargo deny check licenses` flags the crate itself without a `license`; `rust-version` documents the MSRV for `cargo` consumers and pins the toolchain floor). |
| `edition = "2021"` on a 1.96 toolchain | Bump to **`2024`**. Verified blockers are absent: **0** `unsafe`, **0** `gen` identifiers, **0** `if let … else` scrutinee temporaries, no `extern`/`static mut`. Watch the new 2024 lints on the first build: `tail_expr_drop_order`, `if_let_rescope`, `rust_2024_incompatible_pat`. **CORRECTED 2026-10-22 (d) — the "no blockers" claim was INCOMPLETE (T57 hit two real ones):** (a) **edition 2024 makes `std::env::set_var` / `remove_var` UNSAFE** (they race with any concurrent environment access) — and the config tests mutated the process env to drive `Settings::load_from_env()`. With `unsafe_code = "forbid"` the honest fix is to **stop mutating the env**: `Settings::from_pairs(&[(k, v)])` is a test seam that loads from an explicit map (same deserialize + validate path, different source), so the tests no longer need `unsafe` **or** the `ENV_LOCK` mutex (they become parallel-safe). (b) **clippy 1.99 adds 8 `collapsible_if` errors** that clippy 1.96 did not report (the `if let … { if … }` → let-chain collapse, which edition 2024 makes expressible) — fixed with let-chains in `boards/cards/notes/projects/types.rs` + `lib.rs` + one e2e test. **Side effect that must be in the plan (2026-10-22):** `edition = "2024"` implies **`resolver = "3"`**, which enables cargo's Rust-version-aware dependency resolver (`incompatible-rust-versions = "fallback"`). The **dependency set can therefore change** — some crates may be *downgraded* to a version that still supports our MSRV. `Cargo.lock` must be regenerated and the diff **reviewed line by line**; a silent dependency downgrade is a supply-chain event, not a chore. |
| `zeroize = "1"` is a direct dependency but is never used directly (only `secrecy` uses it) | Drop it from `[dependencies]` (it stays a transitive dep of `secrecy`). `cargo udeps` / `cargo machete` would flag it. |
| **`clippy::pedantic` is not on, and the tree is far from pedantic-clean** | **MEASURED 2026-10-22 (c):** `cargo clippy --workspace --all-targets -- -W clippy::pedantic` reports **~370 warnings** (296 lib + 75 test, minus duplicates) — 173 `missing_errors_doc`, 45 `must_use_candidate` (27 fn + 18 method), 22 `doc_markdown`, ~40 `cast_possible_truncation`/`cast_sign_loss`/`cast_possible_wrap`, 10 `ref_option`, 8 `match_single_binding`, 7 `option_option`, 5 `let_else`, 4 `string_add`, 3 `redundant_closure`, 2 `unused_async` + the tail. **Consequence:** `pedantic = "warn"` in the lints table + CI's `cargo clippy -- -D warnings` would make **all ~370 an error** — T57 would silently become a 370-warning refactor, and the `cast_*` family in particular cannot be "fixed" without changing runtime semantics. **Decision: `pedantic` is ADVISORY.** The lints table sets `pedantic = { level = "warn", priority = -1 }` (so a local `cargo clippy` shows the debt as a to-do list) and the **CI clippy step adds `-A clippy::pedantic`** after `-D warnings` (the more specific + later flag wins), keeping pedantic **non-blocking** until a follow-up task drains it. The measured debt (~370) is recorded so the follow-up has a number to work down, not a guess. |
| **CI's `dtolnay/rust-toolchain@stable` OVERRIDES `rust-toolchain.toml`** | The workflow's toolchain step was `uses: dtolnay/rust-toolchain@stable` (in the pre-T55 file at `backend-rs/.github/workflows/ci.yml:69`; the file now lives at the repo root). That action installs `stable` and exports `RUSTUP_TOOLCHAIN`, and **`RUSTUP_TOOLCHAIN` takes precedence over `rust-toolchain.toml`** — so a `rust-toolchain.toml` pinning 1.99.1 would be **silently ignored by CI**, and the "local == CI == Docker" claim would be false. **Decision:** T57 must **edit the workflow** (replace the floating `@stable` action with the pinned channel, or delete the step and let rustup honour `rust-toolchain.toml`), and `.github/workflows/backend-rs.yml` is added to T57's **Touch only**. A pinned `rust-toolchain.toml` that CI ignores is worse than no pin — it looks enforced and is not. |
| **`sqlx-cli` is pinned to `0.8.6`** — the offline-cache tool must match the driver major | The CI installs `cargo install sqlx-cli --version 0.8.6` (`.github/workflows/backend-rs.yml`) and `30-db-schema.md:24` records the same pin. **A 0.8.6 CLI cannot regenerate a 0.9 `.sqlx/` cache**, so T66 (sqlx 0.9) must bump the CLI in the workflow **and** in `30-db-schema.md`. The CLI version and the driver version move **together** — a mismatched pair produces a confusing "cache is out of date" failure instead of a clear one. |

### B6 — Shared-kernel DRY (P2)

* **`format_timestamp` is duplicated 9×** with an identical body:
  `common/card_summary.rs` + `features/{cards,boards,columns,labels,notes,projects,tags,checklists}/types.rs`.
  The stated rationale (`card_summary.rs:84-86`) is *"kept per-module so
  features never import each other's **stores**"* — but that rule is about
  **SQL ownership**, not about `types.rs`. `common/` is precisely the neutral
  shared kernel the architecture already uses for `ids`, `pagination`,
  `validation`, and `label_seed`. **Decision:** one owner,
  `src/common/time.rs::format_timestamp`; delete the other eight; re-export
  from `common::time`. The same over-application is visible in
  `cards/store.rs:468-471` (*"not imported from the labels store"*).
* **`src/common/ownership.rs` is an empty stub** — a module wired into
  `common/mod.rs` whose only content is
  `//! Single owner of user-scoping SQL helpers. Implementation lands with T04/T11.`
  It claims to own helpers that do not exist, while the ownership-check SQL
  *is* duplicated: `cards/store.rs::check_column_owned`,
  `cards/store.rs::check_label_owned`, `boards/store.rs::check_board_owned`,
  `boards/store.rs::check_project_owned`. **Decision:** implement it —
  `common/ownership.rs` is the architecturally correct home for cross-feature
  SQL that is not a feature's own store, and it is the file the architecture
  already names as the owner.
* **`db::with_executor` is a dead no-op** — `pub async fn with_executor<'e, E>(_ex: E) -> Result<(), sqlx::Error> { Ok(()) }`.
  It does nothing; the "compile-time proof" it claims to give is already
  provided by every store fn's own `E: MySqlExecutor<'e>` bound. **Decision:**
  delete it (dead API surface).
* **`AppError::Json` has two byte-identical match arms**
  (`JsonDataError(_) | JsonSyntaxError(_)` and the `_` catch-all) — the
  explicit arm adds nothing over the catch-all. **Decision:** collapse to one
  arm, keep the explanatory comment.
* **`problem_response` takes `detail: String`** — every call site writes
  `.to_string()`. **Decision:** `detail: impl Into<String>`.
* **`/ready` is missing from the OpenAPI paths** — `docs.rs::ApiDoc` lists
  `crate::health` but not `crate::ready`, which is a real served endpoint with
  a documented 503 problem+json body. **Decision:** add it (this one *does*
  touch `openapi.json` → it is a T52-shaped change → see the task split
  below).
* **`main.rs` re-implements sqlx's migration journal** —
  `SELECT version FROM _sqlx_migrations` + a hand-rolled applied/pending diff,
  against a table name that is a sqlx implementation detail.
  `sqlx::Migrator` already exposes applied migrations. **Decision:** use the
  sqlx API; `migrate info` and `migrate run` can then never disagree.

### B7 — Missing CSRF/Origin defence + no CSP (P0 — new finding)

**CSRF.** The API authenticates with a **cookie** (`credentials: 'include'`),
so a browser attaches the session cookie to cross-site requests.
`SameSite=Lax` blocks it for cross-site **POST/PATCH/DELETE** (Lax only sends
the cookie on top-level GET navigations) — so today the protection is real.
But:

* `SameSite=Lax` is a **browser** control. It does nothing for a
  non-browser client (curl, a server-side proxy, a native app) and nothing at
  all if a deployment ever relaxes the cookie's `SameSite` to `None` (needed
  for genuine cross-site embeds).
* The rewrite **is** now the origin that serves the SPA, so it will be
  deployed behind a proxy that may relax cookie attributes for embedding.

**Decision — add a cross-origin check on mutating requests, using the
Go 1.25 `CrossOriginProtection` algorithm (ported to `tower-http` 0.7 as
`CsrfLayer`).** This was **re-decided on 2026-10-22**; the first draft of
this spec proposed a hand-rolled "if `Origin` is present it must be in the
CORS allowlist" check, and that draft had a production cliff:

> **The cliff (D17).** Browsers send `Origin` on **every**
> `POST`/`PUT`/`PATCH`/`DELETE` — same-origin included. The Rust service
> serves the SPA itself, so in production the browser sends
> `Origin: https://app.kanpad.dev` on every write. Under the hand-rolled
> rule that origin must be in `CORS_ORIGINS`, and the spec's own *"empty prod
> allowlist = deny-all (fail closed)"* means an operator who forgets one env
> var gets **every write 403'd** — the whole mutating API, silently broken,
> with a CORS-shaped error that points nowhere near the cause.

The reference algorithm (Filippo Valsorda's research behind Go 1.25's
`net/http.CrossOriginProtection`, which `tower-http` 0.7's `CsrfLayer`
implements step for step) avoids the cliff because it has an
**`Origin`/`Host` fallback**:

| Step | Rule | Effect in KanPad |
|---|---|---|
| 1 | `GET`/`HEAD`/`OPTIONS` → allow | safe methods are never state-changing |
| 2 | `Sec-Fetch-Site: same-origin` or `none` → allow | the same-origin SPA works with **zero** origin configuration |
| 3 | `Sec-Fetch-Site: cross-site` or `same-site` → allow only if the `Origin` is trusted | the compose dev topology (`localhost:5173` → `localhost:3001` is **same-site**, not same-origin — different port) is trusted by default |
| 4 | no `Sec-Fetch-Site` → no `Origin` → allow | curl / server-side proxy / native client (no ambient cookie to abuse) |
| 5 | `Origin`'s authority == the request's effective host → allow | the safety net that makes an **empty** prod allowlist workable for a same-origin SPA |

OWASP's cheat sheet now points at exactly this: *"Starting from 1.25, Go
developers can rely on the built-in `CrossOriginProtection` type… Built-in
defenses are generally preferable because they're maintained by the framework
authors."* `tower-http` 0.7's `CsrfLayer` is that algorithm, and it needs
**no new crate** — only the `csrf` feature flag.

**Three guardrails the adoption requires (they are the reason this is a
task and not a one-line change):**

1. **Canonicalize the allowlist at boot.** `CsrfLayer` matches origins
   **byte-for-byte** and its own docs warn: *"inputs that parse but aren't in
   the canonical browser form above are accepted but will silently never
   match."* `http://localhost:5173/` (trailing slash), `HTTP://LOCALHOST:5173`
   (uppercase), and `http://localhost:5173:80` (explicit default port) would
   each be **silently non-matching** → every write 403s. So the app must
   parse each `CORS_ORIGINS` entry into canonical form (lowercase scheme and
   host, the scheme's default port stripped, no trailing slash, no
   path/query/fragment, no opaque origin) and **fail boot** with an
   actionable message naming the offending entry. One canonical list then
   feeds `CorsLayer::allow_origin`, `CsrfLayer::add_trusted_origin`, and the
   CSP's `connect-src` — one source of truth, three consumers.
2. **Keep the problem+json shape.** `CsrfLayer`'s default rejection is a bare
   403 with an empty body; KanPad's contract requires problem+json. Use
   `CsrfLayer::with_rejection_response` → the shared `problem_response`
   renderer and the **existing** `AppError::Forbidden` mapping
   (`src/error.rs:245,527`). **No new problem-type URI is introduced**, so the
   spec's *"no problem-type URI changes"* non-decision survives — the change
   is **contract-neutral**.
3. **Record the proxy caveat.** The `Origin`/`Host` fallback degrades if a
   reverse proxy rewrites `Host` (`tower-http`'s own "Deployment caveat":
   *"Reverse proxies and load balancers that rewrite `Host`… silently degrade
   the protection"*). Production is behind a proxy → the runbook must state
   that `Host` must be forwarded unchanged. `Sec-Fetch-Site` is also only
   sent to *trustworthy* origins (HTTPS or localhost) — production is HTTPS
   ✅; the plain-HTTP staging box falls through to step 5, which works ✅.

**`SameSite=Lax` + the `__Host-` cookie remain the first line of defence**
(A2) — this check is defence-in-depth, not a replacement.

**CSP.** `10-architecture.md` records *"No CSP header (matches `csp:false`)"*
— a Nest-era parity decision made when the **Nest** service did not serve the
SPA. The **Rust** service now serves `public/index.html` itself (SPA
fallback), so it is the origin that loads the frontend's scripts and styles.
Shipping the SPA **without** a CSP is a real regression in the security
posture of the rewrite.

**Decision — add a CSP header on the Rust origin**, scoped to what the SPA
actually needs:

```
default-src 'self';
script-src 'self' 'sha256-<hash-of-the-inline-theme-bootstrap>';
style-src 'self' 'unsafe-inline';
img-src 'self' data: blob:;
font-src 'self' data:;
connect-src 'self' <each CORS allowlist origin>;
frame-ancestors 'none'; base-uri 'self'; form-action 'self'
```

**Two corrections to the naive policy, both verified against the actual
frontend (2026-10-22):**

* **`connect-src` MUST NOT be `'self'` alone.** In the shipped dev topology
  the SPA is served by Vite at `http://localhost:5173` and calls the API at
  `http://localhost:3001` — `frontend/src/lib/api/client.ts:67`
  (`BASE = import.meta.env.VITE_API_URL ?? ''`) and
  `frontend/vite.config.ts:27`. That is a **cross-origin** XHR, so
  `connect-src 'self'` would have the browser block every API call in dev.
  The fix is derived, not hard-coded: **the origins allowed to call the API
  are exactly the `CORS_ORIGINS` allowlist**, so `connect-src` = `'self'` +
  the allowlist. In production the SPA is served by this same origin, so
  `'self'` alone would suffice — but the allowlist form is correct in both
  and needs no second code path.
* **`script-src 'self'` alone BREAKS the SPA.** `frontend/index.html:12-21`
  contains an **inline `<script>`** (the dark-mode bootstrap: it reads
  `localStorage.theme` / `prefers-color-scheme` and adds a class to
  `<html>`). A CSP with `script-src 'self'` and no `'unsafe-inline'` (and no
  matching hash) **blocks that script** — the page still renders, but the
  theme bootstrap never runs (flash of the wrong theme, and the stored theme
  is ignored). This was missed in the first pass of this spec, which assumed
  the SPA had no inline script.

  **Decision — allow the inline bootstrap by CSP *hash*, not by
  `'unsafe-inline'`.** A hash is the right tool here because the inline
  script is a **fixed literal** in `index.html` — it does not vary
  per-request (which is why the first pass wrongly ruled out hashing). The
  hash is computed over the exact bytes between the `<script>` tags.

  **The hash must not be a hand-maintained constant.** A stale hash breaks
  the theme bootstrap *silently* (no error, just a broken page) — the exact
  failure mode the `__Host-` invariant avoids by being test-locked. So:
  **compute the hash at boot from the `public/index.html` the server
  actually serves** (parse the inline `<script>` blocks, SHA-256 each, emit
  `'sha256-<base64>'`), and add a **test** that asserts the emitted CSP's
  hashes equal the hashes of the inline scripts in the served
  `public/index.html`. Drift then fails a test, not a page.

  *Follow-up (not a blocker):* moving the bootstrap to a static asset
  (`<script src="/theme-init.js">`) removes the hash entirely — `script-src
  'self'` then suffices with no drift surface at all. That is a **frontend**
  change, so it is out of scope for the backend-rs tasks; it is recorded as
  the durable fix.

`style-src 'unsafe-inline'` **is** required: the frontend uses 12 inline
`style={{…}}` React props (e.g. `features/checklists/progress-bar.tsx:23`,
`features/columns/column-droppable.tsx:22`). `script-src` keeps neither
`'unsafe-inline'` nor `'unsafe-eval'` — that is the part that matters.

**Swagger UI.** It is served from the same origin and needs its own
`script-src`/`style-src` allowances. **Decision:** exclude `/swagger-ui*`
from the CSP header (the Swagger UI is a developer surface, not the SPA, and
it is not served in production).

### B8 — No observability surface (P1 — new finding)

The service has `tracing` spans and `x-request-id`, but **no metrics** and
**no structured logs in production**:

* `tracing-subscriber` is configured with `features = ["env-filter", "fmt"]`
  — `fmt` is the **human-readable** formatter. A production log aggregator
  wants **JSON** (`tracing-subscriber`'s `json` feature).
* There is **no `/metrics` endpoint** and no Prometheus scrape target. A
  service with a rate limiter, a DB pool, a session sweeper and a governor
  has nothing to alert on: no request rate, no error rate, no pool
  saturation, no 429 count, no session-store size.
* The DB pool's own numbers are **free signal that is currently discarded** —
  but the source must be stated correctly (see the correction below).

**Correction (verified 2026-10-22 against the vendored
`sqlx-core-0.8.6`):** **sqlx 0.8.6 has NO `PoolMetrics` type and NO
`metrics` feature.** `sqlx-core`'s feature list is `_rt-async-std`,
`_rt-tokio`, `_tls-*`, `any`, `default`, `json`, `migrate`, `offline` — there
is no `metrics` entry, and `grep 'pub struct PoolMetrics' sqlx-core-0.8.6/src`
returns nothing. What sqlx 0.8.6 actually offers is:
* `Pool::size() -> u32` and `Pool::num_idle() -> usize` (gauges),
* `PoolOptions::acquire_time_level(LevelFilter)` and
  `get_acquire_slow_threshold()` — **tracing**, not metrics.

So the plan's *"from `PoolMetrics`"* + *"sqlx's `metrics` feature"* was
wrong on both counts. The correct sources are below.

**Decision.**

1. `tracing-subscriber` gains the `json` feature; the formatter is selected
   by `APP_ENV`: `json` in production, `fmt` (human) in development/test.
2. Add a **`/metrics` endpoint** in Prometheus text format. Series:
   * `kanpad_http_requests_total{method,route,status}` (histogram of
     latency via `http_request_duration_seconds`),
   * `kanpad_http_in_flight`,
   * `kanpad_db_pool_size` ← `Pool::size()`,
   * `kanpad_db_pool_idle` ← `Pool::num_idle()`,
   * `kanpad_db_pool_acquire_duration_seconds` ← **self-instrumented**: a
     small `acquire_timed()` helper that times `pool.acquire()` and records
     a histogram. sqlx 0.8.6 exposes no acquire-latency metric (see the
     correction above), so this is ~10 lines of our own code. Pair it with
     `PoolOptions::acquire_time_level(LevelFilter::INFO)` so a slow acquire
     is *also* a tracing event (free, already in sqlx).
   * `kanpad_throttle_rejections_total`,
   * process metrics (from `procfs`/`metrics` crate exporters).

   The route label must be the **matched route pattern**
   (`/api/v1/cards/{id}`), never the raw path — an unbounded label set is a
   metrics-cardinality incident waiting to happen. The pattern comes from
   axum's `MatchedPath` extension, which requires the **`matched-path`
   feature** on `axum` (it is feature-gated; without it the extension is not
   compiled in). Requests with no matched route (404/405) use the literal
   `"unknown"` — never the raw URI, or a hostile client could spam the label
   space.
3. **The `/metrics` endpoint must NOT be on the public API port.** axum's own
   Prometheus example starts a **second server on a different port** for
   exactly this reason (*"The `/metrics` endpoint should not be publicly
   available"*); `metrics-exporter-prometheus` ships an `http-listener`
   feature for the same purpose. A `/metrics` on the public port publishes
   request rates, error rates and DB-pool saturation to anyone.
   **Decision:** serve `/metrics` on a **separate listener**, enabled by an
   optional `METRICS_PORT` env key — **unset = no metrics endpoint at all**
   (opt-in, fail-closed), set = a dedicated port that is never
   throttled/auth-gated and never touched by the SPA fallback.
4. **No scrape feedback loop.** The `/metrics` scrape must not record itself
   (every scrape would otherwise add a request to the counter it is reading).
   Both `axum-metrics` and `docspec-http` guard this explicitly; do the
   same — skip recording when the matched path is the metrics route.
5. `metrics` + `metrics-exporter-prometheus` provide the registry.
   **Dependency note:** this is a real new dependency — it must be justified
   in the PR (observability is a stated production-readiness requirement,
   and the crate is small + widely used). `metrics-exporter-prometheus` is
   the crate the `metrics` project's own README recommends for a scrape
   endpoint (the similarly-named `metrics-prometheus` is a different crate
   that pulls in the whole `prometheus` crate).

### B9 — Public Rust API ergonomics (P1 — new finding)

The crate is a library (`src/lib.rs`) and its public surface is what Rust
consumers see. Current gaps:

| Gap | Fix |
|---|---|
| `Settings.node_env: String` — validated against a whitelist at boot; `is_production()` compares to `"production"` | `AppEnv` enum (`Development`/`Test`/`Production`) with `FromStr` + `Display`; make illegal states unrepresentable |
| `AuthSession.role: String`, `StoredUser.role: String` — compared with `!=` in `me` | `Role` enum (`User`/`Admin`) with `#[serde(rename_all = "lowercase")]` + `ToSchema(value_type = String)` — the wire format is unchanged, the Rust type is not stringly |
| `AppError::Validation(garde::Report)` — leaks the `garde` crate into the public API | `AppError::Validation(Vec<FieldError>)` — the error type carries only its own data; `garde` stays an implementation detail of the extractor |
| `AppError` is a public, non-`#[non_exhaustive]` enum — adding a variant is a breaking change for downstream matches | `#[non_exhaustive]` |
| No result alias | `pub type AppResult<T> = Result<T, AppError>` — the crate's ubiquitous result type gets one name |
| `AppError::Json(JsonRejection)` / `Path(PathRejection)` / `Query(QueryRejection)` leak `axum` rejection types | **Non-decision: keep them.** They are `axum`'s own rejection types, `axum` is a public dependency anyway (the crate's `IntoResponse` impl is the contract), and the spec already records the "pragmatic coupling — do not claim 'domain has zero axum dep' unless the core is split" note. Splitting the core is a bigger change than the ergonomics warrant. |

### B10 — Concurrency & pool tuning (P1 — new finding)

* **Deadlock / lock-wait retry.** The move transaction
  (`cards/routes.rs::move_card`) already retries a *same-card race* (bounded
  5×). It does **not** retry a MySQL **deadlock** (`sqlx::Error::Database`,
  error code `1213`) or **lock-wait timeout** (`1205`), which the
  `SELECT … FOR UPDATE` + range-`UPDATE` pattern can produce under
  concurrent cross-column moves. The stable ascending lock order makes
  deadlock *unlikely*, not impossible (three-way moves, different tables).
  **Decision:** extend the bounded retry to also cover `1213`/`1205`, with
  the same bounded attempt count and a small backoff, and a unit/integration
  test that proves the retry path is taken (not just present).

  **Three implementation details that are easy to get wrong (verified
  2026-10-22 against the MySQL 8.4 reference manual):**

  1. **Match on the error *number*, not the SQLSTATE.** `1213` is SQLSTATE
     `40001` (deadlock) but `1205` is SQLSTATE `HY000` (lock wait timeout).
     A retry matcher that keys on `40001` alone **silently misses every
     lock-wait timeout**. Match on the MySQL error number (`1213`, `1205`).
  2. **The two errors roll back different things.** `1213` rolls back the
     **whole transaction** (InnoDB breaks the cycle). `1205` rolls back
     **only the current statement** by default — the transaction survives,
     half-applied — unless `innodb_rollback_on_timeout` is enabled. So the
     retry must **explicitly `ROLLBACK` the whole transaction** before
     retrying; it must not assume the tx is already dead.
  3. **`1205` is rarely observed end-to-end.** `innodb_lock_wait_timeout`
     defaults to **50 s**, but the app's `TimeoutLayer` is **30 s** — the
     request is aborted by the timeout long before the lock wait expires.
     Handle `1205` anyway (the timeout is tunable, and a longer
     `innodb_lock_wait_timeout` is a legitimate operator choice), but do not
     over-invest in it; `1213` is the case that actually fires.

* **`READ COMMITTED` for the move tx.** `30-db-schema.md` already notes
  *"consider `READ COMMITTED` for the move tx to reduce MySQL gap-lock
  deadlocks under default `REPEATABLE READ`"* — it was never done.
  **Decision:** set the move tx to `READ COMMITTED`, at the cost of a
  weaker snapshot, which the explicit `FOR UPDATE` re-reads already
  compensate for.

  **The SQL must be issued BEFORE `BEGIN`, or it fails with error 1568.**
  `SET TRANSACTION ISOLATION LEVEL …` (unscoped) applies to the **next**
  transaction only — and MySQL rejects it *inside* a transaction:
  `ERROR 1568 (25001): Transaction characteristics can't be changed while a
  transaction is in progress`. Since sqlx's `pool.begin()` has already sent
  `BEGIN` by the time the body runs, the statement must go **before** it:

  ```rust
  let mut conn = pool.acquire().await?;
  // Scoped to the NEXT transaction only — must precede BEGIN.
  sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
      .execute(&mut *conn)
      .await?;
  let mut tx = conn.begin().await?;   // now a READ COMMITTED transaction
  ```

  (The `SET SESSION TRANSACTION ISOLATION LEVEL …` variant is permitted
  mid-transaction but changes the *session* default, so it would leak onto
  every later transaction on that pooled connection — too broad for a
  single-transaction change.)

  **What it actually buys (do not overstate it).** MySQL §17.7.5 states
  plainly: *"The possibility of deadlocks is not affected by the isolation
  level, because the isolation level changes the behavior of read
  operations, while deadlocks occur because of write operations."* The
  precise, narrower claim is §17.7.2.1: under `READ COMMITTED`, InnoDB
  *"holds locks only for rows that it updates or deletes"* and *"this
  greatly reduces the probability of deadlocks, but they can still happen."*
  So: **`READ COMMITTED` reduces the probability of gap-lock deadlocks; it
  does not eliminate them, and the retry above is still required.** It is a
  mitigation, not a fix.
* **Pool sizing is hard-coded** (`max_connections 10`, `min 1`). A production
  deployment cannot size the pool for its MySQL server. **Decision:**
  `DB_MAX_CONNECTIONS` / `DB_MIN_CONNECTIONS` env keys with the current
  values as defaults.

### B11 — Docker / image hardening (P1 — new finding)

* **No `.dockerignore`** — the build context sends the whole `backend-rs/`
  directory, including `target/` (the orchestration log records a 6.4 GB
  shared cargo cache). The Dockerfile's explicit `COPY` lines mean the
  *image* is unaffected, but the **context upload** is enormous and slow.
  **Decision:** add `backend-rs/.dockerignore` (`target/`, `.env*`, `.sqlx`
  is needed — keep it!, `public/`, `tests/fixtures` is not needed for the
  build but is small).
* **No `HEALTHCHECK`** in the Dockerfile — the orchestrator has no
  container-level liveness signal (the app exposes `/health`, but the image
  does not use it).
* **Base images are not digest-pinned** — `rust:1.96-bookworm` and
  `debian:bookworm-slim` float. **Decision:** pin by digest.
* **`debian:bookworm-slim` runtime** — a Rust binary needs glibc + certs
  only. **Decision:** consider `gcr.io/distroless/cc-debian12` (no shell, no
  package manager, minimal CVEs). **Non-decision: not urgent** — the current
  runtime already installs only `ca-certificates` and runs as a non-root
  `app` user; distroless is an improvement, not a requirement, and it must
  not block the cutover. Record it as a follow-up.
* **`--locked` is not used for CI cargo commands** (only in the Dockerfile).
  **Decision:** add `--locked` to every CI cargo invocation so a
  `Cargo.lock` drift fails the build instead of silently resolving.

### B12 — Test-suite completion + coverage gate (P1 — new finding)

* The suite is strong (57 `#[sqlx::test]` isolated DBs, dual-transport
  assertion cores, a normative harness-ownership header) but **no coverage
  measurement** exists, so "how much of the request path is actually
  exercised" is unquantified. **Decision:** add `cargo-llvm-cov` to CI with a
  **floor** (e.g. `--fail-under-lines 85`) so coverage cannot silently
  regress. The floor is set below the measured value on purpose: a gate that
  starts at the current number blocks every refactor.
  **The gate needs an export format (2026-10-22):** cargo-llvm-cov's
  `--summary-only` and `--output-path` are documented as *"can only be used
  together with `--json`, `--lcov`, or `--cobertura`"*, so a bare
  `--fail-under-lines` produces no readable summary. Use
  `cargo llvm-cov --summary-only --fail-under-lines 85` (or
  `--lcov --output-path lcov.info --fail-under-lines 85`), and treat "no
  summary printed" as a mis-configured gate rather than a passing one.
* **CI supply-chain pinning (D14, 2026-10-22).** Pinning an action to a
  **full-length commit SHA** is, per GitHub's own security-hardening guide,
  *"currently the only way to use an action as an immutable release"*.
  GitHub now also ships **workflow dependency lockfiles** (public preview)
  with `gh actions-lock`, which generate and maintain those SHA pins, and a
  repo/org **policy that can enforce SHA pinning** so a non-pinned `uses:`
  fails the workflow outright. **Decision:** prefer the lockfile mechanism
  (`gh actions-lock` + the committed lockfile) over hand-pinning, with
  `dependabot.yml` (`github-actions` ecosystem) keeping the pins current —
  dependabot reads the `# v4.2.2` comment next to each SHA and updates both.
  **Record the nuance:** dependabot *"will not create alerts for actions
  pinned to SHA values"* (alerts only cover semver-tagged actions), so
  SHA-pinned actions rely on the version-bump PRs, not on security alerts.
* **New tests required by this series** (each is an AC of its task):
  * T54 — `APP_ENV` unset + `NODE_ENV` set → boot error; the cookie-attribute
    matrix per env; the `connect.sid` regression guard.
  * T56 — the CORS preflight test (B3); the CSP header test (including the
    **hash-drift test**: the emitted `script-src` hashes must equal the
    hashes of the inline scripts in the served `public/index.html`); the
    Origin-check 403 test; the span-redaction test.
  * T59 — the `/ready` path appears in `openapi.json` (the existing
    `openapi_snapshot` test covers it once the path is registered).
  * T61 — the deadlock-retry test.
  * T62 — the coverage gate itself.

---

---

## Part C — Dependency hygiene & modernization (2026-10-22)

> **Origin:** a second, deeper review of `backend-rs/Cargo.toml` — the
> *manifest* itself, not the code. Every direct dependency was usage-grepped
> against `src/` + `tests/`, every dependency-graph question was answered
> with `cargo tree -i` (not by reading manifests), every version claim was
> checked against **crates.io** and the **vendored crate sources**, and every
> breaking change was read from the upstream **changelog**, not recalled.
>
> **Why this part exists at all.** `cargo clippy` does not report an unused
> **direct** dependency, and `cargo build` does not report a duplicate one. A
> manifest can carry a dependency that pulls a *second copy of the framework
> the app is built on* and nothing in the build will say so. The audit used
> four independent lenses for exactly that reason:
>
> 1. **Usage grep** — every crate name grepped against `src/` + `tests/` with
>    *qualified-path* patterns, so `tower_http` was never confused with
>    `tower` and `std::time` was never confused with the `time` crate.
> 2. **Dependency graph** — `cargo tree -i <crate>` to find *who* pulls a
>    version, `cargo tree -p <crate>` to enumerate a subtree.
> 3. **Upstream truth** — crates.io for the current version, the crate's own
>    `CHANGELOG` for the breaking surface, and the **vendored source** in
>    `~/.cargo/registry/src/` for anything the changelog left ambiguous.
> 4. **MSRV ceiling** — `cargo info <crate>@<version>` for `rust-version`, to
>    confirm that one toolchain satisfies every target version.

### C1 — Three dead direct dependencies (not four — see the `time` correction) — **LANDED IN T63 (2026-10-22)**

| Crate | Refs in `src/`+`tests/` | Evidence |
|---|:--:|---|
| `axum-valid` | **0** | `src/extract.rs:12-18` says it *cannot* be used (garde 0.20 vs the DTOs' garde 0.22) and the code hand-rolls `ValidJson`/`ValidQuery`. |
| `utoipa-axum` | **0** | `src/docs.rs:37` builds the spec with `#[derive(utoipa::OpenApi)]`, not `OpenApiRouter`. |
| `zeroize` | **0** | Only `secrecy` uses it; `src/admin.rs:37,132` refer to `SecretString` zeroizing — that is *secrecy*, not `zeroize`. |
| ~~`time`~~ | **1 — NOT dead** | **CORRECTED 2026-10-22 (b).** The audit's first pass flagged `time` as dead because `src/lib.rs:20` contains `std::time::Duration`. It is **not** dead: `src/lib.rs:155` uses `Expiry::OnInactivity(time::Duration::hours(24))`, and that `time::` is the **`time` crate**, not `std::time` (`chrono` is not imported in `lib.rs`, and a transitive dependency is not in the extern prelude). `cargo tree -i time` confirms `backend-rs` as a **direct** dependent. The dependency is **required**: `tower_sessions::Expiry::OnInactivity` takes a `time::Duration`. **Keep it** — and record why, so the next audit does not flag it again. |

**The `axum-valid` blast radius is the serious one.**

```
axum-core v0.4.5  →  axum v0.7.9  →  axum-valid v0.20.0  →  backend-rs
garde   v0.20.0   →  axum-valid v0.20.0  →  backend-rs
validator v0.18.1 →  axum-valid v0.20.0  →  backend-rs
matchit v0.7.3    →  axum v0.7.9  →  axum-valid v0.20.0  →  backend-rs
```

The app ships an entire **second web framework** (axum 0.7.9 + axum-core
0.4.5 + its own router) that is never called, alongside the real axum 0.8.9 —
a **two-major-version skew**. Two aggravating details:

* `axum-valid 0.20`'s **default features are `basic, validator`**. The
  manifest writes `features = ["garde"]` *without* `default-features = false`,
  so it also drags in the **`validator`** crate — a second validation
  framework the repo never uses.
* `cargo tree -i garde` → *"specification `garde` is ambiguous: garde@0.20.0,
  garde@0.22.1"* — two versions of the same crate in the lock.

**So the count is THREE, not four** (`axum-valid`, `utoipa-axum`, `zeroize`) —
the `time` flag was a false positive, and the method lesson is recorded in
C1's note above: a grep that counts `std::time` as the `time` crate is
exactly the kind of error a "verified" audit must not make.

**Decision — remove `axum-valid`; do NOT upgrade it.** The code comment's
claim (*"upgrade `axum-valid` past the garde-0.22 line before switching
wrappers"*) was **version-specific and is now stale**: `axum-valid 0.24.0`
uses `garde ^0.22`, and `0.25.0` uses `axum ^0.8` + `garde ^0.23`. But the
hand-rolled extractors are documented as behavior-identical *and*
state-generic, and `Garde<E>`'s `Context: FromRef<State>` bound still has no
blanket `FromRef<S> for ()`. So the dependency goes — **and the comment is
corrected to say so**, because a stale justification is exactly how the next
contributor re-adds it.

### C2 — Unused features, and one that looks unused but is not

| Feature | Status | Evidence |
|---|---|---|
| `tower-http/timeout` | **unused** | the timeout is `tower::ServiceBuilder::timeout(REQUEST_TIMEOUT)` (`src/lib.rs:162-167`) — the **`tower`** crate's feature, not tower-http's. No `tower_http::timeout::*` anywhere. |
| `tower-http/limit` | **unused** | the 2 MiB cap is axum's `DefaultBodyLimit` (`src/extract.rs:143`, `src/error.rs:280`). No `RequestBodyLimitLayer` anywhere. |
| `secrecy/serde` | **REQUIRED — keep** | `Settings` is `Deserialize`d by `envy` and holds two `SecretString` fields; `SecretString: Deserialize` only exists with this feature. Looks removable, is not. |

**`compression-full` is heavier than it reads.** It pulls `zstd` + `flate2` +
`brotli` → `zstd-sys`/`libz-sys` **C builds**. For a JSON API,
`compression-gzip` + `compression-br` is the conventional choice and drops
the C-toolchain requirement. **Decision:** the choice is made **explicitly**
in T67 (recorded either way), not left implicit in the manifest.

### C3 — Version currency (crates.io, 2026-10-22)

**Already current:** axum 0.8.9 · tower 0.5.3 · tower_governor 0.8.0 ·
tower-sessions-sqlx-store 0.15.0 · secrecy 0.10.3 · chrono 0.4.45 ·
envy 0.4.2 · dotenvy 0.15.7 · clap 4.6.7 · rpassword 7.5.4 · thiserror 2.0.21 ·
anyhow 1.0.104 · tracing-subscriber 0.3.23 · serde/serde_json.

**Available, with the breaking surface read from the upstream changelog:**

| Bump | Breaking change that lands on *this* code |
|---|---|
| **tower-http 0.6.11 → 0.7.1** | `compression`: `*` / `identity;q=0` now → **406 Not Acceptable** (RFC 9110 §12.5.3) — a wire-contract change. `services`: trailing slash on a file path → **404** (touches `ServeDir::new("public/")` + the SPA fallback). `set_header` refactor (#672) — `SetResponseHeaderLayer::overriding` is still at `tower_http::set_header::`, so all call sites are fine. `fs` (0.7.1, #718): `ServeDir::try_call` propagates expected I/O errors — the repo has a fallback, so fine. **Adds the `csrf` feature** (`CsrfLayer`) — see B7. |
| **tower-sessions 0.14.0 → 0.15.0** | only two changes: rand 0.9 + **"Fix memory ordering race. #254"** — a concurrency fix in the session layer. The reported "0.14→0.15 all requests 500" is a **user misconfiguration** (the session layer installed twice), not a library bug; the repo installs it once. Verified: `tower-sessions-core` resolves to a **single** 0.14.0 shared by `tower-sessions 0.14.0` and `tower-sessions-sqlx-store 0.15.0` — no duplicate, no trait mismatch. **But 0.15 is unreachable until C4 is resolved.** |
| **sqlx 0.8.6 → 0.9.0** | **`runtime-tokio-rustls` is removed** → `runtime-tokio` + `tls-rustls-ring-native-roots`. `offline` becomes optional (the repo depends on `SQLX_OFFLINE=true` + a committed 76-entry `.sqlx/` cache). MySQL text columns now infer as `String` where 0.8 inferred `Vec<u8>` — **verified not to apply here**: every app column is a non-binary `VarString`/`Long`/`Datetime`, and the only `Blob` columns are the session store's (never queried by the app). `Migrate` trait significantly changed; `resolve_blocking()` is now `#[doc(hidden)]`. `query*()` takes `impl SqlSafeStr` — **not a problem here**: 83 `query!` macros + 2 runtime `sqlx::query()` calls, both with `&'static str` literals. `sqlx.toml` support (`sqlx-toml` feature, opt-in). |
| **utoipa 5 → 6 + swagger-ui 9 → 10** | lockstep (swagger-ui 10 requires utoipa 6). utoipa 6 removes *expression ignore* in `ToSchema`/`IntoParams` — **verified: the repo uses no `ignore` expressions** ✅; drops `serde_norway` for `yaml_serde` — the repo does not enable `yaml` ✅; MSRV 1.88 ✅. But the **emitted OpenAPI changes** → `openapi.json` + the `openapi_snapshot` test must be regenerated together (the T52 rule). |
| **argon2 0.5.3 → 0.6.0** | RustCrypto 2024 wave. **`hash_password` → `hash_password_into`** — a direct hit on `src/features/auth/store.rs:158`. `std` feature removed; `simple` renamed `password-hash`; `password-hash` 0.6 moves the PHC types to the `phc` crate. |
| **bcrypt 0.17.1 → 0.19.3** | cascades the same `password-hash` 0.6 wave → **move it together with argon2**. bcrypt now *errors* on > 72 bytes instead of truncating — the repo already truncates explicitly (`verify_bcrypt_truncated`), so it is compatible. |
| **sha2 0.10.9 → 0.11** | **the lock already carries sha2 0.11.0 transitively**, so this **deduplicates** one of the 33 duplicated crate names. The API used is only `Sha512::digest` → low risk. |
| **garde 0.22.1 → 0.23.0** · **dialoguer 0.11 → 0.12** | minor. dialoguer 0.11 is one of three crates pulling **thiserror 1.0.69** alongside the app's thiserror 2. |
| **tower_governor `=0.8.0` → `0.8`** | 0.8.0 *is* the latest, so the exact pin buys nothing today but blocks every 0.8.x patch. CI already runs `--locked`, which is the reproducibility mechanism that matters. |

### C4 — The session store is the blocker, and the unlock — **LANDED IN T65 (2026-10-22)**

Two upstream constraints, both verified in the **vendored** sources:

```toml
# tower-sessions-sqlx-store-0.15.0/Cargo.toml
[dependencies.sqlx]              version = "0.8.0"      features = ["time", "runtime-tokio"]
[dependencies.tower-sessions-core] version = "0.14.0"   features = ["deletion-task"]
# tower-sessions-0.14.0/Cargo.toml
[dependencies.tower-sessions-core] version = "=0.14.0"
# tower-sessions-0.15.0 (crates.io metadata)
[dependencies.tower-sessions-core] version = "=0.15.0"
```

* **sqlx 0.9 is unreachable today.** The store requires `sqlx ^0.8.0`, so
  sqlx 0.9 pulls a **second** sqlx (`MySqlStore::new(pool)` takes sqlx-0.8's
  `MySqlPool`) → **does not compile**. Upstream's dependabot PR to bump it
  ([#87](https://github.com/maxcountryman/tower-sessions-stores/pull/87)) has
  been **open since 2026-05-21**; the crate's last release was **2025-01-01**
  — 21 months stale.
* **tower-sessions 0.15 is unreachable today.** `tower-sessions 0.15.0`
  requires `tower-sessions-core = 0.15.0`; the store requires
  `^ 0.14.0`, which can never be 0.15.0. Two core versions → the
  `SessionStore` trait the store implements is not the trait
  `SessionManagerLayer` expects → **does not compile**.

**The alternatives were searched, not assumed.** Every "just use a different
crate" path either still pins sqlx 0.8 or replaces the whole session
middleware:

| Option | sqlx 0.9 | tower-sessions 0.15 | chrono-only | Cost |
|---|:--:|:--:|:--:|---|
| **(a) Own the store in-tree, keep `tower-sessions`** | ✅ | ✅ | ✅ | ~200 LOC + tests |
| (b) `axum_session` 0.21 + `axum_session_sqlx` 0.11 | ✅ | n/a (replaces it) | ✅ | **Full session-middleware swap** — different `Session` API, cookie format, memory+DB sync model, IP/UA binding |
| (c) `tower-sessions-ext` (sagoez fork) | ❌ `sqlx ^0.8` | ✅ (its own core) | ❌ | Swap the middleware for a **0-star** fork |
| (d) `tower-sessions-sqlx-store-chrono` (karuna) | ❌ `sqlx ^0.8.3` | ❌ `core ^0.14` | ✅ | Does not solve it |
| (e) Vendor / `[patch.crates-io]` the store | ✅ | ✅ | ✅ | You now maintain a fork; `[patch.crates-io]` is permanent supply-chain surface |
| (f) Stay on sqlx 0.8 / tower-sessions 0.14 | ❌ | ❌ | ❌ | Forever pinned: no `sqlx.toml`, no `SqlSafeStr`, no session race fix |

**Decision — (a): own the session store in-tree.** The reasons are specific
to *this* codebase, not generic:

* **The table is already ours.** The baseline migration creates `sessions`
  fail-fast (deviation 11 in `30-db-schema.md`); the store's `migrate()` is
  **never called** — `src/state.rs:101` only does
  `MySqlStore::new(pool).with_table_name("sessions")`. An in-tree store
  therefore **drops** the `migrate()` responsibility rather than adding one.
* **Zero schema change.** Keep the existing DDL (`id CHAR(22)` / `data BLOB` /
  `expiry_date TIMESTAMP(6)` + `IDX_session_expiry_date`) and keep
  **MessagePack** (`rmp-serde`) so the on-disk format is byte-identical to
  the current store → the runbook's schema-drift gate (step 5(d)) stays green
  and the cutover needs no DDL.
* **Tiny blast radius.** `tower_sessions::Session` is touched in exactly
  **four** files: `src/extract.rs` (the `AuthSession` extractor),
  `src/features/auth/routes.rs`, `src/lib.rs` (the layer), `src/main.rs`
  (the sweeper). The middleware API does not change.
* **The `sqlx/time` feature disappears.** `tower-sessions-sqlx-store` is the
  *only* thing enabling `sqlx/time`. With both `time` and `chrono` on, sqlx
  prefers `time` for `DATETIME` — which is why **eight** store files carry
  `AS "col: chrono::NaiveDateTime"` on every datetime column. Drop the
  store → sqlx picks `chrono` natively → **those overrides delete**
  (`src/features/{boards,cards,checklists,columns,labels,notes,projects,tags}/store.rs`
  — **MEASURED 2026-10-22 (e): eight, not five**; `labels`, `projects` and
  `tags` were missing from this list, and T65's AC would have failed on
  them).
* **The sweeper is already ours.** `src/main.rs:9` imports
  `tower_sessions::session_store::ExpiredDeletion` and spawns the GC; the
  in-tree store implements `delete_expired` and that task keeps working.
* **Nothing else pins `tower-sessions`.** Verified: `axum-login` is **absent**
  from the lock (it is what pinned another project to 0.14 — see the
  precedent below). So taking the store in-house unlocks **both** majors.

**Precedent.** `darksailor.dev/kobo-shelf`, 2026-07-27 — the same problem,
the same conclusion: *"Replace the unmaintained `tower-sessions-sqlx-store`
(last release 2025-01, pinned to sqlx 0.8) with a small in-tree
`…SessionStore` implementing `tower_sessions::SessionStore` over the shared
app-DB pool. … With the store crate gone, sqlx moves 0.8 → 0.9. That
required the 0.9 `SqlSafeStr` migration for dynamic query strings
(`AssertSqlSafe`)… tower-sessions stays 0.14 (still pinned by axum-login
0.18)."* Their one remaining constraint — `axum-login` — does not apply
here.

**The cost, stated honestly:** ~200 LOC of security-critical code the project
now owns and must maintain, versus a 3.8 KB crate it does not.

**What T65 actually found (2026-10-22).** Three things the plan did not
anticipate, all resolved in the same PR:

1. **The sweeper's loop lived behind a FEATURE, not in the store's code.**
   `ExpiredDeletion::continuously_delete_expired` is gated behind
   `tower-sessions-core`'s `deletion-task` feature — which the old store
   enabled **transitively** (`features = ["deletion-task"]` on its own core
   dependency). Dropping the store dropped the feature with it, and
   `main.rs` stopped compiling. The fix is to declare
   `tower-sessions-core = { version = "0.14", features = ["deletion-task"] }`
   **directly** — the SAME core version `tower-sessions 0.14` pins
   (`=0.14.0`), so it is not a second core in the tree. Declaring it keeps the
   sweeper on the upstream implementation instead of a hand-rolled loop.
   **Lesson: a transitive feature can vanish with the crate that enabled it.**
2. **The store interpolated a schema name into every statement — so the
   in-tree one interpolates NOTHING.** The 2026-10-19 invariant ("the store
   must live in the pool's database") was previously *configured* (a schema
   name, validated at boot, with a `panic!` if it was not a valid
   identifier). An in-tree store that takes the pool satisfies the same
   invariant **structurally** — there is no name to get wrong. The
   boot-time screen in `Settings::validate` stays (the name still reaches
   operator tooling that does not quote identifiers), but the
   `AppState::with_pool` panic and its `# Panics` contract are **gone** —
   and with them the CI grep gate's `state.rs` allow-list entry, exactly as
   T55's sequencing note anticipated.
3. **The `sqlx/time` feature was the whole reason for the `AS` overrides.**
   Confirmed: `tower-sessions-sqlx-store` was the only enabler of sqlx's
   `time` feature, so dropping it turns the feature off, sqlx infers
   `chrono::NaiveDateTime` natively, and the overrides in the **eight**
   feature store files become unnecessary. The expiry column is the one
   place the two time types meet: the store converts `OffsetDateTime` →
   `chrono::NaiveDateTime` at **microsecond** precision — the column's
   precision, and the precision the previous store's encoding truncated to
   as well (so the stored value is unchanged). The
mitigations are that the trait is five methods, the on-disk format is
unchanged, the session path already has a test suite, and the whole change
lands in **one** reviewable PR **before** the data migration.

### C5 — The toolchain: 1.96.1 → 1.99.1

**Decision.** Pin **`1.99.1`** in `rust-toolchain.toml`, set
`rust-version = "1.99"` in `Cargo.toml`, and bump the Dockerfile base to
`rust:1.99-bookworm` (digest-pinned per T62). `rustup update stable` on the
developer's machine.

**Why 1.99.1 and not the current pin.** The MSRV ceiling of the whole target
dependency set is **sqlx 0.9.0's 1.94.0**; every other target version is
≤ 1.88 (utoipa 6 / swagger-ui 10 / sha2 0.11 = 1.88, garde 0.23 = 1.87,
argon2 0.6 = 1.85, tower-http 0.7 = 1.65, dialoguer 0.12 = 1.66). Rust
**1.99.1 clears all of it with headroom**, and `edition = "2024"` (needs
≥ 1.85) is comfortably inside it.

**Why now, and not after the cutover.** This is the whole point of the
"one window" in C6. The toolchain pin is the single most expensive thing to
change late: it gates `edition = "2024"` → `resolver = "3"` → the
Rust-version-aware resolver → the **entire dependency set**. Changing it
*after* the data has been migrated means re-resolving the tree against a
different resolver with a live database in front of it. Changing it *now*
means the resolver change and the dependency modernization happen together,
once, on a schema that is still malleable.

**Do not forget the Docker base.** `rust:1.96-bookworm` and
`rust-toolchain.toml @ 1.99.1` are a contradiction; the Dockerfile moves in
the same task. The Dockerfile and the toolchain move **together**, never one
alone.

### C6 — sqlx 0.9's collation change is a no-op *if* the collation is pinned

**The changelog overstates the risk.** sqlx 0.9's changelog says
*"`SET NAMES utf8mb4 COLLATE utf8_general_ci` is no longer sent by default;
instead, `SET NAMES utf8mb4` is sent"*. But the vendored 0.8.6 source says
otherwise: `sqlx-mysql-0.8.6/src/collation.rs:139` maps
`CharSet::utf8mb4 => Collation::utf8mb4_unicode_ci`, and
`src/options/connect.rs:71` emits `SET NAMES utf8mb4 COLLATE
utf8mb4_unicode_ci` — **which is exactly this project's schema collation**.
sqlx 0.9 sends only `SET NAMES utf8mb4` and lets the server choose.

**Decision — pin it explicitly so the driver version cannot matter.**
`sqlx-mysql-0.8.6/src/options/parse.rs:52` shows the MySQL URL parser
**accepts `collation=`**. Adding `collation=utf8mb4_unicode_ci` to the
`DATABASE_URL` makes 0.8.6 and 0.9.0 **byte-identical** on this axis, in
every environment, regardless of the server's default. The pin must be
applied in **all five** places the URL appears: `backend-rs/.env`,
`compose.yaml`, the CI workflow, `docs/data-migration-live.md`, and
`BACKEND-RS-ORCHESTRATION.md`.

**A second sqlx-0.9 landmine: `backend-rs/sqlx.toml`.** The repo already has
one (`[sqlx] database-url-var = "DATABASE_URL"`, from T03). sqlx 0.9
introduces its **own** `sqlx.toml` format, but the `sqlx-toml` feature is
**opt-in and NOT enabled by default** — so under 0.9 the file is **inert**
(the macros read `DATABASE_URL` directly, which is what the file asked for
anyway). It must be **verified**, not assumed: the file's `[sqlx]` table is
not 0.9's shape (0.9's keys are top-level), so if `sqlx-toml` is ever
enabled the file must be migrated first. **Decision: do not enable
`sqlx-toml`** (see the non-decisions) — T65 removes the *cause* of the
`sqlx/time` unification instead, and `sqlx-toml` would also drag
migration-journal renaming into scope.

**Second-order invariant:** with the pin, the *server's* default collation no
longer matters — but the **database's** collation still does for the
`UNIQUE` indexes (`UQ_labels_user_name`), because an index uses its
*column's* collation, not the session's. The baseline already creates the
tables as `utf8mb4_unicode_ci` and the runbook already creates
`kanpad_rs_dev` with `CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci` — so
the invariant holds. **Record it as an invariant, not an accident:** the runbook
must keep creating every Rust database with an explicit
`COLLATE utf8mb4_unicode_ci`, never the server default (`utf8mb4_0900_ai_ci`
on MySQL 8 — a *different* collation with different comparison rules).

### C7 — The `bcrypt` exit criterion

`bcrypt` is **legacy-verification-only** in this codebase: it never mints a
hash (`src/features/auth/store.rs:246` asserts *"must not mint bcrypt
hashes"*); it only verifies the pre-cutover `$2a$`/`$2b$` cost-10 hashes
imported from Nest, and a successful bcrypt login rehashes to Argon2id in
the same request (`src/features/auth/store.rs:121-158`).

**That means the dependency has an end date, and the end date is
measurable.** A bcrypt hash cannot be converted without the plaintext
password, so every imported user must log in once to be upgraded. The
criterion is therefore:

```sql
SELECT COUNT(*) FROM users WHERE password_hash NOT LIKE '$argon2id$%';
-- ✓ 0 → every imported account has been upgraded → bcrypt can be dropped
```

**Decision — record this criterion in the spec now.** Nothing in the specs
records it today, so the dependency would otherwise live forever with a
comment saying "legacy only" and no way to tell when "legacy" is empty. The
cutover window is the natural place to measure it: the runbook's step 8 logs
in one real prod account, and every subsequent login upgrades one more row.

### C8 — Ordering: the one window

**The constraint the whole plan is built around:** the NestJS data has **not**
been migrated yet. Once the Rust backend is live and the Nest data has been
imported, changing the driver, the toolchain, the resolver, or the session
store becomes a production change rather than a development one. That window
is the *only* time these are cheap.

**Ordering principle:** every change that touches the **dependency set**, the
**toolchain**, the **resolver**, or the **DB connection semantics** lands
**before** the feature work — because the feature work (T58–T62) is
mechanical refactoring whose diffs should be measured against the *final*
driver and resolver, not the old ones.

```
T54 → T55 → T57 (toolchain 1.99.1 + edition 2024 + resolver 3)
     → T63 (dead deps) → T65 (session store) → T67 (library majors)
     → T56 (CSRF via CsrfLayer + CORS PUT + CSP + span redaction)
     → T66 (sqlx 0.9) → T58 → T59 → T60 → T61 → T62
```

* **T57 before T63/T65/T67** — the resolver change must be computed against
  the final MSRV, and the toolchain pin must land before any new crate is
  added.
* **T63 before T65** — T63 is zero-risk and shrinks the tree; T65 then has
  fewer crates to reason about.
* **T65 before T67 and T66** — T65 is what makes `tower-sessions 0.15` and
  `sqlx 0.9` reachable at all.
* **T67 before T56** — T56 adopts `tower-http` 0.7's `CsrfLayer`, so
  tower-http 0.7 must already be in.
* **T66 before T58/T59** — a driver major changes the `query!` inference and
  the `.sqlx/` cache; the mechanical refactors should be measured against the
  final driver. T59 also folds in T66's `Migrate`-trait adoption (B6's
  `main.rs` journal fix), so the two must not land in the same PR.
* **T62 last** — it is the aggregate gate that proves the rest.


---

## Tasks (the executable entries live in `50-tasks.md` Phase 7)

| Task | Title | Axes moved | Pri | Depends |
|---|---|---|:--:|---|
| **T54** | Rust-idiom naming: `APP_ENV` + `kanpad.sid` | 1, 5, 9 | P0 | — |
| **T55** | CI activation + lint-gate repair | 6, 8 | P0 | T54 |
| **T57** | Build & supply-chain hardening (lints, release profile, toolchain **1.99.1**, deny.toml, edition 2024) | 1, 8, 9 | P1 | T55 |
| **T63** | Dependency hygiene: **3** dead direct deps + 2 dead features removed | 1, 8 | P0 | T57 |
| **T65** | Session-store ownership: implement `SessionStore` in-tree, drop `tower-sessions-sqlx-store` | 1, 8, 9 | P0 | T63 |
| **T67** | Library modernization: tower-http 0.7, tower-sessions 0.15, utoipa 6 + swagger-ui 10, argon2 0.6, bcrypt 0.19, sha2 0.11, garde 0.23 | 1, 8, 9 | P1 | T65 |
| **T66** | sqlx 0.9 (driver major) + the pinned collation + `.sqlx` regeneration | 8, 9 | P1 | T67 |
| **T56** | Security hardening: CORS `PUT`, **cross-origin check via `CsrfLayer`**, CSP, span redaction | 5, 6 | P0 | T67 |
| **T58** | Public-API ergonomics: `Role`/`AppEnv` enums, `AppResult`, `#[non_exhaustive]` | 1, 10 | P1 | T66 |
| **T59** | Shared-kernel DRY + error-kernel polish + `/ready` in OpenAPI | 2, 3, 10 | P1 | T58 |
| **T60** | Observability: `/metrics` + JSON logs + pool metrics + shutdown timeout | 9 | P1 | T59 |
| **T61** | Concurrency & pool tuning: deadlock retry, `READ COMMITTED`, pool sizing | 4, 9 | P1 | T59 |
| **T62** | Test completion + coverage gate + CI action SHA pins + dependabot + Docker hardening | 6, 8, 9 | P1 | T60 |

**Ordering rationale.** The chain is **dependency-first, feature-second**,
because the NestJS data has not been migrated yet: the toolchain, the
resolver, the dependency set and the DB connection semantics are all cheap
*now* and expensive after the cutover (Part C, C8).

T54 first (it is a rename that touches the CI file T55 relocates). T55
immediately after — moving CI to the root is what makes every later gate
enforceable, and it is the only way the P0 findings in B2/B3 get caught.
**T57 before everything that follows**, because the toolchain pin
(**1.99.1**) + `edition = "2024"` → `resolver = "3"` recompute the **entire
dependency set**; that must happen once, against the final MSRV, before any
crate is added or removed. **T63** (the dead dependencies) is zero-risk and
shrinks the tree before T65 has to reason about it. **T65 (the session
store) is the hinge**: it is what makes `tower-sessions 0.15` and
`sqlx 0.9` reachable at all, so **T67** (the library majors) and **T66**
(the driver major) both wait on it. **T67 before T56** — T56 adopts
tower-http 0.7's `CsrfLayer`, so tower-http 0.7 must already be in; T56 and
T66 are then parallel. **T66 before T58/T59** — a driver major changes the
`query!` inference and the `.sqlx/` cache, so the mechanical refactors must
be measured against the *final* driver (and T59 folds in T66's `Migrate`-trait
adoption, so the two must not land in the same PR). T61 after T59 (the
concurrency change touches the same card-move code path as the DRY refactor —
one change per PR keeps the diff reviewable). T62 last (it is the aggregate
gate that proves the rest).

**T59 splits out the T52-shaped part.** Adding `/ready` to `ApiDoc` changes
`openapi.json` and therefore `frontend/src/lib/api/schema.d.ts`. The T52
rule (*"No standalone openapi/typegen PRs"*) is satisfied because the
regeneration ships **in the same PR as the spec + code change** — no
standalone PR is created. T54/T55/T56/T58/T60/T61/T62 do **not** touch
`openapi.json` (verified: `grep -c 'connect.sid' backend-rs/openapi.json` →
0; the cookie name is not in the spec). **T57 is the one exception
(2026-10-22 (e)):** utoipa reads `CARGO_PKG_DESCRIPTION` / `CARGO_PKG_LICENSE`,
so adding the crate metadata changed the OpenAPI **`info` block** (3 lines:
the description + the licence). **No path, schema or field changed, and the
typegen artifact `frontend/src/lib/api/schema.d.ts` is byte-identical**
(verified with the real `openapi-typescript`) — the T52 rule is satisfied.

---

## Verification matrix (every task)

Per `00-overview.md` Constitution + the `50-tasks.md` per-task checklist:

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic
cargo test -p backend-rs --test api <filter>
cargo test -p backend-rs                       # kernel tasks
cargo sqlx prepare --check --workspace --all -- --all-targets
```

Task-specific additions:

| Task | Extra gate |
|---|---|
| T54 | `APP_ENV` unset + `NODE_ENV` set → boot error (assert the message names `APP_ENV`); `APP_ENV=production` → `__Host-kanpad.sid` + `Secure` + `Path=/` + `HttpOnly` + `SameSite=Lax` + **no `Domain`**; dev/test → `kanpad.sid` + **no `Secure`**; **no env emits `connect.sid`**; `compose config` shows `APP_ENV` on the rust service and `NODE_ENV` on the nest service |
| T55 | the relocated workflow is **discovered and runs green on the PR** (a `gh run list` link in the PR body is the evidence — this is the first time CI runs, so expect to fix *its* latent failures too); **all three** lint-gate greps pass (the `unwrap`/`expect` gate, the panic-spelling gate, and the `format!`-built-SQL gate — T55 added the second and repaired the third); `cargo test --test api cors` green |
| T56 | preflight test green for **all five** verbs; the CSP header is asserted on a 200 **and** on a 404, and its `connect-src` **contains the CORS allowlist origins** (so the dev topology's cross-origin fetch is not blocked) and its `script-src` **contains a `sha256-` hash matching the inline script in the served `public/index.html`** (asserted by recomputing the hash — drift fails the test, not the page); the Origin-check test proves a foreign `Origin` on `POST /api/v1/cards` → `403`; the span-redaction test proves a card `title`/`description` never reaches the rendered span; `grep -c 'instrument(skip(state))' src/` == 0 |
| T57 | `cargo build --release` produces a stripped binary (`file`/`nm` shows no symbols); `cargo deny check advisories licenses bans` green against the **committed** `deny.toml` — the first run is expected to FAIL and the `skip`/`skip-tree` + license allow-list are derived from its output, not guessed; `cargo clippy` green on edition 2024; the `Cargo.lock` diff from the `resolver = "3"` bump is **reviewed line by line** (a silent dependency downgrade is a supply-chain event); `cargo doc --no-deps` has **zero** warnings |
| T58 | `Role` round-trips as `"user"`/`"admin"` on the wire (`auth/me` snapshot unchanged); `AppEnv` rejects an unknown value at boot with the same actionable-message shape as before; `cargo clippy` green |
| T59 | `cargo test -p backend-rs` green; `grep -rc 'pub fn format_timestamp' src/` == **1**; `common::ownership` owns every user-scoping helper (`grep -c 'fn check_.*_owned' src/features/` == 0); `cargo test --test api openapi_snapshot` green; `npm run gen:api -- --check` + `tsc --noEmit` green |
| T60 | `/metrics` returns the Prometheus text format with the documented series, on its **own `METRICS_PORT` listener** (and the endpoint does **not** exist when `METRICS_PORT` is unset — fail-closed); the route label is a **pattern** not a raw path (asserted: `/api/v1/cards/999` and `/api/v1/cards/888` share one series); a scrape does **not** record itself (no feedback loop); production logs are JSON (asserted on the rendered `tracing` output); the pool gauges appear (`kanpad_db_pool_size` ← `Pool::size()`, `kanpad_db_pool_idle` ← `Pool::num_idle()`) and the acquire-duration histogram is populated by our own `acquire_timed()` helper (sqlx 0.8.6 has no `PoolMetrics`) |
| T61 | the deadlock-retry test proves the retry is taken (a fault-injected `1213` is retried and the operation eventually succeeds) and that the matcher keys on the **error number** (a `1205`/`HY000` lock-wait timeout is retried too — a `40001`-only matcher would miss it); the whole transaction is `ROLLBACK` before a retry (a `1205` rolls back only the current statement by default); the move tx runs at `READ COMMITTED` (asserted by a `SELECT @@transaction_isolation` probe inside the tx) **and the `SET TRANSACTION ISOLATION LEVEL` statement is issued BEFORE `BEGIN`** (issued inside a tx it fails with MySQL error 1568); pool sizing honours the env keys |
| T62 | `cargo llvm-cov --summary-only --fail-under-lines 85` green **and a summary is actually printed** (a bare `--fail-under-lines` prints no summary — `--summary-only` requires `--json`/`--lcov`/`--cobertura`); the workflow's action refs are **commit SHAs** (via `gh actions-lock` + the committed lockfile, or hand-pinned + dependabot); `dependabot.yml` covers cargo + npm + github-actions; `docker build` passes the new `.dockerignore` + `HEALTHCHECK` + digest pins; `docker run` reports `healthy` |
| T63 | `cargo tree -i axum@0.7.9` → *"did not match any packages"* (the second axum is gone); `cargo tree -i axum-core@0.4.5` → not found; `cargo tree -i garde` → **no ambiguity** (one version); `cargo tree -i validator` → not found; `grep -rn 'axum_valid\|utoipa_axum\|zeroize::' src/ tests/` → **0** hits (**`\btime::` is deliberately NOT in this grep** — `src/lib.rs:155` legitimately uses the `time` crate, which is **required** by `tower_sessions::Expiry::OnInactivity`; the separate bullet below asserts `time` stays); `cargo tree -e features -i tower-http` shows **no** `timeout` and **no** `limit` feature; `secrecy`'s `serde` feature is **still enabled** (it is load-bearing — `SecretString: Deserialize` for `envy`); the lockfile's duplicated-crate count drops; `cargo fmt/clippy/test` green; `cargo sqlx prepare --check` green |
| T65 | the in-tree store implements `tower_sessions_core::SessionStore` and the **existing** `sessions` table + DDL are unchanged (`SHOW CREATE TABLE` matches the baseline byte-for-byte — the runbook's step 5(d) drift gate stays green); the payload stays **MessagePack**, asserted **byte-identical** to the previous store's `rmp_serde::to_vec(&record)` (so a row written before the change still reads); `tower-sessions-sqlx-store` is **gone** from `Cargo.toml` and the lockfile (**the lock shrank by exactly one package; nothing else moved, no downgrade**); `cargo tree -i sqlx` shows **one** sqlx version; `cargo tree -i tower-sessions-core` shows **one** core version; the `AS "…chrono::NaiveDateTime"` overrides are gone from **all eight** store files — `{boards,cards,checklists,columns,labels,notes,projects,tags}` (measured 2026-10-22 (e): **eight, not five**) — (`grep -rn 'AS "[^"]*: chrono::NaiveDateTime"' src/features/*/store.rs` → 0) and sqlx now infers `chrono` natively; the **16 re-keyed `.sqlx/` entries keep their exact column signatures**; `sqlx/time` is no longer enabled (verified via `cargo tree -e features`); the session suite (login / logout / me / cycle_id / expiry / sweeper) is green |
| T67 | every target version is in `Cargo.toml` at the version this spec names; `argon2`'s `hash_password_into` is used and the Argon2id PHC format is unchanged (`$argon2id$v=19$m=19456,…` — the existing tests pin it); `tower-http` 0.7.1's compression **406** behavior is either accepted or disabled by an explicit test; the `ServeDir` trailing-slash change is verified against the SPA fallback; `utoipa` 6's OpenAPI output is regenerated (`openapi.json` + `frontend/src/lib/api/schema.d.ts` in the **same PR**) and `openapi_snapshot` is green; `Cargo.lock`'s diff is reviewed line by line (no unintended downgrade); `cargo fmt/clippy/test` green |
| T66 | the `DATABASE_URL` carries `collation=utf8mb4_unicode_ci` in **all five** places it appears (`.env`, `compose.yaml`, the CI workflow, `docs/data-migration-live.md`, `BACKEND-RS-ORCHESTRATION.md`); `SELECT @@collation_connection` = `utf8mb4_unicode_ci` is asserted by a test (so the pin is load-bearing, not decorative); the `sqlx` feature names are the 0.9 ones (`runtime-tokio` + `tls-rustls-ring-native-roots`, **not** `runtime-tokio-rustls`); `.sqlx/` is regenerated with `cargo sqlx prepare --workspace --all -- --all-targets` and the **diff is reviewed** (the app's own columns are all non-binary `VarString`/`Long`/`Datetime`, so the 0.9 `Vec<u8>`→`String` inference change must produce **no** Rust type change — if it does, stop and re-read the DTOs); `cargo sqlx prepare --check` green; the `Migrate` trait is adopted (no hand-rolled `SELECT version FROM _sqlx_migrations`); the 2 runtime `sqlx::query()` calls still compile as `&'static str` (no `AssertSqlSafe` needed) and the 83 `query!` macros are unaffected |

---

## Required companion edits (the same-PR rule — no file is updated "later")

| File | Edit |
|---|---|
| `specs/api-rewrite-rs-specs/00-overview.md` | new Amendment entry (2026-10-22): `APP_ENV` replaces `NODE_ENV`; cookie base name `kanpad.sid`; CI relocated to the repo root; `PUT` added to the CORS method set; the quality-baseline program (B1–B12) |
| `specs/api-rewrite-rs-specs/10-architecture.md` | Config table: `NODE_ENV` row → `APP_ENV` (add the `NODE_ENV`-present → boot-error rule) + `DB_MAX_CONNECTIONS`/`DB_MIN_CONNECTIONS`; Sessions: cookie name + the `__Host-` invariant; CORS: allowed methods gain `PUT` + the Origin check; Middleware order: the CSP + Origin-check layers; a new Observability section (`/metrics` + JSON logs) |
| `specs/api-rewrite-rs-specs/11-rewrite-deltas.md` | new sections **A5** (naming deltas — **renumbered from A4 on 2026-10-22 (c)**: A4 was already taken by "Ownership in SQL") + **E4** (CSRF/Origin + CSP) + **E5** (observability) — rationale record only, per this file's header |
| `specs/api-rewrite-rs-specs/50-tasks.md` | **Phase 7** with the T54–T67 entries (Touch only / Depends / Blocks / AC) — the executor source of truth |
| `specs/api-rewrite-rs-specs/README.md` | reading order: add step 10 → `54-rust-idiom-hardening.md`; the "decisions locked" header line gains the 2026-10-22 amendment |
| `CONTRACT-CHANGES.md` | §1 Transport row + §10 Auth notes (cookie name); **no** §2 rename-table entry (the cookie name is not a JSON field) |
| `AGENTS.md` (repo root) | the *"session cookie name is env-dependent"* lesson → new names + `APP_ENV`; new lessons: **"a gate that has never executed is not a gate"** + **"a hand-maintained allowlist drifts — assert it with a test"** |
| `docs/data-migration-live.md` | the operator env block (`APP_ENV=development`), the `set-cookie:` expectation line (`kanpad.sid=`), and the `__Host-` note |
| `compose.yaml` | the **rust** service: `APP_ENV` instead of `NODE_ENV` (the **nest** service keeps `NODE_ENV`) |
| `.github/workflows/backend-rs.yml` | `-e APP_ENV=test` (T54 edited the file at its then-path `backend-rs/.github/workflows/ci.yml`); **the file then moved to the repo root** (T55) |

**Which of these are already done.** The **spec** half of the table is applied
now, because the spec IS the plan: `00-overview.md` (Amendment 11),
`10-architecture.md` (Config + Sessions + CORS + Middleware + Observability,
each marked *2026-10-22*), `11-rewrite-deltas.md` (A5/E4/E5), `50-tasks.md`
(Phase 7), `README.md` (index), `AGENTS.md` (lessons) and this file. The
**runtime** half is deliberately **NOT** applied yet — `CONTRACT-CHANGES.md`,
`docs/data-migration-live.md`, `compose.yaml` and the CI workflow describe
what the *deployed* service does, so editing them before T54/T55/T56 land
would make the runbook and the compose file disagree with the binary (a
stale `compose.yaml` that stops passing `NODE_ENV` would silently boot the
Rust service as `development`). Each runtime file is listed in its task's
`Touch only` and lands **in that task's PR**.

---

## Non-decisions (explicitly out of scope)

* **No wire-contract change.** No route, status, envelope, field name, or
  problem-type URI changes. `openapi.json` changes only for `/ready` (T59).
* **`APP_ENV` values unchanged** (`development|test|production`). The rename
  is of the *key*, not the *domain*; renaming the values too would break
  every operator's muscle memory for no gain.
* **The cookie name is not configurable.** One canonical name, derived from
  `APP_ENV`. A `SESSION_COOKIE_NAME` key is a footgun (it lets an operator
  silently drop `__Host-` in production).
* **`__Host-` stays environment-conditional.** Unconditional `__Host-` +
  `Secure` would break the plain-HTTP staging box and the curl verification
  flow in `docs/data-migration-live.md`.
* **The CORS method list stays hand-written.** axum does not expose the route
  table; a preflight **test** is the guard, not a route-introspection layer.
* **No repository traits / no DI container.** Unchanged from `11-rewrite-deltas`
  A1 — the rewrite's layering is already right; this file does not reopen it.
* **`panic = "abort"` is not enabled.** Per-task panic isolation is worth more
  than the binary size.
* **No per-request CSP nonce.** A nonce would require the served HTML to be
  templated per request, which the static `ServeDir` + SPA fallback does not
  do. **REVERSED 2026-10-22:** this non-decision was wrong — it assumed the
  SPA has no inline script, but `frontend/index.html:12-21` has one (the
  dark-mode bootstrap), so `script-src 'self'` alone **breaks the page**.
  A **CSP *hash*** is used instead: the inline script is a fixed literal, so
  its SHA-256 is stable, and the hash is **computed at boot from the served
  `public/index.html`** (not hand-maintained) with a test that catches drift.
  A per-request *nonce* remains out of scope — a hash covers the one inline
  script that exists.
* **The error core is not split from `axum`.** The `JsonRejection` /
  `PathRejection` / `QueryRejection` variants keep leaking `axum` types (B9);
  splitting the core is a larger change than the ergonomics warrant, and the
  spec already records the pragmatic coupling.
* **Distroless runtime image is a follow-up, not a blocker** (B11).
* **The coverage floor starts below the measured value** (T62) — a gate set at
  today's number blocks every refactor.
* **The `deny.toml` policy is derived, not guessed** (T57) — the first
  `cargo deny` run is expected to fail, and the `skip`/`skip-tree` and
  license allow-list come from its output.
* **`/metrics` is opt-in.** `METRICS_PORT` unset = no metrics endpoint. A
  metrics surface on the public API port would publish request rates, error
  rates and pool saturation to anyone (axum's own Prometheus example uses a
  separate port for exactly this reason).
* **The `__Host-` invariant stays test-locked, not merely documented** (A2) —
  a future `.with_domain(...)` must break a test. `tower-sessions` 0.14's
  defaults already satisfy it (`Path = "/"`, `Domain = None`), so the
  invariant is free until someone changes it.
* **Moving the SPA's inline theme bootstrap to a static asset** is a
  **frontend** change and a follow-up, not a blocker — the CSP hash covers it
  in the meantime.
* **`axum-valid` is removed, not upgraded** (Part C, C1). The hand-rolled
  `ValidJson`/`ValidQuery` extractors stay; `axum-valid 0.24+` *would* now
  work with garde 0.22 + axum 0.8, but its `Garde<E>` extractor still carries
  the `Context: FromRef<State>` bound that has no blanket `FromRef<S> for ()`
  — so it would not be a drop-in anyway. The **comment** explaining the
  removal is corrected rather than deleted, so the reasoning stays on record.
* **The session store is implemented in-tree; it is not vendored, not
  forked, and not replaced by a different middleware** (Part C, C4). The
  alternatives were searched and each either still pins sqlx 0.8 or replaces
  the whole session layer. The cost — ~200 LOC of security-critical code the
  project now owns — is accepted **because the window to do it is now**.
* **The session store's on-disk format does not change** (Part C, C4):
  MessagePack, the existing `sessions` DDL, the existing expiry semantics.
  The Rust `sessions` table starts empty at cutover (the Nest `session`
  table is abandoned and a fresh `SESSION_SECRET` forces re-login), so there
  was no legacy-format constraint to preserve — but keeping the format means
  the change is a **code-ownership** change with **no** data implication,
  and the runbook's schema-drift gate stays green.
* **`sqlx.toml` is not adopted** (Part C, C6). sqlx 0.9 offers a per-crate
  `sqlx.toml` with a "force `chrono` when `time` is enabled" setting, which
  would *also* remove the `AS "…chrono::NaiveDateTime"` overrides — but
  T65 removes the cause (`sqlx/time` is no longer enabled) rather than the
  symptom, and `sqlx-toml` also brings migration-journal renaming into
  scope. Not needed.
* **The `bcrypt` dependency is not removed now** (Part C, C7). It is
  legacy-verification-only and it has a **measurable** end date
  (`SELECT COUNT(*) FROM users WHERE password_hash NOT LIKE '$argon2id$%'`
  -> 0), but the imported Nest hashes can only be upgraded by a login each —
  so the dependency lives until every imported account has logged in once.
  The criterion is recorded so the dependency does not outlive its purpose.
* **`CORS_ORIGINS` stays a comma-separated env string** for now, but its
  entries become **canonical-form-enforced** at boot (Part C / B7). A
  non-canonical entry (trailing slash, uppercase host, explicit default
  port, a path) is a **boot error naming the offending entry**, not a silent
  mismatch. A stricter format (one origin per line, or a JSON array) is a
  follow-up — the canonicalization is what removes the *hazard*; the
  delimiter is only ergonomics.
