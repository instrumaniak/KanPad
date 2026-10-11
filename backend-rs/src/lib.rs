//! backend-rs: KanPad API rewrite (Axum 0.8 + sqlx 0.8 + MySQL, single crate).
//!
//! Composition root for in-process tests. [`create_app`] builds the router;
//! [`build_state`] builds state. This module NEVER spawns the session-expiry
//! sweeper — only `main.rs` does (see 10-architecture.md Sessions).

pub mod common;
pub mod config;
pub mod db;
pub mod docs;
pub mod error;
pub mod extract;
pub mod features;
pub mod state;

use std::time::Duration;

use axum::{
    error_handling::HandleErrorLayer, http::header, response::IntoResponse, routing::get, Json,
    Router,
};
use docs::HealthResponse;
use error::{capture_request_context, handle_timeout};
use state::AppState;
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
};

/// Request-id header name (tower-http keeps its own copy private).
const X_REQUEST_ID: header::HeaderName = header::HeaderName::from_static("x-request-id");

/// Origin timeout bound for every request (handle layer above renders 503).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build application state from already-loaded settings (no I/O, no pool).
pub fn build_state(settings: config::Settings) -> AppState {
    AppState::from_settings(settings)
}

/// Assemble the router. Session sweeper is deliberately NOT spawned here.
///
/// Middleware order is load-bearing (canonical numbers from
/// 10-architecture.md; axum applies `.layer()` calls inside-out, so the code
/// below reads innermost-first):
/// 1. secure-headers `SetResponseHeaderLayer`s — outermost, so 4xx/5xx carry
///    them. (`served-by: rust` + CSP decision land in T05 with the rest of
///    transport setup; no CSP header matches `helmet(csp:false)`.)
/// 2. `SetRequestIdLayer(x-request-id)` — even early rejections carry an id.
/// 4. `PropagateRequestIdLayer` — echoes the id back to the client.
///    (request-context capture for problem+json `instance` runs here, inside
///    the request-id layers and outside the timeout, so timeouts also get an
///    exact instance.)
/// 7. `HandleErrorLayer(handle_timeout)` above `TimeoutLayer(30s)`.
///
/// Gaps are owned elsewhere:
///
/// 3. TraceLayer + 5. CORS + 6. compression (T05),
/// 8. SessionManagerLayer (T11), Governor `route_layer` on `/api/*` (T30).
pub fn create_app(state: AppState) -> Router {
    // `/api/*` sub-router. T30 attaches rate limiting HERE via
    // `api_router.route_layer(GovernorLayer)` — scoped to versioned routes,
    // never global (`/health`, `/ready`, swagger, static stay unthrottled).
    let api_router: Router<AppState> = Router::new();

    Router::new()
        .route("/health", get(health))
        .route("/api-docs/openapi.json", get(openapi_json))
        .nest("/api", api_router)
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
        // [4] echo `x-request-id` on every response, incl. rejections.
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
        // [2] assign (or keep the client's) `x-request-id` + stash it in
        // request extensions for the layers inside.
        .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
        // [1] secure headers, outermost.
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
        .with_state(state)
}

/// Liveness probe: stable shape `{status:"ok"}`, no DB touch, no auth.
#[utoipa::path(get, path = "/health", responses((status = 200, body = HealthResponse)))]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// Serve the committed `openapi.json` stub byte-identically (T40a owns the real export).
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("../openapi.json"),
    )
}
