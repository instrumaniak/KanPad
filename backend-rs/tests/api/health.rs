//! Skeleton suites: health + openapi stub. No DB, no auth.

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use backend_rs::{build_state, config::Settings, create_app};
use serde_json::{json, Value};
use tower::ServiceExt;

fn test_app() -> axum::Router {
    let settings = Settings {
        node_env: "test".to_string(),
        hash_cost: 1,
        port: 3001,
    };
    create_app(build_state(settings))
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
