# 10-reliability — Wave 0 design (findings R1–R8)

> Per-finding design: current state → target design → edge cases → test plan →
> out-of-scope. Executed by tasks T011–T019 (`specs/tasks/`). R4 (transaction retry)
> is Wave 1 and designed in `20-performance.md`. Global DoD: `00-overview.md`.

## R1 — Automated DB backups → T011

**Current state:** only manual `mysqldump` command lines inside
`docs/data-migration-live.md` §1/§9. `docs/cutover.md:18-19` makes restore the *only*
documented DB rollback path ("never `migrate revert` in production") yet no schedule,
no retention, no offsite copy, no drill record. Spec
`specs/security/09-logging-monitoring-ir.md:41` demands "Automated DB backups;
encrypted at rest; tested restore (quarterly)" — listed as test plan, unverified.

**Target design:**
- One script `scripts/backup-db.sh` (POSIX sh/bash) that: runs
  `mysqldump --single-transaction --routines --events --triggers --source-data=2`
  against `$DB_HOST/$DB_NAME` → gzip → timestamped file
  `kanpad-YYYYMMDD-HHMMSS.sql.gz` in `$BACKUP_DIR` (default `/var/backups/kanpad`).
- Retention pruning in the same script: keep last 7 daily + 4 weekly (weekly = snapshot
  taken on a fixed weekday, kept 28 days). Prune must never delete the newest file.
- Offsite hook: optional `$BACKUP_RSYNC_TARGET` or `$BACKUP_Rclone_REMOTE` — if set,
  copy after a successful dump; if unset, skip silently (local-only deployments stay
  zero-config). Never fail the backup because offsite failed — log loudly instead.
- Scheduling documented for **both deploy paths** (D-1): systemd timer unit example
  (`kanpad-backup.timer`, daily 03:17) for the VPS/appliance path, cPanel cron line for
  the shared path. The script itself is identical.
- Integrity: verify each dump with `gzip -t` + a `mysql -e "SELECT 1"` pipe-back smoke
  (import into a throwaway DB is the full drill, below).
- `docs/operations-backup.md` (or a section in `40-operations.md`): procedure +
  the **restore drill template** and a filled-in drill record.

**Restore drill (the R1 acceptance heart):** on a scratch DB: gunzip → pipe to `mysql`
→ assert table count + row counts vs source (`information_schema.tables`/`COUNT(*)`
parity for the 12 app tables) → run `cargo run -- migrate info` → `applied = all,
pending = 0` → one manual login smoke. Record wall-clock time + date in the drill log.

**Edge cases:**
- Prod is MariaDB (flavor ambiguity 10.6 vs 11.4 — see `40-operations.md`): the script
  must work with either client; `mysqldump` from a *newer* client than the server fails
  on some flags — use `mariadb-dump` when available, fall back to `mysqldump`, and pin
  flags verified against both.
- `_sqlx_migrations` must survive round-trip (it does with a plain dump — assert it in
  the drill; never hand-edit it).
- Sessions table: dump includes it; harmless (fresh `SESSION_SECRET` invalidates anyway).
- First-run: `$BACKUP_DIR` creation, permissions 700, script must not echo the password
  (use `MYSQL_PWD` env or a defaults file, never `-p$PASSWORD` on argv).

**Test plan:** shellcheck-clean script; CI-optional dry run (`--dry-run` prints the
commands); drill record committed as evidence. **Out of scope:** encryption at rest
(host-level, documented as a host responsibility), point-in-time recovery/WAL (MySQL
binlog — overkill at this scale).

## R2 — Session cache hygiene → T013

**Current state:** `auth-provider/index.tsx` logout `onSuccess` only does
`setQueryData(['auth','me'], null)`; login only sets `['auth','me']`. `queryClient.clear()`
/ `removeQueries()` appear only in tests. Query keys (`['columns', id]`, `['boards']`,
`['notes']`…) are not user-scoped → on a shared tab, B logs in within `staleTime`
(5 min) and sees A's boards/notes/columns until refetch.

**Target design:**
- Single helper `resetSessionCaches(queryClient)` called on **logout success and login
  success** (and register): `queryClient.clear()` — drops every query + mutation
  observer state. Simplest correct fix; the alternative (user-scoped keys
  `['user', userId, ...]`) is rejected as a wide refactor with the same outcome.
- Ordering: clear **before** setting `['auth','me']` for login (so the new user's `me`
  write isn't wiped), clear **after** nulling `me` for logout (or clear then set — pick
  one order and pin it in the test).
- In-flight requests from user A must not repopulate the cache after clear: React Query
  `clear()` cancels/ignores… verify — if a late response can still `setQueryData`
  through an active observer, the fix is to `queryClient.cancelQueries()` first, or
  key the `me` query per-session. **The red-proof test must simulate a late response.**

**Edge cases:** double-logout; logout while a mutation is in flight (its `onSettled`
invalidate must not resurrect data); login → immediate navigation (no flash of A's
skeletons); `staleTime` still 5 min for the *new* user (fine — cache is empty).

**Test plan:** red-first unit test (vitest): seed cache with fake board/columns data →
logout → assert `queryClient.getQueryData(['columns', …])` undefined → login as B →
assert still empty; plus a "late in-flight response after clear does not repopulate"
test. E2E (after T017): A logout → B login → board list shows only B's boards.

## R3 — compose repair → T012

**Current state:** `compose.yaml` declares `nest: build: context: ./backend` (deleted
in `4051be68`) and `frontend: build: context: ./frontend` with **no**
`frontend/Dockerfile`; `frontend depends_on: nest`. `docker compose up` fails.
MySQL published `3306:3306` on all interfaces (C3).

**Target design:**
- Remove the `nest` service entirely; `frontend` loses `depends_on: nest`.
- New `frontend/Dockerfile`: multi-stage — `node:22-bookworm` builder (`npm ci`,
  `npm run build` with `VITE_API_URL` as build ARG defaulting to the compose-internal
  API URL `http://localhost:3001`… see caveat) → static runtime
  (`nginx:alpine` or `caddy:alpine`) serving `dist/` with SPA fallback to `index.html`,
  `Cache-Control: immutable` for `/assets/*` (hashed) and `no-cache` for `index.html`,
  plus a `/api` reverse-proxy to the `rust` service **for the compose topology only**.
- Compose wiring: `frontend` depends_on `rust` (service_healthy), ports `5173:80`
  (or keep 5173→8080), `rust` gets `CORS_ORIGINS` for the SPA origin, `TRUSTED_PROXIES`
  for the frontend/nginx hop so throttling keys on real client IPs.
- MySQL: bind `127.0.0.1:3306:3306` (C3) — compose-internal traffic doesn't need the
  published port at all; the rust service reaches mysql via the compose network. Publish
  only if a host-side client needs it, and then loopback-only.
- Add `restart: unless-stopped` + healthchecks to `frontend`; `logging` defaults fine.
- `.env.example` for compose-level vars (`DB_PASSWORD`, `APP_ENV`, etc.) — keep dev
  defaults but never document them as production values.

**Edge cases / caveat (decide at implementation):** the SPA in compose can be served
same-origin (`/api` proxied by the static server) → `VITE_API_URL=""` (relative), which
is *different* from the production two-origin topology (D-1: prod stays cross-origin
with baked `VITE_API_URL`). Document this split explicitly: **compose = same-origin dev
appliance; production = two origins** (`docs/deployment.md` unchanged). If we keep the
static server proxying `/api`, CSRF/CORS behavior in compose differs from prod — tests
must not assume either topology; e2e keeps using the direct API base.
- The `rust` service env must gain `TRUSTED_PROXIES` only if we proxy through the
  frontend container; if e2e hits rust directly on 3001, leave it empty.

**Test plan:** `docker compose up --build` from a clean clone works (manual AC, paste
output); CI optional job later (not required now — keep CI cost flat, C4). Regression:
existing suites unaffected (no app-code change).

## R5 — Global 401 handling + retry/timeouts → T014

**Current state:** `error-messages.ts` implements status routing incl. 401 →
"Session Expired" + redirect but is called only from its own test. Queries run with
`retry: false` (single blip = user-visible failure). No `AbortController` anywhere; the
`timeout` error branch in `client.ts` is unreachable. Raw `err.message` shown in many
call sites ("Failed to fetch").

**Target design:**
- `apiFetch` gains a default **timeout via `AbortController`** (e.g. 15 s; configurable
  const) → maps to a typed `ApiError('timeout')`.
- QueryClient defaults: `retry: (failureCount, error) =>` retry **once** for network/
  5xx errors on **idempotent GET queries only** (mutations keep `retry: false`;
  4xx never retried); exponential-ish delay (e.g. 1 s). Keep `staleTime: 5 min`.
- **Global 401 handler**: one place — either a `QueryCache`/`MutationCache` `onError`
  in `App.tsx` or an `onUnauthenticated` callback wired in `apiFetch` — that, for any
  401 *except* the bootstrap `['auth','me']` query (which already redirects via
  `AppLayout`), does: `queryClient.clear()` (reuse T013 helper) → redirect to `/login`
  (with `next` param optional — keep simple: plain `/login`) → toast "Session expired".
  Must be idempotent (multiple 401s in flight → one redirect, one toast).
- Route all user-facing error text through `getErrorMessage` (un-dead it) or delete it
  and centralize in one module — **pick one owner** (AGENTS.md: single owner per
  transport).

**Edge cases:** 401 during login POST itself (no redirect loop — login page stays);
401 from a background refetch while user is typing (redirect is correct but don't clear
the *form* state mid-navigation); timeout error must not be retried as if server error
(network retry policy covers it once); offline (`navigator.onLine`) — out of scope, but
timeout error message should read "Network timeout — check your connection".

**Test plan:** red-first: mock `apiFetch` 401 on a non-auth query → assert clear +
redirect + single toast; timeout: fake timers + abort → typed error; retry policy: 500
retries once, 400 doesn't, mutation doesn't. Existing49 auth tests stay green.

## R6 — Board error states + boundary reset → T015

**Current state:** `board-view.tsx:167-181` checks only `isLoading`/`!board` — `isError`
never consulted → failed columns fetch renders an *empty board* (indistinguishable from
a genuinely empty board). `RouteErrorBoundary` wraps all routes but App passes no
`resetKeys` → after an error the fallback sticks across navigation until "Try Again".

**Target design:**
- `board-view.tsx`: add `isError`/`error` branches for **both** `board` and `columns`
  queries → reuse the existing `ErrorState` component pattern from `board-list.tsx`
  (convention exists; apply it). Retry button → `refetch()` both.
- Same pass: check the other pages that lack error states (notes list, project page)
  — only if cheap; the board view is the AC.
- `App.tsx`: pass `resetKeys={[location.pathname]}` (route location) to the boundary so
  navigation resets the error state; keep `MAX_RETRIES` behavior. Consider per-route
  boundaries only if the global one proves awkward — default is resetKeys (smallest
  change).

**Edge cases:** error *while* dragging (mutation fails → existing rollback covers it;
boundary must not swallow); a 401-driven redirect (T014) must not surface as a boundary
error; skeleton vs error flicker (only show error after `isError` is stable).

**Test plan:** red-first: mock columns query rejection → assert error UI + retry, NOT
empty board; boundary: render error → navigate → assert routes render again; existing
174 board tests stay green.

## R7 — Playwright retarget + CI enable → T017

**Current state:** `playwright.config.ts:23-27` `webServer` boots
`cd ../backend && npm run start:dev` on `:3000` — `backend/` no longer exists; T51
cutover made the API `backend-rs` on `:3001`. CI installs Chromium and prints a pin
comment; `playwright test` never runs (`.github/workflows/backend-rs.yml`).

**Target design:**
- `webServer`: build+start `backend-rs` (`cargo run -- serve` with test env, ephemeral
  port per house rule) **or** point at an already-running dev server; simplest robust
  option: `command: cargo run -- serve` with `DATABASE_URL`/`SESSION_SECRET` env from
  `.env.test`, `url: http://127.0.0.1:3001/health`, `reuseExistingServer: !CI`.
  Port choice: the repo's e2e suites bind ephemeral ports (`wire::bind_blackbox`) —
  Playwright's webServer needs a fixed URL; use 3001 and document the clash rule (never
  run Playwright while a dev server holds 3001), or read `E2E_BASE_URL` if set (the
  helpers already have `e2e_base_url()` semantics — mirror that: `E2E_BASE_URL` pins,
  else start our own).
- `baseURL` → `http://127.0.0.1:3001` **only if the SPA is served same-origin** — it
  isn't in prod (two origins). Playwright must test the *prod-like* topology: serve
  `vite preview` (or `npm run dev`) for the SPA and set `VITE_API_URL` to the API —
  two webServers (Vite + API). This matches T41's `VITE_API_URL` env-switch design.
- CI: add a `playwright` job (or extend `web`): install deps + browsers, start both
  servers, run `playwright test`, upload trace/report on failure. Keep it serial-safe
  (port rule) and mark `continue-on-error: false` once green.
- Retire the "pin" comment; fix `fullyParallel`/`workers` (defaults fine on CI).

**Edge cases:** DB for e2e — CI already has the mysql service; reuse it with a
separate schema/database (`kanpad_e2e`) so unit/api suites don't interfere; session
store is per-database (T65) so no cross-suite wipe issues. Locally: document the
one-command e2e (`npm run test:e2e` boots everything).

**Test plan:** the AC *is* the green run: `playwright test` green in CI + locally;
console-error monitoring fixture (`e2e/test-utils.ts`) must stay quiet.

## R8 → T025 (monitoring) — designed in `40-operations.md` §Monitoring
## R4 → T020 (tx retry) — designed in `20-performance.md` §Retry helper
## R3's secrets sibling (C3) → T018 — designed in `40-operations.md` §Secrets
