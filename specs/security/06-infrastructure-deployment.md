# 06 — Infrastructure & Deployment

**Objective:** Secure the reverse proxy, TLS, cPanel hosting, database, secrets at
rest, and container image.

**Scope:** `release/app-proxy.js`, LiteSpeed/Apache config, cPanel account,
MySQL, `backend-rs/Dockerfile`, env files.

---

## 1. Test plan

### 1.1 Reverse proxy (`app-proxy.js`)
- Verify `Host` header is forwarded unchanged (not rewritten to 127.0.0.1).
  Test: request with `Host: <api-host>` → backend sees same Host.
- `X-Forwarded-For`: proxy must overwrite (not append) per spec; verify spoofing
  is impossible (client cannot inject a trusted XFF that survives).
- `X-Forwarded-Proto`: must reflect real TLS status behind cPanel/Apache
  (TLS termination). If always "http", cookie `Secure` flag (driven by APP_ENV,
  not XFP) still OK, but HSTS/redirects may suffer.
- Proxy binds only to localhost; backend port 3001 not publicly reachable.
- Process restart behavior (Passenger) doesn't leak env or leave zombie processes.
- Graceful shutdown: SIGTERM → drain → pool close within timeout.

### 1.2 TLS
- TLS 1.2 + 1.3 only (no 1.0/1.1). Verified: only 1.2/1.3 succeed.
- Cipher suites: no CBC/RC4/3DES; prefer AEAD (GCM/ChaCha20).
- Cert validity & chain; auto-renewal (Let's Encrypt) tested.
- HSTS on both hosts (frontend currently missing — see 07).
- No mixed content on frontend (all assets/API over HTTPS).

### 1.3 cPanel / hosting
- `release/` directory and `*.zip`/`*.tar` are outside HTTP docroot.
- `.env` not downloadable (verified: 403 on API; check frontend host too).
- `.env` permissions 600.
- Node app runs as non-root (Passenger default).
- No backup files (`.bak`, `.old`, `~`) in docroot.
- Directory listing disabled.

### 1.4 Database (MySQL)
- App DB user has least privilege: SELECT/INSERT/UPDATE/DELETE on app DB only;
  no DROP/CREATE/GRANT/FILE/SUPER.
- MySQL bound to 127.0.0.1 or Unix socket (not 0.0.0.0).
- No remote root login; root password strong.
- TLS between app and DB if not localhost (n/a if socket).
- Backups: encrypted, tested restore, retention.
- `max_connections` ≥ pool max (10) + margin.
- Slow query log / general log not left on in prod (performance + PII).

### 1.5 Secrets at rest
- `SESSION_SECRET` ≥ 32 bytes (T-01).
- `DB_PASSWORD` not default/weak (T-05).
- No secrets in git history (gitleaks/trufflehog scan).
- No secrets in logs (T05 redaction).
- Rotation procedure documented (SESSION_SECRET rotation = restart, all sessions
  invalidated).

### 1.6 Container image (if Docker path used)
- Runs as non-root `app` user (verified in Dockerfile).
- No `DATABASE_URL` baked in (CI gate).
- Base images digest-pinned (verified).
- `--locked` build (verified).
- Healthcheck uses `/health` (DB-free) (verified).
- No dev tools in runtime image.

### 1.7 DNS
- Subdomain takeover check on `api-kanpad-rs`, `kanpad`, `www`.
- SPF/DKIM if email is ever added (future).

## 2. Live findings so far
- TLS 1.2+1.3, valid Let's Encrypt cert, SAN correct. testssl.sh grade A+ on
  both hosts. ✓
- **F-01 (Critical): MySQL/MariaDB 11.4.x was publicly reachable on port 3306**
  (banner grabbed). Ports 3000/3001/8080 filtered; only 80/443/3306 open.
  **Mitigated:** remote access removed by owner (verify closed with nmap).
  SEC-014 still covers least-privilege DB user + bind-to-localhost.
- Prod DB is MariaDB 11.4.x (runbook assumed 10.6.x) — portability targets
  differ (F-02, Info).
- Frontend sends no security headers (R-01).
- testssl.sh: no OCSP stapling (R-08), BREACH potential (R-07), SNI mandatory
  (R-09). OCSP stapling optional.

- **WAF / ModSecurity on shared LiteSpeed host (not the app).** Ban triggered by
  rule `mod_security (id:420043)`: 5 hits in 3600s from `<redacted-ip>`
  (BD), logged Mon Nov 9 22:39:31 2026. Cause: ffuf/testssl/nikto bursts.
  Consequence: all live probing must be single, spaced-out manual `curl` only;
  high-volume testing goes through the local `compose.yaml` stack. If banned
  again, unblock via the hosting control panel (or wait out the window) and
  document the unblock here. The WAF counts requests, not attacks — even benign
  paths count toward the threshold.

## 3. Remediation guidance
- Fix proxy Host/XFF forwarding.
- Rotate weak secrets; least-privilege DB user.
- Restrict MySQL bind; remove remote root.
- Move `release/` out of docroot if present.
- Document rotation procedure.

## 4. Exit criteria
- All 1.1–1.7 pass or have documented residual risk.
- nmap light scan shows only 80/443 open externally.
- Secret scan of repo clean.

## 5. Task mapping
- `SEC-013` proxy hardening (Host/XFF/proto).
- `SEC-014` secret rotation + DB user least privilege (shared: F-01 + T-05).
- `SEC-015` cPanel docroot/file-permission audit.
- `SEC-016` DNS/subdomain takeover check.
- `SEC-027` DB portability re-verification on MariaDB 11.4.x (F-02).