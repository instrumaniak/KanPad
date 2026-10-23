//! backend-rs: KanPad API rewrite (Axum 0.8 + sqlx 0.8 + MySQL, single crate).
//!
//! Composition root for in-process tests. [`create_app`] builds the router;
//! [`build_state`] builds state. This module NEVER spawns the session-expiry
//! sweeper — only `main.rs` does (see 10-architecture.md Sessions).

pub mod admin;
pub mod common;
pub mod config;
pub mod db;
pub mod docs;
pub mod error;
pub mod extract;
pub mod features;
pub mod metrics;
pub mod state;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    error_handling::HandleErrorLayer,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use docs::{HealthResponse, ReadyResponse};
use error::{AppError, ProblemDetails, capture_request_context, handle_timeout};
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256, Sha512};
use state::AppState;
use tower::ServiceBuilder;
use tower_governor::{
    GovernorError, GovernorLayer, governor::GovernorConfigBuilder, key_extractor::KeyExtractor,
};
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    csrf::{CsrfLayer, ProtectionError},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tower_sessions::{
    Expiry, SessionManagerLayer,
    cookie::{Key, SameSite},
};
use utoipa_swagger_ui::{Config as SwaggerConfig, SwaggerUi};

/// Request-id header name (tower-http keeps its own copy private).
const X_REQUEST_ID: header::HeaderName = header::HeaderName::from_static("x-request-id");

/// Derive the 64-byte cookie [`Key`] tower-sessions requires from
/// `SESSION_SECRET` (spec: `≥ 32` bytes, so it is NOT always 64 bytes long).
/// SHA-512 expansion is deterministic (same secret → same key on every
/// instance/restart) and collision-free in practice, so a wrong/pre-cutover
/// secret still fails HMAC verification → anonymous → 401 (the T11
/// force-re-login proof). (`Key::from` panics below 64 bytes and cookie's
/// `key-expansion` HKDF is not forwarded by tower-sessions 0.14, hence the
/// explicit hash step.)
fn session_key(settings: &config::Settings) -> Key {
    Key::from(Sha512::digest(settings.session_secret.expose_secret().as_bytes()).as_slice())
}

/// Origin timeout bound for every request (handle layer above renders 503).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Path of the SPA entry document the server serves (the very path the SPA
/// fallback reads).
///
/// **`backend-rs/public/` is NOT in git** — the Dockerfile copies
/// `frontend/dist/` into it before the image build — so this file usually
/// does NOT exist in a dev tree. The CSP hash computation degrades
/// deterministically around that (see [`boot_inline_script_hashes`]).
pub const SPA_INDEX_PATH: &str = "public/index.html";

/// Build application state from already-loaded settings (no I/O, no pool).
pub fn build_state(settings: config::Settings) -> AppState {
    AppState::from_settings(settings)
}

/// Compute the CSP hashes of the **inline** `<script>` blocks in `html`.
///
/// Each entry is `'sha256-<base64>'` over the EXACT bytes between the
/// `<script>` and `</script>` tags — the same bytes a browser hashes when it
/// evaluates the CSP `script-src` list. A `<script src=...>` is EXTERNAL
/// (covered by `script-src 'self'`) and is skipped.
///
/// This is the whole point of computing the hash instead of hand-maintaining
/// it: a stale constant breaks the SPA's inline theme bootstrap **silently**
/// (the page renders, the stored theme is ignored).
pub fn inline_script_hashes(html: &[u8]) -> Vec<String> {
    let mut hashes = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = find_bytes(&html[cursor..], b"<script") {
        let tag_start = cursor + offset;
        let after_name = tag_start + b"<script".len();
        let Some(tag_end_offset) = find_bytes(&html[after_name..], b">") else {
            break;
        };
        let tag_end = after_name + tag_end_offset;
        let attributes = &html[after_name..tag_end];
        cursor = tag_end + 1;
        if has_src_attribute(attributes) {
            continue;
        }
        let Some(closing_offset) = find_bytes(&html[cursor..], b"</script") else {
            break;
        };
        let closing = cursor + closing_offset;
        let content = &html[cursor..closing];
        hashes.push(format!("'sha256-{}'", base64_standard(&sha256(content))));
        cursor = closing + b"</script".len();
    }
    hashes
}

/// Read the served SPA index and compute its inline-script hashes.
///
/// **DEGRADE PATH (documented + deterministic):** `backend-rs/public/` is not
/// in git (the Dockerfile copies `frontend/dist/` into it before the image
/// build), so in a dev tree the file does NOT exist. The hash list is then
/// EMPTY — no panic, no boot failure — and the CSP is still emitted with
/// `script-src 'self'` and no hash. In such a deployment the browser blocks
/// the SPA's inline theme bootstrap (the page still renders; the stored
/// theme is ignored), which is precisely why the hash is computed from the
/// file the server ACTUALLY serves rather than hand-written.
pub fn boot_inline_script_hashes() -> Vec<String> {
    match std::fs::read(SPA_INDEX_PATH) {
        Ok(bytes) => inline_script_hashes(&bytes),
        Err(_) => Vec::new(),
    }
}

/// The `Content-Security-Policy` value for the SPA-serving origin.
///
/// Scoped to what the SPA actually needs (`54` B7):
///
/// ```text
/// default-src 'self'; script-src 'self' 'sha256-<hash>'; style-src 'self' 'unsafe-inline';
/// img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' <CORS origins>;
/// frame-ancestors 'none'; base-uri 'self'; form-action 'self'
/// ```
///
/// * `connect-src` is `'self'` **plus the canonical CORS allowlist** — in the
///   shipped dev topology the SPA is served by Vite at `http://localhost:5173`
///   and calls the API at `http://localhost:3001` (cross-origin), so
///   `'self'` alone would have the browser block every API call.
/// * `script-src` keeps NEITHER `'unsafe-inline'` NOR `'unsafe-eval'`; the
///   SPA's inline theme bootstrap is allowed by its **hash**.
/// * `style-src 'unsafe-inline'` IS required (the frontend uses ~12 inline
///   `style={{...}}` React props).
/// * `frame-ancestors 'none'` + `base-uri 'self'` + `form-action 'self'`
///   close the framing / base-tag / form-target holes.
pub fn content_security_policy(settings: &config::Settings, script_hashes: &[String]) -> String {
    let mut script_src = String::from("'self'");
    for hash in script_hashes {
        script_src.push(' ');
        script_src.push_str(hash);
    }
    // The SAME canonical list the CORS layer and the CSRF layer use.
    let mut connect_src = String::from("'self'");
    for origin in settings.canonical_cors_origins() {
        connect_src.push(' ');
        connect_src.push_str(&origin);
    }
    format!(
        "default-src 'self'; script-src {script_src}; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: blob:; font-src 'self' data:; connect-src {connect_src}; \
         frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
    )
}

/// Find `needle` in `haystack` (byte-wise).
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// Whether a `<script>` tag's attribute list carries a `src` attribute (an
/// EXTERNAL script — covered by `script-src 'self'`, never hashed).
fn has_src_attribute(attributes: &[u8]) -> bool {
    attributes
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty())
        .any(|token| {
            let name = token.split(|byte| *byte == b'=').next().unwrap_or_default();
            name.eq_ignore_ascii_case(b"src")
        })
}

/// SHA-256 digest of `bytes`.
fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).as_slice().to_vec()
}

/// Standard base64 (RFC 4648, with padding) — the encoding a CSP `sha256-`
/// source uses.
///
/// Hand-rolled on purpose: the crate may not take a new dependency for one
/// encoding (the `Cargo.toml` change budget of this task is the `csrf`
/// feature only), and the implementation is pinned by a unit test against the
/// well-known SHA-256("") vector.
fn base64_standard(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = u32::from(chunk.get(1).copied().unwrap_or_default());
        let third = u32::from(chunk.get(2).copied().unwrap_or_default());
        let triple = (first << 16) | (second << 8) | third;
        encoded.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

/// Assemble the router. Session sweeper is deliberately NOT spawned here.
///
/// Middleware order is load-bearing (canonical numbers from
/// 10-architecture.md; axum applies `.layer()` calls inside-out, so the code
/// below reads innermost-first):
/// 1. secure-headers `SetResponseHeaderLayer`s — outermost, so 4xx/5xx carry
///    them (`served-by: rust`, no CSP header matches `helmet(csp:false)`;
///    prod adds HSTS). Sensitive request headers (`authorization`, `cookie`,
///    `set-cookie`) are marked here too so no span/debug log can print them.
/// 2. `SetRequestIdLayer(x-request-id)` — even early rejections carry an id.
/// 3. `TraceLayer` — method + path + status + latency spans. The span reads
///    ONLY `x-request-id` (safe); query strings are excluded by recording
///    `path()`, and sensitive headers are already marked above.
/// 4. `PropagateRequestIdLayer` — echoes the id back to the client.
///    (request-context capture for problem+json `instance` runs here, inside
///    the request-id layers and outside the timeout, so timeouts also get an
///    exact instance.)
/// 5. Restrictive `CorsLayer` (explicit allowlist from settings, credentials,
///    `content-type` + `x-request-id` only; exposes `x-request-id`).
/// 6. `CompressionLayer` — after Governor (T30 `route_layer`, inside).
/// 7. `HandleErrorLayer(handle_timeout)` above `TimeoutLayer(30s)`.
///
/// Gaps are owned elsewhere:
///
/// 8. Governor `route_layer` on `/api/*` (T30). (SessionManagerLayer is wired
///    above since T10; T11 hardens its cookie/expiry.)
///
/// Route order: `/api` versioned routers (each with a JSON-404 fallback, so
/// `/api/*` misses never reach the SPA files) → `/health` + `/ready` (outside
/// `/api`, never throttled, no auth) → swagger/openapi → static files →
/// SPA fallback (`public/index.html`, never for `/api*`/swagger/api-docs).
///
/// The CSP hash of the SPA's inline theme bootstrap is computed from the
/// served `public/index.html` at BOOT (see [`boot_inline_script_hashes`]);
/// pass the hashes in explicitly with [`create_app_with_csp`].
pub fn create_app(state: AppState) -> Router {
    create_app_with_csp(state, boot_inline_script_hashes())
}

/// [`create_app`], with the CSP's inline-script hashes supplied by the
/// caller.
///
/// `main.rs` computes them at boot from the file the server actually serves
/// (and logs them); this entry point exists so the computation has exactly
/// ONE owner and the tests can pin a fixture's hashes.
pub fn create_app_with_csp(state: AppState, script_hashes: Vec<String>) -> Router {
    let v1 = api_router(&state.settings);
    let cors = cors_layer(&state.settings);
    let csrf = csrf_layer(state.settings.canonical_cors_origins());
    let policy = content_security_policy(&state.settings, &script_hashes);
    let is_production = state.settings.is_production();
    // Swagger UI shell only: the spec itself is served byte-identically from
    // the committed `openapi.json` by `openapi_json` below (CI diffs them),
    // so the UI is configured to fetch that URL without registering a second
    // spec route (which would panic on duplicate registration).
    let swagger: Router<AppState> = SwaggerUi::new("/swagger-ui")
        .config(SwaggerConfig::from("/api-docs/openapi.json"))
        .into();
    // Static files first, SPA `index.html` on miss. `ServeDir::fallback`
    // (not `not_found_service`, which would force a 404 status) preserves the
    // fallback response status.
    let spa = ServeDir::new("public")
        .call_fallback_on_method_not_allowed(true)
        .fallback(Router::new().fallback(spa_index));

    let router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api-docs/openapi.json", get(openapi_json))
        .merge(swagger)
        .fallback_service(spa);
    // Versioned routers + deprecated alias (same handlers, NOT redirects).
    let router = mount_versioned(router, v1);
    // [8] session management, innermost of the shared stack (closest to
    // handlers). Cookie hardening (T11): env cookie name (`kanpad.sid` /
    // `__Host-kanpad.sid`), HMAC-signed with `SESSION_SECRET` (a wrong/pre-cutover
    // secret fails verification → anonymous → 401, the force-re-login
    // proof), sliding 24 h inactivity expiry, `HttpOnly` + `SameSite=Lax` +
    // `Path=/`, `Secure` in production only, never a `Domain`.
    //
    // NO SWEEPER HERE: the expired-row deletion task
    // (`continuously_delete_expired(60s)`) is spawned ONLY in `main.rs`
    // with an `AbortHandle` on shutdown — spawning it here would leak one
    // task + pool connection per in-process test. `grep -rn
    // "tokio::spawn\|continuously_delete_expired" src/lib.rs` must stay
    // empty (CI grep gate candidate).
    let session_key = session_key(&state.settings);
    let session_layer = SessionManagerLayer::new(state.sessions.clone())
        .with_name(state.settings.session_cookie_name())
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(state.settings.is_production())
        .with_path("/")
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(24)))
        .with_signed(session_key);
    let router = router
        .layer(session_layer)
        // [7] origin timeout: handle layer ABOVE the timeout layer, so
        // `Elapsed` becomes 503 problem+json + `Retry-After` (single renderer
        // `handle_timeout`; 504 is proxy semantics).
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_timeout))
                .timeout(REQUEST_TIMEOUT),
        )
        // Problem+json `instance` capture (see [4] note above).
        .layer(axum::middleware::from_fn(capture_request_context))
        // [6] response compression (inside CORS + trace, outside Governor).
        .layer(CompressionLayer::new())
        // [5b] cross-origin protection on MUTATING requests (T56). It sits
        // INSIDE the CORS layer (written BEFORE it in the chain ⇒ CORS is the
        // outer one) so a preflight is short-circuited by CORS first and
        // never reaches the origin check.
        //
        // It uses the Go 1.25 `CrossOriginProtection` algorithm (tower-http
        // 0.7's `CsrfLayer`): safe methods always pass; `Sec-Fetch-Site:
        // same-origin|none` passes; `cross-site|same-site` passes ONLY for a
        // trusted `Origin`; no `Sec-Fetch-Site` + no `Origin` passes
        // (curl/proxy/native); an `Origin` whose authority equals the
        // request's effective host passes (the safety net that makes an
        // EMPTY production allowlist workable for a same-origin SPA).
        //
        // PROXY CAVEAT: the `Origin`/`Host` fallback degrades if a reverse
        // proxy REWRITES `Host` (tower-http's own "Deployment caveat") —
        // production MUST forward `Host` (and `Origin`) unchanged.
        //
        // `SameSite=Lax` + the `__Host-` cookie remain the first line of
        // defence: this is defence-in-depth, not a replacement.
        .layer(csrf)
        // [5] restrictive CORS (inner: preflight short-circuits here, still
        // flows out through the request-id/secure layers above).
        .layer(cors)
        // [4] echo `x-request-id` on every response, incl. rejections.
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
        // [3] http spans: method + path + status + latency (see module docs
        // for the redaction rules).
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request| {
                let id = request
                    .headers()
                    .get(&X_REQUEST_ID)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("unknown");
                tracing::info_span!(
                    "http_request",
                    method = %request.method(),
                    path = %request.uri().path(),
                    request_id = %id,
                )
            }),
        )
        // [2] assign (or keep the client's) `x-request-id` + stash it in
        // request extensions for the layers inside.
        .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
        // Mark secret-bearing request headers sensitive so no span or Debug
        // log can print session cookies / credentials (outer than [3]).
        .layer(SetSensitiveRequestHeadersLayer::new([
            header::AUTHORIZATION,
            header::COOKIE,
            header::SET_COOKIE,
        ]))
        // [1] secure headers, outermost (see HSTS below).
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("served-by"),
            HeaderValue::from_static("rust"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            header::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            header::HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            header::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        // [0] the CSP header — OUTERMOST of the shared stack, so it also
        // lands on 4xx/5xx responses (a 404 must carry it too).
        .layer(axum::middleware::from_fn(
            move |request: Request, next: axum::middleware::Next| {
                let policy = policy.clone();
                async move {
                    // The Swagger UI is a DEVELOPER surface, not the SPA: it
                    // ships its own scripts/styles and is not served in
                    // production, so it is excluded from the policy rather
                    // than given a second, looser one.
                    let excluded = request.uri().path().starts_with("/swagger-ui");
                    let mut response = next.run(request).await;
                    if !excluded && let Ok(value) = HeaderValue::from_str(&policy) {
                        response
                            .headers_mut()
                            .insert(HeaderName::from_static("content-security-policy"), value);
                    }
                    response
                }
            },
        ))
        .with_state(state);

    // [1] prod HSTS, outermost. Omitted outside production (HSTS on
    // `localhost` would pin dev browsers to HTTPS); recorded here.
    if is_production {
        router.layer(SetResponseHeaderLayer::overriding(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
    } else {
        router
    }
}

/// Versioned API routes (`/api/v1/*`) EXCEPT auth login/register (those live
/// in [`api_router`] on their own auth-scoped governor layer). Resource tasks
/// register their routers HERE only; [`create_app`] dual-mounts the result
/// under `/api/v1` and the deprecated `/api` alias, so both prefixes always
/// serve the same handlers.
/// Unknown `/api/*` paths fall back to JSON 404 (never `index.html`).
///
/// Keep the auth split in sync with `features::auth::routes::router` (the
/// canonical full list): logout + me here, login + register in [`api_router`].
fn versioned_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/logout", post(crate::features::auth::routes::logout))
        .route("/auth/me", get(crate::features::auth::routes::me))
        .merge(crate::features::boards::routes::router())
        .merge(crate::features::columns::routes::router())
        .merge(crate::features::cards::routes::router())
        .merge(crate::features::labels::routes::router())
        .merge(crate::features::checklists::routes::router())
        .merge(crate::features::notes::routes::router())
        .merge(crate::features::tags::routes::router())
        .merge(crate::features::projects::routes::router())
        .fallback(api_version_fallback)
}

/// Dual-mount helper: `v1` under `/api/v1`, plus the deprecated `/api` alias
/// (same handlers + `/docs` redirect). Extracted so the alias proof test
/// below exercises the real mounting code, not a copy.
fn mount_versioned(router: Router<AppState>, v1: Router<AppState>) -> Router<AppState> {
    let deprecated = v1.clone().route("/docs", get(api_docs_redirect));
    router.nest("/api/v1", v1).nest("/api", deprecated)
}

/// T30 `/api/*` router: [`versioned_routes`] + auth login/register, wrapped
/// in the two governor `route_layer`s (global per-IP ceiling, then the
/// tighter auth per-IP bucket on login/register only). `route_layer` — never
/// a global `.layer` — so `/health`, `/ready`, swagger/openapi, and
/// static/SPA (mounted on the outer router in [`create_app`]) are never
/// throttled. Both deprecated `/api` and versioned `/api/v1` nests share the
/// same limiter state (one bucket per client IP across prefixes).
fn api_router(settings: &config::Settings) -> Router<AppState> {
    let extractor = ClientIpExtractor {
        trusted: Arc::new(settings.trusted_proxy_list()),
    };
    // One per-minute governor layer: burst `per_min` tokens, one replenished
    // every 60/`per_min` seconds. Types stay inferred (naming the middleware
    // type would need the `governor` crate as a direct dependency).
    // `None` only when a limit is 0 — the router then stays unthrottled for
    // that scope (total constructor, no panics).
    let build_layer = |per_min: u32| {
        let mut builder = GovernorConfigBuilder::default().key_extractor(extractor.clone());
        builder.period(per_minute_period(per_min));
        builder.burst_size(per_min.max(1));
        builder
            .finish()
            .map(|config| GovernorLayer::new(config).error_handler(governor_rejection))
    };
    // Auth-scoped limiter: login/register ONLY (logout/me must never 429 —
    // logging out must never lock users out). `route_layer` wraps only the
    // routes registered above it, so this layer scopes to these two paths.
    let auth_routes = Router::new()
        .route("/auth/login", post(crate::features::auth::routes::login))
        .route(
            "/auth/register",
            post(crate::features::auth::routes::register),
        );
    let auth_routes = match build_layer(settings.throttle_auth_per_min) {
        Some(layer) => auth_routes.route_layer(layer),
        None => auth_routes,
    };
    let api = versioned_routes().merge(auth_routes);
    // Global per-IP abuse ceiling over everything under `/api/*`.
    match build_layer(settings.throttle_global_per_min) {
        Some(layer) => api.route_layer(layer),
        None => api,
    }
}

/// Period for an "N per minute" quota: burst N, one token per 60/N seconds.
fn per_minute_period(per_min: u32) -> Duration {
    Duration::from_nanos(60_000_000_000 / u64::from(per_min.max(1)))
}

/// T30 client-IP key extractor: peer [`ConnectInfo`] by default;
/// `X-Forwarded-For` honored ONLY when the peer is listed in
/// `TRUSTED_PROXIES` (default empty = never trust — direct-to-`:3001` spoof
/// defense). Trusted entries match exact IPs or IPv4 CIDRs; the leftmost
/// `X-Forwarded-For` entry is the client.
///
/// Requests without `ConnectInfo` (in-process `oneshot` probes that never
/// passed through hyper) share one fallback bucket instead of failing:
/// production always sets `ConnectInfo`
/// (`into_make_service_with_connect_info` in `main.rs`), so the fallback
/// only triggers off-stack.
///
/// Per-process memory is correct for the single `:3001` replica; a
/// Redis-backed governor is the scale-out prerequisite (N× limit with 2+
/// replicas otherwise).
#[derive(Debug, Clone)]
struct ClientIpExtractor {
    trusted: Arc<Vec<String>>,
}

/// Fallback key for requests without `ConnectInfo` (see [`ClientIpExtractor`]).
const FALLBACK_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

impl KeyExtractor for ClientIpExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &axum::http::request::Request<T>) -> Result<IpAddr, GovernorError> {
        let Some(peer) = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip())
        else {
            return Ok(FALLBACK_IP);
        };
        if self
            .trusted
            .iter()
            .any(|entry| proxy_entry_matches(entry, peer))
            && let Some(forwarded) = first_forwarded_ip(req.headers())
        {
            return Ok(forwarded);
        }
        Ok(peer)
    }
}

/// Leftmost `X-Forwarded-For` entry when it parses as an IP.
fn first_forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|list| list.split(',').next())
        .and_then(|first| first.trim().parse().ok())
}

/// A `TRUSTED_PROXIES` entry (exact IP or IPv4 CIDR) against the peer IP.
fn proxy_entry_matches(entry: &str, peer: IpAddr) -> bool {
    let entry = entry.trim();
    if let Some((net, prefix)) = entry.split_once('/') {
        return ipv4_cidr_contains(net, prefix, peer);
    }
    entry.parse::<IpAddr>().is_ok_and(|addr| addr == peer)
}

/// IPv4 CIDR containment without a new dependency (unparseable → false).
fn ipv4_cidr_contains(net: &str, prefix: &str, peer: IpAddr) -> bool {
    let (Ok(net), Ok(bits), IpAddr::V4(peer_v4)) = (
        net.trim().parse::<Ipv4Addr>(),
        prefix.trim().parse::<u32>(),
        peer,
    ) else {
        return false;
    };
    if bits > 32 {
        return false;
    }
    let mask = if bits == 0 {
        0
    } else {
        u32::MAX << (32 - bits)
    };
    (u32::from(net) & mask) == (u32::from(peer_v4) & mask)
}

/// Governor rejection renderer: 429s via the shared [`error::problem_response`]
/// shape ([`error::rate_limited_response`]); extractor failures are never 429s
/// (fail safe, never driver text — the `Internal` arm renders generic 500).
fn governor_rejection(error: GovernorError) -> Response {
    match error {
        GovernorError::TooManyRequests { wait_time, .. } => error::rate_limited_response(wait_time),
        GovernorError::UnableToExtractKey | GovernorError::Other { .. } => {
            AppError::Internal(error::Internal).into_response()
        }
    }
}

/// Restrictive CORS from settings: explicit allowlist (no `Any`), cookie
/// credentials, versioned methods, `content-type` + `x-request-id` request
/// headers, `x-request-id` exposed. Unparseable origins are dropped (boot
/// validation in `Settings` rejects them fail-fast, so this is unreachable in
/// practice and keeps router construction total).
///
/// The allowlist is the **canonical** one (`Settings::canonical_cors_origins`)
/// — the same list the CSRF layer trusts and the CSP's `connect-src` names.
fn cors_layer(settings: &config::Settings) -> CorsLayer {
    let origins: Vec<HeaderValue> = settings
        .canonical_cors_origins()
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            // `PUT /api/v1/cards/{id}/labels/{label_id}` is a served route and
            // `PUT` is NOT a CORS-safelisted method, so the browser sends a
            // preflight. Without `PUT` here the preflight is answered without
            // it and the browser BLOCKS the request — while every other verb
            // worked (a maximally confusing partial failure). Guarded by the
            // preflight test in `tests/api/cors.rs`.
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, X_REQUEST_ID])
        .expose_headers([X_REQUEST_ID])
}

/// Cross-origin protection for MUTATING requests (T56) — tower-http 0.7's
/// `CsrfLayer`, which implements the Go 1.25 `CrossOriginProtection`
/// algorithm step for step.
///
/// The trusted-origin set is the **canonical** CORS allowlist (the same list
/// the CORS layer allows and the CSP's `connect-src` names) — one list, three
/// consumers, so they can never disagree.
///
/// The rejection response is the SHARED problem+json renderer through the
/// EXISTING [`AppError::Forbidden`] mapping: the cross-origin rejection is
/// therefore **contract-neutral** — no new problem-type URI, no
/// `openapi.json` change.
///
/// `add_trusted_origin` cannot fail for a canonical entry (the canonicalizer
/// rejects every shape it rejects — a non-http(s) scheme, a missing host, a
/// path/query/fragment, a non-ASCII host). The error branch therefore
/// REFUSES to trust the origin (fail-closed: an untrusted origin is 403'd by
/// the algorithm) and records it — it never falls open.
fn csrf_layer(origins: Vec<String>) -> CsrfLayer<impl FnMut(ProtectionError) -> Response + Clone> {
    let mut layer = CsrfLayer::new();
    for origin in origins {
        // `add_trusted_origin` consumes the layer, so it is cloned: on the
        // (unreachable) error branch the previous layer — with the origins
        // accepted so far — is kept.
        match layer.clone().add_trusted_origin(&origin) {
            Ok(next) => layer = next,
            Err(error) => {
                tracing::error!(
                    origin = %origin,
                    error = %error,
                    "origin rejected by the cross-origin layer — it stays untrusted"
                );
            }
        }
    }
    layer.with_rejection_response(|_error: ProtectionError| AppError::Forbidden.into_response())
}

/// `instance` for handlers that render problem+json directly (same
/// `"{METHOD} {path}#{x-request-id}"` shape as `error.rs`, which reads the
/// middleware-captured context these handlers cannot see).
fn request_instance(method: &Method, uri: &Uri, headers: &HeaderMap) -> String {
    let id = headers
        .get(&X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");
    format!("{method} {}#{id}", uri.path())
}

/// Liveness probe: stable shape `{status:"ok"}`, no DB touch, no auth.
#[utoipa::path(get, path = "/health", responses((status = 200, body = HealthResponse)))]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// Readiness probe: `SELECT 1` through the pool. `200 {status:"ok",db:"up"}`
/// when MySQL answers, `503` problem+json otherwise — ANY pool error maps to
/// `503` (a wrong password is "not ready", not a 500). Fixed strings only:
/// driver errors can name users/hosts, so neither the error nor the URL is
/// interpolated into the body or the logs.
#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, body = ReadyResponse),
        (status = 503, body = ProblemDetails, description = "Database unreachable")
    )
)]
#[tracing::instrument(skip(state, headers))]
pub async fn ready(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "db": "up"})),
        )
            .into_response(),
        Err(_) => {
            tracing::error!("readiness probe failed: database unreachable");
            error::problem_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "service-unavailable",
                "Service Unavailable",
                "The database is temporarily unavailable. Please try again.".to_string(),
                request_instance(&method, &uri, &headers),
                Vec::new(),
            )
        }
    }
}

/// Unknown `/api/*` path (both mounts): JSON 404, never `index.html`.
#[tracing::instrument]
async fn api_version_fallback() -> impl IntoResponse {
    AppError::NotFound.into_response()
}

/// Familiar Nest-era docs path: exact `302` (axum's `Redirect` helpers emit
/// 303/307/308, so the status is set explicitly).
async fn api_docs_redirect() -> impl IntoResponse {
    (StatusCode::FOUND, [(header::LOCATION, "/swagger-ui")])
}

/// Serve the committed `openapi.json` stub byte-identically (T40a owns the real export).
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("../openapi.json"),
    )
}

/// SPA fallback (runs only when no route or static file matched): `/api*`
/// + swagger/api-docs misses become JSON 404 (excluded, never rewritten);
/// anything else serves `public/index.html` when the frontend build is
/// present (dev serves the frontend via Vite, so a missing build is a 404,
/// not a 500). Only `GET`/`HEAD` serve the file; other methods 404.
#[tracing::instrument(skip(uri))]
async fn spa_index(method: Method, uri: Uri) -> impl IntoResponse {
    let path = uri.path();
    if method != Method::GET && method != Method::HEAD {
        return AppError::NotFound.into_response();
    }
    if path == "/api"
        || path.starts_with("/api/")
        || path == "/swagger-ui"
        || path.starts_with("/swagger-ui/")
        || path.starts_with("/api-docs")
    {
        return AppError::NotFound.into_response();
    }
    match tokio::fs::read("public/index.html").await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(_) => AppError::NotFound.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use serde_json::Value;
    use tower::ServiceExt;

    fn probe_state() -> AppState {
        build_state(config::Settings::default())
    }

    /// The hand-rolled base64 encoder is pinned against the well-known
    /// vectors — a CSP `sha256-` source is only as good as this encoding.
    #[test]
    fn base64_matches_the_known_vectors() {
        assert_eq!(base64_standard(b""), "");
        assert_eq!(base64_standard(b"a"), "YQ==");
        assert_eq!(base64_standard(b"ab"), "YWI=");
        assert_eq!(base64_standard(b"abc"), "YWJj");
        // SHA-256("") — the canonical RFC 6234 vector.
        assert_eq!(
            base64_standard(&sha256(b"")),
            "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="
        );
        assert_eq!(
            base64_standard(&sha256(b"abc")),
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
    }

    /// The inline-script scanner: the INLINE script is hashed, the external
    /// one (`src=`) is not (it is covered by `script-src 'self'`).
    #[test]
    fn inline_script_hashes_cover_only_inline_scripts() {
        let html = br#"<html><head>
             <script>
               window.__theme = 'dark';
             </script>
             <script type="module" src="/assets/main.js"></script>
             </head></html>"#;
        let hashes = inline_script_hashes(html);
        assert_eq!(
            hashes.len(),
            1,
            "only the inline script is hashed: {hashes:?}"
        );
        assert!(hashes[0].starts_with("'sha256-"), "{hashes:?}");
        // The hash covers the EXACT bytes between the tags.
        let expected = format!(
            "'sha256-{}'",
            base64_standard(&sha256(
                b"\n               window.__theme = 'dark';\n             "
            ))
        );
        assert_eq!(hashes[0], expected);
    }

    /// A document with no inline script yields NO hash (the CSP then carries
    /// `script-src 'self'` alone).
    #[test]
    fn inline_script_hashes_are_empty_without_inline_scripts() {
        assert!(inline_script_hashes(b"<html><body></body></html>").is_empty());
        assert!(
            inline_script_hashes(b"<script type=\"module\" src=\"/a.js\"></script>").is_empty()
        );
    }

    /// The CSP's `connect-src` names the CANONICAL allowlist — the same list
    /// the CORS layer allows and the cross-origin layer trusts.
    #[test]
    fn content_security_policy_names_the_canonical_allowlist() {
        let settings = config::Settings {
            cors_origins: "http://localhost:5173,https://app.example.com".to_string(),
            ..config::Settings::default()
        };
        let policy = content_security_policy(&settings, &["'sha256-abc'".to_string()]);
        assert!(
            policy.contains("connect-src 'self' http://localhost:5173 https://app.example.com"),
            "{policy}"
        );
        assert!(
            policy.contains("script-src 'self' 'sha256-abc'"),
            "{policy}"
        );
        assert!(!policy.contains("'unsafe-eval'"), "{policy}");
    }

    /// Dual-mount proof on the REAL [`mount_versioned`]: one probe handler
    /// registered once is reachable under both prefixes with identical
    /// bodies, while misses on both prefixes are the same JSON 404.
    #[tokio::test]
    async fn versioned_and_deprecated_aliases_serve_same_handler() {
        async fn probe() -> &'static str {
            "v1-probe"
        }
        let v1 = versioned_routes().route("/__probe", get(probe));
        let app = mount_versioned(Router::new(), v1).with_state(probe_state());

        for path in ["/api/v1/__probe", "/api/__probe"] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "path: {path}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert_eq!(body.as_ref(), b"v1-probe", "path: {path}");
        }

        for path in ["/api/v1/__missing", "/api/__missing"] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "path: {path}");
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "application/problem+json",
                "path: {path}"
            );
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let json: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                json["type"], "https://kanpad.dev/problems/not-found",
                "path: {path}"
            );
        }
    }

    #[tokio::test]
    async fn deprecated_docs_redirect_registered() {
        let app = mount_versioned(Router::new(), versioned_routes()).with_state(probe_state());
        let response = app
            .oneshot(Request::get("/api/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "/swagger-ui"
        );
    }
}
