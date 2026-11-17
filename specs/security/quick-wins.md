# Quick Wins — Fix This Sprint

High-value, low-effort items. Work in this order. Each becomes a `SEC-*` task.

| # | Action | Effort | Severity | Owner |
|---|---|---|---|---|
| QW-0 | **Done (mitigated) — verify closed:** MySQL port 3306 was publicly reachable (banner grabbed); owner removed remote access. Verify: `nmap -Pn -p 3306 <host>` → filtered/closed. | 30m | Critical | owner |
| QW-1 | Regenerate `SESSION_SECRET` ≥ 32 bytes (64 recommended): `openssl rand -hex 32`, update prod `.env`, restart (invalidates all sessions). | 1h | High | owner |
| QW-2 | Set `CORS_ORIGINS=https://<frontend-origin>` in prod env (canonical form); verify preflight. | 15m | High | owner |
| QW-3 | Set `TRUSTED_PROXIES` to proxy CIDR; verify proxy overwrites (not appends) `X-Forwarded-For`. | 1h | High | owner |
| QW-4 | Fix `app-proxy.js` to forward original `Host` unchanged (or document). | 30m | Medium | owner |
| QW-5 | Rotate `DB_PASSWORD` from `password`; create least-privilege DB user; restrict MySQL bind to localhost/socket. | 2h | High | owner |
| QW-6 | Confirm `release/` + zips outside HTTP docroot; `chmod 600 .env`. | 30m | High | owner |
| QW-7 | Add HSTS + CSP + X-Frame-Options + nosniff + referrer-policy to frontend host (LiteSpeed/Apache). | 1h | Medium | owner |
| QW-8 | Decide on Swagger in prod: gate behind auth or disable; implement. | 30m | Low | owner |
| QW-9 | Verify mermaid `securityLevel` is `strict`/`sandbox`. | 30m | Low | owner |
| QW-10 | Add audit logging for auth events (login success/fail, register, logout). | 1d | Medium | owner |
| QW-11 | ~~Run `gitleaks`/`trufflehog` on repo history; remediate any hits.~~ **Done** — gitleaks clean (0 leaks, 358 commits; see 01 §2.5). | 1h | High (if hit) | owner |
| QW-12 | Run `cargo audit` + `npm audit`; triage. | 2h | Medium | owner (→ SEC-018, amended 2026-11-16) |
| QW-13 | Fix CSRF 403 `instance` field to include method+path+request-id (F-03: move `capture_request_context` outer). | 30m | Medium | owner |
| QW-14 | Set Mermaid `securityLevel: 'strict'` explicitly (F-04 — severity downgraded 2026-11-16: installed mermaid 12.3.0 defaults to 'strict'). | 15m | Low | owner |
| QW-15 | Remove ineffective `DOMPurify.sanitize` on markdown source in `markdown-renderer.tsx` (F-05) — or document it. Note it also corrupts prose that looks like HTML tags (`use <Foo>`); real protection is `rehypeRaw → rehypeSanitize` (keep that order). | 15m | Low | owner |

## Server-side verification checklist (T-01..T-07)

Run manually on the production server (single commands, spaced — no automated
scanning, provider blocks IPs):
1. `cat release/.env` (or wherever prod env lives) — check SESSION_SECRET length, CORS_ORIGINS, TRUSTED_PROXIES, DB_PASSWORD, APP_ENV.
2. `ls -la` on `.env` (permissions).
3. `ss -lntp` / `netstat` — confirm 3001 and 3306 not publicly bound.
4. `find <docroot> -name "*.env" -o -name "*.zip" -o -name "*.bak"` — confirm no leaks.
5. `grep -rn "securityLevel" frontend/src` — confirm mermaid setting.
6. Check LiteSpeed/Apache vhost config for frontend headers.

## Cross-references (arch-hardening plan owns several of these)

Fix once, in one place; this folder cites it. (Amended 2026-11-16.)

- QW-1 / QW-6 → `T018` + `specs/arch-hardening-specs/40-operations.md` §5
  (rotation runbook; the same SESSION_SECRET rotation is documented there).
- QW-10 / QW-12 → `T025` / `SEC-018` (monitoring runbook / audit triage —
  amended 2026-11-16: QW-12 was `T025`/`T024`, a metrics task); the product audit trail
  is Wave 2 collab row G (`30-collaboration.md` §5).
- MCP / API-key surface → never audited by this folder until now: phase
  `11-mcp-api-key-surface.md` (SEC-028, added 2026-11-16) — see findings-register
  F-07.
- Backup/restore drill (09 §1.5) → `T011` + `docs/operations-backup.md` (single owner).
- Regression tests in CI (10-remediation §1.2) → `T017` (Playwright + browsers in CI).

## Prerequisite note

Active testing here assumes a local stack. The `compose.yaml` stack is currently
broken (arch-hardening R3/T012) — until T012 lands, run `backend-rs` + `vite`
directly, or treat runtime testing as gated on T012.
