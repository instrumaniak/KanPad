//! T007 `ClientIpExtractor` policy matrix (DB-free).
//!
//! Proves the rightmost-non-trusted walk through the real router: a trusted
//! peer's XFF list resolves to the real client, spoofed leftmost entries are
//! ignored, untrusted peers cannot spoof, and distinct clients get distinct
//! throttle buckets.

use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use backend_rs::{build_state, config::Settings, create_app};

use super::helpers::{PROBLEM_BASE, send_ref};

const BROKEN_DB_URL: &str = "mysql://kanpad:invalid@127.0.0.1:1/kanpad?charset=utf8mb4";

fn proxy_settings(trusted_proxies: &str) -> Settings {
    Settings {
        database_url: Some(BROKEN_DB_URL.to_string()),
        throttle_global_per_min: 1,
        throttle_auth_per_min: 1000,
        trusted_proxies: trusted_proxies.to_string(),
        ..Settings::default()
    }
}

fn proxy_app(trusted_proxies: &str) -> axum::Router {
    create_app(build_state(proxy_settings(trusted_proxies)))
}

fn with_peer(request: Request<Body>, peer: &str) -> Request<Body> {
    let addr: SocketAddr = format!("{peer}:41231")
        .parse()
        .expect("test peer must parse");
    let (mut parts, body) = request.into_parts();
    parts.extensions.insert(ConnectInfo(addr));
    Request::from_parts(parts, body)
}

fn me_request(peer: &str, forwarded_for: Option<&str>) -> Request<Body> {
    let mut builder = Request::get("/api/v1/auth/me");
    if let Some(xff) = forwarded_for {
        builder = builder.header("x-forwarded-for", xff);
    }
    with_peer(builder.body(Body::empty()).expect("request builds"), peer)
}

#[tokio::test]
async fn trusted_peer_rightmost_non_trusted_is_client() {
    let app = proxy_app("127.0.0.1");
    let (status, _, _) = send_ref(
        &app,
        me_request("127.0.0.1", Some("203.0.113.7, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Same client again exhausts its own bucket.
    let (status, headers, raw) = send_ref(
        &app,
        me_request("127.0.0.1", Some("203.0.113.7, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let body: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(body["type"], format!("{PROBLEM_BASE}/rate-limited"));
    let _ = headers;
}

#[tokio::test]
async fn trusted_peer_ignores_spoofed_leftmost_entry() {
    let app = proxy_app("127.0.0.1");
    let (status, _, _) = send_ref(
        &app,
        me_request("127.0.0.1", Some("1.2.3.4, 203.0.113.7, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A request whose only difference is the spoofed leftmost entry still
    // shares the same bucket (203.0.113.7).
    let (status, _, _) = send_ref(
        &app,
        me_request("127.0.0.1", Some("9.9.9.9, 203.0.113.7, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn trusted_peer_without_xff_uses_peer_address() {
    let app = proxy_app("127.0.0.1");
    let (status, _, _) = send_ref(&app, me_request("127.0.0.1", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send_ref(&app, me_request("127.0.0.1", None)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn untrusted_peer_ignores_spoofed_xff() {
    let app = proxy_app("");
    let (status, _, _) = send_ref(&app, me_request("198.51.100.9", Some("1.2.3.4"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Rotating the spoofed value does not reset the bucket.
    let (status, _, _) = send_ref(&app, me_request("198.51.100.9", Some("5.6.7.8"))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn two_clients_with_distinct_xff_get_independent_buckets() {
    let app = proxy_app("127.0.0.1");
    // Client A exhausts its bucket.
    let (status, _, _) = send_ref(
        &app,
        me_request("127.0.0.1", Some("203.0.113.10, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send_ref(
        &app,
        me_request("127.0.0.1", Some("203.0.113.10, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    // Client B is unaffected.
    let (status, _, _) = send_ref(
        &app,
        me_request("127.0.0.1", Some("203.0.113.11, 127.0.0.1")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_xff_falls_back_to_peer_without_500() {
    let app = proxy_app("127.0.0.1");
    let (status, _, _) = send_ref(&app, me_request("127.0.0.1", Some("not-an-ip"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send_ref(&app, me_request("127.0.0.1", Some("not-an-ip"))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}
