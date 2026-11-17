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
  ZAP/nikto/ffuf/sqlmap) runs against a **local `compose.yaml` stack** — **but the
  compose stack is currently broken** (arch-hardening R3/T012: `nest` service →
  deleted `backend/`, missing `frontend/Dockerfile`). Until T012 lands, run the
  layers separately (`backend-rs` + `npm run dev` / `vite preview`) or treat this
  plan as gated on T012.
  Live checks are limited to a handful of single, spaced-out manual `curl` requests
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
2. Numbered plan files `02`–`11` — one per audit domain (`11` = the Wave 3
   MCP/API-key surface, added 2026-11-16 — see §10).
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

## 10. Cross-references (overlap with the arch-hardening plan)

Several items here duplicate work owned elsewhere in the repo's plans. Each is
listed with its canonical owner — fix once, in one place, and mark the other as
blocked-on/cites. (Amended 2026-11-16; see `specs/arch-hardening-specs/00-overview.md`
§Amendment log for the parallel edits there.)

| Item here | Overlaps | Canonical owner |
|---|---|---|
| QW-1, T-01 (SESSION_SECRET), QW-6/T-06 (release/, .env 600) | C3 secrets hygiene | `T018` + `specs/arch-hardening-specs/40-operations.md` §5 (rotation runbook) |
| QW-10 (auth-event audit logging) | monitoring/logging work | `T025` (monitoring runbook) and — for the product-facing audit trail — Wave 2 collab row G (`30-collaboration.md` §5) |
| QW-12 (cargo/npm audit triage) | advisory tooling | `SEC-018` (this folder's audit task) + existing CI `cargo deny check` (amended 2026-11-16 — was `T024`, a metrics task, poor owner fit) |
| QW-7/R-01 (frontend host headers) | nothing in arch-hardening | **this folder** (SEC-009) |
| Backup/restore drill (09 §1.5, SEC-022) | backup procedure ownership | `T011` + `docs/operations-backup.md` (single owner — amended 2026-11-16); the drill record lives there |
| Swagger/OpenAPI exposure (R-02, SEC-010) | nothing in arch-hardening | **this folder** (SEC-010) |
| CSRF `instance` field (R-04, F-03, SEC-011) | nothing in arch-hardening | **this folder** (SEC-011) |
| Regression tests in CI (10-remediation §1.2) | e2e CI wiring | `T017` (Playwright + browsers in CI) |

**Beyond overlap — the Wave 3 gap (added 2026-11-16):** the MCP/API-key
surface designed in `specs/mcp/` (bearer keys, minting, the `/mcp` mount) is
not audited by any phase here. It is covered by the new
[`11-mcp-api-key-surface.md`](11-mcp-api-key-surface.md) (SEC-028) — the
design itself stays owned by `specs/mcp/`; this folder audits it like any
other auth surface. Wave 3a (credentials) is designed to land **before**
Wave 2, so this phase is scheduled alongside those tasks, not after them.
