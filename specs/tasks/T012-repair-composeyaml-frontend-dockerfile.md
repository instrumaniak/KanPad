# T012: Repair compose.yaml + frontend Dockerfile

> **STATUS: pending.** Wave 0 (reliability floor). Closes **R3** (HIGH) + compose half
> of **C3**.

## Goal

`docker compose up --build` works from a clean clone — the reproducible one-command
environment the repo currently lacks (and the load-testing substrate T026 needs).

1. **Remove the `nest` service** (builds `./backend`, deleted in `4051be68`) and the
   `frontend depends_on: nest` edge.
2. **Create `frontend/Dockerfile`** (referenced by compose but nonexistent): multi-stage
   `node:22` builder (`npm ci` + `npm run build`, `VITE_API_URL` as build ARG) → static
   runtime (nginx:alpine or caddy:alpine) with SPA fallback, `Cache-Control: immutable`
   for hashed `/assets/*`, `no-cache` for `index.html`, and a **compose-only `/api`
   reverse-proxy** to the `rust` service (same-origin dev appliance — see edge case).
3. **MySQL loopback-only:** bind `127.0.0.1:3306:3306` (compose-internal traffic uses
   the network; C3/F-01 history: MySQL was once internet-reachable).
4. **Hygiene:** `restart: unless-stopped` + healthchecks on `frontend`; env wiring for
   `CORS_ORIGINS`/`TRUSTED_PROXIES` per the compose hop; dev-default banner
   (`changeme` is dev-only).

## Depends On

(None.)

## Phase

0

## Critical

Yes — as committed the stack cannot build (R3); blocks contributors, load testing
(T026) and the appliance deliverable (D-1).

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R3 (design + same-origin caveat).
- `specs/arch-hardening-specs/40-operations.md` §1 (topology table), §2 (appliance).
- `specs/arch-hardening-specs/00-overview.md` — findings R3, C3; D-1 (both paths);
  global DoD.
- `docs/deployment.md` (production two-origin topology — must NOT be changed: compose
  is same-origin dev, prod stays cross-origin).
- `frontend/vite.config.ts` (dev proxy target), `frontend/public/.htaccess` (SPA
  rewrite reference for the nginx config).
- `backend-rs/.env.example` (`TRUSTED_PROXIES`, `CORS_ORIGINS` semantics).

## Files to Create/Modify

| File | Change |
|---|---|
| `compose.yaml` | Remove `nest` + `depends_on: nest`; MySQL `127.0.0.1:` bind; frontend service rebuilt (context + args + healthcheck + restart); rust env additions for the compose hop |
| `frontend/Dockerfile` | **NEW** — multi-stage build → static server with SPA fallback, asset caching, `/api` proxy |
| `frontend/.dockerignore` | **NEW** — `node_modules`, `dist`, `.env*` |
| `frontend/nginx.conf` (or Caddyfile) | **NEW** (if nginx) — SPA rewrite, cache rules, `/api` proxy_pass to `rust:3001` |
| `.env.example` (root, if needed) | Compose-level vars with dev defaults + warning banner |
| `docs/` (README pointer) | One-line "docker compose up for the local appliance" — full README rewrite is T019 |
| `docs/lessons/ci-quality.md` | Dated, tagged lesson (compose drift: CI path filters omit `compose.yaml`) |

**Must NOT appear in the diff:** production `docs/deployment.md` topology changes,
`.env` values, app code.

## Implementation Steps

1. **Reproduce (evidence):** `docker compose config` / `docker compose up --build` fails
   today on `./backend` — paste the failure as the red proof.
2. Delete `nest` service; drop `frontend → nest` dependency; add `frontend → rust`
   (service_healthy).
3. Author `frontend/Dockerfile`: ARG `VITE_API_URL` (default `""` → same-origin via the
   container proxy), `npm ci && npm run build`, copy `dist/` into static runtime.
4. Static server config: SPA fallback to `index.html`, `/assets/*` immutable,
   `index.html` no-cache, `/api/*` → `http://rust:3001` (preserve `Host`? — for the
   compose hop set `TRUSTED_PROXIES` on rust to the frontend container network so
   throttling keys correctly; verify `X-Forwarded-For` append semantics per T007
   contract).
5. MySQL: `"127.0.0.1:3306:3306"`.
6. Healthchecks: frontend (HTTP GET on its port), keep mysql + rust as-is; restart
   policies consistent.
7. Verify: `docker compose up --build` from clean clone → SPA loads, login works
   against compose API, `docker compose down -v` clean; paste output.
8. Note the CI gap (workflow path filters don't include `compose.yaml` — lesson; adding
   a CI job is out of scope, keep CI cost flat).

## Constraints

- **Prod topology untouched:** production remains two-origin with baked
  `VITE_API_URL` (`docs/deployment.md`); the compose `/api` proxy is compose-only and
  must be commented as such.
- Dev defaults stay dev-only; banner required; never document `changeme` for prod.
- No new services beyond what compose already has (cost guardrail).
- MySQL must not be reachable off-loopback.
- Global DoD gates; no commit without approval.

## Acceptance Criteria

- [ ] `grep -n "nest" compose.yaml` empty; `frontend` no longer depends on `nest`
      (closes **R3**).
- [ ] `docker compose up --build` from a clean clone builds all services and serves a
      working SPA (paste evidence: build tail + login smoke).
- [ ] `frontend/Dockerfile` exists, multi-stage, no secrets/ARGs with real values;
      `.dockerignore` present.
- [ ] Hashed assets served with `Cache-Control: immutable`; `index.html` `no-cache`
      (assert with `curl -sSI`).
- [ ] MySQL published on `127.0.0.1` only (closes compose half of **C3**).
- [ ] Healthchecks + `restart: unless-stopped` on all services.
- [ ] `docs/deployment.md` unchanged; lesson appended.

## Notes

(filled in during/after implementation)

**Open call (decide at implementation):** static runtime = nginx vs caddy. *Default:
nginx:alpine* (ubiquitous, config reviewable, no auto-TLS claims in compose).

## Lessons

(pending)
