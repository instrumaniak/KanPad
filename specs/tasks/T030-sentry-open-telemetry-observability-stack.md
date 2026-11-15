# T030: Sentry + OpenTelemetry observability stack

> **STATUS: pending.** Natural follow-up to **T029** (SQL/DB-side observability stays
> vendor-free). Sentry SaaS (Developer, free) for edge errors + linked distributed
> traces **now**, engineered so the host can later be swapped to self-hosted OSS
> (Bugsink / GlitchTip / OpenObserve / SigNoz) or a self-built tool **without code
> changes** — only env vars and (in one case) a single frontend module. Record the
> exit path; do not build the exit yet.
>
> **Post-review (2026-11-16):** pins corrected (`tracing-opentelemetry 0.33`, not
> `0.32`), grep gate made satisfiable, runtime-init ordering made a decision (no
> `#[tokio::main]`), rolldown-vite spike mandated for the sourcemap plugin, health-path
> span drop required, OTLP exit branch switched to **HTTP/protobuf** (not `tonic`/gRPC),
> `OTEL_SERVICE_NAME` added, OTLP verification proof requires a `debug` exporter
> collector dump, `init_tracing` re-based on `Registry`, shared-ownership with T029
> spelled out. See §Notes.

## Goal

Instrument frontend + backend with one linked distributed trace per request and
first-class error reporting, using **open protocols only**, with the vendor treated as
a deployment detail:

1. **Frontend** — `@sentry/react` (DSN from `import.meta.env.VITE_SENTRY_DSN`),
   `browserTracingIntegration()`, `propagateTraceparent: true`, scoped
   `tracePropagationTargets` (dev: Vite proxy/`/api` same-origin; prod: the API
   origin from `VITE_API_URL`). Sourcemaps + release via `@sentry/vite-plugin`.
2. **Backend** — sentry-rust for error capture (panic integration + `capture_*` at the
   5xx error-boundary site) and **OTel API spans only** via `tracing-opentelemetry`
   over the existing `tower-http TraceLayer` and the **154 existing
   `#[tracing::instrument]` sites**; exported through an **env-switchable exporter**:
   `OTEL_EXPORTER_WITH=sentry` (default; native `SentrySpanProcessor` — errors↔spans
   linked, free) or `=otlp` (`opentelemetry-otlp` HTTP/protobuf → any OTLP endpoint:
   SigNoz / OpenObserve / Sentry OTLP beta — **env-only swap, built in this task**).
3. **Not Sentry**: metrics stay in T029's Prometheus (`/metrics`, T60; Sentry has no
   OTLP metrics ingest); uptime/log retention stay in T025/docs.

Non-negotiable outcome: a card-move click produces **one trace** in the Sentry UI —
navigation span → `POST /api/v1/cards/{id}` request span (`TraceLayer`) → repository
`#[tracing::instrument]` span(s) — and backend panics land as issues in the same
project. (sqlx emits **events, never spans** — there is no `sqlx span(s)` leg;
per-statement data lives in T029's logs/metrics.)

## Depends On

T029 — used as **merge order**, not a semantic dependency (sibling tasks; shared
`init_tracing` builder chain; see Shared ownership below).

## Phase

1 — performance & resilience wave.

## Critical

No — but it closes the "no errors/traces in prod" gap left by T029/T025.

## Shared ownership with T029

- T029 authors the `init_tracing()` builder chain first (registers `SqlxSlowQueryLayer`,
  `sqlx::query=warn` default leg). **This task appends the OpenTelemetry Layer
  after T029's Layer in the same `try_init()` chain**, preserves the `sqlx::query=warn`
  leg and swallow-reinit test semantics, and does not modify `metrics.rs`.
- `config.rs` edits here land after T029's `DB_SLOW_STATEMENT_MS` tests; both reuse the
  existing `Settings` test harness.
- Merge order assumed: **T029 → T030**.

## Research summary (Nov 2026)

**Sentry Developer (free) plan, shared per-org quota across projects**: 1 user,
**5,000 errors/mo**, **5M spans/mo** (traces bill in spans on current SDKs; legacy
transactions gone), 50 replays, 5 GB logs, 30-day lookback, **email alerts only**.
**No pay-as-you-go: hitting the quota silently drops events until the month resets** —
quota hygiene (sampling, health-path exclusion, spike protection) is a design input.

**Backend Rust stack (four otel-line crates, pinned together)** — `sentry` 0.49.5 +
`opentelemetry` **0.32.0** + `opentelemetry_sdk` **0.32.1** + `opentelemetry-otlp`
**"0.32"** (→ 0.32.0; no 0.32.1 exists) + `tracing-opentelemetry` **"0.33"** (→ 0.33.0 —
0.32.x pairs with otel **0.31** and must not be used) +
`sentry-opentelemetry` 0.49.5 (`SentryPropagator`, `SentrySpanProcessor`). All five
move together when the `sentry` line moves to otel 0.33; comment the pin block in
Cargo.toml. MSRV 1.89 ≤ project 1.99 ✓.

**Why not pure OTLP → Sentry OTLP beta now**: open beta (Oct 2025); span events
dropped; no OTLP metrics; errors would never reach Sentry without sentry-rust anyway.
Still enabled as the `=otlp` switch branch (it **is** the env-only exit path), just not
the default.

**Frontend**: `@sentry/react` 11.12.0; distributed tracing is out-of-box via
`sentry-trace` + `baggage`; `propagateTraceparent: true` additionally emits W3C
`traceparent` (set it — required for the pure-OTLP exit path; harmless otherwise).
Ports matter for `tracePropagationTargets`.

**`@sentry/vite-plugin` requires a rolldown-vite spike**: project uses Rolldown-powered
Vite 8 + `@rolldown/plugin-babel`; plugin `^5` composes with it (rolldown-aware since
4.x; debug-ID fixes merged 2026-10), but hidden-source-map behavior under synthetic
loader chunks is unverified (open issue #898 — benign warnings possible). Spike first;
fallback: `build.sourcemap: 'hidden'` + explicit `sentry-cli releases files … upload-sourcemaps`
in the `web` CI job.

**CORS (mandatory backend change)**: `lib.rs::cors_layer()` (:570) allow_headers
currently `content-type` + `x-request-id`; the browser preflights `sentry-trace`,
`baggage` (+ `traceparent`) → add all three or **every traced request fails at
preflight**. CSRF (T56, `csrf_layer` :614, outer of CORS per layer order) is
origin-based — unaffected.

**Versions/features**: JS side current (verified `npm view`, 11.12.0). Rust MSRV 1.89
ok (project 1.99).

## Spec References

- `specs/tasks/T029-sql-query-observability.md` — sibling; metrics stay there, not in
  Sentry.
- `backend-rs/src/lib.rs` — `cors_layer()` (:570; headers at :593) allow_headers;
  `init_tracing()` (:513, default filter `"backend_rs=info,tower_http=info"` at :515 —
  T029 extends it); `metrics_router` un-merged.
- `backend-rs/src/config.rs` — `Settings` env parsing/validation patterns; boot-config
  log; fail-fast validation style (`METRICS_PORT=0`, enum reject) to mirror for sample
  rates and the exporter enum.
- `backend-rs/src/main.rs` — `#[tokio::main]` at :69 today — **not viable**: sentry
  client must init **before** the runtime starts (sentry-rust requirement, enforced by
  its docs). Refactor required (see Steps): sync `fn main()` creates the guard, then a
  manual `tokio::runtime::Builder` builds the runtime and enters the current
  (now inner) async body.
- `backend-rs/src/features/cards/routes.rs:612-719` — `SpanFieldCapture` test pattern
  for span capture across subscribers.
- `frontend/src/main.tsx` — init before render; kebab-case flat modules in
  `frontend/src/lib/` (e.g. `client.ts`, `error-messages.ts`) to mirror for
  `observability.ts`.
- `frontend/vite.config.ts` — Rolldown `build.rolldownOptions` + custom code-split
  groups; `@sentry/vite-plugin` gated on `SENTRY_AUTH_TOKEN`.
- `frontend/.env.example`, `backend-rs/.env.example`, `docs/deployment.md` — env
  documentation surfaces.
- `.github/workflows/backend-rs.yml` — the **`web` job** (≈:603-642) is the only
  frontend build; pass `VITE_SENTRY_RELEASE=${{ github.sha }}` there.
- External: sentry.io React tracing + distributed-tracing guides; Rust Axum guide
  (NewSentryLayer/SentryHttpLayer — **rejected**, see Constraints); OTLP ingest page
  (beta); Envelope spec (develop.sentry.dev) — referenced for the future self-host
  exit, not used here; Sentry Developer plan comparison; broch.tech/posts/
  rust-tracing-opentelemetry (validates the env-switchable OTLP design: HTTP exporter,
  full `/v1/traces` path, `OTEL_SERVICE_NAME`, batch exporting, headers env handling —
  **conceptual; 2023 syntax is superseded**).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/Cargo.toml` | `sentry` 0.49.5 (default features incl. `panic`; `tracing` for layers), `opentelemetry` 0.32.0, `opentelemetry_sdk` 0.32.1 (features `trace`), `tracing-opentelemetry` "0.33" (→ 0.33.0), `opentelemetry-otlp` "0.32" (→ 0.32.0), `sentry-opentelemetry` 0.49.5; 5-line pin comment block (versions matched, churn rule) |
| `backend-rs/src/observability.rs` | NEW — single module: init sentry client (**no-op when `SENTRY_DSN` unset**); exporter switch `OTEL_EXPORTER_WITH` (`sentry` default → `SentryPropagator` + `SentrySpanProcessor`; `otlp` → `TraceContextPropagator` + **HTTP/protobuf** OTLP exporter honoring standard envs `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` (full URL incl. `/v1/traces`), `OTEL_EXPORTER_OTLP_TRACES_HEADERS` (verify honored in 0.32; else parse ~10 lines manually), `OTEL_SERVICE_NAME` (`kanpad-backend` / `kanpad-frontend`), `OTEL_SDK_DISABLED` honored); health-path drop sampler (`/health`, `/ready`, `/metrics` → 0); `traces_sample_rate` from env; `capture_error` convenience (tags: `request_id`); attaches `tracing-opentelemetry` layer into the **T029-extended** `init_tracing` `Registry` chain (appended after T029's `SqlxSlowQueryLayer`); tests |
| `backend-rs/src/config.rs` | NEW envs: `SENTRY_DSN` (empty = disabled; **unset = disabled, malformed = fail boot**; never logged), `SENTRY_ENVIRONMENT` (default from `APP_ENV`), `SENTRY_RELEASE` (optional; else crate version via `release_name!()`), `SENTRY_TRACES_SAMPLE_RATE` (parse f64; **NaN/±∞/<0/>1 → boot error, no clamping**; default `1.0`; production must set `0.1` — documented), `OTEL_EXPORTER_WITH` (enum `sentry|otlp`, invalid → **fail boot**); Rustdoc + unit tests mirroring existing patterns |
| `backend-rs/src/lib.rs` | `cors_layer`: add `sentry-trace`, `baggage`, `traceparent` to allow_headers; keep chain order (CORS outer of CSRF); keep `/metrics` un-merged; no app-logic changes |
| `backend-rs/src/lib.rs` unit tests | OPTIONS preflight regression test asserting all three headers are echoed; existing CSRF/request-id suites re-run green |
| `backend-rs/src/main.rs` (or `lib.rs`) | `#[tokio::main]` removed → sync `fn main()` creating the `sentry::init` guard first, then a manual `tokio::runtime::Builder::…build(); rt.block_on(inner())` shape; `init_tracing_from_env()` first inside the async body; drop unchanged (flush semantics for cPanel/Passenger) |
| `backend-rs/tests/` (integration) | panics become captured events via sentry test-mode transport seam; harness rules respected |
| `frontend/package.json` | `@sentry/react` 11.12.0; `@sentry/vite-plugin` `^5` (dev) |
| `frontend/src/lib/observability.ts` | NEW — the **only** Sentry import in app code: init (DSN, env from `VITE_*`, release, `tracesSampleRate` 1.0 dev / env-driven prod, `tracePropagationTargets` = same-origin `/^\/api/` in dev, **prod origin from `VITE_API_URL`**, `propagateTraceparent: true`, no replay); `beforeSend` defensive scrub (strip `authorization`/`cookie` request headers); module-scope init (never inside a component/effect — StrictMode double-mount discipline documented one line); export typed-minimal `captureException` |
| `frontend/src/main.tsx` | `import './lib/observability'` first line |
| `frontend/vite.config.ts` | `@sentry/vite-plugin` gated on `SENTRY_AUTH_TOKEN` (sourcemaps + release=`VITE_SENTRY_RELEASE`, `hidden-sourcemap`); **spike first** (`npm run build`); fallback: `build.sourcemap: 'hidden'` + `sentry-cli` upload in CI `web` job; benign #898 warnings tolerated; notes record chosen path |
| `frontend/.env.example` | `VITE_SENTRY_DSN`, `VITE_SENTRY_ENVIRONMENT`, `VITE_SENTRY_TRACES_SAMPLE_RATE`, `VITE_SENTRY_RELEASE` |
| `.env.example` (backend) | `SENTRY_DSN`, `SENTRY_ENVIRONMENT`, `SENTRY_RELEASE`, `SENTRY_TRACES_SAMPLE_RATE`, `OTEL_EXPORTER_WITH`, (`OTEL_EXPORTER_OTLP_*` documented for the exit path) |
| `.github/workflows/backend-rs.yml` | `web` job only: pass `VITE_SENTRY_RELEASE=${{ github.sha }}` (shallow `fetch-depth: 1` ok), optional `SENTRY_AUTH_TOKEN` secret for sourcemap upload; **`backend-mariadb` and cache gates untouched** |
| `docs/observability.md` | NEW — env tables (both apps), `OTEL_SERVICE_NAME`, sampling policy (prod 0.1 mandated), **swap runbook**: SaaS → Bugsink/GlitchTip (DSN change only — Bugsink=errors only; GlitchTip spans need `traceLifecycle: 'static'`, replay/release-health do not exist there), spans → SigNoz/OpenObserve/collector (`OTEL_EXPORTER_WITH=otlp` + `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` incl. `/v1/traces` + `OTEL_EXPORTER_OTLP_TRACES_HEADERS`; sentry OTLP beta available as an example endpoint), what transfers (errors/metrics/releases/git sha) vs what doesn't (dashboards, suspect commits, replay); spike protection is **per-project** (Settings > Spike Protection, Manager/Billing/Owner) — not an SDK knob; quota math on free tier; 30-day lookback caveat; `OTEL_SDK_DISABLED=true` for CI; how to read one trace; inline-literal warning (bound params unlogged — statement text does log at debug) |
| `docs/deployment.md` | SENTRY_* container env notes; `OTEL_SDK_DISABLED` note |
| `docs/lessons/rust-backend.md`, `frontend/src/lib/…` | dated, tagged lessons |

**Must NOT appear in the diff:** any sentry-native span/transaction API
(`sentry::start_transaction`, `SpanLayer`, `SentryHttpLayer`, `Sentry.Span`) —
**grep gate (two-part, satisfiable)**:
`grep -E "start_transaction|SpanLayer|SentryHttpLayer" backend-rs/src` → no matches;
`SentrySpanProcessor` appears exactly once, in `observability.rs`, as the sanctioned
exception; `send_default_pii: true`; replay enabled; query strings/bodies/card content
in span attributes or events; `.sqlx` hand-edits; commits without approval.

## Implementation Steps

**Phase 1 — frontend + CORS**
1. Add deps; `observability.ts`; `main.tsx` first import; env examples; `beforeSend`
   scrub (**vitest asserts** `authorization`/`cookie` stripped); `tracePropagationTargets`
   dev/prod mode assert (manual capture in §Notes acceptable; vitest preferred).
2. `vite-plugin` spike: `npm run build` on a throwaway branch; with token only → maps
   hidden; without → no maps; web job green with the chosen approach; record choice in
   §Notes.
3. Backend CORS: add the three headers; regression test (all three echoed); CSRF
   reports unchanged; full `cargo test` passes.

**Phase 2 — backend init**
4. `config.rs` envs + validation + tests (parse f64 strict; enum reject;
   empty/malformed DSN paths; `OTEL_EXPORTER_WITH`).
5. `observability.rs`: no-op-without-DSN (incl. malformed); `sentry` branch
   (`SentryPropagator` + `SentrySpanProcessor` + health-path sampler + rate from env);
   `otlp` branch (`TraceContextPropagator` + `opentelemetry-otlp` **HTTP** exporter
   reading standard envs; manual header parse fallback if unhonored; batch exporter);
   `capture_error`; re-base `init_tracing` on the T029 `Registry` (append after
   SqlxSlowQueryLayer); sync `fn main` + runtime builder refactor.
6. **Instrumentation audit (backend spans)**: enumerate all 154 `#[tracing::instrument]`
   sites; verify the hot read/write paths carry spans (cards board-list/list-by-board,
   card move, notes list, board load, labels toggle, checklist ops) — add missing
   `#[tracing::instrument(skip(self, …), fields(board_id, card_id))]`; **credential-safety
   sweep**: any site passing passwords/tokens/secrets/session ids/auth headers as args
   must use `skip(...)` or drop the arg — record findings in §Notes; confirm the
   `tower-http TraceLayer` span exists in both test and real subscribers; ensure span
   does **not** record query strings (match on `matched_path` only; strip `%query`
   if raw URI reaches the span).
7. Errors end-to-end: force a panic in a throwaway handler/test; confirm Sentry
   (dev project) receives the issue with request_id tag + trace id; remove scratch code.

**Phase 3 — quota hygiene, swap verification, docs**
8. Spike protection: enable on **dev + prod projects** (per-project setting, Manager/
   Billing/Owner); document (no SDK knob); create alert rules (new issue → email) on
   both projects; 1-user free limit noted.
9. **Exit-path verification (the whole point)**: docker run a throwaway
   `otel/opentelemetry-collector` with `otlp` http receiver `:4318` + `exporters: [debug]`;
   `OTEL_EXPORTER_WITH=otlp OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318/v1/traces`
   (verify trailing path); observe the collector's debug span dump including the http
   request span and repo `#[instrument]` spans; paste minimal proof into §Notes
   (span dump excerpt or Sentry event/trace ID); restore `=sentry`. **`nc` is not a
   valid verifier** — reject that shortcut in implementation.
10. `docs/observability.md` swap runbook + quota hygiene + what-doesn't-transfer +
    inline-literal warning + `/v1/traces` path rule + `OTEL_SDK_DISABLED`;
    deployment.md SENTRY_* notes; lessons.

## Constraints

- **Open protocols only**: span creation = OTel API / `#[tracing::instrument]` only;
  no sentry-native span/transaction APIs (two-part grep gate above). Errors may use
  sentry API freely.
- **Env-vars-only swap**: all endpoints/DSNs/sample-rates/exporter-switch via env;
  no URLs/keys/hosts hardcoded (dev DSN never committed — placeholders only).
- Pins: `sentry` 0.49.5 + `opentelemetry*` **0.32** line + `tracing-opentelemetry`
  **0.33** — all five move together; bumping past otel 0.33 requires one joint commit
  (note in §Notes).
- Free-tier hygiene mandatory: health-path sampler drop; no replay; no PII
  (`send_default_pii: false`; span attrs never carry bodies/card content/queries);
  prod sample-rate `0.1` mandated in docs; inline literals warning; spike protection.
- `sentry` client created in **synchronous `fn main` before** runtime build; panic
  integration default-on; flush on drop retained; never `process::exit` without drop.
- CORS change is additive only; T56 CSRF untouched; `x-request-id` contract unchanged
  (T007); OPTIONS regression test mandatory.
- `/metrics` remains T029's Prometheus; do not re-implement metrics in Sentry.
- **No commit without asking**; TDD; both DB legs unchanged; test harness rules apply.

## Acceptance Criteria

- [ ] Two-part grep gate: `grep -E "start_transaction|SpanLayer|SentryHttpLayer" backend-rs/src`
      → no output; `grep -c "SentrySpanProcessor" backend-rs/src/observability.rs` → `1`.
- [ ] Frontend prod build: bundle growth ≤ T016 budget; sourcemaps exist iff
      `SENTRY_AUTH_TOKEN` set (hidden-sourcemap); `web` job green with the chosen
      plugin/fallback path.
- [ ] `curl` preflight (`Origin` + `Access-Control-Request-Headers:
      sentry-trace,baggage,traceparent`) → `200/204` echoing all three.
- [ ] Dev E2E: card move produces one linked trace visible in Sentry dev project
      (navigation → `POST /api/v1/cards/{id}` request span → repository `#[instrument]`
      span); screenshot/dump recorded in §Notes.
- [ ] Release named `<git sha>` exists in the Sentry project with ≥1 uploaded
      sourcemap (recorded once) **only** on token-bearing builds; `web` job sets
      `VITE_SENTRY_RELEASE=${{ github.sha }}`.
- [ ] Forced backend panic in a test → issue captured with `request_id` tag + trace;
      reproduction steps in §Notes.
- [ ] `OTEL_EXPORTER_WITH=otlp` + throwaway collector (`debug` exporter) → collector's
      span dump shows the http request span and repo `#[instrument]` spans with no code
      changes beyond env; excerpt pasted in §Notes; default `=sentry` restored.
- [ ] `SENTRY_DSN` unset → both sides run silently (no panics, no half-init);
      malformed DSN → boot error with clear message (+ unit test).
- [ ] Health/ready/metrics paths produce **no** spans (verified via collector dump or
      Sentry trace dump pasted in §Notes).
- [ ] Metrics endpoint unchanged; T029 slow-statement logging unaffected (re-verified).
- [ ] Instrumentation audit results in §Notes: hot-path repo fns instrumented; zero
      credential-bearing args recorded in span args (grep evidence); request span excludes
      query string.
- [ ] `docs/observability.md`: env tables, `OTEL_SERVICE_NAME`,
      `OTEL_SDK_DISABLED`, sampling policy, swap runbooks (DSN → Bugsink/GlitchTip incl.
      `traceLifecycle: 'static'` note; `OTEL_EXPORTER_WITH=otlp` → SigNoz/OpenObserve/
      Sentry-OTLP incl. `/v1/traces`), what-doesn't-transfer list, 30-day lookback,
      spike protection; `.env.example` files updated; deployment.md; lessons appended.
- [ ] No commit without approval.

## Notes

Post-review (2026-11-16, author: spec review ×3 — codebase/spec/external):
- **Pins corrected**: `tracing-opentelemetry 0.33` (not `0.32` — that pairs with otel
  0.31); `opentelemetry-otlp "0.32"` → 0.32.0, 0.32.1 does not exist; all otel-line
  crates (5 incl. sentry) bump together on future sentry releases.
- **Span reality**: sqlx 0.9.0 emits events, not spans; the trace is
  navigation → request span → repo `#[instrument]` spans; `sqlx-tracing` rejected
  (`0.2.1` pins `sqlx ^0.8` → duplicate major).
- **Main refactor**: `#[tokio::main]` incompatible; sync `fn main` + manual runtime
  builder.
- **Vite plugin**: Rolldown-powered Vite 8 + `@sentry/vite-plugin ^5` composes (spike
  to confirm hidden sourcemaps; issue #898 benign warnings tolerated); fallback
  `sentry-cli` path documented.
- **Exit path**: OTLP branch deliberately HTTP/protobuf (not `tonic`); `OTEL_SERVICE_NAME`
  added; `OTEL_SDK_DISABLED` in CI; `/v1/traces` trailing path required by HTTP exporter;
  verification requires a `debug`-exporter collector dump — not `nc`; botched shutdowns
  flush via drop (no `process::exit`).
- **Ownership**: append OTel layer after T029's `SqlxSlowQueryLayer` in the
  `init_tracing` chain; keep `sqlx::query=warn`; never touch `metrics.rs`; T029 → T030
  merge order.
- Reference: broch.tech/posts/rust-tracing-opentelemetry (env-switchable OTLP design;
  conceptual only — 2023 API superseded).

## Lessons

(pending)
