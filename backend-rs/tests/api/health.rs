//! T05 config/observability/versioning/SPA probes.
//!
//! DB-free except `ready_200_when_db_up`, which uses `DATABASE_URL` when the
//! environment provides one (CI always sets it; otherwise it reports the skip
//! and passes — the DB-down paths below are the deterministic core).

use std::sync::Mutex;

use axum::body::to_bytes;
use axum::http::{header, Method, Request, StatusCode};
use backend_rs::{build_state, config::Settings, create_app};
use serde_json::{json, Value};
use tower::ServiceExt;

const PROBLEMS: &str = "https://kanpad.dev/problems";
const WRONG_PASSWORD: &str = "this-password-is-wrong-xyz";

static PUBLIC_LOCK: Mutex<()> = Mutex::new(());

fn test_settings() -> Settings {
    Settings {
        node_env: "test".to_string(),
        hash_cost: 1,
        port: 3001,
        ..Default::default()
    }
}

fn test_app() -> axum::Router {
    create_app(build_state(test_settings()))
}

/// Pool that can never connect (unroutable port): refused fast, so the test
/// stays deterministic with no live DB.
fn broken_pool_app() -> axum::Router {
    let settings = Settings {
        database_url: Some(format!(
            "mysql://kanpad:{WRONG_PASSWORD}@127.0.0.1:1/kanpad?charset=utf8mb4"
        )),
        ..test_settings()
    };
    create_app(build_state(settings))
}

async fn read_body(
    response: axum::response::Response,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    (status, headers, json)
}

fn request_id(headers: &axum::http::HeaderMap) -> &str {
    headers
        .get("x-request-id")
        .expect("every response must carry x-request-id")
        .to_str()
        .unwrap()
}

fn served_by(headers: &axum::http::HeaderMap) -> &str {
    headers
        .get("served-by")
        .expect("every response must carry served-by: rust")
        .to_str()
        .unwrap()
}

fn assert_transport_headers(headers: &axum::http::HeaderMap) {
    assert!(!request_id(headers).is_empty());
    assert_eq!(served_by(headers), "rust");
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(
        headers.get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );
}

#[tokio::test]
async fn health_returns_200_status_ok() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_transport_headers(response.headers());
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, json!({"status": "ok"}));
}

#[tokio::test]
async fn openapi_stub_served() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/api-docs/openapi.json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_transport_headers(response.headers());
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.contains("application/json"),
        "unexpected content-type: {content_type}"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        json.get("openapi").is_some(),
        "stub must carry openapi version"
    );
    assert!(
        json.pointer("/paths/~1health").is_some(),
        "stub must list /health"
    );
}

#[tokio::test]
async fn ready_503_when_db_down_no_password_leak() {
    let app = broken_pool_app();
    let response = app
        .oneshot(
            Request::get("/ready")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, headers, body) = read_body(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_transport_headers(&headers);
    assert_eq!(
        headers.get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    assert_eq!(body["type"], format!("{PROBLEMS}/service-unavailable"));
    assert_eq!(body["status"], 503);
    assert!(body["instance"].is_string(), "instance must be set: {body}");
    let raw = serde_json::to_string(&body).unwrap();
    assert!(
        !raw.contains(WRONG_PASSWORD),
        "password leaked in body: {raw}"
    );
    assert!(!raw.contains("127.0.0.1:1"), "dial address leaked: {raw}");
}

#[tokio::test]
async fn health_200_while_ready_503_on_broken_pool() {
    // Joint liveness proof: the SAME broken-pool app answers liveness DB-free
    // while readiness reports the outage.
    let app = broken_pool_app();
    let health = app
        .oneshot(
            Request::get("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let app = broken_pool_app();
    let ready = app
        .oneshot(
            Request::get("/ready")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn ready_200_when_db_up() {
    let Some(database_url) = std::env::var("DATABASE_URL")
        .ok()
        .filter(|url| !url.is_empty())
    else {
        eprintln!("SKIP ready_200_when_db_up: DATABASE_URL unset (runs in CI)");
        return;
    };
    let settings = Settings {
        database_url: Some(database_url),
        ..test_settings()
    };
    // Short acquire bound so an unreachable CI DB fails fast instead of
    // hanging the suite (canonical 5 s pool default is for prod).
    let app = create_app(build_state(settings));
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        app.oneshot(
            Request::get("/ready")
                .body(axum::body::Body::empty())
                .unwrap(),
        ),
    )
    .await
    .expect("readiness probe must answer within 10 s")
    .unwrap();

    let (status, headers, body) = read_body(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_transport_headers(&headers);
    assert_eq!(body, json!({"status": "ok", "db": "up"}));
}

#[tokio::test]
async fn versioned_and_deprecated_api_miss_share_json_404() {
    for path in ["/api/v1/no-such-route", "/api/no-such-route"] {
        let app = test_app();
        let response = app
            .oneshot(Request::get(path).body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        let (status, headers, body) = read_body(response).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "path: {path}");
        assert_transport_headers(&headers);
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json",
            "path: {path}"
        );
        assert_eq!(
            body["type"],
            format!("{PROBLEMS}/not-found"),
            "path: {path}"
        );
        let raw = serde_json::to_string(&body).unwrap();
        assert!(
            !raw.to_lowercase().contains("<!doctype"),
            "/api miss must never serve index.html: {path}"
        );
    }
}

#[tokio::test]
async fn api_docs_redirects_to_swagger_ui_before_fallback() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/api/docs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_transport_headers(response.headers());
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/swagger-ui"
    );
}

#[tokio::test]
async fn swagger_ui_and_openapi_are_not_rewritten() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/swagger-ui/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_transport_headers(response.headers());
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        content_type.contains("text/html"),
        "swagger-ui must serve html, got: {content_type}"
    );

    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/api-docs/openapi.json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn spa_fallback_serves_index_html_for_non_api_miss() {
    // No other test touches `public/`, so the lock only needs to cover
    // fixture setup/teardown, never the awaits in between.
    let index = std::path::Path::new("public").join("index.html");
    let created_dir: bool;
    {
        let _guard = PUBLIC_LOCK.lock().unwrap();
        let dir = std::path::Path::new("public");
        created_dir = !dir.exists();
        if created_dir {
            std::fs::create_dir(dir).unwrap();
        }
        std::fs::write(&index, "<!doctype html><html>T05-SPA-MARKER</html>").unwrap();
    }

    let get = |path: &str| Request::get(path).body(axum::body::Body::empty()).unwrap();
    let app = test_app();
    let response = app.oneshot(get("/some/spa/route")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_transport_headers(response.headers());
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        content_type.contains("text/html"),
        "SPA fallback must serve html, got: {content_type}"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(
        body.windows(14).any(|w| w == b"T05-SPA-MARKER"),
        "SPA fallback must serve index.html"
    );

    let app = test_app();
    let response = app.oneshot(get("/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(body.windows(14).any(|w| w == b"T05-SPA-MARKER"));

    std::fs::remove_file(&index).unwrap();
    if created_dir {
        std::fs::remove_dir("public").unwrap();
    }
}

#[tokio::test]
async fn cors_preflight_matrix() {
    // Allowed origin: credentials + ACAO.
    let app = test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/boards")
                .header("Origin", "http://localhost:5173")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_transport_headers(response.headers());
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://localhost:5173"
    );
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-credentials")
            .unwrap(),
        "true"
    );

    // Disallowed origin: no ACAO.
    let app = test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/boards")
                .header("Origin", "http://evil.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_transport_headers(response.headers());
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "disallowed origin must get no ACAO"
    );
}

#[tokio::test]
async fn cors_simple_request_exposes_request_id() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/health")
                .header("Origin", "http://localhost:5173")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://localhost:5173"
    );
    let expose = response
        .headers()
        .get("access-control-expose-headers")
        .expect("x-request-id must be exposed")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        expose.contains("x-request-id"),
        "unexpected expose list: {expose}"
    );
}

#[tokio::test]
async fn x_forwarded_for_ignored_by_default() {
    // Default TRUSTED_PROXIES is empty: a spoofed XFF changes nothing.
    assert!(Settings::default().trusted_proxy_list().is_empty());
    let plain = test_app()
        .oneshot(
            Request::get("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let spoofed = test_app()
        .oneshot(
            Request::get("/health")
                .header("x-forwarded-for", "1.2.3.4")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(plain.status(), StatusCode::OK);
    assert_eq!(spoofed.status(), StatusCode::OK);
}

#[tokio::test]
async fn early_rejections_carry_request_id_and_served_by() {
    // Propagated client id survives on errors (exact problem `instance`).
    let app = test_app();
    let response = app
        .oneshot(
            Request::get("/api/v1/no-such-route")
                .header("x-request-id", "early-1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, headers, body) = read_body(response).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers.get("x-request-id").unwrap(), "early-1");
    assert_eq!(headers.get("served-by").unwrap(), "rust");
    assert_eq!(body["instance"], "GET /api/v1/no-such-route#early-1");
}
