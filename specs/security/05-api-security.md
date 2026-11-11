# 05 — API Security (CORS, CSRF, Rate Limiting, Headers, Errors)

**Objective:** Verify CORS allowlist, CSRF defense, rate limiting, security
headers on all responses, and error disclosure.

**Scope:** middleware stack (`lib.rs`), CORS config, CsrfLayer, governor,
secure-headers layers, error renderer (`error.rs`), metrics endpoint.

---

## 1. Test plan

### 1.1 CORS (live + code)
- Preflight from `https://<frontend-origin>` for every verb
  (GET/POST/PUT/PATCH/DELETE/OPTIONS) → 200 with ACAO = exact origin,
  credentials true.
- Preflight from a hostile origin → no ACAO header (rejected).
- Verify `Access-Control-Allow-Headers` = `content-type,x-request-id` (no
  `Authorization` — auth is cookie-only).
- Verify `Access-Control-Expose-Headers` = `x-request-id`.
- Empty allowlist in prod → boot fails (fail closed). Omitted key → dev default
  (document this footgun; production must set explicitly).
- Omitted key in prod (no `CORS_ORIGINS` set at all) must also fail boot or be
  pinned to the canonical frontend origin — test the omitted case explicitly,
  not just the empty-string case.
- Non-canonical origin entry → boot error naming the entry.

### 1.2 CSRF
- Cross-site mutating request (different registrable domain) with
  `Sec-Fetch-Site: cross-site` → 403 problem+json.
- Same-site cross-origin (frontend → API, same registrable domain) → allowed because
  origin is in trusted set.
- No-Origin (curl/server-side) mutating request → allowed (no ambient cookie).
- `Origin`/`Host` fallback: verify proxy forwards `Host` unchanged (else fallback
  can never match).
- Safe methods (GET/HEAD/OPTIONS) always allowed.

### 1.3 Rate limiting
- Auth endpoint: 6th request within 1 min → 429 + `Retry-After` + problem+json.
- Global: 121st request → 429.
- `/health`, `/ready`, `/swagger-ui`, `/api-docs/*` are NOT throttled.
- Verify per-IP keying behind proxy (TRUSTED_PROXIES). If all users share one
  bucket → global DoS / brute-force amplification.
- 429 body uses shared `problem_response` constructor (same shape).

### 1.4 Security headers (every response, incl. 404/429/500)
- `x-content-type-options: nosniff`
- `x-frame-options: DENY`
- `referrer-policy: strict-origin-when-cross-origin`
- `served-by: rust`
- `content-security-policy` (API — verified on this branch; **re-verify after
  T007/T008 API-only split, where the API CSP is expected to be removed**)
- `strict-transport-security` (prod)
- No `server` version leak beyond `LiteSpeed` (proxy) — acceptable.
- `x-request-id` present and echoed.

### 1.5 Error disclosure
- 500 responses: generic detail only, no stack trace, SQL text, file paths, or
  secret substrings.
- Pool/timeout → 503 problem+json + `Retry-After`, no internals.
- Body-size limit → 400/413, no panic.
- Verify T05: bad DB password at boot → 503 with no password substring in body
  or logs.

### 1.6 OpenAPI / Swagger exposure
- Decide: disable in prod or gate behind auth. Record decision.
- **Re-check after T007/T008 API-only split:** the router composition may have
  changed; re-verify whether `/swagger-ui` + `/api-docs/*` are still reachable.
- If kept: ensure no sensitive example data in DTOs.

### 1.7 Metrics endpoint
- `METRICS_PORT` unset → no `/metrics` at all (fail-closed). If set, verify it
  is on a separate listener, not publicly exposed, and that scrapes are not
  counted in request metrics (no feedback loop).
- Verify `route` label uses matched pattern (not raw path) — cardinality.

## 2. Live findings so far
- CORS exact origin + credentials verified (preflight 200). ✓
- CSRF cross-site → 403 verified. ✓
- Security headers present on all API responses (incl. 401/403/404). ✓
- HSTS present on API (`max-age=31536000; includeSubDomains`). ✓
- Swagger UI + openapi.json publicly reachable (R-02). ⚠
- CSRF 403 `instance` = "unknown unknown#unknown" (R-04). ⚠
- **F-03 (Medium, code review):** root cause — `capture_request_context` is
  applied INNER to CSRF/CORS/Compression in `lib.rs`, so CSRF rejections fire
  before the task-local `REQUEST_CONTEXT` is scoped. Fix: move it outer (wrap
  CSRF/CORS).
- testssl.sh: API TLS grade A+, HSTS 365d + includeSubDomains, BREACH potential
  on `/` (R-07), no OCSP stapling (R-08). ✓/⚠

## 3. Remediation guidance
- Frontend host headers: separate task (file 07).
- Swagger: gate or disable via `APP_ENV` check in router composition.
- Metrics: ensure `METRICS_PORT` unset in prod unless scraped by a trusted
  collector; firewall the port.
- Fix CSRF `instance` population.

## 4. Exit criteria
- All 1.1–1.7 pass.
- Regression tests in CI for CORS preflight (all verbs), CSRF 403, 429, header
  presence.

## 5. Task mapping
- `SEC-009` frontend security headers (file 07).
- `SEC-010` Swagger/OpenAPI exposure decision + implementation.
- `SEC-011` CSRF instance field fix.
- `SEC-012` metrics exposure review.
- `SEC-026` CORS_ORIGINS prod configuration verification (T-02).