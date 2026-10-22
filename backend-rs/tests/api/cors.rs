//! CORS preflight guard (T55, `54` B3).
//!
//! **The bug this suite exists to prevent.** `CorsLayer::allow_methods` is a
//! hand-written list — axum does not expose the registered route table, so
//! the list CANNOT be derived from the router. It drifted: `PUT` was missing
//! while `PUT /api/v1/cards/{id}/labels/{label_id}` **is** served and the
//! frontend **calls** it (`frontend/src/features/cards/cards.api.ts`). `PUT`
//! is not a CORS-safelisted method, so the browser sends a **preflight**; the
//! server answered `Access-Control-Allow-Methods` without `PUT` and the
//! browser blocked the request. In the shipped topology (compose sets
//! `VITE_API_URL=http://localhost:3001`, so the SPA at `:5173` calls `:3001`
//! **cross-origin** and the Vite proxy is bypassed) every other verb worked —
//! only label-attach failed, with a browser-console CORS error instead of an
//! application error.
//!
//! **Why a test and not a derived list.** The class of bug is "a hand-
//! maintained policy drifts". The policy cannot be derived from the router,
//! so the guard must be automated: every verb the API actually serves must
//! survive a CORS preflight.
//!
//! DB-free: a preflight never reaches the handler (the CORS layer answers it).

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use tower::ServiceExt;

use super::helpers::test_app;

/// The dev-default allowlisted origin (`config::DEFAULT_CORS_ORIGIN`).
const ORIGIN: &str = "http://localhost:5173";

/// Every verb the API serves. `PUT` is the one that drifted — it is here
/// because `PUT /api/v1/cards/{id}/labels/{label_id}` is a real route.
const SERVED_VERBS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

/// Send a CORS preflight for `method` and return the response.
async fn preflight(method: &str) -> axum::response::Response {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/cards/1/labels/2")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
        .body(Body::empty())
        .unwrap();
    test_app().oneshot(request).await.unwrap()
}

/// Every served verb must survive a preflight (the drift guard).
#[tokio::test]
async fn preflight_allows_every_served_verb() {
    for method in SERVED_VERBS {
        let response = preflight(method).await;
        assert!(
            response.status().is_success(),
            "preflight for {method} must succeed, got {}",
            response.status()
        );
        let allow = response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            allow.contains(method),
            "CORS must allow {method}, got {allow:?}"
        );
    }
}

/// The preflight must echo the allowlisted origin and the credentials
/// permission — the SPA authenticates with a cookie, so
/// `Access-Control-Allow-Credentials` is load-bearing.
#[tokio::test]
async fn preflight_allows_the_configured_origin_with_credentials() {
    let response = preflight("POST").await;
    let allow_origin = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        allow_origin, ORIGIN,
        "the allowlisted origin must be echoed verbatim"
    );
    let credentials = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        credentials, "true",
        "the cookie session needs Access-Control-Allow-Credentials"
    );
}

/// A FOREIGN origin must NOT be granted access — the allowlist is explicit,
/// never `Any` (the layer pairs `allow_credentials(true)`, which browsers
/// reject with `*`).
#[tokio::test]
async fn preflight_does_not_grant_a_foreign_origin() {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/cards/1/labels/2")
        .header(header::ORIGIN, "https://evil.example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();
    let allow_origin = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        !allow_origin.contains("evil.example.com"),
        "a foreign origin must never be granted access, got {allow_origin:?}"
    );
}

/// The CORS preflight is answered by the CORS layer, NOT by a route: an
/// unknown path still gets a CORS answer (and never a 404 body).
#[tokio::test]
async fn preflight_is_answered_before_the_router() {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/definitely-not-a-route")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the CORS layer answers the preflight before routing"
    );
}
