# T008: Make backend-rs API only

## Goal

`backend-rs` stops serving any frontend asset. It serves **only**: `/health`, `/ready`, `/api/v1/*`,
the deprecated `/api/*` dual-mount, `/api-docs/openapi.json`, `/swagger-ui*`, and `/metrics` (its own
listener, opt-in). Every other path — `/`, `/boards`, `/index.html`, `/assets/*`, anything else —
returns the shared problem+json 404 (`application/problem+json`, `type=…/not-found`, non-empty
`instance`, `x-request-id`, `served-by: rust`), never HTML. There is no `ServeDir`, no SPA fallback,
no `public/` directory, no Content-Security-Policy and no inline-script hash computation.

The SPA is deployed to its own origin (`https://kanpad.raziur.com`) by the static host (Apache/LiteSpeed
+ `frontend/public/.htaccess`) and calls the API cross-origin at `https://api-kanpad-rs.raziur.com` via
`VITE_API_URL`, baked at build time.

Rationale: the CSP existed solely because Rust served the document (`10-architecture.md` §E4 — "the Rust
service serves the SPA itself"). With no document there is nothing to protect and no policy to emit —
which is also what removes the production CSP that blocked the Google-hosted Inter font. No font download
and no self-hosting is needed; that is a separate, deferred hardening task.

Sub-goals:

1. Delete the SPA service and the CSP from `lib.rs`/`main.rs`, plus every helper only they use.
2. Replace the SPA fallback with an API-only JSON-404 fallback (same `AppError::NotFound` shape as an
   `/api` miss).
3. Drop the dependencies/features that existed only for the SPA (`tower-http/fs`, `tokio/fs`,
   `sha2::Sha256` — keep `Sha512` for the session key).
4. Strip the SPA from the image and from CI (`COPY public/`, the `frontend-dist` artifact pipeline, the
   SPA contract gates, the `/app/public/index.html` assert).
5. Make the deployment real: two origins, documented build sequence, `frontend/.env.example`.
6. Pin all of it with tests: no CSP on any response, and every non-API path is problem+json 404.

## Depends On

None. T008 is independent of T006. **T007 depends on this task** (it fixes the CORS/CSRF/throttle
behaviour that only becomes real once the SPA is cross-origin).

## Phase

0

## Critical

Yes. This is a production topology change that deliberately removes a security header (CSP) and deletes
a served surface. It must land as one reviewed unit: a half-applied change leaves either two SPAs (the
static host and the Rust binary) or an API that still emits a CSP with no document to protect.

## Spec References

- `specs/api-rewrite-rs-specs/10-architecture.md` — the security-headers paragraph (`:112`), the
  middleware-order block, the router-composition notes (`:266-269`) and the `tower-http` pinned-feature
  row (`:29`, which lists `fs` + the ServeDir trailing-slash behaviour).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — problem+json is the **only** error shape; the
  `not-found` slug and `instance`.
- `specs/api-rewrite-rs-specs/11-rewrite-deltas.md` §E4 — the CSRF + CSP on the SPA-serving origin
  amendment; superseded by this task for the CSP half (CSRF stays, T007 depends on it).
- `specs/api-rewrite-rs-specs/50-tasks.md` — T05 (SPA fallback AC), T56 (CSP), T67 (`ServeDir`
  trailing-slash test).
- `specs/tasks/T007-b-e-r-s-production-proxy-c-o-r-s-error-shape-hardening.md` — consumes this task.
- `AGENTS.md` — "a gate that has never executed is not a gate"; verify every claim against the tree.

## Files to Create/Modify

### Create

- `docs/deployment.md` — two-origin topology: what is deployed where, the build sequence for both halves
  (`npm run build` in `frontend/` → upload `dist/`; `cargo build --release` → API), the
  `frontend/dist/.htaccess` check (Vite copies `public/`), the pointer to the env table T007 adds, and
  the post-deploy smoke list.
- `frontend/.env.example` — `VITE_API_URL=https://api-kanpad-rs.raziur.com`. The real `.env` is
  git-ignored, so without this the production API URL exists only on the developer's machine.

### Modify

- `backend-rs/src/lib.rs` — delete the SPA service and the CSP: the `ServeDir` import (L47),
  `SPA_INDEX_PATH` (L82), `inline_script_hashes` (L89-120), `boot_inline_script_hashes` (L135-140),
  `content_security_policy` (L162-179), `base64_standard`/`sha256`/`find_bytes`/`has_src_attribute` (L181-237), `create_app_with_csp`
  (fold its body into `create_app`), the SPA service (L297-309), the CSP `SetHeaderLayer` (L416-438),
  `spa_index` (L925-948), and the hash/base64 unit tests. Replace `.fallback_service(spa)` with
  `.fallback(api_only_fallback)`. Rewrite the module + `create_app` doc comments (they currently promise
  an SPA fallback and an out-of-band CSP hash). **Also scrub inline comments that mention the CSP's
  `connect-src` / the SPA (e.g. in `cors_layer` and the route-order note).**
- `backend-rs/src/main.rs` — import `create_app` only; delete the hash compute/log block (~L152-166) and
  its now-unused imports.
- `backend-rs/Cargo.toml` — drop `fs` from `tower-http` (L37) and from `tokio` (L16); fix the stale
  "Neither tower-http layer is used" comment. `sha2` stays (`Sha512` for the session key); `Sha256` goes.
- `backend-rs/tests/api/health.rs` — replace `spa_fallback_serves_index_html_for_non_api_miss`
  (L344-391) with `non_api_path_is_problem_json_404` (`/`, `/boards`, `/index.html`,
  `/assets/app.js` × GET/HEAD/POST → 404 problem+json, non-empty `instance`, `x-request-id`,
  `served-by: rust`, no `<html` in the body). Add the CSP-absence assertion to
  `assert_transport_headers` so every existing suite becomes a guard for free. Remove the now-unused
  `PUBLIC_LOCK` mutex and `use std::sync::Mutex` (they only existed for the SPA test's cwd/public writes).
- `backend-rs/tests/api/cors.rs` — delete `csp_of`, `directive`, `SPA_INDEX_FIXTURE` and the five CSP
  tests (L508-653); retarget `one_canonical_list_feeds_cors_cross_origin_and_csp` →
  `…_and_csrf` (CORS echo + CSRF trusted origin, no CSP); import `create_app` only. **Also scrub the
  module doc (L31-34) which describes a CSP guard.**
- `backend-rs/tests/api/tower_http_07.rs` — delete the `ServeDir` trailing-slash test (L91-102) and its
  doc bullet; keep the compression-406 half.
- `backend-rs/tests/fixtures/spa-index.html` — delete.
- `backend-rs/Dockerfile` — remove `COPY --chown=app:app public/ ./public/` (L73-74); rewrite the SPA
  contract comment block (L21-24) to state the API-only contract.
- `backend-rs/.dockerignore` — remove `public/` from the COPY list (L11, L40-41) and the "frontend dist
  copied in by CI" paragraph.
- `.github/workflows/backend-rs.yml` — delete the `frontend-dist` upload (L591-595), the populate step
  (L619-623), the `grep 'COPY .*public/'` gate (L652); replace the `/app/public/index.html` assert
  (L701-702) with `test ! -e /app/public`. Keep `vite build` as the frontend gate and the
  `openapi.json` assert.
- `frontend/public/.htaccess` — add a comment that `/api` is deliberately **not** proxied here (the SPA
  calls the API origin directly); rewrite rules unchanged.
- `README.md` — deployment section: static SPA + API subdomain, `VITE_API_URL`, two build outputs.
- `specs/api-rewrite-rs-specs/10-architecture.md` — replace the CSP paragraph with "no CSP: the API serves
  no document"; remove the `ServeDir`/SPA-fallback lines; drop `fs` from the `tower-http` feature row.
- `specs/api-rewrite-rs-specs/11-rewrite-deltas.md` — mark §E4's CSP sub-section as superseded by T008
  (annotate; do not rewrite history).
- `specs/api-rewrite-rs-specs/50-tasks.md` — annotate T05's SPA-fallback AC, T56's CSP and T67's
  `ServeDir` test as superseded by T008.
- `BACKEND-RS-ORCHESTRATION.md` — scrub the two ServeDir/public references (L580, L881); add a one-line
  note that the Rust cache paths and DATABASE_URL guidance still apply.
- `specs/api-rewrite-rs-specs/54-rust-idiom-hardening.md` — annotate the T56 CSP sub-section as
  superseded by T008 (do not rewrite history).

## Implementation Steps

TDD throughout: prove the defect with a test/curl first, then fix.

### Step 0 — Reproduce (evidence for the report)

```bash
# (a) the API currently serves the SPA
curl -sSI http://127.0.0.1:3001/ | grep -iE "content-type|content-security-policy"
# observed: content-type: text/html; charset=utf-8 · content-security-policy: …
grep -rn "ServeDir\|content_security_policy\|create_app_with_csp\|SPA_INDEX_PATH" backend-rs/src | wc -l
# observed: > 0 — the inventory that must reach 0
```

### Step 1 — `lib.rs`: delete the SPA service and the CSP

1. Delete the imports/definitions listed in §Files (`ServeDir`, `SPA_INDEX_PATH`, `inline_script_hashes`,
   `boot_inline_script_hashes`, `content_security_policy`, `base64_standard`, `sha256`, `find_bytes`,
   `has_src_attribute`, `create_app_with_csp`, `spa_index`), and the hash/base64 unit tests.
2. Fold `create_app_with_csp`'s body into `create_app` and remove the CSP layer block, so `create_app`
   no longer takes a policy or a hash list.
3. Replace `.fallback_service(spa)` with `.fallback(api_only_fallback)`:
   ```rust
   /// API-only fallback: every unmatched path is the shared problem+json 404.
   /// There is no static file service and no SPA rewrite — the frontend is
   /// deployed to its own origin (docs/deployment.md).
   async fn api_only_fallback() -> impl IntoResponse {
       AppError::NotFound.into_response()
   }
   ```
4. Keep the `/api` and `/api/v1` JSON-404 fallbacks (they preserve `instance`/`x-request-id` parity for
   `/api` vs `/api/v1` misses).
5. Rewrite the module doc and the `create_app` doc comments so they describe an API-only service.

### Step 2 — `main.rs`: drop the hash path

- Import `create_app` only; delete the `script_hashes`/`tracing::info!` block (~L152-166) and its
  now-unused imports (`boot_inline_script_hashes` via the crate root, `std::path` if unused). Boot no
  longer touches `public/index.html`.

### Step 3 — `Cargo.toml`: drop the SPA-only features

- `tower-http` features lose `fs`; `tokio` features lose `fs`; the stale "Neither tower-http layer is
  used" comment is corrected to record that `fs` was removed in T008.
- `sha2` stays (Sha512 for the session key); `Sha256` leaves with the CSP.
- Verify: `cargo build --locked` succeeds after the feature removal (confirms `tokio/fs` and
  `tower-http/fs` are not required transitively), and `cargo tree -e features -i tower-http | grep -w fs`
  is empty.

### Step 4 — Tests

- `health.rs`: `non_api_path_is_problem_json_404` (GET/HEAD/POST × `/`, `/boards`, `/index.html`,
  `/assets/app.js`) → 404, `content-type: application/problem+json`, `type` ends `/not-found`, non-empty
  `instance`, `x-request-id`, `served-by: rust`, body contains no `<html`. Add the CSP-absence assertion
  to `assert_transport_headers`. Remove `PUBLIC_LOCK` + `use std::sync::Mutex`.
- `cors.rs`: remove the CSP helpers/tests; retarget `one_canonical_list_feeds_cors_cross_origin_and_csp`
  → `…_and_csrf`. **Also, in the same edit:** delete the `SPA_INDEX_FIXTURE` const and its `include_str!`,
  delete `csp_of`/`directive` if they become unused, and strip the CSP `connect-src` assertions from the
  renamed canonical-list test (it currently calls `create_app_with_csp`/`inline_script_hashes`). Leaving any
  of these behind fails to compile once `create_app_with_csp` is gone.
- `tower_http_07.rs`: delete the `ServeDir` trailing-slash test + doc bullet; keep the compression-406
  half.
- `serve_boot.rs`: update the boot-sequence comment at L98 ("sweeper → CSP hashes → bind") to
  "sweeper → bind".

### Step 5 — Docker + CI

- `Dockerfile`: drop `COPY --chown=app:app public/ ./public/` and rewrite the SPA-contract comment block
  (L21-24) to state the API-only contract.
- `.dockerignore`: drop `public/` from the COPY list and rewrite the "frontend dist copied in by CI"
  paragraph (L40-41) so it no longer references the SPA.
- `.github/workflows/backend-rs.yml`: delete the `frontend-dist` upload, the populate step, the
  `grep 'COPY .*public/'` gate; **replace it with an inverted guard** `! grep -Eq 'COPY .*public/'` so a
  future re-add of the SPA copy fails CI; replace the `/app/public/index.html` assert with
  `test ! -e /app/public`. Keep `vite build` (frontend gate) and the `openapi.json` assert. `docker-e2e`
  keeps `needs: [backend, web]` as a build gate — only the **artifact transfer** goes.

### Step 6 — Deploy surface

- `frontend/.env.example` (new), `.htaccess` comment, `docs/deployment.md` (new), `README.md`.
- Verify `npm run build` produces `frontend/dist/.htaccess` (Vite copies `public/`).

### Step 7 — Specs + lessons

- `10-architecture.md`, `11-rewrite-deltas.md`, `50-tasks.md` (annotate, don't rewrite history).
- `AGENTS.md` lessons (§Notes).

## Constraints

- **No route is removed except the SPA fallback.** Swagger UI + `/api-docs/openapi.json` stay in
  production; `openapi.json` and `frontend/src/lib/api/schema.d.ts` are unchanged.
- **No CSP in this task, and no CSP added to `.htaccess`.** Self-hosted font + static-host CSP is a
  separate, deferred follow-up (see §Notes 1).
- **Cookie attributes unchanged.** Cross-origin works because both hosts are same-site (`raziur.com`) and
  `SameSite=Lax` + `credentials: 'include'` already deliver `__Host-kanpad.sid` to the API host. Do not
  add `SameSite=None`.
- **No new dependency.** No replacement static-file server of any kind.
- Playwright's `:3000`/`:5173` harness is out of scope (T41/T50) — but no test may assume Rust serves
  the SPA.
- Commands per `BACKEND-RS-ORCHESTRATION.md` (`CARGO_HOME`, `CARGO_TARGET_DIR`, `DATABASE_URL`).

## Acceptance Criteria

- [ ] `grep -rn "ServeDir\|SPA_INDEX_PATH\|content_security_policy\|create_app_with_csp" backend-rs/src`
      returns empty.
- [ ] `tower-http` `fs` and `tokio` `fs` absent from `Cargo.toml`; `Sha256` gone, `Sha512` retained.
- [ ] No API response carries `content-security-policy` (shared transport assertion + direct test).
- [ ] `/`, `/boards`, `/index.html`, `/assets/app.js` × GET/HEAD/POST → `404 application/problem+json`,
      `type=…/not-found`, non-empty `instance`, `x-request-id`, `served-by: rust`, no `<html` in body.
- [ ] `/health`, `/ready`, `/api/v1/*`, `/api/*`, `/api-docs/openapi.json`, `/swagger-ui/` unchanged.
- [ ] Image has no `/app/public`; running container answers `/` with problem+json 404.
- [ ] CI has no `frontend-dist`→`public` copy, no `COPY public/` gate, no `/app/public/index.html` assert.
- [ ] `frontend/dist/.htaccess` exists after `npm run build`.
- [ ] `frontend/.env.example` documents `VITE_API_URL=https://api-kanpad-rs.raziur.com`.
- [ ] `docs/deployment.md` lands with the two-origin topology + build sequence + smoke list.
- [ ] Gates: `cargo fmt --check` 0 · `cargo clippy --workspace --all-targets -- -D warnings` 0 · suites
      green on MySQL 8.4 **and** MariaDB 10.6 · `cargo sqlx prepare --check` (`SQLX_OFFLINE=true`) 0 ·
      `openapi.json` + `frontend/src/lib/api/schema.d.ts` unchanged.

## Notes

### 1. Why API-only (and why the font stops being a problem)

The CSP existed solely because Rust served the document (`10-architecture.md` §E4: "the Rust service
serves the SPA itself"). No document ⇒ no CSP ⇒ the `font-src 'self' data:` violation disappears and
Inter keeps loading from `fonts.gstatic.com`. Deferred follow-up: self-host the woff2 **and** add a CSP to
`.htaccess` with the inline theme-script hash (`frontend/index.html:12-21`). Neither belongs in this
task — the whole point is that the API no longer owns the document.

### 2. Same-site ≠ same-origin (cookie + CSRF mechanics)

`kanpad.raziur.com` → `api-kanpad-rs.raziur.com` is cross-origin but same-site, so `SameSite=Lax` +
`credentials: 'include'` (`frontend/src/lib/api/client.ts:70`) still deliver `__Host-kanpad.sid` (scoped to
the API host). But the CSRF `Origin`/`Host` fallback can no longer match (`Host` is the API subdomain),
so `CORS_ORIGINS` becomes load-bearing — exactly what T007 hardens. T008 must not touch cookie
attributes.

### 3. `.htaccess` has no `/api` rule (deliberately)

`frontend/public/.htaccess` rewrites everything to `/index.html` and does **not** proxy `/api`. An API
call aimed at the static host returns HTML. Therefore `VITE_API_URL` must be baked at build time; do
**not** "fix" this with a proxy rule — that restores the same-origin coupling this task removes.

### 4. Dropping `tower-http/fs` is safe

`utoipa-swagger-ui` 10.0.2 has no `tower-http` dependency (verified against the local registry), so
Swagger UI serves its own embedded assets. The only `tower-http::fs` consumer was `ServeDir`.

### 5. The `ServeDir` trailing-slash test must die with `ServeDir`

Keeping `tower_http_07.rs`'s trailing-slash half would pin a service that no longer exists. The
compression-406 half is unrelated and stays.

### 7. Decisions recorded (review 2026-11-09)

- `docs/deployment.md` ownership: **T008 creates the base topology, T007 extends** it with the env table
  and proxy contract.
- Spec files (`10-architecture.md`, `11-rewrite-deltas.md`, `50-tasks.md`, `BACKEND-RS-ORCHESTRATION.md`,
  `54-rust-idiom-hardening.md`): **annotate in the same commit**, do not rewrite history.
- `frontend/.env.example`: include the production value plus a **commented dev default**
  (`# VITE_API_URL=http://localhost:3001`).
- CI: **add the inverted guard** `! grep -Eq 'COPY .*public/'`.
## Lessons

See `specs/tasks/T007-b-e-r-s-production-proxy-c-o-r-s-error-shape-hardening.md` → "Lessons migrated from AGENTS.md".
