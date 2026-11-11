# 00-overview — Architecture hardening: constitution

> Plan date: 2026-11-11. Source: Sr. Software Architect review of the whole codebase
> (backend-rs, frontend, infra/CI) for **scalability, reliability, performance, at low
> infrastructure & maintenance cost**. This file is the constitution: scope, locked
> decisions, the findings register (the evidence base every task cites), and the global
> definition of done. Design details live in `10`/`20`/`30`/`40`.

## Why

KanPad will be **self-hosted by an organization and used by employees and teams** for
cross-functional collaboration, project management, product development and brainstorming
(100+ users, org-wide). The current codebase is production-grade in many respects (see
"Verified strengths" below) but was built and validated as a **single-user** system with a
**manual, single-box deployment**. This plan closes the reliability gaps first, then the
performance gaps at the target scale, then designs the collaboration model — while keeping
the infrastructure footprint at zero new services (no Redis, no k8s, no queue, no message
broker; reuse MySQL, which already holds sessions).

## Scope

**In scope:** reliability floor (backups, session-cache leak, error handling, bundle,
e2e, secrets, docs), performance & resilience at 100+ users (transaction retries, board
rendering, pagination, search, metrics, capacity), dual-path operations (Docker appliance
+ existing cPanel runbook), monitoring (hosted uptime + logs), and the collaboration
**design** (spec only).

**Out of scope (explicitly):** real-time/WebSocket (deferred — polling with existing
invalidation is adequate; revisit SSE only if collaboration feels laggy), horizontal
scale-out implementation (readiness only — sessions are already in MySQL so the app is
replica-ready), collaboration **implementation** (tasks T028+ are written after the T027
design review), Redis/queue/k8s adoption.

## Locked decisions (confirmed with user, 2026-11-11)

| # | Decision |
|---|----------|
| D-1 | **Both deployment paths supported**: ship a working Docker Compose "appliance" AND keep the cPanel/LiteSpeed two-origin runbook current. |
| D-2 | **Scale target: 100+ users org-wide** — pagination, capacity modelling and the collaboration model are urgent; realtime is not. |
| D-3 | **Reliability first, then collaborate** — Wave 0 → Wave 1 → Wave 2 (design) sequencing. |
| D-4 | **Monitoring: hosted uptime check on `/ready` + JSON log retention** — no self-hosted Prometheus/Grafana for now. |
| D-5 | **T010 (fractional sort keys) lands before T020 (tx-retry helper)** — T010 rewrites the cards/columns write paths first and already extends the retry matcher to `1062`; T020 then extracts the helper and applies it to the remaining single-shot transactions. |
| D-6 | Spec directory name: `specs/arch-hardening-specs/`. Task granularity: one task = one agent session (≤ ~8 steps, ≤ ~10 source files). |

## Global definition of done (applies to EVERY task)

- **TDD**: failing test first. No test deletion or weakening. Red-proof any new gate
  (prove it fails on a probe before trusting it — AGENTS.md lesson).
- **Persistence verification**: every POST/PATCH/DELETE test adds one assertion that
  re-reads state via a *different* API call or a direct sqlx SELECT (AGENTS.md rule).
- **Both engines**: migrations, queries and tests pass on **MySQL 8.4 and MariaDB 10.6+**
  (prod flavor ambiguity noted in `40-operations.md` — verify, don't assume).
- **Gates** (paste outputs as evidence): `cargo fmt --check` 0 ·
  `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic` 0 ·
  `cargo sqlx prepare --check` 0 (regenerate `.sqlx/` with the pinned CLI on MySQL,
  database `kanpad`, per T009) · frontend: `npm run gen:api -- --check`, `tsc --noEmit`,
  `vitest` · coverage floor stays green.
- **Contract changes** only via the T52 same-PR process (spec + utoipa + `openapi.json` +
  `schema.d.ts` together). Additive-only wire changes unless a task says otherwise.
- **Never hand-edit `_sqlx_migrations`**; forward-only migrations; backup before any
  production migration (T011 makes this mechanical).
- **No commit/push without explicit user approval**; then Conventional Commit, summary
  line 20–50 chars.
- **Lessons** appended to the relevant `docs/lessons/*.md` (dated, tagged).
- **Traceability**: each task's Acceptance Criteria cite finding IDs from the register
  below; closing a finding = checking an AC.

## Finding ID convention

`R` = reliability, `P` = performance, `S` = scalability, `C` = cost/maintenance.
Severity: Critical > High > Medium > Low. `EXISTS`/`MISSING` in evidence means verified
in the tree vs verified absent — no speculation.

## Findings register

### Reliability

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| R1 | **HIGH** | No automated DB backups — only manual `mysqldump` lines in the migration runbook; restore is the documented *only* DB rollback path; no retention, no drill record | `docs/data-migration-live.md`, `docs/cutover.md:18-19`, `specs/security/09-logging-monitoring-ir.md:41` unverified | T011 |
| R2 | **HIGH** | Logout/login never clears the React Query cache → user B sees user A's data within `staleTime` (5 min) on a shared tab; query keys not user-scoped | `frontend/src/features/auth/auth-provider/index.tsx:23-35`; `clear()`/`removeQueries` only called in tests | T013 |
| R3 | **HIGH** | Dev `compose.yaml` cannot build: `nest` service → `backend/` deleted (commit `4051be68`); `frontend` has no Dockerfile yet is declared a build context; `frontend` depends on `nest` | `compose.yaml:24-27,63-69` | T012 |
| R4 | MEDIUM | Deadlock/lock-wait (`1213`/`1205`) retry + `READ COMMITTED` exist **only** in `move_card`; `create_card`, `delete_card`, `update_card`, bulk `move_cards` and ~18 other position-shifting transactions are single-shot → generic client-visible 500 under concurrent drag-drop | `backend-rs/src/features/cards/routes.rs:234,251-380`; grep `1213\|retry` → only that file; `backend-rs/src/error.rs:389` | T020 (after T010) |
| R5 | MEDIUM | No 401/session-expiry handling in the live path — `error-messages.ts` (incl. "Session Expired → /login") is dead code; `retry: false` on all queries; no AbortController/timeouts | `frontend/src/lib/error-messages.ts:44-59` (test-only callers), `frontend/src/App.tsx:58-69` | T014 |
| R6 | MEDIUM | Board view has no `isError` branch — a failed `['columns', boardId]` fetch silently renders an *empty board*; single app-wide error boundary without `resetKeys` → stuck fallback | `frontend/src/features/boards/board-view.tsx:167-181`, `frontend/src/App.tsx:77`, `frontend/src/components/route-error-boundary.tsx:60-72` | T015 |
| R7 | MEDIUM | Playwright e2e dead: config boots the deleted `../backend` on `:3000`; CI "pins" but never runs `playwright test` | `frontend/playwright.config.ts:23-27`, `.github/workflows/backend-rs.yml` (playwright step) | T017 |
| R8 | MEDIUM | No alerting/uptime monitoring of any kind — `/metrics` exists but nothing scrapes it; downtime is user-discovered | repo-wide search for monitoring configs → empty; `specs/security/09-logging-monitoring-ir.md` open | T025 (+ D-4) |

### Performance

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| P1 | **HIGH** | Route code-splitting defeated in the production build: all 9 `React.lazy` routes merge into one entry chunk (rolldown `codeSplitting.groups` capture); `dist/` has 5 JS files, 1 dynamic import | `frontend/vite.config.ts:32-60`; `frontend/dist/assets/` (measured) | T016 |
| P2 | **HIGH** | 3.1 MB mermaid chunk is `modulepreload`ed + statically imported from the entry → ~1.29 MB gzip initial JS on every cold load, including `/login` | `frontend/dist/index.html:24`, `frontend/dist/assets/index-*.js` line 1; measured gzip sizes | T016 |
| P3 | **HIGH** | Board loads ALL cards unpaginated in one response and renders them all — no virtualization, no `useInfiniteQuery`, O(N) `CardDetailPanel` instances per board | `frontend/src/features/columns/columns.api.ts:52-54`, `backend-rs/src/features/columns/routes.rs:69-70`, no `virtual` in `frontend/src` | T021 |
| P4 | MEDIUM-HIGH | Cross-column drag writes `setQueryData` on **every `dragOver` event**; `allColumns` prop identity changes per write → full-board re-render at pointer-move rate | `frontend/src/features/cards/drag-drop-context.tsx:100-186`, `frontend/src/features/boards/board-view.tsx:256,269` | T021 |
| P5 | MEDIUM | Lists use `LIMIT/OFFSET` with **no page ceiling**; notes/projects search is `LIKE '%q%'` (leading wildcard — `IDX_notes_title` can never serve it; no FULLTEXT anywhere) | `backend-rs/src/extract.rs:115-135`, `backend-rs/src/features/notes/store.rs:354-357`, grep `FULLTEXT` → 0 | T022 (ceilings), T023 (FTS) |
| P6 | LOW-MED | Board/archived/notes lists fetch with backend defaults (limit 50) and have **no pagination UI** → items beyond page 1 invisible | `frontend/src/features/boards/use-boards.ts:38-46`, `board-list.tsx` (no page state) | T022 |
| P7 | LOW | Metrics gaps: `acquire_timed` never called in production (acquire-duration histogram always empty); `kanpad_db_up` only refreshed by `/ready` hits; global 30 s timeout + 10-conn pool means ~10 slow queries starve everyone (11th → 503) | `backend-rs/src/state.rs:177-184` (only caller is a test), `backend-rs/src/metrics.rs:104-161`, `backend-rs/src/lib.rs:72` | T024 (metrics); timeout/pool interplay = capacity input (T026) |

### Scalability

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| S1 | **HIGH** (strategic) | No collaboration model — single-user ownership on every table; no workspace/team/membership/sharing/permission concept; admin = single CLI-minted superadmin, no admin panel, no activity log; last-write-wins concurrency | grep `team\|share\|member\|permission` in `backend-rs/src` → nothing; README feature list unchecked | T027 (design), T028+ (impl) |
| S2 | MEDIUM | Rate limiting is per-process in-memory (documented single-replica assumption) — horizontal scale-out multiplies every limit by N; sessions already in MySQL so the app is replica-ready, only the governor is not | `backend-rs/src/lib.rs:356-358` (own comment), no redis dep in `Cargo.toml` | deferred (readiness note in `20-performance.md`) |
| S3 | MEDIUM | Single-box, single-process with no documented capacity model — no user-count/concurrency assumptions anywhere in specs; pool=10, 120 req/min/IP defaults untested against real org load | `backend-rs/.env.example`, spec greps | T026 |
| S4 | LOW | No queue/worker infra beyond the 60 s session sweeper (fine today; blocks async features like export/email later); nested children (board columns) unpaginated by contract | grep `tokio::spawn` → 2 hits in `main.rs`; `backend-rs/src/common/pagination.rs:4` | out of scope (noted for collaboration follow-ups) |

### Cost / maintenance

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| C1 | MEDIUM | Manual deploys: build on a machine → upload; 3+ divergent proxy script copies (`scripts/app-proxy.js`, `release/app-proxy{,-v2,-plus}.js`); no deploy script; README documents the deleted Nest stack | `release/`, `scripts/`, `README.md:47-131` | T019 |
| C2 | MEDIUM | No bundle-size gate in CI; `chunkSizeWarningLimit` raised to 800; build-only packages (`shadcn`, `tailwindcss`, `@tailwindcss/vite`, `tw-animate-css`) in prod `dependencies`; undeclared `@radix-ui/react-tabs`/`-tooltip` imports (work via hoisting only) | `frontend/vite.config.ts:31`, `frontend/package.json`, `frontend/src/components/ui/{tabs,tooltip}.tsx` | T016 (gate), T018 (deps) |
| C3 | LOW | Secrets at mode 644 (spec wants 600); `release/.env` shares the live `SESSION_SECRET` with `backend-rs/.env`; dev compose publishes MySQL on 0.0.0.0:3306; prod DB version documented inconsistently (MariaDB 10.6 vs 11.4) | `ls -l`, `compose.yaml:14`, `docs/mariadb-compat.md:3` vs `specs/security/06-infrastructure-deployment.md:77` | T018 (secrets), T012 (bind), T011/T025 (version check) |
| C4 | LOW / POSITIVE | Production footprint already minimal (static SPA + ~17 MB binary + existing DB on shared LiteSpeed host) — the low-cost baseline to preserve; CI heavy but $0 (public repo) | `docs/deployment.md`, `backend-rs/Dockerfile` | preserve (constraint in every ops task) |

## Verified strengths (must not regress)

- Backend: `/health` vs `/ready` split with budget-bounded direct dial; graceful shutdown
  with bounded pool close; RFC 9457 problem+json with no driver-text leakage; Argon2id
  with dummy-verify timing equalization; Origin/Sec-Fetch-Site CSRF; per-IP throttling
  with `TRUSTED_PROXIES` awareness; digest-pinned non-root Docker image; thin-LTO stripped
  release binary; 90% line coverage with an 85% floor; MySQL + MariaDB CI matrix.
- No N+1 anywhere sampled — batched `IN (...)` enrichment, single-join aggregates,
  set-based position gap open/close.
- Frontend: single `apiFetch` layer (zero stray `fetch()`), generated OpenAPI types with
  CI drift gate, React Compiler enabled, dnd-kit sensor tuning, optimistic updates with
  rollback, ~720 unit tests.

## Wave overview (informational — status lives in taskpad `status.yaml`)

| Wave | Phase | Tasks | Theme |
|------|-------|-------|-------|
| 0 | 0 | T011–T019 | Reliability floor (R1–R3, R5–R7, P1–P2, C1–C3) |
| 1 | 1 | T010*, T020–T026 | Performance & resilience at 100+ users (R4, P3–P7, S3) |
| 2 | 2 | T027 (+T028+ after review) | Collaboration design (S1) |

\* T010 is a pre-existing pending task that gates T020 (D-5).

Sequencing/serialization rules encoded as taskpad `depends` edges so parallel agents never
collide: CI workflow edits T016→T017; auth layer T013→T014; board view T021→T022;
cards/columns write paths **T010→T020**.
