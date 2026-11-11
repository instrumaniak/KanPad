# 07 — Frontend Security

**Objective:** Secure the SPA: XSS via markdown/Mermaid, clickjacking, open
redirects, bundle hygiene, and host-level headers.

**Scope:** `frontend/src/**` (markdown rendering, mermaid, routing, API client),
frontend host config (LiteSpeed/Apache).

---

## 1. Test plan

### 1.1 XSS (stored + reflected)
- Card title/description, note body, label names, project/board names with
  payloads (see 04). Render and inspect DOM for script execution.
- DOMPurify: verify applied to all markdown-derived HTML; config reviewed
  (no `ADD_TAGS` for script/iframe/style; no `ALLOW_DATA_ATTR` unless needed).
- `react-markdown`: verify no `rehype-raw` (or sanitized if used).
- Mermaid: verify `securityLevel` is `strict` or `sandbox` (not `loose`);
  test `click` directives and HTML in labels.

### 1.2 Clickjacking
- Frontend sends `X-Frame-Options: DENY` or CSP `frame-ancestors 'none'`.
  Currently missing (R-01) — fix at host config.

### 1.3 Open redirect
- Router navigation from query params (`?redirect=`, `?next=`, `?returnTo=`) →
  verify only same-origin/relative paths accepted; no `//evil.com`.

### 1.4 Bundle hygiene
- Search built JS for secrets, API keys, internal URLs, source maps.
- `npm audit` (frontend) — triage.
- Verify no `.map` files deployed (verified: none real).

### 1.5 Host-level headers
- Add HSTS, CSP, X-Frame-Options, nosniff, referrer-policy to frontend host.
- CSP for SPA: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
  img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' https://<api-origin>;
  frame-ancestors 'none'; base-uri 'self'; form-action 'self'`.

### 1.6 Client-side storage
- No auth tokens in localStorage (verified: only theme/sidebar). Confirm no
  session data in sessionStorage/cookies accessible to JS (HttpOnly verified).

### 1.7 Prototype pollution / ReDoS
- Review any client-side parsing of untrusted input (search, filters).

## 2. Live findings so far
- Frontend sends no security headers (R-01). testssl.sh confirms no HSTS,
  no X-Frame-Options, no CSP, no nosniff on the frontend host. ⚠
- TLS grade A+ on frontend host (testssl.sh). ✓
- No source maps deployed. ✓
- No auth tokens in localStorage. ✓

## 2.5 Code review findings (white-box)
- **F-04 (Medium):** Mermaid `securityLevel` not explicitly set in
  `mermaid-diagram.tsx` — relies on mermaid default. Set to `'strict'`.
- **F-05 (Low):** `markdown-renderer.tsx` calls `DOMPurify.sanitize(content)` on
  the markdown source (ineffective; DOMPurify expects HTML). Real protection is
  `rehypeSanitize`. Remove the misleading call.
- **F-06 (Info):** Card markdown preview (`markdown-preview.tsx`) allows the
  `input` element with type/checked/disabled — verify intended (checklist-in-md).
- Positives: no `dangerouslySetInnerHTML`, no `eval`, no open redirect; card
  preview restricts protocols to http/https/mailto; no source maps. ✓

## 3. Remediation guidance
- Add host-level headers (Apache/LiteSpeed config or `.htaccess`).
- Set mermaid `securityLevel: 'strict'`.
- Audit DOMPurify config; tighten if needed.
- Add open-redirect guard in router.

## 4. Exit criteria
- All 1.1–1.7 pass.
- Host headers verified live.
- `npm audit` triaged.

## 5. Task mapping
- `SEC-009` frontend host security headers.
- `SEC-007` XSS hardening (shared with 04).
- `SEC-017` open-redirect + bundle audit.