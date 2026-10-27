//! The two `tower-http` 0.7 behaviour changes that land on this codebase
//! (T67).
//!
//! Both are **observable contract changes**, so both are asserted here rather
//! than assumed:
//!
//! 1. **`CompressionLayer` now answers 406** when the client's
//!    `Accept-Encoding` excludes everything the server can offer *and*
//!    excludes `identity` (RFC 9110 §12.5.3). tower-http 0.6 answered
//!    200 with an uncompressed body. A browser never sends such a header
//!    (it always accepts gzip), so this only affects exotic clients — but
//!    "only affects exotic clients" is exactly the kind of assumption that
//!    rots, so the behaviour is pinned by a test.
//! 2. **`ServeDir` rejects a trailing slash** on a file path (404 instead of
//!    serving the file). The SPA is served through
//!    `ServeDir::new("public").fallback(...)`, so a request for
//!    `/index.html/` must NOT serve the file.
//!
//! DB-free: none of these touch a handler — the compression layer answers
//! before the router, and the SPA fallback is a static-file path.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;

use super::helpers::test_app;

/// A request that carries an explicit `Accept-Encoding`.
async fn get_with_accept_encoding(path: &str, accept_encoding: &str) -> axum::response::Response {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .header(header::ACCEPT_ENCODING, accept_encoding)
        .body(Body::empty())
        .unwrap();
    test_app().oneshot(request).await.unwrap()
}

/// A request with no `Accept-Encoding` at all.
async fn get_without_accept_encoding(path: &str) -> axum::response::Response {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .unwrap();
    test_app().oneshot(request).await.unwrap()
}

/// The ordinary case is unchanged: a client that accepts gzip gets a
/// compressed (or at least successful) response.
#[tokio::test]
async fn a_client_that_accepts_gzip_is_served() {
    let response = get_with_accept_encoding("/health", "gzip, deflate").await;
    assert!(
        response.status().is_success(),
        "a normal Accept-Encoding must still be served, got {}",
        response.status()
    );
}

/// A client that refuses *every* encoding the server can produce **and**
/// refuses `identity` gets the RFC 9110 §12.5.3 answer: **406**.
///
/// (tower-http 0.6 answered 200 with an uncompressed body here. The change
/// is a *contract* change, so it is pinned — not silently absorbed.)
#[tokio::test]
async fn a_client_that_refuses_every_encoding_gets_406() {
    let response = get_with_accept_encoding("/health", "identity;q=0").await;
    assert_eq!(
        response.status(),
        StatusCode::NOT_ACCEPTABLE,
        "tower-http 0.7 answers 406 when the client accepts no encoding the \
         server can produce (RFC 9110 §12.5.3)"
    );
}

/// A client that sends no `Accept-Encoding` at all is served normally —
/// `identity` is implied.
#[tokio::test]
async fn a_client_without_accept_encoding_is_served() {
    let response = get_without_accept_encoding("/health").await;
    assert!(
        response.status().is_success(),
        "no Accept-Encoding means identity is acceptable, got {}",
        response.status()
    );
}

/// A **trailing slash** on a file path must NOT serve the file
/// (tower-http 0.7's `services` change). The SPA fallback is not a licence
/// to serve `/index.html/` either.
#[tokio::test]
async fn a_trailing_slash_does_not_serve_the_file() {
    let response = get_without_accept_encoding("/index.html/").await;
    assert_ne!(
        response.status(),
        StatusCode::OK,
        "a trailing slash must not serve the file (tower-http 0.7 rejects it)"
    );
}
