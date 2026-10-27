//! T40a OpenAPI snapshot gate: the committed `backend-rs/openapi.json` must
//! always equal what `cargo run -p backend-rs -- export-openapi` renders from
//! the compiled `ApiDoc`. Any handler/DTO change that alters the spec fails
//! here until the author re-exports and commits the file. The second test
//! proves `GET /api-docs/openapi.json` serves those exact bytes (no DB touch
//! — the handler reads the embedded file, so default settings suffice).

use axum::{
    body::{Body, to_bytes},
    extract::Request,
};
use tower::ServiceExt;
use utoipa::OpenApi;

/// The committed spec, embedded at compile time (same file `export-openapi`
/// regenerates via stdout redirect).
const COMMITTED: &str = include_str!("../../openapi.json");

fn generated() -> String {
    backend_rs::docs::ApiDoc::openapi()
        .to_pretty_json()
        .expect("ApiDoc must serialize to JSON")
}

/// Byte-identical drift gate: fails on any spec change until `openapi.json`
/// is re-exported and committed.
#[test]
fn export_matches_committed_openapi_json() {
    assert_eq!(
        generated(),
        COMMITTED,
        "openapi.json drifted from the compiled ApiDoc; regenerate with \
         `cargo run -p backend-rs -- export-openapi > backend-rs/openapi.json` \
         and commit the result"
    );
}

/// Structural pin: the spec carries every list envelope, the problem+json
/// error schemas, and the main resource paths (guards against silently
/// dropping a `schemas(...)` entry while staying byte-equal by luck).
#[test]
fn snapshot_carries_envelopes_errors_and_paths() {
    let spec: serde_json::Value =
        serde_json::from_str(&generated()).expect("generated spec must parse");
    assert_eq!(spec["openapi"], "3.1.0");
    assert_eq!(spec["info"]["title"], "KanPad backend-rs");
    for envelope in [
        "Page_BoardListResponse",
        "Page_CardSummary",
        "Page_LabelResponse",
        "Page_NoteSummaryResponse",
        "Page_ProjectResponse",
        "Page_TagResponse",
    ] {
        assert!(
            spec["components"]["schemas"].get(envelope).is_some(),
            "missing envelope component: {envelope}"
        );
    }
    for error in ["ProblemDetails", "FieldError"] {
        assert!(
            spec["components"]["schemas"].get(error).is_some(),
            "missing error component: {error}"
        );
    }
    for path in [
        "/health",
        "/api/v1/boards",
        "/api/v1/cards/{id}",
        "/api/v1/auth/me",
    ] {
        assert!(
            spec["paths"].get(path).is_some(),
            "missing path in spec: {path}"
        );
    }
}

/// `GET /api-docs/openapi.json` serves the committed bytes verbatim as
/// `application/json` (proves serve == export without touching the DB).
#[tokio::test]
async fn api_docs_serves_committed_openapi_json() {
    let state = backend_rs::build_state(backend_rs::config::Settings::default());
    let app = backend_rs::create_app(state);
    let response = app
        .oneshot(
            Request::get("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap(),
        "application/json"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        body.as_ref(),
        COMMITTED.as_bytes(),
        "/api-docs/openapi.json must serve the committed file byte-identically"
    );
}
