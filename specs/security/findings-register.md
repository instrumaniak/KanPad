# Findings Register

Running register. One row per finding. Update status as work progresses:
`Open` → `In progress` → `Fixed` → `Verified`, or `Mitigated (verify closed)` /
`Open (verify)` for triage items requiring server-side confirmation, or
`Accepted (residual)` for explicitly accepted risk.

**Due** is derived from 00 §2 SLA (from 2026-11-10): Critical 7d → 2026-11-17,
High 14d → 2026-11-24, Medium 30d → 2026-12-10, Low 60d → 2027-01-09,
Info best effort → `—`.

> **2026-11-16 note (rewritten — an earlier version of this note wrongly
> claimed the Critical SLA had lapsed):** SLAs are anchored to 2026-11-10, so
> F-01's Critical window (due 2026-11-17) has not formally lapsed as of today —
> but F-01 remains `Mitigated (verify closed)`, the verification (single
> spaced `nmap -Pn -p 3306 <host>` + confirm bind/user) is still outstanding,
> and it gates this register's audit-complete criterion ("zero Critical
> open"). Dates are not being re-based silently; treat them as anchored to
> 2026-11-10.

**Task** column: `SEC-*` files under `specs/tasks/` do not exist yet — these
are forward-references; create stubs at implementation start. Shared
tasks are intentional: SEC-007 covers T-07 + F-04..F-06 (XSS hardening),
SEC-009 covers R-01 + R-05, SEC-011 covers R-04 + F-03, SEC-014 covers F-01 + T-05.
Beyond SEC-025, SEC-026 (CORS), SEC-027 (DB portability) and SEC-028 (MCP/API-key
surface, added 2026-11-16) are registered here so each finding has a unique
home; shared tasks are marked explicitly.

| ID | Phase | Sev | CVSS | Title | Component | Status | Owner | Due | Task |
|---|---|---|---|---|---|---|---|---|---|
| R-01 | 07 | High | 7.4 | Frontend host sends no security headers | frontend host (LiteSpeed) | Open | owner | 2026-11-24 | SEC-009 (shared w/ R-05) |
| R-02 | 05 | Medium | 5.3 | Swagger UI + OpenAPI spec publicly exposed | backend-rs (docs.rs/router) | Open | owner | 2026-12-10 | SEC-010 |
| R-03 | 07 | Low | 2.6 | SPA catch-all returns 200 for arbitrary paths | frontend host | Open | owner | 2027-01-09 | SEC-017 |
| R-04 | 05 | Info | 0.0 | CSRF 403 `instance` field is "unknown unknown#unknown" | backend-rs (error.rs/CsrfLayer) | Open | owner | — | SEC-011 (shared w/ F-03) |
| R-05 | 07 | Info | 0.0 | Frontend has no HSTS (covered by R-01) | frontend host | Open | owner | — | SEC-009 (shared w/ R-01) |
| R-06 | 06 | Info | 0.0 | Cert CN = API host (www variant on the shared host's default); SAN covers both — monitoring only | infra/TLS | Open | owner | — | — |
| R-07 | 05 | Low | 3.1 | BREACH potential (HTTP compression on responses) | both hosts (LiteSpeed) | Open | owner | 2027-01-09 | — |
| R-08 | 06 | Info | 0.0 | No OCSP stapling | both hosts (LiteSpeed) | Open | owner | — | — |
| R-09 | 06 | Info | 0.0 | SNI mandatory (no-SNI returns shared-host cert) | both hosts (LiteSpeed) | Open | owner | — | — |
| T-01 | 02 | High | 7.5 | SESSION_SECRET may be < 32 bytes (16 bytes seen in release/.env) | config/secret | Open (verify) | owner | 2026-11-24 | SEC-001 |
| T-02 | 05 | High | 7.5 | CORS_ORIGINS may fall back to localhost default in prod | config/CORS | Open (verify) | owner | 2026-11-24 | SEC-026 |
| T-03 | 05 | High | 7.5 | TRUSTED_PROXIES unset → single global throttle bucket | config/throttle | Open (verify) | owner | 2026-11-24 | SEC-002 |
| T-04 | 05 | Medium | 5.9 | Proxy may rewrite Host → breaks CSRF fallback | app-proxy.js | Open (verify) | owner | 2026-12-10 | SEC-013 |
| T-05 | 06 | High | 8.1 | DB password may be weak/default (`password`) | config/DB | Open (verify) | owner | 2026-11-24 | SEC-014 (shared w/ F-01) |
| T-06 | 06 | High | 7.5 | release/ + .env may be in HTTP docroot | hosting | Open (verify) | owner | 2026-11-24 | SEC-015 |
| T-07 | 04 | Low | 2.6 | Mermaid securityLevel not pinned (relies on default; installed mermaid 12.3.0 defaults to 'strict') | frontend/mermaid | Open (verify) | owner | 2026-12-10 | SEC-007 (shared w/ F-04..F-06) |
| F-01 | 06 | **Critical** | 9.1 | MySQL/MariaDB (11.4.x) exposed to the public internet on port 3306 | infra/MySQL | Mitigated (verify closed) | owner | 2026-11-17 (verify) | SEC-014 (shared w/ T-05) |
| F-02 | 06 | Info | 0.0 | Prod DB is MariaDB 11.4.x (runbook assumed 10.6.x) — portability targets differ | infra/MySQL | Open | owner | — | SEC-027 |
| F-03 | 05 | Medium | 5.3 | `capture_request_context` runs inside CSRF/CORS layer → `instance` = "unknown unknown#unknown" on 403 (R-04 root cause) | backend-rs/lib.rs middleware order | Open | owner | 2026-12-10 | SEC-011 (shared w/ R-04) |
| F-04 | 07 | Low | 2.6 | Mermaid `securityLevel` not explicitly set (installed mermaid 12.3.0 defaults to 'strict') | frontend/mermaid-diagram.tsx | Open | owner | 2026-12-10 | SEC-007 (shared w/ T-07, F-05, F-06) |
| F-05 | 07 | Low | 3.1 | DOMPurify applied to markdown source — ineffective **and corrupts tag-like prose** (`use <Foo>`, `<placeholder>`); real protection is `rehypeRaw → rehypeSanitize` | frontend/markdown-renderer.tsx | Open | owner | 2027-01-09 | SEC-007 (shared w/ T-07, F-04, F-06) |
| F-06 | 07 | Info | 0.0 | Card markdown preview allows `input` element (checkbox) — verify intended | frontend/markdown-preview.tsx | Open | owner | — | SEC-007 (shared w/ T-07, F-04, F-05) |
| F-07 | 11 | Medium | 5.0 | MCP / API-key surface (Wave 3) is unaudited by this folder: bearer pipeline, key minting, scope enforcement, `/mcp` mount, `API_KEY_SECRET` handling, `kp_live_` literals | specs/mcp/ (design) + backend-rs (implementation) | Open | owner | — | SEC-028 |

## Columns

- **ID:** R-xx (recon) / T-xx (triage, server-verify) / F-xx (found during audit).
- **Phase:** file number in this folder.
- **Sev:** Critical/High/Medium/Low/Info.
- **CVSS:** v3.1 base score.
- **Status:** Open / In progress / Fixed / Verified / Mitigated (verify closed) /
  Open (verify) / Accepted (residual).
- **Task:** `SEC-xxx` reference once raised (task files do not exist yet under
  `specs/tasks/` — forward-references only). New tasks beyond SEC-025
  (SEC-026 CORS, SEC-027 DB portability) are registered here so each
  finding has a unique home; shared tasks are marked explicitly.

## Verified positives (not findings)

Documented in `01-reconnaissance-findings.md` §2.5/§2.6. Summary:
- API security headers present on every response (incl. 401/403/404); HSTS 365d
  on API.
- CORS exact origin + credentials verified; hostile origin rejected.
- CSRF cross-site → 403 verified.
- TLS 1.2+1.3, Let's Encrypt, valid chain; testssl.sh grade A+ both hosts.
- No source maps, no `.env` exposure, no account enumeration via login message.
- gitleaks: 0 committed secrets (358 commits).
- ffuf: no real file exposures.
- Ownership predicates present on all user-scoped resources; foreign/missing → 404.
- Input DTOs contain only allowed fields (no mass assignment).
- sqlx `?` bindings everywhere; no `format!`-built SQL.
- Error renderer genericizes 500/503; no SQL/secret/driver text in bodies.
- No `dangerouslySetInnerHTML`, no `eval`, no open redirect.
- Frontend API client uses `credentials: 'include'`; no tokens in localStorage.
- Card markdown preview restricts protocols to http/https/mailto.

## Template for new findings

```
| F-XX | NN | Sev | X.X | Title | component | Open | owner | due | SEC-XXX |
```

Include: reproduction steps, evidence (request/response), affected versions, and
remediation in the task file. Do not store secrets or real user data in this file.

## Cross-references (overlap with the arch-hardening plan)

Items tracked here whose canonical owner lives elsewhere (fix once, in one
place; the other cites it). Amended 2026-11-16.

- T-01 / QW-1 / QW-6, T-06 → `T018` + `specs/arch-hardening-specs/40-operations.md` §5
- QW-10 (auth-event audit logging) → `T025`; product-facing audit trail → Wave 2
  collab activity-log task (`specs/arch-hardening-specs/30-collaboration.md` §5, row G)
- QW-12 (cargo/npm audit) → `SEC-018` + CI `cargo deny check` (amended 2026-11-16 — was `T024`, a metrics task)
- MCP / API-key surface (F-07) → `11-mcp-api-key-surface.md` (SEC-028, added 2026-11-16)
- Backup/restore drill (SEC-022) → `T011` + `docs/operations-backup.md` (single owner)
- Regression tests in CI (10-remediation §1.2) → `T017`
- No home in arch-hardening (this folder is the owner): R-01/R-02/R-03/R-04/R-05/R-07
  (host headers, Swagger gating, SPA 404, CSRF instance) — SEC-009/SEC-010/SEC-011/SEC-017.
