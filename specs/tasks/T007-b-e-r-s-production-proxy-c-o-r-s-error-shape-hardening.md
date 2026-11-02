# T007: BE-RS: production proxy, CORS & error-shape hardening

## Goal

Three production-side defects that the MariaDB incident (T006) brought to light. Each is independently
observable on the live deployment today; together they make the service fragile under normal traffic and
inconsistent with its own contract.

1. **The proxy collapses every client into one throttle bucket.**
   `scripts/app-proxy.js:50` **overwrites** `x-forwarded-for` with its own peer address
   (`clientReq.socket.remoteAddress` = `127.0.0.1`, because LiteSpeed is the only caller), and
   `TRUSTED_PROXIES` is empty (`config.rs:695`), so `ClientIpExtractor` (`lib.rs`) keys every request on
   `127.0.0.1`. The governors are therefore **process-global, not per-IP**: measured live on 2026-11-02,
   **the 6th login attempt from anywhere in the world 429s for everyone** for the rest of the minute
   (default `throttle_auth_per_min = 5`, `throttle_global_per_min = 120` — `config.rs:1433-1434`). One
   busy user, one scanner, or one brute-force attempt can lock the whole site out.
2. **Production CORS/CSP still points at localhost.** The live CSP is
   `connect-src 'self' http://localhost:5173`, i.e. `CORS_ORIGINS` was never changed from the dev default.
   The canonical `CORS_ORIGINS` list feeds **three** consumers (CORS allowlist, CSRF trusted origins, CSP
   `connect-src` — `10-architecture.md:132`), so a wrong value is wrong in three places at once. It is
   also *silently* wrong: nothing at boot rejects a loopback origin in production.
3. **Body/extractor rejections bypass the problem+json kernel on the auth routes.**
   `auth/routes.rs:69,141` take raw `Json<T>` while every other feature uses the project's `ValidJson`
   (`extract.rs:150` struct, `:174-180` `from_request`). Result (measured): `POST /api/auth/register` with `{}` returns
   `422 text/plain`, not `400 application/problem+json` — the only endpoints where a client cannot get a
   machine-readable error. Note the **ordering trap** that makes the naive fix wrong (see §Notes 2).

4. **Production CSP blocks the Inter web font.** The backend emits
   `Content-Security-Policy: … font-src 'self' data: …` (see `backend-rs/src/lib.rs::content_security_policy`),
   but `frontend/src/index.css` loads Inter from Google's CDN
   (`https://fonts.gstatic.com/s/inter/v18/…UcCO3Fwr…woff2`). In dev the SPA is served by Vite, which sends
   **no** CSP, so the font loads; in production the Rust backend serves the SPA and attaches the strict CSP to
   the document, so the cross-origin font request is blocked and the UI falls back to system fonts. The defect
   is invisible locally because the dev topology never enforces the production CSP on the document (same
   "production-only enforcement" class as defect 2). It is **not** fixed by relaxing `font-src` (that would
   weaken the CSP and keep an external runtime dependency) — it is fixed by **self-hosting** the font so
   `font-src 'self'` already covers it.

Also in scope, because it is the same "the proxy must not fight the app" contract:
`scripts/app-proxy.js:49,84` **rewrite `Host`** to `127.0.0.1:<port>`, which breaks the CSRF layer's
`Origin`/`Host` fallback that `10-architecture.md:134` explicitly says production must preserve, and
`:51` sets `x-forwarded-proto` from `clientReq.socket.encrypted` (always `false` behind LiteSpeed's TLS
termination), so a plain-HTTP hop is advertised as plain HTTP.

End state: **the app sees the real client IP and the real public host, a production boot refuses a
loopback CORS origin, every rejection on every route renders problem+json, and the Inter font loads under
the production CSP.**

## Depends On

T006 (recommended order, not a code dependency): ship the MariaDB fix first so login works in production,
then verify this task's per-client throttling and CORS behaviour against a working login. T007's own code
changes are independent of T006.

## Phase

0

## Critical

Yes. The shared throttle bucket is a site-wide denial-of-service with a 5-requests-per-minute trigger.

## Spec References

- `specs/api-rewrite-rs-specs/10-architecture.md`
  - CORS section `## CORS (restrictive, dev-only open)` (line ~126) and the middleware-order block: the
    canonical-origin rule (one list → three consumers, `:132`) and the **deployment caveat** (`:134`):
    "*the `Origin`/`Host` fallback degrades if a reverse proxy rewrites `Host` … the runbook must require
    `Host` to be forwarded unchanged*". Today's proxy violates it.
  - Config table (`CORS_ORIGINS` at `:201`, `TRUSTED_PROXIES` at `:203`) — precedence, fail-fast contract.
- `specs/api-rewrite-rs-specs/10-architecture.md` also owns the `invalid-json` type-URI registry (line ~82);
  `20-api-contract.md` says parse/validation → 400 problem+json but does not name the slug.
- `specs/api-rewrite-rs-specs/20-api-contract.md` — problem+json is the **only** error shape; `x-request-id`
  on every response. (The `invalid-json` slug itself lives in 10-architecture.md's type-URI registry.)
- `specs/api-rewrite-rs-specs/50-tasks.md` — T30 (throttling), T49/T56 (CORS + CSRF), T11 (auth).
- `AGENTS.md` — "operator-facing text must only carry verified facts"; CI-gate discipline; Rust test-harness
  ownership (`tests/api/helpers.rs` is the in-process plumbing owner; import explicitly).
- `scripts/app-proxy.js` — the committed prod wrapper (the `release/` copy is a build artifact and is
  git-ignored; edit the `scripts/` one).

External references (verified 2026-11-02):

- `X-Forwarded-For` semantics (de-facto convention, documented by MDN / AWS ALB / most reverse proxies;
  RFC 7239 defines the `Forwarded` header, §5.2 "Forwarded For", and *allows* a proxy to remove the field
  entirely — it does NOT mandate append-only): the list is append-only by convention; entries are added by
  each hop. **LiteSpeed's documented behaviour is to APPEND the peer IP to an existing `X-Forwarded-For`**
  in most modes (LSWS "Keep Header from Trusted IP" = identical to "Trusted IP Only" *except* that the
  backend header is **not** modified to include the peer; in the other modes the peer is appended). It does
  **not** replace by default — so a client-supplied leftmost entry can survive to the backend, and the
  current leftmost-wins extractor is spoofable. The Node wrapper is hop 2 and must append, not overwrite.
- `tower-http` (0.7 — the repo pins `"0.7"`; the `csrf` module shipped in 0.7.0, there is no 0.6.x
  `CsrfLayer`) `CsrfLayer` uses the request's **effective host** (the `Host` header) for the
  `Origin`/`Host` fallback — rewriting `Host` to `127.0.0.1:PORT` makes a legitimate same-origin `POST`
  look cross-origin whenever `Sec-Fetch-Site` is absent (curl, older browsers, the plain-HTTP staging box).
- `tower_governor` keys are per-`KeyExtractor`; the limiter is in-process memory (already documented as the
  scale-out caveat). Nothing changes there — only the key.

## Files to Create/Modify

### Create

- `backend-rs/tests/api/proxy_headers.rs` — the `ClientIpExtractor` policy matrix through the real router:
  trusted peer + XFF chain, untrusted peer (spoof ignored), trusted peer with no XFF, and two distinct XFF
  clients getting **independent** buckets (the assertion that would have caught the outage).
- `docs/deployment.md` — the required production environment (one table: `APP_ENV`, `CORS_ORIGINS`,
  `TRUSTED_PROXIES`, `SESSION_SECRET`, `HASH_COST`, `DB_*`, `DB_STARTUP_CHECK`, `REGISTRATION_ENABLED`,
  `PORT`) + the proxy contract (which headers LiteSpeed/Node must forward) + the post-deploy smoke list.
  This is the operator-facing artifact that did not exist, which is *why* prod kept the dev CORS value.

### Modify

- `scripts/app-proxy.js` — header handling only (no routing/behaviour change):
  - **stop overwriting** `Host`; forward it unchanged and set `x-forwarded-host` for completeness;
  - **append** the Node peer (`127.0.0.1`) to the incoming `x-forwarded-for` instead of replacing it;
    the proxy must NOT strip a client-supplied list (it cannot reliably tell spoofed from real — that is
    the extractor's job, below);
  - **preserve** the incoming `x-forwarded-proto`; fall back to `https` in production (the wrapper knows
    because LiteSpeed terminates TLS) rather than reporting the plaintext hop;
  - keep `x-forwarded-for` (append, not replace) for the WebSocket upgrade path consistent with the HTTP
    path — it currently rewrites `host` there too (`:84`) and forwards **no** XFF at all, so add it.
- `backend-rs/src/config.rs` — production guard for CORS: `validate()` rejects a **loopback** origin
  (`http://localhost:*`, `http://127.0.0.1:*`, `http://[::1]:*`) when `APP_ENV=production`, with an error
  naming `CORS_ORIGINS` and the offending entry (the existing canonical-form rejection stays as-is). Also
  emit the canonical origin list in the boot-config log so the value is visible without guessing.
- `backend-rs/tests/api.rs` — register the new suite: `#[path = "api/proxy_headers.rs"] mod proxy_headers;`
  (explicit `#[path]`, no bare `mod`, inserted in the existing alphabetical order). A suite file that is
  never registered simply never runs — that is a silent no-op guard.
- `backend-rs/src/lib.rs` — `ClientIpExtractor`: switch from leftmost-XFF to rightmost-non-trusted-walk
  (see Step 1 item 3); keep the `ConnectInfo`-from-extensions lookup and the 0.0.0.0 fallback.
- `backend-rs/src/features/auth/routes.rs` — `login`/`register` take
  `payload: Result<Json<T>, JsonRejection>` and `map_err(AppError::Json)?` **before**
  `normalize_email` + `validate()` (see §Notes 2 — do **not** switch them to `ValidJson`).
- `backend-rs/tests/api/throttle.rs` — extend the existing `throttle_settings(…, trusted_proxies)` harness:
  two clients with distinct `X-Forwarded-For` values behind a trusted peer must exhaust **their own**
  buckets, not a shared one.
- `backend-rs/tests/api/auth_register.rs` **and** `auth_session.rs` — rejection-shape tests through the
  **real** `create_app` router (missing field, malformed JSON, wrong content-type, over-limit body) →
  `400 application/problem+json` `invalid-json` + `instance`; plus the **normalize-before-validate**
  regression (`"  RAZIUR@EXAMPLE.COM  "` still logs in) in `auth_session.rs`. Register cases extend
  `register_full_flow`'s existing 400/409/403 matrix; login cases extend `auth_session_full_flow`.
  **There is no `auth_login.rs`.** `validation.rs` already covers the extractor contract
  (`register_oversized_body_is_400_invalid_json`, `register_malformed_and_missing_content_type`,
  `login_rejects_empty_and_overlong_password`) but only via **probe routes** wired with `ValidJson` — so
  those tests stay green while the real auth routes use raw `Json<T>`. The new cases must go through the
  real router or they prove nothing about this bypass.
- `backend-rs/tests/api/auth_session.rs` — a **second** `#[sqlx::test]` fn (beside
  `auth_session_full_flow`) that removes `sessions` from **its own per-test database**
  (`RENAME TABLE`/`DROP TABLE`; safe here because `#[sqlx::test]` gives every test a private database
  which is discarded afterwards), performs a login, and asserts the middleware-generated 500 still carries
  `x-request-id`, `served-by: rust`, and the CSP header — the contract that the bodyless 500 path must not
  silently lose observability. This must **not** go in `health.rs`: that suite is entirely `#[tokio::test]`
  and DB-free (`build_state(test_settings())`, no pool), so it cannot host a `#[sqlx::test]`. `auth_session.rs`
  already owns the per-test pool, `app_with_env`, `session_ids` and `login_fresh_session`.
- `frontend/public/fonts/inter.woff2` — the Inter variable font (weight 100 900) currently fetched from
  `fonts.gstatic.com`, copied into the build so the backend serves it from the same origin (new).
- `frontend/src/index.css` — change the `@font-face` `src` URL from the Google CDN URL to `/fonts/inter.woff2`.
- `specs/api-rewrite-rs-specs/10-architecture.md` — CORS/CSRF section: the loopback-in-production boot
  rejection; the proxy contract (append XFF, never rewrite `Host`, `TRUSTED_PROXIES` holds exactly the
  proxy hops).
- `BACKEND-RS-ORCHESTRATION.md` + `AGENTS.md` — the prod env checklist pointer and the lessons.

## Implementation Steps

TDD throughout: prove the defect with a test/curl first, then fix.

### Step 0 — Reproduce (evidence for the report)

```bash
# (a) shared throttle bucket — 5/min for the WHOLE site, not per client
for i in $(seq 1 7); do
  curl -sS -o /dev/null -w "attempt $i -> %{http_code}\n" -X POST \
    https://kanpad.raziur.com/api/auth/login -H 'content-type: application/json' \
    -d '{"email":"nobody@example.com","password":"x"}'
done
# observed 2026-11-02: 401 401 401 401 401 429 429

# (b) prod CORS/CSP still points at dev
curl -sSI https://kanpad.raziur.com/health | grep -i "content-security-policy"
# observed: ... connect-src 'self' http://localhost:5173; ...

# (c) auth body rejection is not problem+json
curl -sS -i -X POST https://kanpad.raziur.com/api/auth/register \
  -H 'content-type: application/json' -d '{}' | head -4
# observed: HTTP/2 422 · content-type: text/plain; charset=utf-8
```

Record all three in the final report as the "before" evidence.

### Step 1 — P0: per-client throttle keys (proxy + trusted hops)

1. `scripts/app-proxy.js` — build the forwarded headers explicitly:

   ```js
   const forwardedFor = [clientReq.headers['x-forwarded-for'], clientReq.socket.remoteAddress]
     .filter(Boolean)
     .join(', ');

   const headers = {
     ...clientReq.headers,
     // NEVER rewrite Host: the CSRF Origin/Host fallback depends on it
     // (10-architecture.md:134). The app does not do vhost routing.
     'x-forwarded-for': forwardedFor,
     // LiteSpeed terminates TLS and sets this; keep it, do not report the plaintext hop.
     'x-forwarded-proto': clientReq.headers['x-forwarded-proto'] || 'https',
     'x-forwarded-host': clientReq.headers.host || '',
   };
   ```

   Apply the same forwarding to `server.on('upgrade')` so the WebSocket path cannot drift.

2. Production env: `TRUSTED_PROXIES=127.0.0.1` (exactly the Node hop). Do **not** trust a wider range.
   `TRUSTED_PROXIES` must list **every** proxy hop in the chain (here: 127.0.0.1 only) so the extractor
   can walk the XFF list right-to-left and skip them.

3. **App-side spoof resistance (new, required):** change `ClientIpExtractor` (`lib.rs`) from "leftmost
   XFF when peer is trusted" to "walk the XFF list **right-to-left**, skipping entries that are in
   `TRUSTED_PROXIES`, and take the first non-trusted entry" (the same algorithm `tower_governor`'s
   `SmartIpKeyExtractor` uses). With one trusted hop this resolves to the real client even when a spoofed
   entry was prepended; an untrusted peer ignores XFF entirely. Document this in `docs/deployment.md`
   together with the operator verification: `curl` an echo endpoint and print the raw `X-Forwarded-For`
   to confirm LiteSpeed appended (not passed through).

### Step 2 — P1: production CORS/CSP correctness

- `Settings::validate()`: add the loopback-origin rejection for `APP_ENV=production` (error names
  `CORS_ORIGINS` and the entry). Test both directions: prod + `http://localhost:5173` → `Err`;
  dev/test + the same value → `Ok` (the dev default must keep working).
- Include the canonical origins in the boot-config log line (`Settings::log_boot_config`) so
  `journalctl`/cPanel logs answer "what CORS list is live?" without a redeploy.
- Set the real value in production: `CORS_ORIGINS=https://kanpad.raziur.com` (the SPA is served from the
  same origin, so `connect-src` becomes `'self' https://kanpad.raziur.com`; the CSRF trusted set gains the
  real origin; CORS echoes it).
- Add the CSP assertion to `backend-rs/tests/api/cors.rs`, which already owns every CSP test — extend
  `csp_connect_src_lists_the_cors_allowlist` (and keep `one_canonical_list_feeds_cors_cross_origin_and_csp`
  green): under production settings the emitted `connect-src` must contain the canonical origin and
  **must not** contain `localhost`. Do not re-implement the `csp_of`/`directive` helpers; assert per
  directive, never by substring over the whole policy.

### Step 3 — P0/P1: the extractor policy tests

`tests/api/proxy_headers.rs` (in-process, uses `tests/api/helpers.rs`; import explicitly):

| Case | Setup | Expectation |
|---|---|---|
| trusted peer + list `"203.0.113.7, 127.0.0.1"` | `TRUSTED_PROXIES=127.0.0.1`, `ConnectInfo(127.0.0.1)` | key = `203.0.113.7` (rightmost non-trusted) |
| trusted peer + spoofed list `"1.2.3.4, 203.0.113.7, 127.0.0.1"` | same | key = `203.0.113.7` (spoofed leftmost ignored) |
| trusted peer, no XFF | same peer | key = `127.0.0.1` |
| **untrusted** peer + spoofed XFF | `TRUSTED_PROXIES` empty, peer `198.51.100.9`, `XFF: 1.2.3.4` | key = `198.51.100.9` (spoof ignored) |
| two clients, distinct XFF | trusted peer, two `XFF` values | each gets its **own** `throttle_auth_per_min` budget (the regression for this task) |
| malformed XFF (`"not-an-ip"`) | trusted peer | falls back to the peer address, no 500 |

Extend `tests/api/throttle.rs` with the last row at the HTTP level (429 only after each client's own
budget is exhausted). Do not duplicate plumbing: the existing `throttle_settings`/`throttle_app` helpers
already parameterise `trusted_proxies`.

### Step 4 — P1: problem+json on every auth rejection

`auth/routes.rs` — keep the normalize-before-validate order (**critical**, see §Notes 2):

```rust
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    payload: Result<Json<LoginInput>, JsonRejection>,
) -> AppResult<impl IntoResponse> {
    let Json(mut input) = payload.map_err(AppError::Json)?;   // 400 invalid-json (415 for missing content-type), problem+json
    input.email = normalize_email(&input.email);
    input.validate().map_err(AppError::from)?;
    // …
}
```

Do the same for `register` (its `403` registration-closed check stays **first** in the handler body — a
role-denied 403 must not be turned into a body-parse 400; see the ordering caveat above).

Also verify the `#[tracing::instrument(skip(state, session))]` attribute on both handlers does **not**
record the now-`Result<…>` payload (the repo already has a CI grep gate for body recording; keep
`input`/`payload` in the skip list).

Scan the rest of the tree for the same bypass:
`grep -rnE "\): (Json|Query|Path)<" backend-rs/src/features` — everything must be `ValidJson`/
`ValidQuery`/`Result<…, Rejection>` (features already are; auth is the exception being fixed).

Tests: `{}` / malformed / over-limit body → `400` + `content-type: application/problem+json` +
`type: …/invalid-json` + a non-empty `instance`; a request with the **wrong content-type** → `415`
(`UnsupportedMediaType`, `AppError::Json` maps only `MissingJsonContentType` to 415 today) — update the
acceptance criterion and `20-api-contract.md` accordingly, or deliberately change the mapping. Padded
mixed-case email still authenticates. `openapi.json` snapshot unchanged.

**Ordering caveat (inherent, not a bug):** because parsing happens in the extractor *before* the handler
runs, a malformed body with registration closed returns `400 invalid-json`, not the documented `403`
registration-closed. The `registration_enabled` check is the first statement in the handler body, so it
cannot gate parse failures. Document this in `20-api-contract.md` and do not try to "fix" it by moving
validation before parsing (that would break normalize-before-validate).

### Step 6 — P1: self-host the Inter font (production CSP)

The production CSP is `font-src 'self' data:` and the frontend loads Inter from
`https://fonts.gstatic.com/…`, so the font is blocked in production (and works in dev only because Vite
sends no CSP). Fix by self-hosting — do **not** add `fonts.gstatic.com` to `font-src` (that would loosen
the CSP and keep an external runtime dependency).

1. Download the woff2 currently referenced in `frontend/src/index.css` into
   `frontend/public/fonts/inter.woff2` (verify it is a valid woff2, ~24 KB).
2. Edit `frontend/src/index.css`: change the `@font-face` `src` to
   `url('/fonts/inter.woff2') format('woff2');` (keep `font-weight: 100 900`, `font-display: swap`).
3. Rebuild: `npm run build` in `frontend/` (Vite copies `public/` to `dist/`); the Dockerfile then copies
   `dist/` into `backend-rs/public/`, so the font ships at `/fonts/inter.woff2`.
4. Verify: `curl -sI https://kanpad.raziur.com/fonts/inter.woff2` returns 200 and the browser console no
   longer reports a CSP `font-src` violation; the live CSP stays `font-src 'self' data:` (no loosening).
5. Confirm `frontend/src/index.css` no longer references `fonts.gstatic.com`.

### Step 5 — Contracts, docs, deploy

- The `DROP TABLE sessions` middleware-5xx test (Step 3 list) pins that *even* a bodyless 5xx keeps
  `x-request-id` + `served-by` + CSP.
- `docs/deployment.md`: required env + proxy contract + post-deploy smoke:
  1. `curl -sSI …/health | grep -i content-security-policy` → `connect-src 'self' https://kanpad.raziur.com`;
  2. `OPTIONS /api/v1/auth/login` with `Origin: https://kanpad.raziur.com` → ACAO + credentials;
     with `Origin: https://evil.example` → no ACAO;
  3. six logins from **two** different clients (or two XFF values in staging) → each gets 5, the 6th of
     one does **not** 429 the other;
  4. `POST /auth/register` with `{}` → `400` problem+json.
- `10-architecture.md` (+ `BACKEND-RS-ORCHESTRATION.md` pointer): the proxy contract and the boot guard.
- `AGENTS.md`: lessons in §Notes 4.

## Constraints

- **Scope:** `scripts/app-proxy.js`, `config.rs` (CORS guard + boot log), `auth/routes.rs`, and the four
  test files + docs. No feature behaviour change, no route change, no `openapi.json` change.
- **No new dependency.** `tower_governor`, `tower-http`, `axum`, `serde` suffice.
- **Do not disable or loosen** the CSRF layer, the CORS allowlist, the governor, or `SameSite` to make the
  proxy simpler. The fix is to send correct headers.
- **Do not make `TRUSTED_PROXIES` permissive** (no CIDR wider than the single loopback hop) — the extractor
  is only as safe as the edge's header handling.
- **The auth handler order is a contract:** `registration_enabled` check → parse → normalize → validate.
  Moving validation before normalization silently breaks padded/mixed-case logins.
- **No credential/PII in logs.** The client IP is already logged/used as a throttle key; keep it out of
  request bodies and problem documents.
- Commands per `BACKEND-RS-ORCHESTRATION.md` (`CARGO_HOME`, `CARGO_TARGET_DIR`, `DATABASE_URL`); the
  MariaDB leg from T006 must stay green too (this task touches no SQL).

## Acceptance Criteria

- [ ] Live: the 6th login attempt from client A no longer 429s client B; each client has its own
      `throttle_auth_per_min` budget (proven by the `proxy_headers`/`throttle` tests **and** a two-client
      live check).
- [ ] Untrusted peers cannot spoof `X-Forwarded-For` (unit + integration test).
- [ ] `scripts/app-proxy.js` forwards `Host` unchanged, **appends** to `X-Forwarded-For`, and preserves
      `X-Forwarded-Proto`; the WebSocket path sends the same headers.
- [ ] `APP_ENV=production` + a loopback `CORS_ORIGINS` entry **fails boot** naming `CORS_ORIGINS`; the dev
      default still boots.
- [ ] Production CSP is `connect-src 'self' https://kanpad.raziur.com` (no `localhost`), and the boot log
      prints the canonical origin list.
- [ ] The Inter font loads in production: `/fonts/inter.woff2` returns 200, no `font-src` CSP violation in
      the browser console, and the live CSP remains `font-src 'self' data:` (unchanged — no loosening).
- [ ] `frontend/src/index.css` no longer references `fonts.gstatic.com`.
- [ ] `POST /api/auth/login` and `/register` with a missing field / malformed JSON / over-limit body return
      `400` + `application/problem+json` (`invalid-json`) + `instance`; a wrong content-type returns `415`
      — never `422 text/plain`.
- [ ] A padded, mixed-case email still authenticates (normalize-before-validate preserved).
- [ ] A session-store failure (simulated by dropping `sessions` in a per-test DB) still returns a 5xx that
      carries `x-request-id`, `served-by: rust`, and the CSP header.
- [ ] Staging smoke (four checks in Step 5) recorded in the task log.
- [ ] Gates: `cargo fmt --check` 0 · `cargo clippy --workspace --all-targets -- -D warnings
      -A clippy::pedantic` 0 · suites green on **MySQL 8.4 and MariaDB 10.6** · `cargo sqlx prepare --check`
      with `SQLX_OFFLINE=true` 0 (no SQL changed) · `openapi.json` +
      `frontend/src/lib/api/schema.d.ts` unchanged.
- [ ] Docs landed: `docs/deployment.md`, `10-architecture.md`, `BACKEND-RS-ORCHESTRATION.md`, `AGENTS.md`.

## Notes

### 1. Measured evidence (2026-11-02, live `kanpad.raziur.com`)

```
$ for i in $(seq 1 7); do curl … -X POST …/api/auth/login -d '{"email":"nobody@example.com","password":"x"}'; done
attempt 1 -> 401
attempt 2 -> 401
attempt 3 -> 401
attempt 4 -> 401
attempt 5 -> 401
attempt 6 -> 429        # ← and every other client on the planet is now 429 for the rest of the minute
attempt 7 -> 429

$ curl -sSI https://kanpad.raziur.com/health | grep -i content-security-policy
content-security-policy: … connect-src 'self' http://localhost:5173; …

$ curl -sS -i -X POST https://kanpad.raziur.com/api/auth/register -H 'content-type: application/json' -d '{}' | head -4
HTTP/2 422
content-type: text/plain; charset=utf-8
```

Root cause of (a): `scripts/app-proxy.js:50` sets `'x-forwarded-for': clientReq.socket.remoteAddress` —
it *replaces* the list instead of appending. Behind LiteSpeed that value is always `127.0.0.1`, and
`TRUSTED_PROXIES` is unset, so `ClientIpExtractor` returns the peer (`127.0.0.1`) for every request. Both
governors then have exactly **one** bucket for the entire site (`config.rs:1433-1434`: global 120/min,
auth 5/min).

### 2. The ordering trap in the auth extractor fix

`LoginInput`/`RegisterInput` validate the email with **`validate_email`, a custom garde rule**, and the
handlers deliberately **normalize first** (`normalize_email` = trim + lowercase) and validate second
(`auth/routes.rs:143-144, 74-75`). The obvious fix — swap `Json<T>` for the project's `ValidJson<T>` —
would run `validate_email` on the **raw** input, so `"  RAZIUR@EXAMPLE.COM  "` would start 400ing where it
currently logs in. That is a silent behaviour regression dressed as a lint fix.

Correct shape: `Result<Json<T>, JsonRejection>` in the handler, `?`-mapped through `AppError::Json`, which
keeps (i) the normalize-then-validate order and (ii) the shared problem+json renderer. Regression test:
padded + mixed-case email authenticates.

### 3. Why the proxy rewrites `Host` at all

It doesn't need to: the Rust service has no vhost routing (one `Router`, SPA fallback included). The rewrite
is leftover from a generic reverse-proxy template — and it actively breaks the CSRF `Origin`/`Host`
fallback that the architecture doc calls out as a production requirement. Removing it is a **strict
improvement** with no functional dependence to break.

`x-forwarded-proto` is a similar leftover: `clientReq.socket.encrypted` is `false` (LiteSpeed→Node is
plaintext), so the header currently claims `http` for an `https` request. Nothing in the app consumes it
today (cookies are `Secure` unconditionally in production), which is exactly why it must be fixed now,
before something starts deriving absolute URLs from it.

### 4. Suggested `AGENTS.md` lessons (write these when the work lands)

1. **A reverse proxy that overwrites `X-Forwarded-For` turns a per-IP throttle into a global one.** The
   symptom is not a proxy bug — it is "the whole site 429s after five logins". XFF is append-only, and the
   app must trust it only from a known hop.
2. **One canonical origin list feeds CORS, CSRF and CSP.** A stale `CORS_ORIGINS` is wrong in three places
   at once, and the CSP makes it visible in every response — check the header, not the env file. Fail boot
   on a loopback origin in production.
3. **Proxies must forward `Host` unchanged** whenever the app implements an `Origin`/`Host` CSRF fallback
   (`10-architecture.md:134` says so; the committed proxy violated it since it was written).
4. **A 422 `text/plain` on one route family is an extractor bypass, not a validation bug.** The kernel's
   problem+json guarantee is only as wide as the extractors that route through `AppError`.
5. **Fixing an extractor must preserve each handler's documented ordering** (normalize → validate for
   auth). `ValidJson` is the house pattern *except* where a handler normalizes first — and a test with a
   padded email is what proves it.

### 6. Font / CSP measured evidence (2026-11-03, live `kanpad.raziur.com`)

```
$ curl -sI https://kanpad.raziur.com | grep -i content-security-policy
content-security-policy: … font-src 'self' data:; …

# browser console:
# Loading the font 'https://fonts.gstatic.com/s/inter/v18/…UcCO3Fwr…woff2' violates
# Content Security Policy directive: "font-src 'self' data:".
```

Root cause: the dev server (Vite) sends no CSP, so the defect is invisible locally; production enforces
`font-src 'self' data:` on the document and blocks the cross-origin font. Fix is self-hosting the woff2
(Step 6), not CSP relaxation — keeping `font-src 'self' data:` strict.

Suggested additional lesson (extend §Notes 4): **A CSP is only enforced where it is attached.** A frontend
that loads third-party assets passes in dev (no CSP) and fails in prod (CSP on the document). Verify
against a production-like serve (backend serving the built SPA), not the Vite dev server — and either
self-host the asset or add the origin to the CSP deliberately.

### 5. Out of scope (deliberately)

- Anything MariaDB-related (T006).
- The in-process governor's scale-out caveat (N replicas ⇒ N× the limit) — already documented; a
  Redis-backed limiter is a separate decision.
- Rate-limiting by user id after authentication (the current policy is strictly per-IP, pre-auth).
