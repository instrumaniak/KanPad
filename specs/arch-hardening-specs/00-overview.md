# 00-overview — Architecture hardening: constitution

> Plan date: 2026-11-11. Source: Sr. Software Architect review of the whole codebase
> (backend-rs, frontend, infra/CI) for **scalability, reliability, performance, at low
> infrastructure & maintenance cost**. This file is the constitution: scope, locked
> decisions, the findings register (the evidence base every task cites), and the global
> definition of done. Design details live in `10`/`20`/`30`/`31`/`32`/`33`/`40`.

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
**design** — workspaces/authz (`30`), SSE realtime invalidation (`31`), comments &
mentions (`32`), in-app notification inbox (`33`) (spec only).

**Out of scope (explicitly):** WebSockets/CRDT editing (D-7 chose SSE + cache
invalidation), email/SMTP notification delivery (in-app only — `33`), horizontal
scale-out implementation (readiness only — sessions are already in MySQL so the app is
replica-ready; realtime fanout is in-process, single-replica — S2), collaboration
**implementation** (tasks pre-authored as T031–T042 at user request 2026-11-16; the
T027 review still gates implementation — see amendment log), Redis/queue/k8s adoption.

## Locked decisions (confirmed with user, 2026-11-11)

| # | Decision |
|---|----------|
| D-1 | **Both deployment paths supported**: ship a working Docker Compose "appliance" AND keep the cPanel/LiteSpeed two-origin runbook current. |
| D-2 | **Scale target: 100+ users org-wide** — pagination, capacity modelling and the collaboration model are urgent; realtime is not. |
| D-3 | **Reliability first, then collaborate** — Wave 0 → Wave 1 → Wave 2 (design) sequencing. |
| D-4 | **Monitoring: hosted uptime check on `/ready` + JSON log retention** — no self-hosted Prometheus/Grafana for now. |
| D-5 | **T010 (fractional sort keys) lands before T020 (tx-retry helper)** — T010 rewrites the cards/columns write paths first and already extends the retry matcher to `1062`; T020 then extracts the helper and applies it to the remaining single-shot transactions. |
| D-6 | Spec directory name: `specs/arch-hardening-specs/`. Task granularity: one task = one agent session (≤ ~8 steps, ≤ ~10 source files). |
| D-7 | **Realtime is SSE + React Query cache invalidation, in Wave 2** (user decision 2026-11-16, supersedes the D-2 "realtime deferred" clause). Server→client only; events are metadata-only; `activity_log.id` is the resume cursor; browser falls back to polling automatically. Designed in `31-realtime-sse.md`. Comments + @mentions + in-app notifications are designed in `32`/`33` (user decision same date; they were PRD "Vision" items promoted to Wave 2 scope). Email stays out. |

## Global definition of done (applies to EVERY task)

- **TDD**: failing test first. No test deletion or weakening. Red-proof any new gate
  (prove it fails on a probe before trusting it — AGENTS.md lesson).
- **Persistence verification**: every POST/PATCH/DELETE test adds one assertion that
  re-reads state via a *different* API call or a direct sqlx SELECT (AGENTS.md rule).
- **Both engines**: migrations, queries and tests pass on **MySQL 8.4 and MariaDB 11.4**
  — the CI matrix legs (`.github/workflows/backend-rs.yml`). Prod flavor (10.6 vs 11.4) is
  unverified; the T011 restore drill records `SELECT VERSION()` and `40-operations.md` §6
  owns the correction (incl. adding a 10.6 CI leg if prod is confirmed 10.6).
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
| R4 | MEDIUM | After T010 (fractional `sort_key`s + scoped `1062` retry on card create/move landed), the retry envelope now covers **six write sites** — `create_card`, `update_card`'s move worker (`cards/routes.rs:112,325`), checklist-item create/update (`checklists/routes.rs:260,359`), column update (`columns/routes.rs:192`), and bulk `move_cards` (`columns/routes.rs:348`). Single-shot position/lock-taking sites remain: `delete_card`, `delete_checklist`/`delete_item`, `create_column`/`delete_column`, notes/boards/labels/tags transactions, `common/rebalance.rs` — generic client-visible 500 under concurrency | `grep MAX_ATTEMPTS\|is_transient_lock_error` → 6 loop sites (amended 2026-11-16 — the prior "only `cards/routes.rs` + `retry` module" claim was stale); `FOR UPDATE` grep → 32 sites across cards/checklists/columns store+routes + `common/rebalance.rs`; `error.rs` generic-500 arm | T020 (T010 done — gate satisfied) |
| R5 | MEDIUM | No 401/session-expiry handling in the live path — `error-messages.ts` (incl. "Session Expired → /login") is dead code; `retry: false` on all queries; no AbortController/timeouts | `frontend/src/lib/error-messages.ts:44-59` (test-only callers), `frontend/src/App.tsx:58-69` | T014 |
| R6 | MEDIUM | Board view has no `isError` branch — a failed `['columns', boardId]` fetch silently renders an *empty board*; single app-wide error boundary without `resetKeys` → stuck fallback | `frontend/src/features/boards/board-view.tsx:167-181`, `frontend/src/App.tsx:77`, `frontend/src/components/route-error-boundary.tsx:60-72` | T015 |
| R7 | MEDIUM | Playwright e2e dead: config boots the deleted `../backend` on `:3000`; CI "pins" but never runs `playwright test` | `frontend/playwright.config.ts:23-27`, `.github/workflows/backend-rs.yml` (playwright step) | T017 |
| R8 | MEDIUM | No alerting/uptime monitoring of any kind — `/metrics` exists but nothing scrapes it; downtime is user-discovered | repo-wide search for monitoring configs → empty; `specs/security/09-logging-monitoring-ir.md` open | T025 (+ D-4) |

### Performance

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| P1 | **HIGH** | Route code-splitting defeated in the production build: all 9 `React.lazy` routes merge into one entry chunk (rolldown `codeSplitting.groups` capture); `dist/` has 5 JS files, 1 dynamic import. **2026-11-17 (T016): fixed + closed** — catch-all `vendor`/`app` groups removed; post-fix dist has route-level chunks. **Second amendment (2026-11-17, same day): all app routes are now static imports** (user directive — perceive-snappiness first); markdown/pele stay lazy; one `vendor` chunk; 58 JS files | `frontend/vite.config.ts`; `frontend/dist/assets/` (measured) | T016 |
| P2 | **HIGH** | The mermaid chunk (measured 2026-11-17: **4.9 MB raw / 1,355,682 B gzip**; was 2026-11-13/2026-11-16 figures, now stale) was `modulepreload`ed from the entry → initial JS incl. `/login` ≈ **1.84 MB gzip**. **2026-11-17 (T016): closed by replacing the renderer with pele 1.13.0** (`pele/lazy`, per-type chunks on demand; mermaid chunk gone) → **initial gzip = 257,986 B** (entry 69,500 + runtime 429 + vendor 188,057; TS second amendment — all app routes static, `markdown-renderer` + pele stay lazy with idle prefetch) | `frontend/dist/index.html:23-26`, `frontend/dist/assets/` (measured 2026-11-17) | T016 |
| P3 | **HIGH** | Board loads ALL cards unpaginated in one response and renders them all — no virtualization, no `useInfiniteQuery`, O(N) `CardDetailPanel` instances per board. **2026-11-19 (T021): rendering half closed** — `@tanstack/react-virtual` on `column-card-list` + `board-list-view` (bounded DOM window; full item list to `SortableContext`; dragged row pinned via custom `rangeExtractor`; `CardDetailPanel` mount-gated on first open). Unpaginated data path stays with **T022**; column containment + visual affordances with **T044** (Stage A containment shipped inside T021) | `frontend/src/features/columns/columns.api.ts:52-54`, `backend-rs/src/features/columns/routes.rs:69-70`, `frontend/src/features/columns/column-card-list.tsx`, `frontend/src/features/boards/board-view/board-list-view.tsx` | T021 (rendering), T022 (hydration), T044 (visual) |
| P4 | MEDIUM-HIGH | Cross-column drag writes `setQueryData` on **every `dragOver` event**; `allColumns` prop identity changes per write → full-board re-render at pointer-move rate. **2026-11-19 (T021): closed** — dragOver writes rAF-coalesced to ≤ 1/frame, structural sharing (only changed columns re-spread; unchanged cards keep object identity; no order change → no write), `columns ?? EMPTY_COLUMNS` hoisted, counts memoized | `frontend/src/features/cards/drag-drop-context.tsx`, `frontend/src/features/cards/drag-over-cache.ts`, `frontend/src/features/boards/board-view/board-view.tsx` | T021 |
| P5 | MEDIUM | Lists use `LIMIT/OFFSET` with **no page ceiling**; notes/projects search is `LIKE '%q%'` (leading wildcard — `IDX_notes_title` can never serve it; no FULLTEXT anywhere) | `backend-rs/src/extract.rs:115-135`, `backend-rs/src/features/notes/store.rs:354-357`, grep `FULLTEXT` → 0 | T022 (ceilings), T023 (FTS) |
| P6 | LOW-MED | Board/archived/notes lists fetch with backend defaults (limit 50) and have **no pagination UI** → items beyond page 1 invisible | `frontend/src/features/boards/use-boards.ts:38-46`, `board-list.tsx` (no page state) | T022 |
| P7 | LOW | Metrics gaps: `acquire_timed` never called in production (acquire-duration histogram always empty); `kanpad_db_up` only refreshed by `/ready` hits; global 30 s timeout + 10-conn pool means ~10 concurrent slow queries starve everyone (an 11th waits on pool checkout up to the 30 s timeout, then 503) | `backend-rs/src/state.rs:177-184` (only caller is a test), `backend-rs/src/metrics.rs:104-161`, `backend-rs/src/lib.rs:72` | T024 (metrics); timeout/pool interplay = capacity input (T026) |

### Scalability

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| S1 | **HIGH** (strategic) | No collaboration model — single-user ownership on every table; no workspace/team/membership/sharing/permission concept; admin = single CLI-minted superadmin, no admin panel, no activity log; last-write-wins concurrency | grep `team\|share\|member\|permission` in `backend-rs/src` → nothing; README feature list unchecked | T027 (design review); implementation tasks authored 2026-11-16 at user request — **T031–T042** (rows A–L; T028/T029/T030 remain the storybook/SQL-observability/Sentry tasks), see `30-collaboration.md` §6 and the amendment log |
| S2 | MEDIUM | Rate limiting is per-process in-memory (documented single-replica assumption) — horizontal scale-out multiplies every limit by N; sessions already in MySQL so the app is replica-ready, only the governor is not | `backend-rs/src/lib.rs:356-358` (own comment), no redis dep in `Cargo.toml` | deferred (readiness note in `20-performance.md`) |
| S3 | MEDIUM | Single-box, single-process with no documented capacity model — no user-count/concurrency assumptions anywhere in specs; pool=10, 120 req/min/IP defaults untested against real org load | `backend-rs/.env.example`, spec greps | T026 |
| S4 | LOW | No queue/worker infra beyond the 60 s session sweeper (fine today; blocks async features like export/email later); nested children (board columns) unpaginated by contract | grep `tokio::spawn` → 2 hits in `main.rs`; `backend-rs/src/common/pagination.rs:4` | out of scope (noted for collaboration follow-ups) |
| S5 | **HIGH** (strategic) | No comments/mentions — **verified missing**: zero tables, endpoints or components; PRD lists them as "Vision (future)" but Wave 2 requires cross-functional discussion on the work | grep `comment\|mention\|notification` in `backend-rs/src`, `frontend/src` → 0 (2026-11-16 re-check) | T027 (review); implementation rows K/L (`32`/`33`) landed as **T041/T042** (2026-11-16, ahead of the review at user request) |
| S6 | HIGH | No realtime invalidation — collaborator edits are invisible to other sessions until `staleTime` (5 min) or manual refresh; no `EventSource`/WebSocket anywhere; board multi-user editing is not viable without it (D-7) | grep `EventSource\|text/event-stream\|new WebSocket` in `frontend/src` → 0; no SSE route in `backend-rs/src` | T027 (review); rows J/L (`31`) landed as **T040/T042** (2026-11-16, ahead of the review at user request) |

### Cost / maintenance

| ID | Sev | Finding | Evidence | Closed by |
|----|-----|---------|----------|-----------|
| C1 | MEDIUM | Manual deploys: build on a machine → upload; 3+ divergent proxy script copies (`scripts/app-proxy.js`, `release/app-proxy{,-v2,-plus}.js`); no deploy script; README documents the deleted Nest stack | `release/`, `scripts/`, `README.md:47-131` | T019 |
| C2 | MEDIUM | No bundle-size gate in CI; `chunkSizeWarningLimit` raised to 800; build-only packages (`shadcn`, `tailwindcss`, `@tailwindcss/vite`, `tw-animate-css`) in prod `dependencies`; undeclared `@radix-ui/react-tabs`/`-tooltip` imports (work via hoisting only). **2026-11-17 (T016): CI half closed** — `frontend/scripts/check-bundle-budget.mjs` wired into the `web` job after `vite build` (initial gzip ≤ 310,000 B; per-chunk ≤ 225,000 B; red-proofed — second amendment re-set at 257,986 B measured); `chunkSizeWarningLimit` 800 → 700. Deps half stays with T018 | `frontend/vite.config.ts`, `frontend/package.json`, `frontend/src/components/ui/{tabs,tooltip}.tsx` | T016 (gate, done), T018 (deps) |
| C3 | LOW | Secrets at mode 644 (spec wants 600); `release/.env` shares the live `SESSION_SECRET` with `backend-rs/.env`; dev compose publishes MySQL on 0.0.0.0:3306; prod DB version documented inconsistently (MariaDB 10.6 vs 11.4) | `ls -l`, `compose.yaml:14`, `docs/mariadb-compat.md:3` vs `specs/security/06-infrastructure-deployment.md:77` | T018 (secrets), T012 (bind), T011/T025 (version check) |
| C4 | LOW / POSITIVE | Production footprint already minimal (static SPA + ~17 MB binary + existing DB on shared LiteSpeed host) — the low-cost baseline to preserve; CI heavy but $0 (public repo) | `docs/deployment.md`, `backend-rs/Dockerfile` | preserve (constraint in every ops task) |

## Verified strengths (must not regress)

- Backend: `/health` vs `/ready` split with budget-bounded direct dial; graceful shutdown
  with bounded pool close; RFC 9457 problem+json with no driver-text leakage; Argon2id
  with dummy-verify timing equalization; Origin/Sec-Fetch-Site CSRF; per-IP throttling
  with `TRUSTED_PROXIES` awareness; digest-pinned non-root Docker image; thin-LTO stripped
  release binary; 90% line coverage with an 85% floor; MySQL + MariaDB CI matrix.
- No N+1 anywhere sampled — batched `IN (...)` enrichment, single-join aggregates,
  set-based position updates via fractional `sort_key`s (T010).
- Frontend: single `apiFetch` layer (zero stray `fetch()`), generated OpenAPI types with
  CI drift gate, React Compiler enabled, dnd-kit sensor tuning, optimistic updates with
  rollback, ~720 unit tests.

## Wave overview (informational — status lives in taskpad `status.yaml`)

| Wave | Phase | Tasks | Theme |
|------|-------|-------|-------|
| 0 | 0 | T011–T019 | Reliability floor (R1–R3, R5–R7, P1–P2, C1–C3) |
| 1 | 1 | T010*, T020–T026 | Performance & resilience at 100+ users (R4, P3–P7, S3) |
| 2 | 2 | T027 (review gate) + T031–T042 (rows A–L, authored 2026-11-16 ahead of the review at user request) | Collaboration design: workspaces/authz (`30`), SSE realtime (`31`), comments & mentions (`32`), in-app notifications (`33`) (S1, S5, S6) |

\* T010 was a pre-existing pending task gating T020 (D-5); **T010 is done** — T020's gate is satisfied.

Sequencing/serialization rules encoded as taskpad `depends` edges so parallel agents never
collide: CI workflow edits T016→T017; auth layer T013→T014; board view T021→T022;
cards/columns write paths **T010→T020**.

## Amendment log

Dated amendments to this constitution (per README rule: spec/tree disagreements are
fixed in place and recorded here). Design details in `10`/`20`/`30`/`31`/`32`/`33`/`40` carry their
own amendment notes in-line where relevant.

- **2026-11-17 — P1/P2/C2 (T016):** renderer swapped from mermaid to pele 1.13.0
  (`pele/lazy`, exact pin); catch-all `vendor`/`app` codeSplitting groups removed;
  initial gzip re-measured at **257,986 B** (was ≈1.84 MB; second amendment —
  all app routes static, single vendor chunk); budget gate
  (310,000 B initial / 225,000 B per-chunk) wired into CI `web` job; the
  2026-11-13/2026-11-16 P2 measured figures above are superseded by the 2026-11-17
  re-paste. 300 KB absolute target retired — the gate is a regression guard.

- **2026-11-16 — Wave-2 implementation tasks authored ahead of the review
  (user decision):** the rows A–L implementation task files now exist and are wired
  in `status.yaml` with the normative `30-collaboration.md` §6 dependency edges:
  T031 (A, M1/M2 migrations + personal-workspace backfill, ← T027) · T032 (B, authz
  kernel) · T033 (C, boards/columns/cards enforcement) · T034 (D, projects/notes/
  tags/checklists/labels) · T035 (E, membership management API) · T036 (F, optimistic
  concurrency 409) · T037 (G, activity log + read APIs) · T038 (H, admin panel) ·
  T039 (I, frontend sharing UX) · T040 (J, SSE transport + presence) · T041 (K,
  comments + mentions + notifications backend) · T042 (L, frontend). The T027 review
  still gates implementation — **O-7…O-21 are recorded defaults, not ratified
  decisions**; any override must be applied to T031–T042 (and their ACs) before the
  affected task starts. T027's own review scope (walking O-7…O-21, §6 refinement)
  is unchanged; its task-authoring step is now partially pre-done (files exist,
  IDs assigned). Speculative ID reservation remains rejected. Companion syncs:
  `30-collaboration.md` §6 preamble now names T031–T042 in place (header amended
  with an in-line note); `specs/README.md` + `specs/mcp/*` floors updated
  (next free ≥ T043); S1/S5/S6 "Closed by" cells and the wave table name T031–T042;
  out-of-scope line records the pre-authoring; stale `D-1…D-6` references →
  `D-1…D-7`.
- **2026-11-16 — Wave-2 scope promotion (pass 3, D-7; no other decisions changed):**
  1. **D-7 added** — realtime is SSE + React Query cache invalidation (server→client;
     metadata-only frames; `activity_log.id` resume cursor; automatic polling fallback),
     replacing the D-2 "realtime deferred" clause **for scope purposes only** — D-2's text
     stands as historical record. Design: `31-realtime-sse.md` (new). Out-of-scope line
     updated in kind (WebSockets stay out).
  2. **Comments + @mentions + in-app notification inbox promoted to Wave 2 design scope**
     (user decision 2026-11-16; they were PRD "Vision" items). New findings **S5**
     (comments/mentions verified MISSING) and **S6** (no realtime invalidation). New files
     `32-comments-mentions.md` and `33-notifications.md`. Email/SMTP delivery stays out.
  3. `30-collaboration.md` hardened: `activity_log` gains `request_id`, `ip_hash` (SHA-256,
     salted), and `changes` (scalar before→after; field names only for free-text) per audit-log
     practice; DB-level append-only via `BEFORE UPDATE/DELETE … SIGNAL SQLSTATE '45000'`
     trigger + `SELECT, INSERT`-only app role; board/workspace scoped read APIs added;
     `activity_log.id` is the SSE resume cursor; task rows extended **A→L** (J = SSE transport +
     presence, K = comments/mentions/notifications backend, L = frontend); open calls split into
     resolved (O-1..O-6, user sign-off 2026-11-16) and sibling-owned (O-7..O-21, defaults recorded).
  4. `31`/`32`/`33` carry their own in-file amendment notes; `T027` constraint line amended to
     match D-7 (it previously said "realtime stays out of scope").
  5. **MCP/Wave 3 unaffected by the SSE promotion:** the `/mcp` tool surface still has no
     server-push channel; `mcp/*` "realtime deferred" statements remain accurate (SSE here is
     browser-facing invalidation only). MCP's `actor_key_id`/`via` requirement stays binding on
     row G; Wave 2 comments write those same attribution columns in `notifications` too (`33` §8).
  6. Notifications and presence rows are **not** audit events and never enter `activity_log`;
     notification fanout and comment writes share the mutation transaction (`32` §6, `33` §2).
- **2026-11-16 — spec-review amendments, pass 2 (no scope/decision change):**
  1. **R4 re-derived against the tree** (the pass-1 amendment had re-verified
     only card create/move): the retry envelope now covers six write sites,
     including `columns/routes.rs:348` (`move_cards`) and `:192` — both were
     wrongly listed as single-shot. `20-performance.md` §R4 current-state text
     rewritten to match; T020's remaining scope = the sites the FOR UPDATE
     re-enumeration finds single-shot (`delete_card` et al.), not `move_cards`.
  2. `20-performance.md` T026 pool-starvation wait corrected to **30 s** (was
     "5 s → 503"), matching this register's P7 wording and the global timeout.
  3. `30-collaboration.md` §5 `activity_log` gains `actor_key_id` + `via`
     (Wave 3 dependency — `specs/mcp/60-security.md` §4, `91-task-map.md` §4).
  4. `30-collaboration.md` §3: rollback of per-route authz enforcement is
     redeploy-of-previous-release; backup-restore stays last resort.
  5. `40-operations.md`: Wave 3 (`/mcp`) noted as riding the same origin/container;
     §3 metrics note for `kanpad_mcp_*`.
  6. `10-reliability.md` typo ("Existing49").
- **2026-11-16 — post-review amendments (no scope/decision change):**
  1. DoD "both engines" now names the CI matrix legs (MySQL 8.4 + MariaDB 11.4); prod
     flavor unverified — T011 drill records `SELECT VERSION()`; `40-operations.md` §6
     owns correction + optional 10.6 CI leg. (Was: "MariaDB 10.6+".)
  2. Findings register re-verified against the tree post-T010: **R4** reworded (T010
     landed fractional `sort_key`s + scoped `1062` retry on card create/move; remaining
     single-shot sites = `delete_card`, bulk `move_cards`, ~18 other txs; gate T010 done).
     *(Superseded by pass 2 above — this claim was itself stale; see the pass-2 entry.)*
  3. **P2** re-measured (2026-11-13 dist): mermaid 4.9 MB raw / ~1.36 MB gzip; initial
     modulepreloaded JS ≈ 1.85 MB gzip. (Was: "3.1 MB / ~1.29 MB".) **P7** reworded: an
     11th concurrent slow query waits on pool checkout up to the 30 s timeout, then 503.
  4. Wave-2 collaboration implementation task IDs are **not pre-reserved** — T028/T029/T030
     already exist (storybook, SQL observability, Sentry/OTel). `30-collaboration.md` §6
     (now rows A–L, IDs by taskpad at authoring; collab set landed as T031–T042 on
     2026-11-16, see amendment log) and T027's task file were updated to match.
  5. In-line amendments recorded in the design files: **R2** (`10-…`) — `queryClient.clear()`
     semantics verified against installed `@tanstack/query-core` 5.107.1 (late query
     responses cannot repopulate; `MutationCache.clear()` does NOT cancel in-flight
     mutations); ordering pinned to `cancelQueries()` → `clear()` → set/null `me`.
     **R3/T012** (`10-…`) — compose topology pinned to same-origin `/api` proxy with
     `VITE_API_URL=""` default; prod stays two-origin (D-1). **R1** (`10-…`) —
     `docs/operations-backup.md` is the single owner of the procedure + drill record.
     **T022** (`20-…`) — card source pinned: per-column infinite scroll past N=50
     embedded, columns embed keeps structure/counts. **T020/R4** (`20-…`) — T010 gate
     marked satisfied. **Activity log** (`30-…`) — `metadata` pinned to native JSON
     (was the unresolved "JSON/BLOAT-free TEXT" phrasing). **T023** (`20-…`) — FULLTEXT
     verified on 11.4 legs + 10.6 caveat. **40-operations §3/§6** — backup doc single
     owner; CI MariaDB leg runs 11.4 — 10.6 correction path recorded.
  6. D-5 gate status: T010 done; T020 unblocked (decision text unchanged — historical).

- **2026-11-19 — P3/P4 (T021):** rendering half of P3 closed; P4 closed. Card lists
  virtualized with `@tanstack/react-virtual` (full item list to `SortableContext`;
  vertical strategy; outer virtualizer cell + inner sortable node layer separation;
  custom `rangeExtractor` pins the dragged/editing row; `CardDetailPanel` mount-gated
  on first open). dragOver cache writes rAF-coalesced to ≤ 1/frame, with structural
  sharing (no order change → no write; only changed columns re-spread; unchanged
  cards keep object identity). Counts memoized; `columns ?? EMPTY_COLUMNS` hoisted.
  **Stage A containment shipped inside T021** (user sign-off — the virtualizer's
  scroll element must be bounded; T044 keeps Stages B1–B4 and was amended inline).
  Measured: **13 DOM rows mounted of 500** in a real-browser 500-card board (both
  board and list views; `aria-rowcount=500` preserved), time-to-interactive ≈ 1.2 s;
  keyboard drag verified end-to-end on a 120-card column (stepwise, focus preserved
  on the pinned source row, persisted — e2e). Initial gzip 267,420 B (from 257,986 B;
  +9.4 KB from react-virtual) — budget gate green (310,000 B cap). Full-vitest + `tsc -b`
  clean; drag-drop unit + e2e suites green; `gen:api --check` no-op (no wire change);
  no POST/PATCH/DELETE endpoints touched (dragEnd's PATCH covered by existing suites).
  The "Closed by T021" register line for P3 is hereby split (rendering → T021 done;
  hydration → T022; visual → T044).
