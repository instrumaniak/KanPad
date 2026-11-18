# Deployment — two-origin topology (T008)

KanPad is deployed as two independent origins:

- **SPA (static host)** — `https://kanpad.raziur.com`
  Built with `npm run build` in `frontend/`; the resulting `frontend/dist/` is
  uploaded to the static host (Apache/LiteSpeed, `frontend/public/.htaccess`).
- **API** — `https://api-kanpad-rs.raziur.com`
  The `backend-rs` Rust service (`cargo build --release`), exposing only
  `/health`, `/ready`, `/api/v1/*`, the deprecated `/api/*` dual-mount,
  `/api-docs/openapi.json`, `/swagger-ui*` and `/metrics` (opt-in listener).

The SPA calls the API cross-origin via `VITE_API_URL`, baked at build time.
There is no `/api` proxy rule in `.htaccess` — an API call aimed at the static
host returns the SPA's `index.html`.

## Build sequence

1. Frontend: `cd frontend && npm run build` → produces `frontend/dist/` (including
   `frontend/dist/.htaccess`, copied from `public/` by Vite).
2. Upload `frontend/dist/` to the static host.
3. Backend: `cd backend-rs && cargo build --release` → `target/release/backend-rs`.
   Build the container image with `docker build` (no `public/` directory, no
   frontend artifact).
4. Deploy the API container with `DATABASE_URL`, `SESSION_SECRET`, etc. set at
   runtime (never baked into the image).

## Environment

- `frontend/.env.example` documents `VITE_API_URL=https://api-kanpad-rs.raziur.com`
  (plus a commented dev default `# VITE_API_URL=http://localhost:3001`).
- The real `frontend/.env` is git-ignored; production values must be set in CI
  or on the build machine before `npm run build`.

## Deploy-time secret checklist (T018)

Run on **every host that holds env files**, before and after each deploy.
All five canonical on-disk env files (adjust the list to the host):

```sh
# GNU/Linux hosts (`stat -c`); use a BSD/macOS equivalent elsewhere.
for f in .env backend-rs/.env backend-rs/.env.test frontend/.env release/.env; do
  [ -e "$f" ] || continue   # existence-aware, not completeness-aware — keep this list current per host!
  [ "$(stat -c %a "$f")" = 600 ] && echo "ok   $f" || { echo "MISSING 600 — run: chmod 600 $f"; }
done
```

- [ ] Every on-disk env file is mode **600** (root `.env` is read by
      `compose.yaml` for `SESSION_SECRET`/`DB_PASSWORD` interpolation — it is an
      env file like any other).
- [ ] No `.env` / `.env.test` inside any uploaded or docroot tree — the release
      staging tree excluded from uploads is a **packaging** property, not a
      filename convention (guard follow-up: exclude `.env*` from release
      artifacts; see `specs/tasks/T018-secrets-hygiene-dependency-declarations.md`,
      Open call #4).
- [ ] `release/` (local staging tree) is chmod **700** on the build machine and is
      never uploaded with its `.env`.
- [ ] No `.env`, `*.zip`, `*.tar*` inside the static docroot.
- [ ] Any staging/remote copy of an env file counts as a secret too — update it on
      rotation or destroy it.

### SESSION_SECRET rotation runbook (T018)

If a copy of an env file holding `SESSION_SECRET` is suspected exposed (e.g. a
tree containing it was uploaded):

1. **Announce re-login first** — sessions everywhere using that secret die.
   (KanPad deployments where self is the only user: a plain notice suffices.)
2. Generate a fresh secret with `openssl rand -hex 32`. **Never echo, print, or
   log it** — write-only pipelines only (e.g. `openssl rand -hex 32 > "$(mktemp)"`,
   substitute from that file inside an editor, then securely remove the temp file:
   `shred -u` or `rm -f` + it lived on this machine only).
3. Write it in-place **from the temp file** (editor, or `sed -i` whose command line
   never carries the value — e.g. a small helper script that reads stdin). Prefer
   **distinct secrets per instance** (e.g. dev vs release-testing) to decouple cookie
   domains.
4. `sed -i` recreates the file — **re-`chmod 600`** afterwards; verify with
   `stat -c %a`.
5. Restart affected services.
6. Verify: a protected endpoint (e.g. `GET /api/v1/auth/me`) with a **pre-rotation
   cookie returns 401**; a fresh login returns 200. **Re-run this check on every
   instance that received the new file** — especially the remote/staging host, which
   is the one that matters.
7. Update or decommission every copy (including remote/staging hosts) — a stale
   copy keeps the old secret alive. The old secret must never be reused. Securely
   remove any temp file that held the new secret (step 2) and re-`chmod 600` after
   any in-place edit (`sed -i` recreates the file).
8. `SESSION_SECRET` rotation does **not** re-key anything else; DB passwords are a
   separate decision (see T018 Notes, Open call #5).

## Post-deploy smoke list

- `GET https://api-kanpad-rs.raziur.com/health` → `{"status":"ok"}`.
- `GET https://api-kanpad-rs.raziur.com/api-docs/openapi.json` matches the
  committed `openapi.json`.
- `GET https://kanpad.raziur.com/` serves the SPA.
- Any non-API path on the API host (`/`, `/boards`, `/index.html`,
  `/assets/app.js`) → `404 application/problem+json`, no HTML, no CSP header.
- No `content-security-policy` header on any API response.
- Cookie (`__Host-kanpad.sid`) is delivered cross-origin (same-site
  `raziur.com`, `SameSite=Lax` + `credentials: 'include'`).

## Required production environment

| Variable | Required value / notes |
|---|---|
| `APP_ENV` | `production` |
| `CORS_ORIGINS` | `https://kanpad.raziur.com` (canonical; feeds CORS + CSRF) |
| `TRUSTED_PROXIES` | `127.0.0.1` (exactly the Node hop) |
| `SESSION_SECRET` | ≥32 bytes |
| `HASH_COST` | ≥2 |
| `DB_*` / `DATABASE_URL` | production database |
| `DB_STARTUP_CHECK` | `require` (default) |
| `REGISTRATION_ENABLED` | as needed |
| `PORT` | API port |

## Proxy contract (hard requirements)

- LiteSpeed must be in an **append** mode ("Trusted IP Only" / "Use Last IP")
  so the peer IP is appended to `X-Forwarded-For`. Never "Keep Header from
  Trusted IP" (pass-through).
- The Node wrapper (`scripts/app-proxy.js`) must forward `Host` unchanged,
  append to `X-Forwarded-For`, and preserve `X-Forwarded-Proto` (falling back
  to `https`).
- If LiteSpeed runs on a non-loopback interface, add that IP to
  `TRUSTED_PROXIES`.

## Post-deploy smoke (T007)

1. `curl -sSI …/health | grep -ci content-security-policy` → `0`; `served-by: rust` present.
2. `OPTIONS /api/v1/auth/login` with `Origin: https://kanpad.raziur.com` → ACAO + credentials; with `https://evil.example` → no ACAO.
3. Six logins from two different clients → each gets 5; the 6th of one does not 429 the other.
4. `POST /api/auth/register` with `{}` → `400` problem+json.
5. `GET https://api-kanpad-rs.raziur.com/boards` → `404 application/problem+json`.
