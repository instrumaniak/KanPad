# Security Audit — Overview, Scope & Methodology

**Project:** KanPad (live: `https://<frontend-origin>` frontend, `https://<api-origin>` backend)
**Role:** Senior Security Specialist
**Status:** Plan (execution tracked per file in this folder)
**Owner:** project owner (solo). No external budget — open-source tooling only.

---

## 1. Purpose

This folder holds the security audit plan for KanPad. Each numbered file can be
worked through independently, one after another, then converted into task files
under `specs/tasks/` (prefix `SEC-`) and fixed one by one. Every file follows the
same shape:

1. **Objective** — what this phase proves or disproves.
2. **Scope** — in / out.
3. **Threat model** — assets, actors, trust boundaries.
4. **Test plan** — concrete, reproducible checks (commands where useful).
5. **Live findings so far** — what reconnaissance already established.
6. **Remediation guidance** — direction, not necessarily final code.
7. **Exit criteria** — what "done" looks like.
8. **Task mapping** — which `SEC-*` tasks this feeds.

## 2. Methodology

- **Frameworks:** OWASP WSTG v4.2 (web), OWASP ASVS 4.0, OWASP Top 10 (2021),
  plus the service's own security constitution in
  `specs/api-rewrite-rs-specs/10-architecture.md` (CORS, CSRF, headers, cookies,
  throttling, problem+json).
- **Style:** white-box primary + local black-box. The hosting provider blocks IPs
  after rapid requests, so **no automated scanning against the live host** is
  permitted. All active testing (throttle floods, SQLi probes, brute force,
  ZAP/nikto/ffuf/sqlmap) runs against a **local `compose.yaml` stack**. Live
  checks are limited to a handful of single, spaced-out manual `curl` requests
  for header/cookie/CORS verification (already completed — see 01). White-box
  code review of `backend-rs/` and `frontend/` is the primary method for the
  remaining phases.
- **Severity rubric (CVSS v3.1 aligned):**

  | Severity | CVSS | SLA |
  |---|---|---|
  | Critical | ≥ 9.0 | 7 days |
  | High | 7.0–8.9 | 14 days |
  | Medium | 4.0–6.9 | 30 days |
  | Low | 0.1–3.9 | 60 days |
  | Info | 0.0 | best effort |

- **Rules of engagement:** no automated scanning against live (provider blocks
  IPs via ModSecurity rule 420043, ~5 hits/3600s). No DoS, no account takeover
  against real users, no data exfiltration. Test accounts only. All active
  testing runs against a local `compose.yaml` stack. Live verification is
  manual, single-request, spaced (see 01 §0).

## 3. Scope

**In scope**
- `backend-rs/` (Rust/Axum): auth, users, projects, boards, columns, cards,
  labels, checklists, notes, tags, health, metrics, CLI.
- `frontend/` (React SPA): auth flows, markdown/mermaid rendering, API client.
- Deployment: cPanel shared hosting, `release/app-proxy.js`, LiteSpeed/Apache
  fronting, MySQL.
- CI/CD: `.github/workflows/backend-rs.yml`, Dockerfile, supply chain.
- Live environment configuration (env vars, TLS, headers, DNS, DB grants).

**Out of scope (residual risk only)**
- cPanel provider physical/infrastructure security.
- Social engineering / phishing of users.
- The deprecated Nest `backend/` (reference only; not deployed).

## 4. Assets & Trust Boundaries

**Assets (CIA):** user credentials (Argon2id + legacy bcrypt); session integrity
(cookie sessions, no JWT, sliding 24h); user data (projects, boards, columns,
cards, labels, checklists, notes, tags); multi-tenancy isolation; availability
(rate limits, DB pool, graceful shutdown).

**Actors:** anonymous visitor; authenticated user; admin (single-admin guard);
attacker on the public internet; attacker on a sibling subdomain; attacker with a
valid low-privilege account.

**Trust boundaries:** Browser ↔ LiteSpeed/Apache (TLS) ↔ Node proxy
(app-proxy.js) ↔ Rust binary; Rust binary ↔ MySQL (localhost/socket); Rust
binary ↔ metrics listener (separate port, opt-in).

## 5. Tooling (free / open-source only)

- `curl`, `openssl`, `nmap` (host/port discovery, light).
- OWASP ZAP (baseline + active) — not currently installed; manual curl-based
  equivalents used. Install later if desired.
- `cargo audit`, `cargo deny`, `npm audit`.
- `sqlmap` only against local compose stack, never live.
- `nikto` for web server config checks (not installed; optional).
- Browser DevTools + mitmproxy for client-side checks.
- `gitleaks` / `trufflehog` for secret scanning of the repo.

## 6. Deliverables

1. `specs/security/01-reconnaissance-findings.md` — live recon results.
2. Numbered plan files `02`–`10` — one per audit domain.
3. `specs/security/quick-wins.md` — fix-this-week items.
4. `specs/security/findings-register.md` — running register.
5. (Later) `specs/tasks/SEC-*.md` task files, one per finding/remediation —
   currently forward-references only; create stubs when starting implementation.
6. (Later) regression tests wired into CI.

## 7. How to use this folder

Work the files in numeric order. For each, run the test plan, record findings in
`findings-register.md`, then raise `SEC-*` task files. Fix tasks one at a time
with a failing test first (TDD, per project convention), re-run the relevant
phase's checks, and mark the finding `Verified`.

## 8. Already-known positives (do not re-litigate)

- Cookie session (no JWT), `__Host-kanpad.sid` in prod, `HttpOnly`, `SameSite=Lax`,
  `Secure` in prod, HSTS on API, CSP on API (verified on this branch — **re-verify
  after T007/T008 API-only split; the API CSP is expected to be removed**), nosniff, X-Frame-Options DENY,
  problem+json errors, sqlx parameterized queries, garde validation, Argon2id,
  CSRF CsrfLayer (403 on cross-site verified live), CORS allowlist exact origin,
  rate limiter, non-root container user, digest-pinned bases, `--locked` builds.
- No auth tokens in localStorage (only theme/sidebar UI state).
- No source maps deployed.

## 9. Live recon summary (details in 01)

Top issues already confirmed live:
- **MySQL/MariaDB 11.4.x was publicly reachable on port 3306** (Critical, F-01)
  — mitigated (remote access removed); verify closed.
- Frontend origin (`<frontend-host>`) serves **no security headers at all**.
- Swagger UI + `/api-docs/openapi.json` publicly reachable (schema disclosure).
- SPA fallback returns `200 index.html` for arbitrary paths.
- CSRF correctly returns 403 for cross-site mutating requests (verified).
- CORS correctly allows exact frontend origin with credentials (verified).

Additional verified (testssl.sh / gitleaks / ffuf):
- TLS grade A+ on both hosts (TLS 1.2+1.3, forward secrecy, HSTS on API).
- No committed secrets in git history (gitleaks).
- Weak shared SESSION_SECRET (16 bytes) in (gitignored) env files (T-01).
- No real file exposures (ffuf + curl).