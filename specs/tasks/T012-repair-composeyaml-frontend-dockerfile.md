# T012: Repair compose.yaml + frontend Dockerfile — the production self-host appliance

> **STATUS: in progress (re-scoped 2026-11-17).** Wave 0 (reliability floor). Closes
> **R3** (HIGH) + compose half of **C3**. This task builds the **production self-host
> appliance** (D-1 Path A): what a home-lab community member runs on their own
> machine — NOT a dev appliance (contributors use native toolchains: nodejs +
> rustup). Same-origin, single-domain, deployed behind any TLS-terminating proxy
> (Caddy/Traefik/Dokploy/Portainer) or plain `docker compose up` for localhost.

## Goal

`docker compose up --build` on a clean machine yields a **production-ready
self-hosted KanPad** (MariaDB + Rust API + nginx-served SPA) — the D-1 Path A
appliance the community can run without a repo toolchain. Closes R3 (build was
impossible) and C3 (DB/rust off loopback-only).

1. **Remove the `nest` service** (built `./backend`, deleted in `4051be68`) and the
   `frontend depends_on: nest` edge.
2. **Create `frontend/Dockerfile`**: multi-stage **node:26-alpine** builder
   (matches the CI Node pin 26 LTS; `npm ci` + `npm run build`, `VITE_API_URL` as
   build ARG, default `""` → same-origin via the nginx proxy) → static runtime
   **nginx:alpine** (pinned decision — see below) serving `dist/` with SPA
   fallback, `/assets/*` immutable, `index.html` no-cache, **security headers**,
   and a **compose-only same-origin `/api` reverse-proxy** to `rust`. Both bases
   digest-pinned (T62).
3. **Baked, not dev, credentials:** `APP_ENV=production`; `SESSION_SECRET` and
   `CORS_ORIGINS` are **required** (`:?` — compose aborts with guidance when
   unset; `.env.example` documents generation). No `changeme` anywhere. `DB_PASSWORD`
   (the bundled DB's `kanpad` user) is **required** the same way;
   `MYSQL_RANDOM_ROOT_PASSWORD=yes` for both bundled DBs so no root credential is
   ever needed at runtime.
4. **MariaDB by default, MySQL one edit away:** ONE `db` service — MariaDB 11.4
   (digest-pinned, official `healthcheck.sh --connect --innodb_initialized`)
   active by default; the MySQL 8.4 block (digest-pinned, TCP healthcheck) sits
   in the same file **commented out** — swap by uncommenting it (see the inline
   instructions; the `mysql-data` volume line at the bottom comments in too).
   **External database** = comment out both DB blocks and set `DB_*`/`DATABASE_URL`
   (`DATABASE_URL` wins; collation pin `utf8mb4_unicode_ci` required). No profiles,
   no custom networks, no `container_name` — Dokploy/Portainer-clean.
5. **Boot contract:** `command: ["sh", "-c", "backend-rs migrate run && exec backend-rs serve"]`
   — every start/upgrade applies pending migrations first, then serves. `migrate
   run` is production-legal (verified — only `migrate revert` is prod-gated); a
   failed migration fails closed (rust never starts; logs carry the
   `diagnose_migrate` cause+hint). `--auto-migrate` is NOT used (refused in
   production, by code).
6. **Images as small as we responsibly can (this task, not follow-ups):**
   **rust runtime = `gcr.io/distroless/static-debian12:debug-nonroot`** (busybox
   for a working HEALTHCHECK + home-lab debuggability, non-root by default) with a
   **fully static musl binary** (ring + rustls only, no openssl/aws-lc — verified);
   glibc/bookworm-slim/curl all dropped (~84MB saved). BuildKit **cache mounts**
   for the cargo registry and `target/` so local rebuilds are fast (CI unchanged —
   fresh runners see an empty cache; binary `cp`'d out inside the same RUN per the
   T62 cache-mount lesson). **frontend base = node:26-alpine** (musl bindings exist
   for rolldown/oxide — verified in the lockfile). **nginx:alpine stays** (pinned).
7. **Network posture:** nothing published except `frontend` on
   `127.0.0.1:5173:80` (localhost IS a secure context, so try-outs log in fine; LAN/
   hostname HTTP can never satisfy Secure `__Host-` cookies — HSTS + Secure is why
   LAN HTTP logins fail; HTTPS via Dokploy/Portainer/any TLS proxy is required for
   remote access). DB + rust publish **no** ports. No custom networks (dokploy
   compatibility).

## Depends On

(None.)

## Phase

0

## Critical

Yes — as committed the stack cannot build (R3); blocks contributors, load testing
(T026) and the appliance deliverable (D-1).

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R3 (design + same-origin topology).
- `specs/arch-hardening-specs/40-operations.md` §1 (Path A = this appliance), §2.
- `specs/arch-hardening-specs/00-overview.md` — findings R3, C3; D-1; global DoD.
- `docs/deployment.md` (org two-origin production — must NOT be changed; the
  appliance implements the same-origin path instead).
- `docs/self-hosting.md` (NEW — the appliance operator's guide).
- `frontend/vite.config.ts`, `frontend/public/.htaccess` (SPA fallback reference).
- `backend-rs/.env.example` (`TRUSTED_PROXIES` semantics: bare IPs/CIDRs; append-
  only walk; malformed entries ignored; direct peer must be trusted).
- `backend-rs/src/main.rs` (`migrate run` works in production; `migrate revert`
  does not; `--auto-migrate` is dev-only).
- `backend-rs/src/config.rs` (`SESSION_SECRET` ≥32 bytes, fail-fast in every env;
  `is_production()`).
- `backend-rs/Dockerfile` (digest-pin pattern, HEALTHCHECK, cache-mount lesson).
- Digest pins: `rust:1.99-alpine` `44819b27…`, `node:26-alpine` `6bd9ab43…`,
  `nginx:alpine` `fed7e14b…`, `mariadb:11.4` `5e73baf8…`, `mysql:8.4` `a81d0439…`,
  `gcr.io/distroless/static-debian12:debug-nonroot` `d5563cc7…`.

## Files to Create/Modify

| File | Change |
|---|---|
| `compose.yaml` | **Rewrite**: header banner (self-host prod, not dev). `nest` **removed** (it could not build). `frontend`: build context + `VITE_API_URL` ARG (default `""`), `127.0.0.1:5173:80`, curl-able wget healthcheck, `unless-stopped`. `rust`: digest-pinned musl build, `command: sh -c 'backend-rs migrate run && exec backend-rs serve'`, `APP_ENV=production`, required `SESSION_SECRET`/`CORS_ORIGINS`/`DB_PASSWORD`, `DB_STARTUP_TIMEOUT_SECS=120`, no ports, `unless-stopped`. `db` (MariaDB active): digest-pinned, `healthcheck.sh`, `mariadb-data` volume, kanpad user + random root. The commented `[MySQL]` block: digest-pinned, TCP healthcheck, `mysql-data` (commented in too). External-DB instructions as comments. **No** custom networks/subnets/pinned IPs/ports. |
| `backend-rs/Dockerfile` | **REWRITE**: alpine musl builder (digest), `apk add musl-dev`, BuildKit cache mounts for `/usr/local/cargo/registry` + `/app/target` (binary `cp`'d out of the cache-mounted target inside the same RUN), runtime `gcr.io/distroless/static-debian12:debug-nonroot` (digest), NO curl/useradd (busybox + built-in nonroot), HEALTHCHECK via `/busybox/wget` honoring `${PORT}`, `SQLX_OFFLINE=true`, `--locked` |
| `frontend/Dockerfile` | Builder base **node:26-bookworm-slim → node:26-alpine** (digest; musl bindings verified). Runtime unchanged (nginx:alpine, digest). Header comments: appliance/same-origin framing |
| `frontend/nginx.conf` | Keep (security headers, `= /api` + `/api/` blocks, append-XFF + Host contract); comments reworded **dev/prod-appliance** → **appliance (same-origin self-host)** only |
| `.env.example` | **REWRITE for self-hosters**: `APP_ENV=production` fixed, `SESSION_SECRET` (gen: `openssl rand -hex 32`) required, `CORS_ORIGINS` (your origin) required, `DB_PASSWORD` (mariadb/mysql kanpad user; gen commands) required, `DB_NAME=kanpad`, optional `TRUSTED_PROXIES` + `REGISTRATION_ENABLED` comments. No `changeme` |
| `docs/self-hosting.md` | **NEW** — operator's guide (quickstart, TLS requirement, Dokploy/Portainer walkthroughs, DB selection/switch, external DB, upgrade flow, backups → T011, TRUSTED_PROXIES how-to, size table, hub-image note) |
| `README.md` | Pointer reworded to the self-host appliance + `docs/self-hosting.md` |
| `docs/lessons/ci-quality.md` | Append tagged lessons (musl/ring works; secure-context cookie rule; dokploy/portainer constraints; distroless HEALTHCHECK) |
| `specs/tasks/status.yaml` | T012 stays `done` (human flip at commit, per process) |

**Must NOT appear in the diff:** production `docs/deployment.md` changes, real
`.env` values, app code, `_sqlx_migrations`, compose-time service additions.

## Implementation Steps

1. Reproduce (red): `docker compose build` fails at the current tree (paste).
   Already captured twice; keep the evidence.
2. `backend-rs/Dockerfile` rewrite (musl + distroless + cache mounts + digest
   pins). Verify: `docker images` shows the new size.
3. `frontend/Dockerfile`: swap builder to `node:26-alpine@digest`.
4. `frontend/nginx.conf` comment reword only; keep directives.
5. `compose.yaml` rewrite (see table). Validate: `docker compose config --quiet`.
6. `.env.example` rewrite; `docs/self-hosting.md`; README pointer; lessons.
7. **Verify from a pristine copy** (rsync into /tmp/opencode, no `.env`, no
   history): `docker compose up --build -d` → fresh volume → MariaDB healthy via
   `healthcheck.sh` → migrate-in-CMD applies schema → rust healthy → frontend healthy;
   register → login → `/auth/me` via nginx proxy; `Cache-Control` asserts; `/api`
   exact 404s `problem+json`; SPA fallback; boot **fails closed** without
   `SESSION_SECRET` (assert by running config validation path without it — e.g.
   `docker compose up` without the var → error). `down -v` clean; `docker images`
   size table in Notes.
8. Note the CI gap (nothing in `.github/workflows/` builds compose/frontend
   images — intentional; keep CI cost flat).

## Constraints

- **Prod appliance, not dev:** no dev defaults, no `changeme`, no loopback-only
  relaxations beyond the one frontend publication; LAN access requires TLS.
- **Healthcheck honesty:** every image that defines HEALTHCHECK must actually be
  probeable (nginx→busybox wget kept; rust→distroless busybox wget; MariaDB→its own
  `healthcheck.sh`).
- **Doctrine:** dokploy/portainer-compatible (no ports/networks/`container_name`
  beyond the noted loopback 5173).
- `docs/deployment.md` unchanged. No new services. No app code changes.
- Digest-pin every base (T62). No commit without approval.

## Acceptance Criteria

- [ ] `grep -n "nest" compose.yaml` empty; frontend no longer depends on nest
      (closes **R3**).
- [ ] No `changeme`, no dev `SESSION_SECRET`, no dev defaults anywhere in
      `compose.yaml`/`.env.example` (grep evidence pasted).
- [ ] `docker compose config --quiet` clean; services: frontend, rust, `db`
      (MariaDB active) with the MySQL 8.4 block commented (swap instructions
      inline; external-DB mode = comment both).
- [ ] From a pristine copy: `docker compose up --build -d` boots MariaDB (healthy,
      canonical `healthcheck.sh`), applies all migrations (migrate-in-CMD logs),
      rust serves, frontend serves; register → login → `/auth/me` through the proxy
      (paste evidence).
- [ ] Boot fails closed without `SESSION_SECRET` (paste the error).
- [ ] `docker images` size table pasted; appliance total ≤ ~450MB.
- [ ] SPA fallback deep-route 200 text/html; hashed assets `immutable`; index
      `no-cache`; security headers present (paste `curl -sSI`).
- [ ] `/api` exact-match returns `application/problem+json` (not HTML).
- [ ] Healthchecks + `unless-stopped` on all services; no service publishes ports
      except frontend `127.0.0.1:5173:80`.
- [ ] `docs/deployment.md` unchanged; `docs/self-hosting.md` present; lesson
      appended; spec ACs ticked by human review.

## Notes

**Implemented 2026-11-17 (production self-host appliance, re-scoped from the 2026-11-16
dev appliance).** Evidence gathered in a pristine rsync copy at
`/tmp/opencode/t012-clean` (tracked files + these changes, no `.env`, no git history).

- **Environment/config bugs found and fixed during verification (all code-verified):**
  1. Compose `command` **appends** to the image `ENTRYPOINT`; the original
     backend-rs Dockerfile's `ENTRYPOINT` made the runtime `… backend-rs
     /busybox/sh …` → `unrecognized subcommand`.
  2. `gcr.io/distroless/static-debian12:debug-nonroot` **inherits
     `Entrypoint: ["/busybox/sh"]`** — every exec read ELFs as shell scripts
     (`ELF: not found` / `line 2: syntax error: unexpected newline`). Fix:
     `ENTRYPOINT []` reset + binary in `CMD` (compose `command` replaces `CMD`
     wholesale). **Lesson in ci-quality.md 2026-11-17.**
  3. Production `CORS_ORIGINS` **must not be loopback** (T56 fail-closed).
     `.env.example`/`self-hosting.md` now document a non-loopback example value
     and explain the same-origin bypass; the smoke ran with
     `CORS_ORIGINS=https://kanpad.example.com`.

- **Build OK** (fresh copies): `kanpad-rust` (musl static via `rust:1.99-alpine`
  + `musl-dev`, BuildKit cache mounts) and `kanpad-frontend`
  (`node:26-alpine` — musl rolldown/oxide bindings present in the lockfile).
  rust release build 7m31s cold; cache-mount rebuilds collapse to seconds.
- **Sizes (docker images):** rust **20.5MB** (was 106MB), frontend **69.7MB**,
  mariadb:11.4 334MB → appliance ≈ **425MB** (rust −81%).
- **Boot evidence (fresh volume):** MariaDB healthy via image-shipped
  `healthcheck.sh` → `backend-rs migrate run` applied **3 migrations** →
  `listening … port: 3001` → rust HEALTHY (distroless busybox `wget` on
  `/health`) → frontend HEALTHY. JSON logs (`APP_ENV=production`) confirmed.
- **Fail-closed gate verified:** boot without required env vars aborts at
  `docker compose config` with actionable guidance — no `changeme` anywhere.
- **Smoke (both DB engines, via nginx same-origin proxy):** register 201 →
  login 200 → `/auth/me` 200; SPA security headers (`nosniff`/
  `Referrer-Policy`/`X-Frame-Options`); `/` `no-cache`; hashed asset
  `immutable`; deep route 200 text/html; exact `/api` → `application/problem+
  json` (not HTML); restart `unless-stopped` ×3.
- **MySQL swap path proven:** the commented `[MySQL]` block uncommented and
  `[MariaDB]` commented (block boundaries machine-checked, `compose config`
  green) → MySQL 8.4.13 fresh volume: TCP-healthcheck healthy, migrations
  applied, register 201. (Swap variant kept in /tmp, not committed.)
- `docker compose down -v` clean for both engine variants.

**Pinned decisions (2026-11-17 re-scope — supersede all earlier ones):**
production self-host appliance target; MariaDB 11.4 **default**, MySQL 8.4
comment/uncomment; external DB via commented blocks + `DB_*`/`DATABASE_URL`; shared
`db` network alias; **rust runtime distroless/static debug-nonroot with musl
static binary**; **node:26-alpine builder**; **nginx:alpine static runtime stays**
(busybox httpd rejected: 2026 pipelined-request proxy CVE, no per-path Cache-Control,
no Host/XFF proxy-header control; caddy:alpine 65MB is bigger than nginx); BuildKit
cache mounts in rust builder; **`frontend` is the only published port
(127.0.0.1:5173)**; Secure-cookie/HTTPS requirement documented instead of weakened;
`migrate run` in CMD chain instead of `--auto-migrate`; dokploy/portainer-clean;
image publishing to Docker Hub NOT in scope (build from source; registry swap
documented for the template task).

Supersedes: earlier 2026-11-16 dev-appliance framing and the `10.113.42.0/24` pinned
subnet / `TRUSTED_PROXIES=10.113.42.10` scheme (incompatible with dokploy Swarm;
replaced by a `db` alias + no custom network + `TRUSTED_PROXIES` as optional deploy
env, default empty = spoof-proof).

Stale references from the old dev-appliance framing: `frontend/CONVENTIONS.md:57`,
`docs/lessons/frontend.md:15`, `docs/cutover.md` (runtime `VITE_API_URL` in
compose.yaml, "dev appliance" wording) — recorded for T019.

**As-built deviation (recorded post-review 2026-11-17):** the spec originally called
for two named services (`db-mariadb` active / `db-mysql` commented) sharing a `db`
network alias. As built, **one `db` service** with the MariaDB block active and the
MySQL block commented achieves the same one-command swap with fewer moving parts and
no alias/hostname trickery — the config renders cleanly in all three modes (MariaDB
default, swapped MySQL, external-DB with both commented — all machine-checked). The
Files table, Goal and ACs were amended to match; spec now describes reality.

## Lessons

See `docs/lessons/ci-quality.md` — 2026-11-16 (compose drift, TCP healthcheck,
/32-vs-CIDR, Host forwarding) and 2026-11-17 (**musl static + ring/rustls**,
**distroless `:debug` inherits a shell entrypoint**, **production loopback CORS
refusal + same-origin bypass**, cache-mounted `target/` is not in the layer,
digest pins hide base semantics).
