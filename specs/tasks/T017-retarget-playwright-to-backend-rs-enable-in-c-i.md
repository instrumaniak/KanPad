# T017: Retarget Playwright to backend-rs + enable in CI

> **STATUS: done (2026-11-18), pending user review.** Wave 0. Closes **R7** (MEDIUM).
> **Depends on T016** (done — see `specs/tasks/status.yaml:T016`; serialized
> CI-workflow edits — never two agents in `backend-rs.yml` at once).
>
> **Amended 2026-11-18** (pre-implementation review, no implementation): the diff
> plan now includes the e2e spec fixtures (~120 hardcoded `:3000` refs), pin
> semantics are defined for a *two*-server setup, the auth-throttle trap and the
> CI job placement are decided, migration uses the backend-rs binary
> (`cargo run -- migrate run`, not sqlx-cli), and CI extends `docker-e2e`. See
> "Amendments" and "Notes" below.

## Goal

The frontend e2e suite is dead: `playwright.config.ts:23-27` boots
`cd ../backend && npm run start:dev` on `:3000` — `backend/` was deleted (`4051be68`)
and T51 moved the API to `backend-rs` on `:3001`. CI installs Chromium and prints a
pin comment; `playwright test` never runs.

1. **Retarget the config:** two webServers matching the **prod-like two-origin
   topology** — Vite (`npm run dev` with `VITE_API_URL=http://localhost:3001`) for the
   SPA, and the Rust API (`cargo run -- serve`) with test env; or pin already-running
   servers (`E2E_BASE_URL` / `E2E_SPA_URL`). One shared, env-driven API-origin
   constant replaces the hardcoded `http://localhost:3000` fixtures.
2. **CI:** real `playwright test` run in the existing `docker-e2e` job (it already
   builds the image, serves `:3001`, has a MySQL 8.4 service and installs Chromium):
   provision + migrate `kanpad_e2e`, run suite against the API on `:3001` + Vite SPA
   on `:5173`, upload trace/report on failure; delete the stale "pin" comment and
   fix the stale job name/workflow header.

## Depends On

T016 (CI workflow contention) — done.

## Phase

0

## Critical

No — R7 is MEDIUM, but it restores the e2e safety net that Wave 1 tasks (T021/T022)
will lean on.

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R7 (design: two-server topology,
  port rules, DB isolation, CI job).
- `specs/arch-hardening-specs/00-overview.md` — finding R7; global DoD.
- `frontend/playwright.config.ts` (the defect), `frontend/e2e/` (14 specs, ~65–70
  test declarations — exact count from the red-proof run, API-driven fixtures +
  console-error monitor), `frontend/e2e/test-utils.ts`,
  `.github/workflows/backend-rs.yml` (`docker-e2e` job: pin step; `web` job),
  `backend-rs/tests/README.md` (port rules — ephemeral binds; Playwright's fixed-URL
  webServer needs the documented clash rule).
- `docs/deployment.md` (two-origin prod topology the e2e should mirror).
- `docs/lessons/ci-quality.md` — target lesson file (see below).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/playwright.config.ts` | Two webServers (SPA `npm run dev` with `VITE_API_URL=http://localhost:3001`; API `cargo run -- serve` waiting on `url: /health`); `baseURL`; `E2E_BASE_URL` (API pin) + `E2E_SPA_URL` (SPA pin) support; API webServer `timeout` ≥ 120 s (cold `cargo run` builds exceed 30 s); SPA `reuseExistingServer: !CI`, API `reuseExistingServer: false`; keep CI `retries`/`workers` sane |
| `frontend/e2e/api-base.ts` (new) **or** exported from `test-utils.ts` | Shared `E2E_API` constant: `process.env.E2E_BASE_URL ?? 'http://localhost:3001'` |
| `frontend/e2e/*.spec.ts` (all 14) | Replace **all hardcoded `http://localhost:3000`** (~120 refs, incl. every `beforeAll` fixture and local `const API_BASE_URL`) with the shared constant. No assertion weakening; the monitoring fixture stays |
| `frontend/package.json` | `test:e2e` extended from `"playwright test"` to provision env + DB, or delegates to a one-command wrapper script (e.g. `frontend/scripts/e2e-local.sh`, POSIX sh; documented manual steps for non-POSIX shells): create `kanpad_e2e` if missing, `cargo run -- migrate run`, then run Playwright |
| `.github/workflows/backend-rs.yml` | `docker-e2e` job: create `kanpad_e2e`, migrate via one-off container (`docker run --rm --network host kanpad-rust:ci migrate run`), start e2e API container with `DATABASE_URL=...kanpad_e2e`, run Playwright (with `E2E_BASE_URL` preset ⇒ no API webServer in CI), upload trace/report on failure; **replace the pin step**; **update the workflow header comment (line ~16) and the `docker-e2e` job name** (both say "playwright pinned to :3001" and reference T41/T50) |
| `frontend/.env.example` / docs | One-command local e2e instructions (incl. prerequisites: MySQL, `kanpad_e2e`) |
| `docs/lessons/ci-quality.md` | Dated, tagged lesson ("a pinned-but-never-run step is not a gate") |

**Must NOT appear in the diff:** `backend-rs` app code; `helpers.rs` (Rust harness
untouched — Playwright is a separate client); spec-suite content changes beyond the
shared-constant retarget above; no new `uses:` in the workflow without a commit-SHA
pin + trailing tag comment (T62/T24 rule; `.github/dependabot.yml` covers
github-actions).

## Pin semantics (two webServers — defined, mirrors Rust helpers' shape)

`E2E_BASE_URL` mirrors `e2e_base_url()` in `backend-rs/tests/api/helpers.rs` for the
**API** axis only; the SPA gets its own pin:

| Set | Behavior |
|---|---|
| (neither set — default) | Playwright starts **both** webServers (Vite on `:5173`, API on `:3001`). One-command local e2e. |
| `E2E_BASE_URL=<api-origin>` | API webServer is **omitted**; fixtures + health checks target the pin; SPA still starts on `:5173` (or `E2E_SPA_URL` if also set). CI uses this — the `docker-e2e`-managed API container serves `:3001`. |
| `E2E_SPA_URL=<spa-origin>` | SPA webServer is **omitted**; `baseURL` = the pin. |

Default local run must work with **no env set** (start both). `E2E_BASE_URL` already
has one exporter today: the `docker-e2e` smoke step puts `E2E_BASE_URL=http://localhost:3001`
into `$GITHUB_ENV` — do not rely on it for the e2e step's container lifetime (the
smoke container is stopped there); the e2e phase manages its own container.

## Migration mechanism

Use the **backend-rs binary** (`cargo run -- migrate run` locally; `migrate run` in
the `kanpad-rust:ci` image in CI). Do **not** require sqlx-cli for the e2e path.
Schema: `CREATE DATABASE IF NOT EXISTS kanpad_e2e CHARACTER SET utf8mb4 COLLATE
utf8mb4_unicode_ci;` then `DATABASE_URL=...kanpad_e2e?charset=utf8mb4&collation=utf8mb4_unicode_ci
cargo run -- migrate run`. Local alternative for throwaway setups:
`cargo run -- serve --auto-migrate` (dev/compose-only, refuses production; `APP_ENV=test`
is fine). Session store is per-database (T65) → no cross-suite wipe issues; the Rust
suites' per-test `#[sqlx::test]` DBs live in the same server, undisturbed.

## Throttle trap (hard requirement)

`THROTTLE_AUTH_PER_MIN` defaults to **5/min per IP** (`src/config.rs`) and
`THROTTLE_GLOBAL_PER_MIN` to 120/min — every request from the suite comes from
127.0.0.1, so the default limits **will** 429 mid-run. The e2e env (CI + local script)
MUST set `THROTTLE_AUTH_PER_MIN=10000` and `THROTTLE_GLOBAL_PER_MIN=10000`.
**Do NOT use `0`** — `config.rs`/`.env.example` document `0` as “disables that
limiter”, but `api_router()`'s `build_layer()` coerces the burst with `.max(1)`,
so `0` yields a limiter of 1 req/min (verified empirically: 18 fixture-register
failures with `429` on a full-suite run). Rationale: the suite tests app behavior,
not the governor; `DB_STARTUP_CHECK` stays at its default `require`. The
`.max(1)`/`finish()` mismatch is a backend-rs bug to file separately (out of
T017's diff).

## CORS / CSRF notes (fixtures are non-browser clients)

- `CORS_ORIGINS` defaults to exactly `http://localhost:5173` — correct for the SPA
  dev server on 5173; no override needed. If the SPA ever moves port (e.g. `vite
  preview` on 4173), the API's `CORS_ORIGINS` **must** be extended or the suite dies
  cross-origin. Do not "fix" this by reusing the Vite `/api` proxy — that defeats the
  two-origin requirement.
- Playwright's `request` fixtures send no `Origin`/`Sec-Fetch-*` headers, so the CSRF
  layer treats them as non-cross-origin — fine as-is. Fixtures must **not** add an
  `Origin` header (the T56 fail-closed layer would 403 them).
- The browser path (baseURL `:5173`, absolute `VITE_API_URL`) is the T007-hardened
  cross-origin path — it must stay green and the monitoring fixture must stay quiet.

## vite dev vs vite preview

Use **`npm run dev` (vite dev)** for the SPA webServer. `VITE_API_URL` is read by dev
at server start (shell env wins over `.env*`); the dev server needs no build.
`vite preview` serves `dist/` with `VITE_API_URL` **baked at build time** and defaults
to port **4173** (≠ the API's default CORS origin) — both wrong for zero-config e2e.
The Vite dev proxy (`vite.config.ts`, `/api → localhost:3001`) becomes unused when
`BASE` is absolute; it is not part of the tested path.

## `reuseExistingServer` policy

- SPA (`npm run dev`): `reuseExistingServer: !CI` (a dev already on 5173 is fine —
  with the caveat documented in `.env.example` that only a dev server started with
  `VITE_API_URL=http://localhost:3001` exercises the two-origin path; a same-origin
  dev server silently degrades coverage).
- API (`cargo run -- serve`): **`reuseExistingServer: false`, always.** Reusing a
  stray server can't guarantee `kanpad_e2e` + test env. Port-clash rule: never run
  while something holds `:3001`; use `E2E_BASE_URL` to target an external server
  deliberately (CI does exactly this).
- Port rule unchanged: Playwright fixed ports `3001`/`5173` never collide with the
  Rust suites' ephemeral (`bind_blackbox`, port 0) binds.

## CI design (decided: extend `docker-e2e`, do not add a new job)

`docker-e2e` (`needs: [backend, web]`) already: builds `kanpad-rust:ci` with
`docker build-push-action`; smoke-runs it on `:3001` (`APP_ENV=test`, `SESSION_SECRET`,
`DATABASE_URL=...kanpad_ci`); exports `E2E_BASE_URL`; installs chromium
(`--only-shell`); runs `npm ci`. Adding a separate job with a Rust toolchain would
duplicate the build (or re-resolve) for no gain, and make `webServer.timeout` fragile.

Replace the pin step with:

1. Create `kanpad_e2e` on the job's MySQL service:
   `mysql ... -e "CREATE DATABASE IF NOT EXISTS kanpad_e2e CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"`
2. Migrate via a one-off container (same image, no toolchain needed):
   `docker run --rm --network host -e APP_ENV=test -e SESSION_SECRET=<test-secret> -e DATABASE_URL='mysql://root:<ci-root-pw>@127.0.0.1:3306/kanpad_e2e?charset=utf8mb4&collation=utf8mb4_unicode_ci' kanpad-rust:ci /usr/local/bin/backend-rs migrate run`
3. Start the e2e API container fresh (name e.g. `kanpad-rust-e2e`; host network;
   `PORT=3001`, `APP_ENV=test`, `SESSION_SECRET=…`, `DATABASE_URL=…kanpad_e2e…`,
   `THROTTLE_AUTH_PER_MIN=10000`, `THROTTLE_GLOBAL_PER_MIN=10000`); wait-loop `/health`.
4. `npx playwright install chromium --only-shell` (existing).
5. Run `npm run test:e2e` with `E2E_BASE_URL=http://localhost:3001` set ⇒ API
   webServer omitted, SPA webServer starts Vite with `VITE_API_URL=http://localhost:3001`.
6. `if: failure()` → upload `frontend/playwright-report/` and `frontend/test-results/`
   via `actions/upload-artifact` (**SHA-pinned + tag comment**, per T62).
7. Always stop/remove the e2e container (`if: always()`).
8. Delete the pin step; **rename the `docker-e2e` job** (currently "docker build +
   vitest + playwright pinned to :3001") and **fix the workflow header comment**
   ("vitest / playwright pinned to :3001"); keep `needs: [backend, web]`.

Local one-command path (`npm run test:e2e`) mirrors steps 2–5 with `cargo run --
serve` for the API instead of the image (API webServer runs when `E2E_BASE_URL` is
unset).

## Implementation Steps

1. **Red proof:** `npx playwright test` today fails at webServer startup (missing
   `../backend`) — paste the failure.
2. Shared `E2E_API` constant + retarget all hardcoded `localhost:3000` fixture refs
   in the 14 spec files (grep must come up empty; no assertion or fixture logic
   changes beyond the URL swap).
3. `playwright.config.ts`: two webServers + `E2E_BASE_URL`/`E2E_SPA_URL` pin support
   (skip the matching webServer when pinned; see table), `baseURL`, API webServer
   `timeout: 300000`, SPA `reuseExistingServer: !CI`.
4. DB: create `kanpad_e2e` (charset/collate pin) + `cargo run -- migrate run`; verify
   the binary's `migrate run` works (it is real, not the T03 stub).
5. E2E env: `THROTTLE_AUTH_PER_MIN=10000`, `THROTTLE_GLOBAL_PER_MIN=10000`; local + CI.
6. Run locally end-to-end: suite green (paste count — count via Playwright's reporter
   output, do not assert a hard 65); console-error fixture quiet.
7. CI: implement the `docker-e2e` steps above; serialize with any other agent's
   `backend-rs.yml` edit (depends edge); artifacts on failure; delete the pin
   comment; job rename + header comment.
8. Docs + lesson; flip task status through the usual mechanism (Global DoD).

## Constraints

- Prod-like topology: e2e must exercise the two-origin path (cross-origin
  `VITE_API_URL`), not a same-origin shortcut or the Vite dev proxy — that's what
  T007 hardened. Do not set a same-origin `BASE` even "temporarily".
- Port rule: never collide with the Rust API test suites' ephemeral binds or a running
  dev server; fixed ports `3001`/`5173`; document the pin escape hatch
  (`E2E_BASE_URL`/`E2E_SPA_URL`).
- Playwright suite content changes only via the **enumerated repair classes** below
  (see Lessons — the suite had never run, so bit-rot was pervasive; no assertion
  weakening, no dropped tests, monitoring fixture stays):
  1. URL retarget: shared `E2E_API` constant replacing every hardcoded
     `http://localhost:3000`, fixtures moved from the deprecated `/api` alias to
     the canonical `/api/v1`.
  2. Fixture contract drift: `tagIds` → `tag_ids` (snake_case DTO); archive/restore
     via `PATCH /boards/:id {is_archived:true|false}`; list archived via
     `?archived=true`; note DELETE expectation `200` → `204`; waitForResponse
     predicates → `/api/v1/notes`.
  3. Locator repairs against the CURRENT UI (app-shell rewritten against the living
     Notes sidebar — the Projects sidebar is gone; `.or()` CTA auto-waits for the
     lazy empty state; `h3` heading scoping for board cards — no strict-mode
     violations).
  4. Authed-route guard: drop the redundant UI-login block where `page.request`
     already authenticated the browser context (T043-era `/login` bounce).
  5. One two-origin regression probe added (monitoring fixture assertion that the
     browser hits `http://localhost:3001`); one board-view-toggle race made
     deterministic via `waitForResponse` on the view_mode PATCH.
- New workflow `uses:` steps pinned to commit SHA + tag comment (T62).
- No secret in the repo; `SESSION_SECRET` in CI is a test value like the smoke step's.
- Global DoD; no commit without approval.

## Implementation record (2026-11-18)

**Red proof:** `npx playwright test` on the untouched tree →
```
[WebServer] /bin/sh: 1: cd: can't cd to ../backend
Error: Process from config.webServer was not able to start. Exit code: 2
```

**Progression of the suite (fresh `kanpad_e2e` each milestone, `--reporter=list`):**
1. After retarget, first full run: **8 passed, 18 failed (all `429` on fixture register),
   44 did-not-run** — the auth bucket is 5/min per IP (`src/config.rs`), every request
   from 127.0.0.1. Fix: `THROTTLE_*_PER_MIN=10000`.
2. Second run (throttle fixed): **41 passed** — remainder was bit-rot (see repair
   classes). My own local `kanpad_e2e` was dropped/recreated and re-migrated between
   milestone runs to keep CI parity.
3. Targeted repair rounds (notes/projects/archived-boards/boards/drag-drop/app-shell).
4. CI-path rehearsal (manual API on :3001 like the container, `E2E_BASE_URL` set):
   **67 passed, 3 failed** — board-view-toggle reload race (view_mode PATCH in
   flight), note DELETE `200`→`204`, Undo test flow — all fixed.
5. Final default-path run: **70 passed (3.2m), exit 0, console-error fixture quiet.**
   CI-path rehearsal after fixes: same shape.

**Scope additions beyond the original plan (all required for the above, all honest):**
- `frontend/src/features/boards/archived-boards.tsx` — `is_archived` client filter
  (API's `archived=true` means "include archived"); without it the "Board2 should
  NOT appear" assertion fails by construction.
- `frontend/src/layouts/app-layout.tsx` — mobile Notes bottom-sheet title was stale
  "Projects" (dead Projects-sidebar vestige); now "Notes".
- Two stale comments fixed to match reality (`vite.config.ts` proxy, `client.ts` BASE).

**Deviations recorded:** CI runs `npm run test:e2e:raw` (plain `playwright test`) with
`E2E_BASE_URL` preset — the spec's step 5 named `npm run test:e2e`; calling the full
wrapper in CI would attempt a second DB provision against a job that manages its own
container. Wrapper remains the local entry point. The spec's `migrate run` example
passed no binary path; CI calls `/usr/local/bin/backend-rs migrate run` because
`docker run <img> <args>` *replaces* CMD (`serve`) rather than appending to it.

**Status:** see header — done, pending user review. Commit NOT made (per DoD).

## Acceptance Criteria

- [x] Red proof of current failure pasted (missing `../backend`). (see Implementation record)
- [x] `grep -rn 'localhost:3000' frontend/e2e` returns nothing (shared constant only). (120 refs → 0)
- [x] `npx playwright test` green locally against backend-rs — all specs, paste the
      Playwright reporter count; console-error fixture quiet. (**70 passed (3.2m),
      exit 0**; CI-path rehearsal identical; `✗` count 0; `Browser Console Errors:` absent)
- [x] CI runs the real suite (no pin comment; job name + workflow header comment
      updated); failure uploads `playwright-report/`/`test-results/`. (docker-e2e:
      create/migrate kanpad_e2e → start e2e API → `npm run test:e2e:raw` with
      `E2E_BASE_URL` → `if: failure()` upload via SHA-pinned upload-artifact v7.0.2)
- [x] Two webServers confirmed exercised: SPA `:5173`, API `:3001`, absolute
      `VITE_API_URL`. (Assertion pinned in `board-view-toggle.spec.ts`:
      `monitoring.networkRequests.some(r => r.url.startsWith('http://localhost:3001'))`)
- [x] `E2E_BASE_URL` pin documented and works (skips API webServer); `E2E_SPA_URL`
      documented; semantics table honored. (verified in both default and pinned
      runs; `.env.example` documents both)
- [x] `kanpad_e2e` provisioned + migrated in CI (`migrate run` via the built image —
      no sqlx-cli dependency); DB isolation documented; no interference with Rust
      `#[sqlx::test]` suites. (`cargo run -- migrate run` locally; image binary in CI;
      fresh drop+create+migrate per validation run)
- [x] `THROTTLE_AUTH_PER_MIN=10000` and `THROTTLE_GLOBAL_PER_MIN=10000` set for e2e
      (CI + local) — suite cannot 429. (`0` is NOT valid — see “Throttle trap”.)
- [x] Lesson appended (closes **R7**). (`docs/lessons/ci-quality.md`,
      `## Lesson (2026-11-18) — Playwright retarget: …`)

## Notes

### Amendments (2026-11-18)

- Expanded diff plan: the e2e spec fixtures were the missing piece — **120 refs to
  `http://localhost:3000` across all 14 spec files** (all `beforeAll` API fixtures +
  local `API_BASE_URL` constants in `card-search.spec.ts`/`board-view-toggle.spec.ts`).
  Fixed ports + shared constant now explicit.
- `E2E_BASE_URL` semantics defined for a two-server setup (`E2E_BASE_URL` pins API,
  `E2E_SPA_URL` pins SPA; neither ⇒ start both). The old "E2E_BASE_URL respected if
  set, else start both" was ambiguous with two webServers and with the smoke step's
  existing `$GITHUB_ENV` export.
- Migration via the backend-rs binary (`cargo run -- migrate run` / image `migrate run`),
  not sqlx-cli. (`--auto-migrate` exists on `serve` for local throwaways.)
- Auth/global throttles set effectively-unlimited (10000/min) for the e2e env —
  default limits 429 at 5/min per IP from 127.0.0.1. Amended during implementation:
  `0` does NOT disable the limiter (`build_layer()` coerces burst to `.max(1)`;
  governor's `finish()` returns `None` only on `burst_size == 0`) — the first
  full-suite run went 18 failed / 8 passed / 44 did-not-run with `429` on every
  fixture register. Backend-rs bug to file separately (out of T017's diff).
- CI placement decided: extend `docker-e2e` (already builds the image, smoke-runs
  `:3001`, exports `E2E_BASE_URL`, installs Chromium). No new job; no second toolchain.
- `.env.test` is gitignored/local-only (points at `DB_PORT=3337`) — e2e env must be
  passed explicitly, never sourced from a committed file.
- Current test count: ~65–70 declarations (`grep` 65× `test(` + 5× `monitoringTest(`);
  hard "65" dropped from ACs.
- Status bookkeeping: `specs/tasks/status.yaml` already says `in_progress`; this
  file's header said `pending` — header now aligned as pending-until-PR, flip through
  the usual mechanism at completion.
- Workflow's pin comment references T41/T50 (stale task IDs) — deletion is in scope;
  job name + header comment updated accordingly.
- CSRF for fixtures: Playwright `request` sends no `Origin`/`Sec-Fetch-*` → CSRF layer
  treats as same-origin ⇒ no header changes needed; do not add `Origin`.
- `vite preview` rejected (build-time bake; port 4173 ≠ default `CORS_ORIGINS`); use
  `npm run dev`.

## Lessons

Full write-up in `docs/lessons/ci-quality.md` — `## Lesson (2026-11-18) —
Playwright retarget: a pinned-but-never-run step is not a gate`. Headline items:

- A pinned-but-never-run step is not a gate — only a green run against the new
  topology counts (R7 closed by evidence, not by YAML).
- Fixtures are non-browser clients (no `Origin`/`Sec-Fetch-*` ⇒ CSRF same-origin);
  keep them header-free. Browser path stays cross-origin via absolute `VITE_API_URL`.
- `THROTTLE_*_PER_MIN=0` does NOT disable the governor (`build_layer` coerces burst
  to `.max(1)`; governor's `finish()` returns `None` only on `burst_size == 0`) —
  0 means 1 req/min. Set 10000. (backend-rs bug to file separately.)
- Reviving a never-run suite is an inventory exercise: fixture URLs (`/api/notes` →
  `/api/v1`), fixture payloads (`tagIds` → `tag_ids`), deleted verb routes
  (`PATCH /boards/:id/archive` → `PATCH /boards/:id {is_archived}`), dead locators
  (Projects sidebar gone — rewrite against Notes sidebar), `/login` bouncing authed
  users (T043), `page.request` sharing the page cookie jar.
- The first red run IS the inventory; classify every failure, fix in one pass,
  re-run fully.
