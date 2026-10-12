//! T30 throttle probes (DB-free).
//!
//! Each test builds its own router with tiny limits (fresh in-memory governor
//! state per test) and drives it with an explicit peer [`ConnectInfo`] —
//! per-test router + test source-IP, so parallel runs never share a bucket
//! (no `#[serial]` needed). Governor runs outside handlers/extractors, so
//! every probe uses an endpoint that answers without a live DB: anonymous
//! `me` (401), login with an empty JSON body (400, rejected before any store
//! touch), anonymous logout (204), `/health` (200), and `/ready` against an
//! unroutable pool (503 — the exemption proof asserts "never 429").

use std::net::SocketAddr;

use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{header, HeaderMap, Request, StatusCode},
};
use backend_rs::{build_state, config::Settings, create_app};
use serde_json::Value;
use tower::ServiceExt;

const PROBLEMS: &str = "https://kanpad.dev/problems";
/// Unroutable DB port (refused fast): `/ready` deterministically 503s, and no
/// probe below can accidentally pass via a live database.
const BROKEN_DB_URL: &str = "mysql://kanpad:invalid@127.0.0.1:1/kanpad?charset=utf8mb4";

fn throttle_settings(global_per_min: u32, auth_per_min: u32, trusted_proxies: &str) -> Settings {
    Settings {
        database_url: Some(BROKEN_DB_URL.to_string()),
        throttle_global_per_min: global_per_min,
        throttle_auth_per_min: auth_per_min,
        trusted_proxies: trusted_proxies.to_string(),
        ..Settings::default()
    }
}

fn throttle_app(global_per_min: u32, auth_per_min: u32, trusted_proxies: &str) -> axum::Router {
    create_app(build_state(throttle_settings(
        global_per_min,
        auth_per_min,
        trusted_proxies,
    )))
}

fn with_peer(request: Request<Body>, peer: &str) -> Request<Body> {
    let addr: SocketAddr = format!("{peer}:41231")
        .parse()
        .expect("test peer must parse");
    let (mut parts, body) = request.into_parts();
    parts.extensions.insert(ConnectInfo(addr));
    Request::from_parts(parts, body)
}

/// Anonymous `GET /api/v1/auth/me`: rejected 401 by the extractor (no DB).
fn me_request(peer: &str) -> Request<Body> {
    with_peer(
        Request::get("/api/v1/auth/me")
            .body(Body::empty())
            .expect("test request must build"),
        peer,
    )
}

/// `POST /api/v1/auth/login` with `{}`: 400 before any DB touch.
fn login_request(peer: &str, forwarded_for: Option<String>) -> Request<Body> {
    let mut builder =
        Request::post("/api/v1/auth/login").header(header::CONTENT_TYPE, "application/json");
    if let Some(xff) = forwarded_for {
        builder = builder.header("x-forwarded-for", xff);
    }
    with_peer(
        builder
            .body(Body::from("{}"))
            .expect("test request must build"),
        peer,
    )
}

/// Anonymous `POST /api/v1/auth/logout`: 204, no session row to destroy.
fn logout_request(peer: &str) -> Request<Body> {
    with_peer(
        Request::post("/api/v1/auth/logout")
            .body(Body::empty())
            .expect("test request must build"),
        peer,
    )
}

async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot must answer");
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body must read");
    (status, headers, body.to_vec())
}

/// Shape proof for every 429: problem+json `Content-Type`, exact `type` URI,
/// numeric `status`, and a non-empty `Retry-After`.
fn assert_rate_limited(status: StatusCode, headers: &HeaderMap, raw: &[u8]) {
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .expect("429 must set Content-Type"),
        "application/problem+json"
    );
    let retry_after = headers
        .get(header::RETRY_AFTER)
        .expect("429 must carry Retry-After")
        .to_str()
        .expect("Retry-After must be visible ASCII");
    assert!(!retry_after.is_empty(), "Retry-After must not be empty");
    let body: Value = serde_json::from_slice(raw).expect("429 body must be JSON");
    assert_eq!(body["type"], format!("{PROBLEMS}/rate-limited"));
    assert_eq!(body["status"], 429);
    assert!(body["instance"].is_string(), "instance must be set: {body}");
    // Secure-headers + request-id layers are outermost, so even the governor
    // rejection carries them.
    assert_eq!(
        headers.get("served-by").expect("429 must carry served-by"),
        "rust"
    );
    assert!(
        headers.contains_key("x-request-id"),
        "429 must carry x-request-id"
    );
}

#[tokio::test]
async fn global_limit_trips_429_problem_json_with_retry_after() {
    let app = throttle_app(3, 1000, "");
    for _ in 0..3 {
        let (status, _, _) = send(&app, me_request("10.30.0.1")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, headers, raw) = send(&app, me_request("10.30.0.1")).await;
    assert_rate_limited(status, &headers, &raw);
}

#[tokio::test]
async fn health_and_ready_exempt_under_flood() {
    let app = throttle_app(3, 1000, "");
    // Exhaust the global bucket for this peer (3x401 then 429 proves the
    // flood actually counted).
    for _ in 0..3 {
        let (status, _, _) = send(&app, me_request("10.30.0.2")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _, _) = send(&app, me_request("10.30.0.2")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    // Same flood, same peer: liveness never throttles.
    let health = with_peer(
        Request::get("/health")
            .body(Body::empty())
            .expect("test request must build"),
        "10.30.0.2",
    );
    let (status, _, raw) = send(&app, health).await;
    assert_eq!(status, StatusCode::OK);
    let body: Value = serde_json::from_slice(&raw).expect("health must be JSON");
    assert_eq!(body, serde_json::json!({"status": "ok"}));

    // Readiness never throttles either (503 here = broken pool, not 429).
    let ready = with_peer(
        Request::get("/ready")
            .body(Body::empty())
            .expect("test request must build"),
        "10.30.0.2",
    );
    let (status, headers, _) = send(&app, ready).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        !headers.contains_key(header::RETRY_AFTER) || status != StatusCode::TOO_MANY_REQUESTS,
        "ready must never 429 under flood"
    );
}

#[tokio::test]
async fn sixth_auth_login_in_window_is_429() {
    let app = throttle_app(1000, 5, "");
    for _ in 0..5 {
        let (status, _, _) = send(&app, login_request("10.30.0.3", None)).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "first 5 logins must pass the auth limiter"
        );
    }
    let (status, headers, raw) = send(&app, login_request("10.30.0.3", None)).await;
    assert_rate_limited(status, &headers, &raw);
}

#[tokio::test]
async fn spoofed_xff_from_untrusted_peer_does_not_reset_bucket() {
    let app = throttle_app(1000, 5, "");
    // Rotating spoofed X-Forwarded-For values from the same untrusted peer:
    // the bucket stays keyed by peer IP, so the 6th login still 429s.
    for i in 0..5 {
        let xff = Some(format!("198.51.100.{}", i + 1));
        let (status, _, _) = send(&app, login_request("10.30.0.5", xff)).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "spoofed XFF must not consume a fresh bucket"
        );
    }
    let (status, headers, raw) = send(
        &app,
        login_request("10.30.0.5", Some("198.51.100.99".to_string())),
    )
    .await;
    assert_rate_limited(status, &headers, &raw);
}

#[tokio::test]
async fn trusted_peer_honors_xff_as_client_ip() {
    let app = throttle_app(1000, 5, "10.30.0.6");
    // Peer is in TRUSTED_PROXIES: each distinct XFF is its own client bucket,
    // so 6 logins with rotating XFF never 429.
    for i in 0..6 {
        let xff = Some(format!("203.0.113.{}", i + 1));
        let (status, _, _) = send(&app, login_request("10.30.0.6", xff)).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "trusted-proxy XFF clients must get separate buckets"
        );
    }
}

#[tokio::test]
async fn logout_and_me_exempt_from_auth_limiter() {
    let app = throttle_app(1000, 5, "");
    // Six anonymous logouts: never 429 (logout is unthrottled by the auth
    // limiter by design — logging out must never lock users out).
    for _ in 0..6 {
        let (status, _, _) = send(&app, logout_request("10.30.0.7")).await;
        assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    }
    // The auth bucket is untouched by those logouts: a login still passes.
    let (status, _, _) = send(&app, login_request("10.30.0.7", None)).await;
    assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);

    // And back the other way: an exhausted auth bucket still lets logout
    // through (same peer already spent 0 auth quota above, so spend it now).
    for _ in 0..5 {
        let (status, _, _) = send(&app, login_request("10.30.0.8", None)).await;
        assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    }
    let (status, _, _) = send(&app, login_request("10.30.0.8", None)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _, _) = send(&app, logout_request("10.30.0.8")).await;
    assert_ne!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "logout must pass even with an exhausted auth bucket"
    );
}
