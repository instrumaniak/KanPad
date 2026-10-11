//! T04 shared-kernel probe tests (DB-free; `AppState` carries no pool yet).
//!
//! Covers: problem+json shape for every rejection class, timeout 503 +
//! `Retry-After`, `x-request-id` on early rejections, strict `PaginationParams`,
//! unknown-field tolerance, and the DefaultBodyLimit 2 MB arm.

use std::collections::HashMap;
use std::time::Duration;

use axum::{
    body::to_bytes,
    error_handling::HandleErrorLayer,
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    http::{header, request::Parts, HeaderMap, StatusCode},
    middleware,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use backend_rs::{
    build_state,
    config::Settings,
    create_app,
    error::{capture_request_context, handle_timeout, AppError},
    extract::PaginationParams,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower::ServiceExt;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

/// Request-id header name (tower-http keeps its own copy private).
const X_REQUEST_ID: header::HeaderName = header::HeaderName::from_static("x-request-id");

const PROBLEMS: &str = "https://kanpad.dev/problems";

fn test_settings() -> Settings {
    Settings {
        node_env: "test".to_string(),
        hash_cost: 1,
        port: 3001,
    }
}

/// Thin wrappers that route real axum rejections through [`AppError`],
/// proving the `From` mappings over HTTP.
struct AppPath<T>(T);

impl<T, S> FromRequestParts<S> for AppPath<T>
where
    T: serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| Self(value))
            .map_err(AppError::Path)
    }
}

struct AppQuery<T>(T);

impl<T, S> FromRequestParts<S> for AppQuery<T>
where
    T: serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(value)| Self(value))
            .map_err(AppError::Query)
    }
}

struct AppJson<T>(T);

impl<T, S> FromRequest<S> for AppJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(AppError::Json)
    }
}

#[derive(Debug, Deserialize)]
struct EchoIn {
    name: String,
}

#[derive(Debug, Deserialize)]
struct FlagQuery {
    active: bool,
}

async fn paged(params: PaginationParams) -> Json<Value> {
    Json(json!({"page": params.page, "limit": params.limit}))
}

async fn get_item(AppPath(id): AppPath<i32>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"id": id})))
}

async fn flag(AppQuery(query): AppQuery<FlagQuery>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"active": query.active})))
}

async fn echo(AppJson(body): AppJson<EchoIn>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"name": body.name})))
}

async fn slow() -> Json<Value> {
    tokio::time::sleep(Duration::from_millis(500)).await;
    Json(json!({"status": "slow"}))
}

async fn db_down() -> Result<Json<Value>, AppError> {
    Err(AppError::Db(sqlx::Error::WorkerCrashed))
}

async fn fail(Query(params): Query<HashMap<String, String>>) -> Result<Json<Value>, AppError> {
    match params.get("kind").map(String::as_str) {
        Some("not-found") => Err(AppError::NotFound),
        Some("unauthorized") => Err(AppError::Unauthorized),
        Some("forbidden") => Err(AppError::Forbidden),
        Some("conflict") => Err(AppError::Conflict),
        Some("validation") => {
            let mut report = garde::Report::new();
            report.append(
                garde::Path::new("name"),
                garde::Error::new("length: must be at most 255"),
            );
            Err(AppError::Validation(report))
        }
        _ => Err(AppError::Internal(backend_rs::error::Internal)),
    }
}

/// Probe router: test routes + the same shared-stack shape as [`create_app`]
/// (short timeout so the timeout test stays fast).
fn probe_router() -> Router {
    Router::new()
        .route("/t/paged", get(paged))
        .route("/t/items/{id}", get(get_item))
        .route("/t/flag", get(flag))
        .route("/t/echo", post(echo))
        .route("/t/slow", get(slow))
        .route("/t/db-down", get(db_down))
        .route("/t/fail", get(fail))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_timeout))
                .timeout(Duration::from_millis(50)),
        )
        .layer(middleware::from_fn(capture_request_context))
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
        .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
}

async fn read_problem(response: Response) -> (StatusCode, HeaderMap, Value) {
    let status = response.status();
    let headers = response.headers().clone();
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .expect("problem response must set Content-Type")
        .to_str()
        .expect("Content-Type must be valid ASCII")
        .to_owned();
    assert_eq!(
        content_type, "application/problem+json",
        "error responses must be problem+json"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).expect("problem response must be valid JSON");
    (status, headers, json)
}

fn assert_problem_shape(body: &Value, status: u16, type_uri: &str) {
    assert_eq!(body["status"], json!(status), "body: {body}");
    assert_eq!(body["type"], json!(type_uri), "body: {body}");
    assert!(body["title"].is_string(), "title must be set: {body}");
    assert!(body["detail"].is_string(), "detail must be set: {body}");
    assert!(body["instance"].is_string(), "instance must be set: {body}");
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .expect("early rejections must carry x-request-id")
        .to_str()
        .expect("x-request-id must be ASCII")
        .to_owned()
}

#[tokio::test]
async fn prod_health_carries_request_id_and_secure_headers() {
    let app = create_app(build_state(test_settings()));
    let response = app
        .oneshot(
            Request::get("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert!(
        !request_id(headers).is_empty(),
        "every response carries x-request-id"
    );
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(
        headers.get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );
}

#[tokio::test]
async fn pagination_defaults_and_valid_values() {
    let app = probe_router();

    let response = app
        .oneshot(
            Request::get("/t/paged")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"page": 1, "limit": 50})
    );

    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/paged?page=2&limit=10")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"page": 2, "limit": 10})
    );
}

#[tokio::test]
async fn pagination_page_zero_is_strict_400() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/paged?page=0")
                .header("x-request-id", "pg-0")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, headers, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEMS}/validation-error"));
    let errors = body["errors"].as_array().expect("errors[] must be set");
    assert!(
        errors.iter().any(|e| e["field"] == json!("page")),
        "errors[] must name page: {body}"
    );
    assert_eq!(request_id(&headers), "pg-0");
    assert_eq!(body["instance"], json!("GET /t/paged#pg-0"));
}

#[tokio::test]
async fn pagination_limit_over_max_is_strict_400_not_clamped() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/paged?limit=999")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, _, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEMS}/validation-error"));
    let errors = body["errors"].as_array().expect("errors[] must be set");
    assert!(
        errors.iter().any(|e| e["field"] == json!("limit")),
        "errors[] must name limit: {body}"
    );
}

#[tokio::test]
async fn pagination_non_integer_and_negative_are_400() {
    for query in ["?page=abc", "?page=-1", "?limit=1.5", "?limit=0"] {
        let app = probe_router();
        let response = app
            .oneshot(
                Request::get(format!("/t/paged{query}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, _, body) = read_problem(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "query: {query}");
        assert_problem_shape(&body, 400, &format!("{PROBLEMS}/validation-error"));
        assert!(body["errors"].is_array(), "errors[] must be set: {body}");
    }
}

#[tokio::test]
async fn path_rejection_is_problem_json_with_request_id() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/items/abc")
                .header("x-request-id", "path-1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, headers, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEMS}/invalid-path"));
    assert_eq!(request_id(&headers), "path-1");
    assert_eq!(body["instance"], json!("GET /t/items/abc#path-1"));
}

#[tokio::test]
async fn query_rejection_is_problem_json_with_request_id() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/flag?active=notabool")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, headers, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEMS}/invalid-query"));
    assert!(!request_id(&headers).is_empty());
}

#[tokio::test]
async fn unknown_json_fields_are_ignored() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::post("/t/echo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"name":"hi","unknown_field":1,"nested":{"x":[]}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"name": "hi"})
    );
}

#[tokio::test]
async fn malformed_json_is_400_problem() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::post("/t/echo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(r#"{"name": "#))
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, _, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEMS}/invalid-json"));
}

#[tokio::test]
async fn missing_json_content_type_is_415_problem() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::post("/t/echo")
                .body(axum::body::Body::from(r#"{"name":"hi"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, _, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_problem_shape(&body, 415, &format!("{PROBLEMS}/invalid-json"));
}

#[tokio::test]
async fn oversized_body_is_400_not_413() {
    // Axum's default 2 MiB body limit surfaces as `JsonRejection::BytesRejection`,
    // which the catch-all arm renders as 400 `invalid-json` (recorded decision).
    let big = format!("{{\"name\":\"{}\"}}", "a".repeat(2 * 1024 * 1024 + 1));
    let app = probe_router();
    let response = app
        .oneshot(
            Request::post("/t/echo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(big))
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, _, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEMS}/invalid-json"));
}

#[tokio::test]
async fn slow_route_times_out_with_503_and_retry_after() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/slow")
                .header("x-request-id", "slow-1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, headers, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_problem_shape(&body, 503, &format!("{PROBLEMS}/service-unavailable"));
    assert!(
        headers.get(header::RETRY_AFTER).is_some(),
        "timeout must carry Retry-After"
    );
    assert_eq!(request_id(&headers), "slow-1");
}

#[tokio::test]
async fn forced_db_error_renders_generic_500() {
    let app = probe_router();
    let response = app
        .oneshot(
            Request::get("/t/db-down")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, headers, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_problem_shape(&body, 500, &format!("{PROBLEMS}/internal"));
    assert_eq!(body["detail"], json!("An unexpected error occurred."));
    let raw = serde_json::to_string(&body).unwrap();
    assert!(!raw.contains("sqlx"), "no driver text may leak: {raw}");
    assert!(
        !raw.contains("WorkerCrashed"),
        "no error debug may leak: {raw}"
    );
    assert!(!request_id(&headers).is_empty());
}

#[tokio::test]
async fn fail_kinds_map_status_and_type_uri() {
    let cases = [
        ("not-found", 404, "not-found"),
        ("unauthorized", 401, "unauthorized"),
        ("forbidden", 403, "forbidden"),
        ("conflict", 409, "conflict"),
        ("internal", 500, "internal"),
        ("validation", 400, "validation-error"),
    ];
    for (kind, status_code, slug) in cases {
        let app = probe_router();
        let response = app
            .oneshot(
                Request::get(format!("/t/fail?kind={kind}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, _, body) = read_problem(response).await;
        assert_eq!(
            status,
            StatusCode::from_u16(status_code).unwrap(),
            "kind: {kind}"
        );
        assert_problem_shape(&body, status_code, &format!("{PROBLEMS}/{slug}"));
        if kind == "validation" {
            assert!(
                body["errors"].is_array(),
                "validation needs errors[]: {body}"
            );
        }
    }
}
