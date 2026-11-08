# T007: BE-RS: production proxy, CORS & error-shape hardening

## Goal

Three production-side defects that the MariaDB incident (T006) brought to light. Each is independently
observable on the live deployment today; together they make the service fragile under normal traffic and
inconsistent with its own contract.

1. **The proxy collapses every client into one throttle bucket.**
   `scripts/app-proxy.js:50` **overwrites** `x-forwarded-for` with its own peer address
   (`clientReq.socket.remoteAddress` = `127.0.0.1`, because LiteSpeed is the only caller), and
   `TRUSTED_PROXIES` is empty (`config.rs`, `default_trusted_proxies`), so `ClientIpExtractor` (`lib.rs`) keys every request on
   `127.0.0.1`. The governors are therefore **process-global, not per-IP**: measured live on 2026-11-02,
   **the 6th login attempt from anywhere in the world 429s for everyone** for the rest of the minute
   (default `throttle_auth_per_min = 5`, `throttle_global_per_min = 120` — see `default_throttle_auth` /
   `default_throttle_global` in `config.rs`). One
   busy user, one scanner, or one brute-force attempt can lock the whole site out.
2. **Production CORS still points at localhost.** The live trusted-origin set is
   `http://localhost:5173`, i.e. `CORS_ORIGINS` was never changed from the dev default. Since T008 the SPA
   is deployed to its own origin (`https://kanpad.raziur.com`) and calls this API cross-origin, so the
   canonical `CORS_ORIGINS` list feeds **two** consumers (the CORS allowlist and the CSRF trusted-origin
   set — `10-architecture.md:132`), and a wrong value blocks every mutating call from the browser. It is
   also *silently* wrong: nothing at boot rejects a loopback origin in production.
3. **Body/extractor rejections bypass the problem+json kernel on the auth routes.**
   `auth/routes.rs:69,141` take raw `Json<T>` while every other feature uses the project's `ValidJson`
   (`extract.rs:150` struct, `:174-180` `from_request`). Result (measured): `POST /api/auth/register` with `{}` returns
   `422 text/plain`, not `400 application/problem+json` — the only endpoints where a client cannot get a
   machine-readable error. Note the **ordering trap** that makes the naive fix wrong (see §Notes 2).

Also in scope, because it is the same "the proxy must not fight the app" contract:
`scripts/app-proxy.js:49,84` **rewrite `Host`** to `127.0.0.1:<port>`, which breaks the CSRF layer's
`Origin`/`Host` fallback that `10-architecture.md:134` explicitly says production must preserve, and
`:51` sets `x-forwarded-proto` from `clientReq.socket.encrypted` (always `false` behind LiteSpeed's TLS
termination), so a plain-HTTP hop is advertised as plain HTTP.

End state: **the app sees the real client IP and the real public host, a production boot refuses a
loopback CORS origin, every rejection on every route renders problem+json, and the statically hosted SPA
can call the API cross-origin from a browser with no CORS or CSRF rejection.**

## Depends On

- **T008 (hard dependency, lands first).** T008 makes `backend-rs` API-only: `ServeDir`, the SPA fallback
  and the CSP are gone before this task starts. T007's CORS work is therefore genuinely cross-origin, and
  there is no CSP to update — do **not** re-add one, and do not move one into `frontend/public/.htaccess`
  (that is a separate, deferred task with a self-hosted font).
- T006 (recommended order, not a code dependency): ship the MariaDB fix first so login works in
  production, then verify this task's per-client throttling and CORS behaviour against a working login.
  T007's own code changes are independent of T006.

## Phase

0

## Critical

Yes. The shared throttle bucket is a site-wide denial-of-service with a 5-requests-per-minute trigger;
and after T008 the SPA calls this API cross-origin, so a stale `CORS_ORIGINS` blocks every mutating call
from the browser.

## Spec References

- `specs/api-rewrite-rs-specs/10-architecture.md`
  - CORS section `## CORS (restrictive, dev-only open)` (line ~126) and the middleware-order block: the
    canonical-origin rule (one list → **two** consumers: CORS + CSRF, `:132`) and the **deployment
    caveat** (`:134`): "*the `Origin`/`Host` fallback degrades if a reverse proxy rewrites `Host` … the
    runbook must require `Host` to be forwarded unchanged*". Today's proxy violates it — and after T008 the
    fallback cannot rescue a wrong allowlist, because the SPA's `Host` is a different subdomain.
  - T008's amendments remove the CSP row and the `ServeDir`/SPA-fallback rows from this document; this
    task adds nothing back (no CSP, no SPA fallback).
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
- **Same-site ≠ same-origin.** `kanpad.raziur.com` and `api-kanpad-rs.raziur.com` share the registrable
  domain `raziur.com`, so `SameSite=Lax` still lets the browser attach `__Host-kanpad.sid` (scoped to the
  API host) to a cross-origin `fetch` with `credentials: 'include'` (`frontend/src/lib/api/client.ts:70`,
  already set). Cross-origin still applies to CORS, so the allowlist is load-bearing.

## Files to Create/Modify

### Create

- `backend-rs/tests/api/proxy_headers.rs` — the `ClientIpExtractor` policy matrix through the real router:
  trusted peer + XFF chain, untrusted peer (spoof ignored), trusted peer with no XFF, and two distinct XFF
  clients getting **independent** buckets (the assertion that would have caught the outage).

### Modify

- `backend-rs/.env.example` — the `TRUSTED_PROXIES` comment currently states the **leftmost**
  `X-Forwarded-For` entry wins and that a trusted proxy "MUST overwrite that header rather than append to
  it" — both are the exact opposite of this task's rightmost-walk extractor and append-only proxy. Rewrite
  it to describe the right-to-left walk, and state that the proxy appends.
- `docs/deployment.md` — **created by T008** (two-origin topology + build sequence); this task extends it
  with the required production environment (one table: `APP_ENV`, `CORS_ORIGINS`, `TRUSTED_PROXIES`,
  `SESSION_SECRET`, `HASH_COST`, `DB_*`, `DB_STARTUP_CHECK`, `REGISTRATION_ENABLED`, `PORT`) + the proxy
  contract (which headers LiteSpeed/Node must forward) + the post-deploy smoke list.

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
- `backend-rs/tests/api/cors.rs` — extend `one_canonical_list_feeds_cors_cross_origin_and_csrf` and add the
  CSP-absence guard (Step 2).
- `backend-rs/tests/api/health.rs` — add the CSP-absence assertion to `assert_transport_headers` (Step 2/5).
- `backend-rs/src/lib.rs` — `ClientIpExtractor` rightmost-walk rewrite (Step 1 item 3).
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
  `x-request-id` and `served-by: rust` — the contract that the bodyless 500 path must not silently lose
  observability. It must **not** assert a `content-security-policy` header: since T008 the API serves no
  document and emits no policy. This must **not** go in `health.rs`: that suite is entirely `#[tokio::test]`
  and DB-free (`build_state(test_settings())`, no pool), so it cannot host a `#[sqlx::test]`. `auth_session.rs`
  already owns the per-test pool, `app_with_env`, `session_ids` and `login_fresh_session`.
- `frontend/public/fonts/inter.woff2` and `frontend/src/index.css` — **removed from this task.** T008 deletes
  the CSP that blocked the font, so Inter keeps loading from Google's CDN with no change. Self-hosting the
  font + a static-host CSP is a separate, deferred follow-up (see T008 §Notes 1).
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

# (b) prod CORS/CSRF still points at dev — the SPA origin is not allowed
curl -sS -i -X OPTIONS https://api-kanpad-rs.raziur.com/api/auth/login \
  -H 'Origin: https://kanpad.raziur.com' \
  -H 'Access-Control-Request-Method: POST' | head -8
# observed: no access-control-allow-origin for the real SPA origin

# (b2) the API carries no CSP at all — the T008 contract, pinned here
curl -sSI https://api-kanpad-rs.raziur.com/health | grep -ci "content-security-policy"
# observed: 0

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
   can walk the XFF list right-to-left and skip them. This only works if LiteSpeed appends the peer IP
   (see the hard requirement in the `docs/deployment.md` proxy contract, Step 5); in "Keep Header from
   Trusted IP" (pass-through) mode the spoofed client entry survives and the throttle stays global.

3. **App-side spoof resistance (new, required):** change `ClientIpExtractor` (`lib.rs`) from "leftmost
   XFF when peer is trusted" to "walk the XFF list **right-to-left**, skipping entries that are in
   `TRUSTED_PROXIES`, and take the first non-trusted entry" (the same algorithm `tower_governor`'s
   `SmartIpKeyExtractor` uses). With one trusted hop this resolves to the real client even when a spoofed
   entry was prepended; an untrusted peer ignores XFF entirely. Document this in `docs/deployment.md`
   together with the operator verification: `curl` an echo endpoint and print the raw `X-Forwarded-For`
   to confirm LiteSpeed appended (not passed through).

### Step 2 — P1: production CORS correctness

- `Settings::validate()`: add the loopback-origin rejection for `APP_ENV=production` (error names
  `CORS_ORIGINS` and the entry). The guard keys on the **loopback host regardless of scheme** — i.e. it
  rejects `http://localhost:*`, `https://localhost:*`, `http://127.0.0.1:*`, `https://127.0.0.1:*`,
  `http://[::1]:*` and `https://[::1]:*` — because a loopback origin is never a legitimate production
  SPA origin in any scheme. Test both directions: prod + `http://localhost:5173` → `Err`; prod +
  `https://localhost:5173` → `Err`; dev/test + the same value → `Ok` (the dev default must keep
  working).
- Include the canonical origins in the boot-config log line (`Settings::log_boot_config`) so
  `journalctl`/cPanel logs answer "what CORS list is live?" without a redeploy.
- Set the real value in production: `CORS_ORIGINS=https://kanpad.raziur.com`. This is now a genuine
  cross-origin allowlist entry (the SPA is served from `https://kanpad.raziur.com`, the API from
  `https://api-kanpad-rs.raziur.com`), so CORS echoes it and the CSRF trusted set gains it. The cookie
  still flows because both hosts are same-site (`raziur.com`) — see Spec References.
- `backend-rs/tests/api/cors.rs` owns the canonical-list guard. Extend
  `one_canonical_list_feeds_cors_cross_origin_and_csrf` (renamed by T008: the CSP consumer is gone): under
  production settings the CORS echo is the canonical origin, the CSRF trusted set contains it, and
  **neither contains `localhost`**. Assert per-consumer via a preflight + a cross-origin POST, never by
  substring over a header blob.
- Add the **CSP-absence** guard that T008's contract needs: no production response carries
  `content-security-policy` (`/health`, `/api/v1/*`, a problem+json 404). T008 adds this to
  `health.rs::assert_transport_headers`; extend it here for the production-settings app used by the CORS
  tests.
- Do **not** re-add a CSP, and do not move one into `frontend/public/.htaccess` — the static-host CSP plus
  a self-hosted font is a separate, deferred task.

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

Also verify the `#[tracing::instrument(skip(state, session, payload))]` attribute on both handlers does **not**
record the now-`Result<…>` payload (the repo already has a CI grep gate for body recording; the skip list must
follow the rename from `input` to `payload` — a stale `input` entry would silently record the body).

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

### Step 5 — Contracts, docs, deploy

- The `DROP TABLE sessions` middleware-5xx test (§Files, `auth_session.rs`) pins that *even* a bodyless 5xx
  keeps `x-request-id` and `served-by: rust`. It must **not** assert a `content-security-policy` header:
  since T008 the API serves no document and emits no policy.
- `docs/deployment.md` (created by T008): extend it with the required env table + the proxy contract + the
  post-deploy smoke:
  - **Proxy contract (hard requirements, not hints):** LiteSpeed must be in an **append** mode
    ("Trusted IP Only" or "Use Last IP (for AWS ELB)") so the peer IP is appended to
    `X-Forwarded-For`; **never** "Keep Header from Trusted IP" (pass-through), which leaves a
    client-supplied spoofed entry as the selected one. If LiteSpeed runs on a non-loopback interface, that
    interface's IP must be added to `TRUSTED_PROXIES`. Record the confirmed mode in the task log.
  - **Proxy-header verification is a manual staging gate** (no automated harness for `scripts/app-proxy.js`
    in this task): capture the raw forwarded headers (`Host`, `X-Forwarded-For`, `X-Forwarded-Proto`,
    `X-Forwarded-Host`) for both the HTTP and the WebSocket upgrade path and record them in the task log.
  1. `curl -sSI …/health | grep -ci content-security-policy` → `0` (T008's contract) and the header
     `served-by: rust` present;
  2. `OPTIONS /api/v1/auth/login` with `Origin: https://kanpad.raziur.com` → ACAO + credentials;
     with `Origin: https://evil.example` → no ACAO;
  3. six logins from **two** different clients (or two XFF values in staging) → each gets 5, the 6th of
     one does **not** 429 the other;
  4. `POST /auth/register` with `{}` → `400` problem+json;
  5. `GET https://api-kanpad-rs.raziur.com/boards` → `404 application/problem+json` (no HTML — proves the
     proxy is not serving a stale SPA).
- `10-architecture.md` (+ `BACKEND-RS-ORCHESTRATION.md` pointer): the proxy contract and the boot guard.
- `AGENTS.md`: lessons in §Notes 4.

## Constraints

- **Scope:** `scripts/app-proxy.js`, `config.rs` (CORS guard + boot log), `auth/routes.rs`,
  `backend-rs/.env.example` (TRUSTED_PROXIES comment), and the test files `throttle.rs`,
  `auth_register.rs`, `auth_session.rs`, `cors.rs`, `health.rs`, `proxy_headers.rs` + docs. No feature
  behaviour change, no route change, no `openapi.json` change, and **no** SPA/CSP work — T008 removed it.
- **No new dependency.** `tower_governor`, `tower-http`, `axum`, `serde` suffice.
- **Do not disable or loosen** the CSRF layer, the CORS allowlist, the governor, or `SameSite` to make the
  proxy simpler. The fix is to send correct headers.
- **Do not make `TRUSTED_PROXIES` permissive** (no CIDR wider than the single loopback hop) — the extractor
  is only as safe as the edge's header handling.
- **The auth handler order is a contract:** `registration_enabled` check → parse → normalize → validate.
  Moving validation before normalization silently breaks padded/mixed-case logins.
- **No credential/PII in logs.** The client IP is already logged/used as a throttle key; keep it out of
  request bodies and problem documents.
- **The wrapper is production-only.** It runs behind LiteSpeed's TLS termination, so `x-forwarded-proto`
  falls back to `https` when the header is absent; running it without TLS termination would mislabel
  requests. Gate the fallback on an explicit env (e.g. `PROXY_ASSUME_HTTPS`) if a non-TLS staging box is
  ever needed.
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
- [ ] Production `CORS_ORIGINS=https://kanpad.raziur.com` is echoed by CORS and trusted by CSRF for a
      cross-origin `POST` from `https://kanpad.raziur.com`; no `localhost` origin is trusted; the boot log
      prints the canonical list.
- [ ] No API response carries `content-security-policy` (T008's API-only contract holds after this task).
- [ ] `POST /api/auth/login` and `/register` with a missing field / malformed JSON / over-limit body return
      `400` + `application/problem+json` (`invalid-json`) + `instance`; a wrong content-type returns `415`
      — never `422 text/plain`.
- [ ] A padded, mixed-case email still authenticates (normalize-before-validate preserved).
- [ ] A session-store failure (simulated by dropping `sessions` in a per-test DB) still returns a 5xx that
      carries `x-request-id` and `served-by: rust`.
- [ ] Staging smoke (five checks in Step 5) recorded in the task log.
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

$ curl -sS -i -X OPTIONS https://api-kanpad-rs.raziur.com/api/auth/login \
    -H 'Origin: https://kanpad.raziur.com' -H 'Access-Control-Request-Method: POST'
HTTP/2 200
# no access-control-allow-origin: the real SPA origin was never in CORS_ORIGINS

$ curl -sS -i -X POST https://kanpad.raziur.com/api/auth/register -H 'content-type: application/json' -d '{}' | head -4
HTTP/2 422
content-type: text/plain; charset=utf-8
```

Root cause of (a): `scripts/app-proxy.js:50` sets `'x-forwarded-for': clientReq.socket.remoteAddress` —
it *replaces* the list instead of appending. Behind LiteSpeed that value is always `127.0.0.1`, and
`TRUSTED_PROXIES` is unset, so `ClientIpExtractor` returns the peer (`127.0.0.1`) for every request. Both
governors then have exactly **one** bucket for the entire site (see `default_throttle_global` /
`default_throttle_auth` in `config.rs`: global 120/min,
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

It doesn't need to: the Rust service has no vhost routing (one `Router`). The rewrite
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
2. **One canonical origin list feeds CORS and CSRF.** A stale `CORS_ORIGINS` is wrong in both consumers at
   once and fails *silently* — the browser reports a CORS error while the server logs a normal 200. Check
   the preflight, not the env file. Fail boot on a loopback origin in production.
3. **Proxies must forward `Host` unchanged** whenever the app implements an `Origin`/`Host` CSRF fallback
   (`10-architecture.md:134` says so; the committed proxy violated it since it was written).
4. **A 422 `text/plain` on one route family is an extractor bypass, not a validation bug.** The kernel's
   problem+json guarantee is only as wide as the extractors that route through `AppError`.
5. **Fixing an extractor must preserve each handler's documented ordering** (normalize → validate for
   auth). `ValidJson` is the house pattern *except* where a handler normalizes first — and a test with a
   padded email is what proves it.

### 5. After the split: same-site, cross-origin (topology that makes this task real)

The SPA (`https://kanpad.raziur.com`) and the API (`https://api-kanpad-rs.raziur.com`) are **same-site**
(registrable domain `raziur.com`) but **cross-origin**. Consequences, in the order they bite:

1. **The cookie still flows.** `__Host-kanpad.sid` is scoped to the API host, `SameSite=Lax` is satisfied
   (same-site request), and `frontend/src/lib/api/client.ts:70` already sends `credentials: 'include'`.
   Do **not** relax to `SameSite=None` — that gives up the browser-level CSRF defence for nothing.
2. **The CSRF `Origin`/`Host` fallback can no longer match.** `Host` is `api-kanpad-rs.raziur.com` while
   `Origin` is `https://kanpad.raziur.com`. With `Sec-Fetch-Site: same-site` present the layer demands a
   **trusted** origin, so `CORS_ORIGINS` is the only thing standing between a legitimate SPA and a 403.
   That is why a stale value is a total outage of every mutating call, and why the boot guard exists.
3. **CORS is load-bearing, not decorative.** T55's preflight guard covers the production topology: a verb
   missing from `allow_methods` blocks the browser before the request is sent.

### 7. Decisions recorded (review 2026-11-09)

- Loopback CORS boot guard keys on the **loopback host regardless of scheme** (http + https).
- Proxy-header verification is a **manual staging gate** (no Node harness added in this task).
- LiteSpeed mode is a **deploy precondition**: must append the peer IP; "Keep Header from Trusted IP"
  (pass-through) is not acceptable.
- `#[tracing::instrument]` skip list follows the rename: `skip(state, session, payload)`.
