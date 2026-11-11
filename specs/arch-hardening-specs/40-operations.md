# 40-operations — dual-path deployment, backups, monitoring, secrets

> Operations design for **both deploy paths (D-1)** at low cost (C4: preserve the
> minimal footprint — no new infra services). Executed by T011, T012, T018, T019, T025.
> Constitution: `00-overview.md`. Current production topology reference:
> `docs/deployment.md` (two-origin LiteSpeed + Node proxy) — unchanged by this plan
> except where noted.

## 1. Topology (both paths, one codebase)

| | **Path A — Docker appliance (new)** | **Path B — cPanel/LiteSpeed (existing)** |
|---|---|---|
| SPA | container (nginx/caddy) serving `dist/`, `/api` proxied (same-origin) | static host upload, cross-origin API (`VITE_API_URL` baked) |
| API | `backend-rs` container (digest-pinned image, non-root, `/health` HEALTHCHECK) | Rust binary behind `scripts/app-proxy.js` (Node hop) |
| DB | compose `mysql:8.4` service, named volume, loopback-published | host MariaDB (see §6 version check) |
| TLS | host/proxy concern (not in compose — document Caddy auto-TLS as option) | LiteSpeed/Let's Encrypt (host-provided, A+ verified) |
| Backups | same `scripts/backup-db.sh` + systemd timer | same script + cPanel cron |
| Monitoring | same hosted uptime check on `/ready` + JSON logs | same |

**Path A exists for:** contributors, load testing (T026), and orgs that want a
one-command self-host (README checklist item "Docker & templates"). **Path B remains
production** until/unless the org standardizes on a VPS. Neither path may silently
become "the only documented way" (README must show both).

## 2. compose appliance (T012) — see `10-reliability.md` §R3 for the fix design

Deliverable summary: `compose.yaml` builds from clean clone; `frontend/Dockerfile`
(static + SPA fallback + immutable asset caching + `/api` proxy for compose topology);
`nest` service gone; MySQL loopback-only; healthchecks/restarts everywhere;
`TRUSTED_PROXIES`/`CORS_ORIGINS` set for the compose hop. **Dev defaults
(`changeme`) are dev-only** — the file must carry a warning banner pointing at §5.

## 3. Backups & restore (T011) — full design in `10-reliability.md` §R1

Ops-side contract: script at `scripts/backup-db.sh`, retention 7d/4w, optional offsite
hook, systemd timer + cPanel cron examples, drill template + first drill record. This
is R1's closure and the **prerequisite for every production migration** (cutover
doctrine: restore is the only DB rollback).

## 4. Monitoring & logging (T025, D-4)

**Chosen level: hosted uptime + `/ready` + JSON logs** (no self-hosted Prometheus).

- **Uptime:** external check (healthchecks.io-style dead-man's-switch or any hosted
  prober) hitting `GET /ready` every 1–5 min; alert on consecutive failures +
  a ping on recovery. `/ready` is the right probe (DB-aware, budget-bounded 500 ms
  default) — `/health` only proves the process lives. **Never probe `/metrics`**
  (unauthenticated, port-restricted — `.env.example` rule).
- **Logs:** `APP_ENV=production` already emits JSON (T60) to stdout. Define retention:
  **90 days** (spec `09-logging-monitoring-ir.md` gap SEC-021) — on Path B the Node
  wrapper/host manages rotation (document `logrotate` example for Path A: size+age
  capped, `copytruncate` or signal-based per the wrapper's restart behavior).
- **Alerting thresholds (document, not implement):** `/ready` failing > 2 min; 5xx rate
  > 1% over 5 min (derivable from logs); disk > 85%; backup job missed > 26 h. These
  live in this file's runbook table so an operator has one page.
- **Metrics:** `/metrics` stays opt-in (`METRICS_PORT`); T024 makes the series useful;
  scraping is a *future* option — if ever added, it's network-local only. No
  Grafana/Prometheus ships now (D-4).

## 5. Secrets & config hygiene (T018, C3)

- `chmod 600` on every `.env` (`backend-rs/.env`, `release/.env`, frontend `.env`);
  add a check to the deploy runbook (`stat -c %a`).
- **`release/.env` shares the live `SESSION_SECRET` with `backend-rs/.env`** — if that
  `release/` tree was ever uploaded (it contains `public/`), **rotate**:
  `SESSION_SECRET` new value → all sessions invalidate → users re-login (acceptable;
  announce). Rotation runbook: generate (`openssl rand -hex 32`), update env, restart,
  verify login works, old cookie rejected.
- `frontend/package.json`: undeclared `@radix-ui/react-tabs`/`-tooltip` imports added
  as explicit deps; build-only packages (`shadcn`, `tailwindcss`,
  `@tailwindcss/vite`, `tw-animate-css`) → `devDependencies` (also C2).
- Compose MySQL never published on `0.0.0.0` (T012) — F-01 history: prod MySQL was
  once internet-reachable (`specs/security/06-…:73-76`).
- `.env.example` stays the documented contract (no real values); CI already greps
  Dockerfiles for `DATABASE_URL` — keep.

## 6. Prod DB flavor check (C3, blocks nothing but honesty)

`docs/mariadb-compat.md:3` says MariaDB **10.6.28**;
`specs/security/06-infrastructure-deployment.md:77` says **11.4.x** live. One is stale.
T011's drill (which connects to the real DB) must **record `SELECT VERSION()`** and
both docs get corrected (T019 owns doc sync). CI already tests both engines — the
docs must name what production actually runs.

## 7. Deploy sequence (T019 — docs/proxy consolidation)

- **Single proxy owner:** today `scripts/app-proxy.js` + `release/app-proxy{,-v2,-plus}.js`
  (3+ divergent copies; `release/` is gitignored). Target: **one tracked**
  `scripts/app-proxy.js` (the ephemeral-port version) is the source of truth; the
  `release/` copies are build artifacts or deleted; `docs/deployment.md` references
  only the tracked one. The T007 proxy contract (Host unchanged, XFF append,
  X-Forwarded-Proto, `TRUSTED_PROXIES`) is the acceptance checklist for whichever copy
  survives.
- **README rewrite:** Rust stack, both paths, correct build commands
  (`cargo build --release`, `npm run build`), remove all `cd backend`/Nest/migrate.js
  instructions, link `docs/deployment.md` + this file. Also fix stale cutover
  references to a retained Nest rollback image (`docs/cutover.md:14`,
  `docs/data-migration-live.md:722`) — Nest is deleted (`4051be68`); rollback story is
  now backup-restore only.
- **Playwright/e2e docs** land with T017 (one-command local e2e).

## 8. Cost guardrails (C4 — preserve)

Every ops task must not add: a new daemons beyond the ones above, paid services
(hosted uptime free tier is fine), or a second deployment mechanism. If a task seems to
need one, that's an escalation to the user, not a silent addition.
