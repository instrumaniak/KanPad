# Self-hosting KanPad — the Docker appliance

KanPad ships as a single-machine, single-domain appliance for home labs and
small teams: MariaDB + Rust API + nginx-served SPA, same-origin. One command,
no toolchain on the host.

The appliance is **production-shaped**, not a demo: production cookie flags
(`Secure`, `__Host-`), JSON logs, fail-closed configuration, migrations applied
at boot. (HSTS is emitted by the Rust API; the TLS proxy should also send
`Strict-Transport-Security` for the frontend origin — Dokploy/Portainer templates
do this for you.)

> **Requirement: HTTPS.** Secure session cookies are refused by browsers over
> plain HTTP on non-localhost hosts. `http://localhost:5173` works (localhost
> is a secure context); your LAN IP / hostname does not. Put a TLS-terminating
> proxy in front — see “Remote access (TLS)” below.

## Prerequisites

- Docker with the Compose plugin (`docker compose version`), Linux or macOS.
- A free host port `5173` (loopback only).

## Quick start (localhost trial)

```bash
cp .env.example .env
# Fill in the 3 required values:
#   SESSION_SECRET — openssl rand -hex 32
#   CORS_ORIGINS   — https://kanpad.example.com (your eventual public origin;
#                    never consulted in the same-origin appliance — any
#                    non-loopback value works for a localhost try-out)
#   DB_PASSWORD    — openssl rand -hex 24
docker compose up --build -d
```

Open http://localhost:5173 — register your account, and you're in.
(Your `CORS_ORIGINS` may be the example value for a localhost try-out: same-origin
traffic bypasses CORS, and localhost is a secure context so Secure cookies stick.)

Stop / wipe data:

```bash
docker compose down        # stop, keep data (volumes persist)
docker compose down -v     # stop AND delete the database volume
```

## Remote access (TLS is required)

Any TLS-terminating reverse proxy works. The appliance itself speaks HTTP on
:80 inside the frontend container; nothing else is published.

**Dokploy** — create a *Docker Compose* service from this repo (or paste
`compose.yaml`), set `SESSION_SECRET`, `CORS_ORIGINS=https://<your-domain>`,
`DB_PASSWORD` in the env editor, add domain → service `frontend` port `80`.
Dokploy handles TLS (Let's Encrypt) and redeploys on image updates.

**Portainer** — *Stacks → Add stack → Web editor*, paste `compose.yaml`,
provide the same three env vars, deploy. Put a reverse proxy (Caddy/Traefik/
Nginx Proxy Manager) in front of port 5173 or your domain.

**Caddy** (minimal):

```
kanpad.example.com {
    reverse_proxy localhost:5173
}
```

**Tailscale Serve** — `tailscale serve --bg 5173` gives you a trusted HTTPS
URL on your tailnet with zero config. Great for home labs.

> **LAN HTTP does not work.** Browsers refuse Secure cookies over plain HTTP
> — you will be stuck at “Session expired” on every login. This is the
> appliance working as designed.

## Choosing the database

**MariaDB 11.4 LTS (default)** is bundled and matches CI and the production
host DB. **MySQL 8.4 LTS** is one edit away:

1. In `compose.yaml`, comment out the `db:` block labelled `[MariaDB]`.
2. Uncomment the `[MySQL]` block (instructions are inline).
3. `docker compose up --build -d`.

**Your own server instead?** Comment out both DB blocks and set:

```
DB_HOST=your-db-host  DB_PORT=3306  DB_USERNAME=kanpad  DB_PASSWORD=…  DB_NAME=kanpad
```

or `DATABASE_URL=mysql://user:pass@host:3306/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci`
(the collation pin is enforced in production). The app requires no root access
to your server — a dedicated `kanpad` user with rights on one database is fine.

## Upgrading

```bash
git pull                      # or download the new compose.yaml
docker compose up --build -d  # rebuild images, apply pending migrations, restart
```

Migrations run automatically at boot (`migrate run` before `serve`); a failed
migration fails closed — the API never starts. Rollback = restore from backup
(see the T011 backup tooling spec, or your host's own backup tooling).

## Backups

The appliance's data is one named volume (`kanpad_mariadb-data`). Dump it with
`mysqldump --single-transaction …` on a timer (see the T011 backup tooling
spec, `specs/tasks/T011-automated-d-b-backups-restore-drill.md`), or bind-mount
a host directory as the DB volume and back that directory up.

## Trusted proxies (optional)

Behind your own reverse proxy you may set `TRUSTED_PROXIES` so API throttling
keys on real client IPs. Use the proxy's **stable** address — e.g. Dokploy's
ingress/overlay network CIDR (`dokploy-network` or your chosen overlay range) —
not a container task IP, which drifts on redeploy.

Default (empty) = `X-Forwarded-For` is never trusted (spoof-proof; all users
share one throttle bucket).

## Troubleshooting

```bash
docker compose ps
docker compose logs -f rust      # migration/boot errors are logged here
docker compose logs -f db
```

- **Can't log in on LAN** → missing TLS; see “Remote access (TLS)”.
- **Boot fails with SESSION_SECRET error** → set it (see Quick start).
- **Boot fails with CORS_ORIGINS error** → set it to a non-loopback origin (https,
  no trailing slash, e.g. your domain). Loopback origins are refused in production
  by design; the value is never used by same-origin browsers anyway.
- **API unreachable, frontend 502** → `docker compose logs rust` — usually a pending-migration failure on upgrade; restore backup or fix the schema.

## Image sizes (built 2026-11-17)

| Image | Size |
|---|---|
| frontend (nginx:alpine + 6.7MB dist) | ~70 MB |
| rust (distroless static, musl binary) | ~20 MB |
| mariadb:11.4 | ~334 MB |
| **Stack** | **~425 MB** |

Images are built from source by `docker compose up --build`. Once the project
publishes prebuilt images (e.g. to Docker Hub), the `build:` lines can be
swapped for `image:` pulls — no other changes required.
