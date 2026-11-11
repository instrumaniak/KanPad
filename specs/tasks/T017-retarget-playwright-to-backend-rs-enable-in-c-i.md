# T017: Retarget Playwright to backend-rs + enable in CI

> **STATUS: pending.** Wave 0. Closes **R7** (MEDIUM). **Depends on T016** (serialized
> CI-workflow edits — never two agents in `backend-rs.yml` at once).

## Goal

The frontend e2e suite is dead: `playwright.config.ts:23-27` boots
`cd ../backend && npm run start:dev` on `:3000` — `backend/` was deleted (`4051be68`)
and T51 moved the API to `backend-rs` on `:3001`. CI installs Chromium and prints a
pin comment; `playwright test` never runs.

1. **Retarget the config:** two webServers matching the **prod-like two-origin
   topology** — Vite (`npm run dev` or `vite preview`) for the SPA with `VITE_API_URL`
   pointing at the API, and the Rust API (`cargo run -- serve`) with test env; or reuse
   a running dev server (`reuseExistingServer`). Support `E2E_BASE_URL` pinning (mirror
   `backend-rs` helpers' `e2e_base_url()` semantics).
2. **CI:** real `playwright test` job (or `web`-job stage): install browsers, start
   servers (separate DB/schema `kanpad_e2e`), run suite, upload trace on failure; delete
   the stale "pin" comment.

## Depends On

T016 (CI workflow contention).

## Phase

0

## Critical

No — R7 is MEDIUM, but it restores the e2e safety net that Wave 1 tasks (T021/T022)
will lean on.

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R7 (design: two-server topology,
  port rules, DB isolation, CI job).
- `specs/arch-hardening-specs/00-overview.md` — finding R7; global DoD.
- `frontend/playwright.config.ts` (the defect), `frontend/e2e/` (14 specs, 65 tests,
  API-driven fixtures + console-error monitor), `frontend/e2e/test-utils.ts`,
  `.github/workflows/backend-rs.yml` (playwright step + `web` job),
  `backend-rs/tests/README.md` (port rules — ephemeral binds; Playwright's fixed-URL
  webServer needs the documented clash rule).
- `docs/deployment.md` (two-origin prod topology the e2e should mirror).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/playwright.config.ts` | webServer(s): SPA + API; `baseURL`; `E2E_BASE_URL` pin support; keep CI `retries`/`workers` sane |
| `frontend/package.json` | `test:e2e` script that provisions env (DB name, SESSION_SECRET, VITE_API_URL) |
| `.github/workflows/backend-rs.yml` | Replace pin with a real run; DB provisioning for `kanpad_e2e`; artifact upload on failure |
| `frontend/.env.example` / docs | One-command local e2e instructions |
| `docs/lessons/frontend.md` (or `ci-quality.md`) | Dated, tagged lesson ("a pinned-but-never-run step is not a gate") |

**Must NOT appear in the diff:** `backend-rs` app code; `helpers.rs` (Rust harness
untouched — Playwright is a separate client); spec-suite content changes beyond what
env retargeting requires.

## Implementation Steps

1. **Red proof:** `npx playwright test` today fails at webServer startup (missing
   `../backend`) — paste the failure.
2. Config: two webServers (API `cargo run -- serve` with `.env.test`-style env +
   `url: /health`; SPA vite with `VITE_API_URL` → API). Decide fixed ports (document
   clash rule: never while a dev server holds them) vs `E2E_BASE_URL` pin; default:
   `E2E_BASE_URL` respected if set, else start both.
3. DB: provision `kanpad_e2e` schema + `sqlx migrate run` (per-database sessions = no
   cross-suite wipe issues, T65).
4. Run locally end-to-end: all 65 tests green (paste); console-error fixture quiet.
5. CI job: same steps; `needs` ordering with existing jobs; artifacts on failure;
   delete the pin comment. **Serialize with T016's CI edit (depends edge).**
6. Docs + lesson.

## Constraints

- Prod-like topology: e2e must exercise the two-origin path (cross-origin
  `VITE_API_URL`), not a same-origin shortcut — that's what T007 hardened.
- Port rule: never collide with the Rust API test suites' ephemeral binds or a running
  dev server; document the chosen mechanism.
- Playwright suite content changes only where env retargeting requires (no assertion
  weakening; the monitoring fixture stays).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Red proof of current failure pasted (missing `../backend`).
- [ ] `npx playwright test` green locally against backend-rs (all specs; paste count).
- [ ] CI runs the real suite (no pin comment); failure uploads trace/report.
- [ ] `E2E_BASE_URL` pin documented and works (mirrors Rust helpers' semantics).
- [ ] DB isolation documented (`kanpad_e2e`); no interference with Rust api suites.
- [ ] Lesson appended (closes **R7**).

## Notes

(filled in during/after implementation)

## Lessons

(pending)
