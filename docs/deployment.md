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
