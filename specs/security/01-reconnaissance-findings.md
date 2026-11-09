# 01 — Reconnaissance Findings (live)

**Date:** 2026-11-09
**Method:** read-only curl/openssl against live URLs. No destructive actions.
**Targets:** `https://<frontend-origin>` (frontend), `https://<api-origin>` (backend).

**Note (2026-11-09):** The hosting provider blocked our IP after the ffuf/testssl
bursts. **No further automated scanning against the live host.** All remaining
active testing runs against a local `compose.yaml` stack. Live checks are limited
to single, spaced-out manual requests. The recon below is the complete live
dataset; the rest of the audit is white-box + local.

## 0. Provider / WAF constraints (READ BEFORE ANY LIVE CHECK)

The ban was caused by **ModSecurity (mod_security) on the shared LiteSpeed host**,
not by the app. Ban details (for the record):

- Rule: `mod_security (id:420043)`.
- Trigger: 5 hits in 3600s from IP `<redacted-ip>`.
- Timestamp of the logged event: Mon Nov 9 22:39:31 2026.
- Cause: ffuf / testssl / nikto bursts exceeded the WAF threshold.

Rules of engagement derived from this (apply to every future live check):

1. **No automated scanning against the live host.** No ffuf, nikto, sslscan,
   sqlmap, testssl.sh bursts, or any tool that issues many requests in quick
   succession. The WAF counts requests, not "attacks" — even benign paths count.
2. **Live checks = single, spaced-out manual `curl` only.** One request per
   minute minimum; never in a loop; never more than a handful per session.
   Always use `--max-time` and `-servername` for TLS.
3. **Everything destructive / high-volume runs against the local `compose.yaml`
   stack**, where there is no WAF and no IP-ban risk.
4. **If banned again:** unblock via the hosting control panel (or wait out the
   window, typically 1h+). Do not retry while banned — retries extend the ban.
   Document the unblock in this file.
5. **Prefer white-box code review + local compose** for all remaining phases
   (02–10). Live verification is reserved for header/CORS/CSRF/cookie/TLS
   spot-checks that cannot be reproduced locally.

---

## 1. Commands used (reproducible)

```bash
# Headers + body
curl -sS -i --max-time 15 https://<api-origin>/health
curl -sS -i --max-time 15 https://<api-origin>/ready

# CORS preflight (all verbs)
curl -sS -i -X OPTIONS https://<api-origin>/api/v1/boards \
  -H "Origin: https://<frontend-origin>" -H "Access-Control-Request-Method: POST" \
  -H "Access-Control-Request-Headers: content-type"
curl -sS -i -X OPTIONS https://<api-origin>/api/v1/cards/1/labels/1 \
  -H "Origin: https://<frontend-origin>" -H "Access-Control-Request-Method: PUT"

# CSRF
curl -sS -i -X POST https://<api-origin>/api/v1/auth/login \
  -H "Origin: https://evil.example.com" -H "Sec-Fetch-Site: cross-site" \
  -H "Content-Type: application/json" -d '{"email":"a@b.com","password":"x"}'

# Exposure probes
curl -sS -o /dev/null -w "%{http_code}\n" https://<api-origin>/swagger-ui/
curl -sS -o /dev/null -w "%{http_code}\n" https://<api-origin>/.env
curl -sS -I https://<frontend-origin>/

# TLS
echo | openssl s_client -connect <api-host>:443 -servername <api-host> 2>/dev/null \
  | openssl x509 -noout -subject -issuer -dates
```

## 2. Findings

### R-01 — Frontend origin sends no security headers (High)
- **Where:** `https://<frontend-origin>/` (LiteSpeed-served SPA).
- **Evidence:** response headers contain only `server: LiteSpeed` + cache headers. No
  `strict-transport-security`, `content-security-policy`, `x-frame-options`,
  `x-content-type-options`, `referrer-policy`.
- **Impact:** SPA is clickjackable (no frame-ancestors), no MIME-sniff protection,
  no HSTS on the primary origin (the API's HSTS `includeSubDomains` does not cover
  `<frontend-host>`), no CSP. Any XSS in the SPA has no CSP mitigation.
- **Remediation:** add headers in the LiteSpeed/Apache vhost config for the
  frontend host (or via `.htaccess` if cPanel allows): HSTS, CSP (at least
  `frame-ancestors 'none'`, `base-uri 'self'`), `X-Frame-Options: DENY`,
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`.
  Also set CSP `connect-src` to the API origin.

### R-02 — Swagger UI + OpenAPI spec publicly exposed (Medium)
- **Where:** `https://<api-origin>/swagger-ui/` (200),
  `/api-docs/openapi.json` (200).
- **Evidence:** full OpenAPI 3.1 document returned, listing every route and DTO.
- **Impact:** schema disclosure aids attacker reconnaissance (parameter names,
  validation rules, endpoint inventory). Not a vuln by itself, but increases
  attack surface and is unnecessary in production.
- **Remediation:** either disable swagger-ui in prod (`APP_ENV=production` gate),
  or gate `/swagger-ui` + `/api-docs/*` behind admin auth, or accept as low risk
  (needed for typegen). Record the decision.

### R-03 — SPA catch-all returns 200 for arbitrary paths (Low)
- **Where:** `https://<frontend-origin>/<anything>` → `200 text/html` (index.html).
- **Impact:** masks 404s; minor. Could hide misconfigurations. Not exploitable.
- **Remediation:** ensure real 404 for genuinely unknown asset paths; keep SPA
  fallback only for client-side routes. Low priority.

### R-04 — CSRF 403 `instance` field is "unknown unknown#unknown" (Info)
- **Where:** response to cross-site mutating request.
- **Evidence:** `instance":"unknown unknown#unknown"`.
- **Impact:** breaks log correlation / incident triage on CSRF rejections. The
  CsrfLayer short-circuits before the matched path is available.
- **Remediation:** populate `instance` from request method+path+request-id even
  on the CSRF rejection path (or accept, since 403s are rare and logged).

### R-05 — Frontend has no HSTS (Info)
- Covered by R-01. The API host's HSTS `includeSubDomains` does not extend to
  `<frontend-host>`.

### R-06 — Cert CN naming (Info)
- **Evidence:** CN = `<api-host>`, SAN covers both. Fine.
- **Action:** monitor expiry (2027-02-06) and automate renewal.

### R-07 — BREACH potential (HTTP compression on responses) (Low)
- **Where:** both hosts (LiteSpeed `br gzip` compression detected by testssl.sh).
- **Evidence:** testssl.sh reports "potentially NOT ok, br gzip HTTP compression detected"
  on `/` (404).
- **Impact:** theoretical BREACH if a page reflects user input and contains a secret
  (CSRF token, session). API uses problem+json; frontend is a static SPA. Low risk.
- **Remediation:** disable compression on responses containing secrets, or add
  BREACH mitigations (no user-controlled reflection near secrets). Low priority.

### R-08 — No OCSP stapling (Info)
- **Evidence:** testssl.sh reports "OCSP stapling not offered".
- **Impact:** clients must fetch OCSP separately (minor latency/privacy). Low.
- **Remediation:** enable OCSP stapling in LiteSpeed/Apache. Optional.

### R-09 — SNI mandatory (Info)
- **Evidence:** request without SNI returns `*.<shared-host-domain>` cert (shared host
  default). Modern clients use SNI; not exploitable.
- **Remediation:** none required; note for debugging (curl must use -servername).

### F-01 — MySQL/MariaDB publicly exposed on port 3306 (Critical)
- **Where:** `<frontend-host>` / `<api-host>` (both resolve to
  `<redacted-ip>`).
- **Evidence:** `nmap -Pn -p 3306` → open. Banner grab returns
  `MariaDB 11.4.x banner (<auth-plugin>)`. Ports 3000/3001/8080 filtered;
  only 80/443/3306 open externally.
- **Impact:** database is reachable from the internet. If credentials are weak
  (T-05 suggests `password`) or a user has a weak password, full data breach.
  Also exposes MariaDB version and auth plugin.
- **Remediation:** bind MySQL to 127.0.0.1 or a Unix socket; or firewall 3306
  at the host. Verify no remote admin accounts. Rotate any credentials that may
  have been exposed.

### F-02 — Prod DB is MariaDB 11.4.x (runbook assumed 10.6.x) (Info)
- **Evidence:** banner `MariaDB 11.4.x`.
- **Impact:** portability test matrix targets MariaDB 10.6; prod runs 11.4.
  SQL must be re-verified on 11.4 (likely fine, but confirm). Not a security
  issue per se but affects audit confidence.

### Positives verified live (do not re-test)
- API security headers present on every response (incl. 401/403/404).
- CORS: exact origin `https://<frontend-origin>`, credentials true, methods include
  PUT; hostile origin rejected (no ACAO header).
- CSRF: cross-site → 403; no-Origin (curl) → 401 (reaches auth).
- TLS 1.2 + 1.3, Let's Encrypt, valid chain.
- Error bodies are RFC 9457 problem+json, generic detail.
- No source maps, no `.env` exposure, no account enumeration via login message.
- `/health` DB-free liveness, `/ready` reports DB status.

## 2.5 Tool results (testssl.sh / gitleaks / ffuf)

### testssl.sh (TLS audit)
- **API host:** Overall Grade **A+**. TLS 1.2 + 1.3 only, forward secrecy (ECDHE),
  AEAD ciphers, HSTS 365 days + includeSubDomains, cert valid (Let's Encrypt,
  expires 2027-02-06), no Heartbleed/POODLE/BEAST/FREAK/DROWN/LOGJAM/RC4.
  BREACH potential (HTTP compression), no OCSP stapling.
- **Frontend host:** TLS also A+, but **no HSTS** (confirms R-01/R-05). No
  X-Frame-Options/CSP/nosniff (confirms R-01).

### gitleaks (secret scan)
- Git history (358 commits): **0 leaks** (after excluding target/node_modules/release).
- Working tree: 18 hits, all false positives except the weak shared
  `SESSION_SECRET` in `release/.env`, `backend-rs/.env`, `backend/.env`
  (confirms T-01/QW-1). `config.rs:1092` is a test fixture; `target/` and
  `backend/release/4.js` are build artifacts.

### ffuf (directory/file exposure)
- Both hosts: only SPA fallback (200 index.html) / 403 responses. No real file
  leaks (consistent with manual checks). LiteSpeed rate-limited our IP after the
  burst — no further active scanning.

### nikto / sslscan
- Not usable: Nikto 2.1.5 reports "No web server found" (OpenSSL 3.x / SNI
  incompatibility with shared host); sslscan timed out (host rate-limit). testssl.sh
  supersedes both for TLS.

## 2.6 Code review findings (white-box, local)

White-box review of `backend-rs/src/` and `frontend/src/` (no live traffic).

### Positives confirmed
- Ownership predicates (`check_*_owned`) present on all user-scoped resources;
  foreign/missing IDs → 404 (no oracle). ✓
- Input DTOs contain only allowed fields (no `user_id`/`role`/`id` in
  Deserialize inputs) → no mass assignment. ✓
- sqlx `?` bindings everywhere; `ordering` is an allowlisted `match` (not
  user input); LIKE escape is correct (backslash-first). ✓
- Error renderer genericizes 500/503; no SQL/secret/driver text in bodies. ✓
- `SESSION_SECRET` byte-length check uses `.len()` on `&str` (byte length). ✓
- Password byte cap (72) uses byte length. ✓
- No `dangerouslySetInnerHTML`, no `eval`, no open redirect. ✓
- Frontend API client uses `credentials: 'include'`, no token in localStorage. ✓
- Card markdown preview: rehypeSanitize with protocols restricted to
  http/https/mailto. ✓

### Findings
- **F-03 (Medium):** `capture_request_context` middleware is applied INNER to
  CSRF/CORS/Compression (lib.rs), so CSRF 403 rejections fire before the
  task-local `REQUEST_CONTEXT` is scoped → `instance` = "unknown unknown#unknown"
  (live R-04). Fix: move `capture_request_context` to wrap CSRF/CORS.
- **F-04 (Medium):** Mermaid `securityLevel` not explicitly set in
  `mermaid-diagram.tsx` (`mermaid.initialize({ startOnLoad, theme })`) — relies
  on mermaid's default. Set explicitly to `'strict'` (or `'sandbox'`).
- **F-05 (Low):** `markdown-renderer.tsx` calls `DOMPurify.sanitize(content)`
  on the markdown *source* (DOMPurify expects HTML → ineffective). Real
  protection is `rehypeSanitize`. Remove the misleading call or document it.
- **F-06 (Info):** Card markdown preview allows the `input` element (checkbox)
  with type/checked/disabled — verify intended (checklist-in-markdown). Low.

## 3. Pre-audit triage items (require server-side verification)

These cannot be confirmed from outside and must be checked on the server. They are
likely already exploitable if misconfigured.

| # | Item | How to verify | Severity if bad |
|---|---|---|---|
| T-01 | `SESSION_SECRET` ≥ 32 bytes | `wc -c` on the secret value in prod `.env` | High |
| T-02 | `CORS_ORIGINS` set to canonical frontend origin (not localhost default, not `*`) | read prod `.env` + live preflight (done: origin is allowed → likely OK) | High |
| T-03 | `TRUSTED_PROXIES` set so XFF is trusted (else global throttle bucket) | read prod `.env` + check rate-limit behavior | High |
| T-04 | `app-proxy.js` forwards original `Host` (else CSRF fallback broken) | read proxy config / test Origin+Host | Medium |
| T-05 | `DB_PASSWORD` not default/weak; MySQL bound to localhost/socket | read prod `.env` + `ss -lntp` | High |
| T-06 | `release/` + zips outside HTTP docroot; `.env` chmod 600 | filesystem check | High |
| T-07 | Mermaid `securityLevel` strict/sandbox | grep frontend source | Medium |

## 4. Recon commands to re-run after hardening

Re-run section 1 commands; expect:
- Frontend responses carry HSTS/CSP/X-Frame-Options/nosniff/referrer-policy.
- (If swagger gated) `/swagger-ui/` → 401/404.
- CSRF 403 `instance` populated with method+path+request-id.