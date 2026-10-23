//! Shared error kernel: `thiserror`-only [`AppError`] rendered as RFC 9457
//! `application/problem+json`.
//!
//! Every error carries a stable `type` URI from the `10-architecture.md`
//! registry (`https://kanpad.dev/problems/<slug>`, never `about:blank`) and an
//! always-set `instance` of the form `"{METHOD} {path}#{x-request-id}"` (fed by
//! [`capture_request_context`], which the middleware stack runs inside the
//! request-id layers). `problem_response` is the single constructor behind
//! [`AppError`], the T30 governor 429 handler, and the T11 `AuthSession`
//! rejection so all three render an identical shape.

use axum::{
    Json,
    extract::{Request, rejection::JsonRejection},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tower_http::request_id::RequestId;
use utoipa::ToSchema;

/// Base URI for every problem `type`. Slugs come from the registry in
/// `10-architecture.md` (validation-error, invalid-json, invalid-path,
/// invalid-query, unauthorized, forbidden, not-found, conflict, rate-limited,
/// service-unavailable, internal).
pub const PROBLEM_BASE: &str = "https://kanpad.dev/problems";

/// Content type rendered for every error (never axum's default plain-text).
pub const PROBLEM_JSON: &str = "application/problem+json";

/// `Retry-After` seconds attached to origin timeouts (matches the 30 s
/// `TimeoutLayer`; 504 is proxy semantics, so the origin uses 503).
const TIMEOUT_RETRY_AFTER: &str = "30";

/// One field-level failure inside the problem `errors[]` extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct FieldError {
    /// Dotted field path (`page`, `cards.0.title`).
    pub field: String,
    /// Human-readable rule message.
    pub message: String,
}

/// RFC 9457 body. `status` duplicates the HTTP status (kept for the frontend
/// `ApiError` which preserves it); `errors` is omitted when empty.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProblemDetails {
    /// Stable per-error URI, e.g. `https://kanpad.dev/problems/validation-error`.
    pub r#type: String,
    /// Short human-readable summary.
    pub title: String,
    /// HTTP status code (mirrors the response status).
    pub status: u16,
    /// Request-specific detail. Never carries driver/SQL/secret text.
    pub detail: String,
    /// `"{METHOD} {path}#{x-request-id}"`, always set.
    pub instance: String,
    /// Field-level failures (validation-class errors only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<FieldError>,
}

/// Marker for unexpected failures. A unit struct on purpose: there is no
/// message payload that could leak driver, SQL, or secret text into the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Internal;

/// Shared application error. `thiserror`-only; `anyhow` is allowed solely at
/// the `main()` boundary, never here. `#[non_exhaustive]` so new variants
/// are not a breaking change for downstream matches.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AppError {
    /// Unknown route under `/api/*` or foreign/not-owned id (no oracle).
    #[error("not found")]
    NotFound,
    /// Missing/invalid session (T11 `AuthSession` rejects through here).
    #[error("unauthorized")]
    Unauthorized,
    /// Authenticated but role-denied (never a substitute for 404).
    #[error("forbidden")]
    Forbidden,
    /// Wrong-state operation (e.g. deleting an active board).
    #[error("conflict")]
    Conflict,
    /// Field-level validation failures (strict
    /// [`crate::extract::PaginationParams`], the `ValidJson`/`ValidQuery`
    /// extractors, and hand-built short-circuit helpers — each maps its
    /// `garde::Report` here through `From`, keeping `garde` out of the
    /// public API).
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    /// Malformed JSON / wrong content-type / over-limit body.
    #[error(transparent)]
    Json(JsonRejection),
    /// Unparseable path params (axum's default is already 400 but plain-text).
    #[error(transparent)]
    Path(axum::extract::rejection::PathRejection),
    /// Unparseable query string (same plain-text default as path).
    #[error(transparent)]
    Query(axum::extract::rejection::QueryRejection),
    /// sqlx failure, mapped by matching the *inner* error (never one status
    /// per variant). Full error is logged server-side; the body stays generic.
    #[error("database error")]
    Db(sqlx::Error),
    /// Origin timeout via [`handle_timeout`] (503 + `Retry-After`).
    #[error("request timed out")]
    Timeout,
    /// Unexpected failure. Carries no payload (see [`Internal`]).
    #[error("internal error")]
    Internal(Internal),
}

impl From<garde::Report> for AppError {
    fn from(report: garde::Report) -> Self {
        Self::Validation(validation_errors(&report))
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::Json(rejection)
    }
}

impl From<axum::extract::rejection::PathRejection> for AppError {
    fn from(rejection: axum::extract::rejection::PathRejection) -> Self {
        Self::Path(rejection)
    }
}

impl From<axum::extract::rejection::QueryRejection> for AppError {
    fn from(rejection: axum::extract::rejection::QueryRejection) -> Self {
        Self::Query(rejection)
    }
}

/// Crate-wide result alias for handlers (`AppResult<Json<T>>`, …).
pub type AppResult<T> = Result<T, AppError>;

/// Method + path + request id captured per request so every rejection —
/// including framework rejections that never reach a handler — can render an
/// exact `instance`. Runs inside the request-id layers (they supply the id).
#[derive(Debug, Clone)]
struct RequestContext {
    method: String,
    path: String,
    request_id: String,
}

tokio::task_local! {
    static REQUEST_CONTEXT: RequestContext;
}

/// Stash `RequestContext` for the duration of the request. The fallback id
/// `unknown` only triggers outside the middleware stack (unit tests that do
/// not scope a context); production traffic always flows through this layer.
pub async fn capture_request_context(request: Request, next: Next) -> Response {
    let context = RequestContext {
        method: request.method().to_string(),
        path: request.uri().path().to_owned(),
        request_id: request
            .extensions()
            .get::<RequestId>()
            .and_then(|id| id.header_value().to_str().ok())
            .unwrap_or("unknown")
            .to_owned(),
    };
    REQUEST_CONTEXT.scope(context, next.run(request)).await
}

fn current_instance() -> String {
    REQUEST_CONTEXT
        .try_with(|context| format!("{} {}#{}", context.method, context.path, context.request_id))
        .unwrap_or_else(|_| "unknown unknown#unknown".to_string())
}

/// Single problem+json constructor shared by [`AppError`], the T30 governor
/// 429 renderer, and the T11 `AuthSession` rejection. `errors` is omitted
/// from the body when empty.
pub(crate) fn problem_response(
    status: StatusCode,
    error_type: &'static str,
    title: &'static str,
    detail: String,
    instance: String,
    errors: Vec<FieldError>,
) -> Response {
    let problem = ProblemDetails {
        r#type: format!("{PROBLEM_BASE}/{error_type}"),
        title: title.to_string(),
        status: status.as_u16(),
        detail,
        instance,
        errors,
    };
    let mut response = Json(problem).into_response();
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
    response
}

/// T30 governor 429 renderer: the shared [`problem_response`] shape
/// (`type: https://kanpad.dev/problems/rate-limited`, problem+json) plus a
/// `Retry-After` in seconds. The governor `route_layer` sits inside
/// [`capture_request_context`]'s task-local scope, so `instance` is exact on
/// real traffic (the `unknown` fallback only triggers off-stack).
pub(crate) fn rate_limited_response(retry_after_secs: u64) -> Response {
    let mut response = problem_response(
        StatusCode::TOO_MANY_REQUESTS,
        "rate-limited",
        "Too Many Requests",
        "Rate limit exceeded. Please wait before retrying.".to_string(),
        current_instance(),
        Vec::new(),
    );
    let seconds = retry_after_secs.max(1).to_string();
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&seconds).unwrap_or(HeaderValue::from_static("60")),
    );
    response
}

fn validation_errors(report: &garde::Report) -> Vec<FieldError> {
    report
        .iter()
        .map(|(path, error)| FieldError {
            field: path.to_string(),
            message: error.message().to_owned(),
        })
        .collect()
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let instance = current_instance();
        match self {
            Self::NotFound => problem_response(
                StatusCode::NOT_FOUND,
                "not-found",
                "Not Found",
                "The requested resource was not found.".to_string(),
                instance,
                Vec::new(),
            ),
            Self::Unauthorized => problem_response(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized",
                "Authentication is required.".to_string(),
                instance,
                Vec::new(),
            ),
            Self::Forbidden => problem_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "Forbidden",
                "You do not have permission to perform this action.".to_string(),
                instance,
                Vec::new(),
            ),
            Self::Conflict => problem_response(
                StatusCode::CONFLICT,
                "conflict",
                "Conflict",
                "The request conflicts with the current state of the resource.".to_string(),
                instance,
                Vec::new(),
            ),
            Self::Validation(errors) => {
                // Rebuild the exact `detail` garde's own `Report` Display
                // produced: one `"<field>: <message>"` per line (`<message>`
                // alone when the field path is empty), then trimmed.
                let joined = errors
                    .iter()
                    .map(|error| {
                        if error.field.is_empty() {
                            error.message.clone()
                        } else {
                            format!("{}: {}", error.field, error.message)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let trimmed = joined.trim();
                let detail = if trimmed.is_empty() {
                    "Request validation failed.".to_string()
                } else {
                    trimmed.to_owned()
                };
                problem_response(
                    StatusCode::BAD_REQUEST,
                    "validation-error",
                    "Validation Failed",
                    detail,
                    instance,
                    errors,
                )
            }
            Self::Json(rejection) => {
                // 413 decision (recorded): axum's `DefaultBodyLimit` (2 MiB) surfaces
                // over-limit bodies as `JsonRejection::BytesRejection`, which falls
                // into the catch-all arm below and renders 400 `invalid-json` — the
                // same class as every other body-parse failure, never a bare 413.
                // `JsonRejection` is `#[non_exhaustive]`, so the catch-all also
                // future-proofs new variants.
                match rejection {
                    JsonRejection::MissingJsonContentType(_) => problem_response(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "invalid-json",
                        "Unsupported Media Type",
                        rejection.body_text(),
                        instance,
                        Vec::new(),
                    ),
                    JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
                        problem_response(
                            StatusCode::BAD_REQUEST,
                            "invalid-json",
                            "Invalid JSON",
                            rejection.body_text(),
                            instance,
                            Vec::new(),
                        )
                    }
                    _ => problem_response(
                        StatusCode::BAD_REQUEST,
                        "invalid-json",
                        "Invalid JSON",
                        rejection.body_text(),
                        instance,
                        Vec::new(),
                    ),
                }
            }
            Self::Path(rejection) => problem_response(
                StatusCode::BAD_REQUEST,
                "invalid-path",
                "Invalid Path",
                rejection.body_text(),
                instance,
                Vec::new(),
            ),
            Self::Query(rejection) => problem_response(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "Invalid Query",
                rejection.body_text(),
                instance,
                Vec::new(),
            ),
            Self::Db(error) => {
                // Log the full driver error server-side; the body stays generic
                // so table/column/SQL text never reaches the client.
                tracing::error!(error = %error, "database error");
                match &error {
                    sqlx::Error::RowNotFound => problem_response(
                        StatusCode::NOT_FOUND,
                        "not-found",
                        "Not Found",
                        "The requested resource was not found.".to_string(),
                        instance,
                        Vec::new(),
                    ),
                    sqlx::Error::Database(database_error)
                        if database_error.code().as_deref() == Some("1062")
                            || database_error.kind() == sqlx::error::ErrorKind::UniqueViolation =>
                    {
                        problem_response(
                            StatusCode::CONFLICT,
                            "conflict",
                            "Conflict",
                            "A resource with the same unique value already exists.".to_string(),
                            instance,
                            Vec::new(),
                        )
                    }
                    // Pool exhaustion / closed pool / transport failure: the
                    // runtime 503 mapping (boot connection-refused stays fatal;
                    // `/ready` mirrors this red state in T05).
                    sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_) => {
                        problem_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "service-unavailable",
                            "Service Unavailable",
                            "The database is temporarily unavailable. Please try again."
                                .to_string(),
                            instance,
                            Vec::new(),
                        )
                    }
                    _ => problem_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal",
                        "Internal Server Error",
                        "An unexpected error occurred.".to_string(),
                        instance,
                        Vec::new(),
                    ),
                }
            }
            Self::Timeout => {
                let mut response = problem_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service-unavailable",
                    "Service Unavailable",
                    "The request took too long to process. Please try again.".to_string(),
                    instance,
                    Vec::new(),
                );
                response.headers_mut().insert(
                    header::RETRY_AFTER,
                    HeaderValue::from_static(TIMEOUT_RETRY_AFTER),
                );
                response
            }
            Self::Internal(_) => problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal Server Error",
                "An unexpected error occurred.".to_string(),
                instance,
                Vec::new(),
            ),
        }
    }
}

/// Convert middleware errors into [`AppError::Timeout`]. Single 503 renderer:
/// `TimeoutLayer` can only fail with `tower::timeout::error::Elapsed`, so
/// every error here is an origin timeout (504 is proxy semantics).
pub async fn handle_timeout(_error: axum::BoxError) -> impl IntoResponse {
    AppError::Timeout.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        extract::{FromRequest, FromRequestParts, Query},
    };
    use serde::Deserialize;
    use serde_json::Value;

    fn test_context() -> RequestContext {
        RequestContext {
            method: "GET".to_string(),
            path: "/probe".to_string(),
            request_id: "test-id".to_string(),
        }
    }

    async fn render(error: AppError) -> (StatusCode, axum::http::HeaderMap, Value) {
        REQUEST_CONTEXT
            .scope(test_context(), async {
                let response = error.into_response();
                let status = response.status();
                let headers = response.headers().clone();
                let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                let content_type = headers
                    .get(header::CONTENT_TYPE)
                    .expect("Content-Type must be set")
                    .to_str()
                    .unwrap()
                    .to_owned();
                assert_eq!(content_type, PROBLEM_JSON);
                let json: Value = serde_json::from_slice(&body).unwrap();
                (status, headers, json)
            })
            .await
    }

    #[derive(Debug)]
    struct StubDatabaseError {
        code: &'static str,
        message: &'static str,
        unique_violation: bool,
    }

    impl std::fmt::Display for StubDatabaseError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "stub db error: {}", self.message)
        }
    }

    impl std::error::Error for StubDatabaseError {}

    impl sqlx::error::DatabaseError for StubDatabaseError {
        fn message(&self) -> &str {
            self.message
        }

        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            Some(self.code.into())
        }

        fn kind(&self) -> sqlx::error::ErrorKind {
            // `ErrorKind` is neither `Clone` nor `Copy` (non-exhaustive,
            // `Debug + PartialEq + Eq` only), so rebuild it from the flag.
            if self.unique_violation {
                sqlx::error::ErrorKind::UniqueViolation
            } else {
                sqlx::error::ErrorKind::Other
            }
        }

        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
    }

    fn stub_db_error(code: &'static str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(StubDatabaseError {
            code,
            message: "Duplicate entry 'x' for key 'uq'",
            unique_violation: false,
        }))
    }

    /// Real MySQL dup-key shape under sqlx 0.8: `code()` is the `23000`
    /// SQLSTATE (the `1062` number is unexposed), identity only in `kind()`.
    fn stub_unique_violation_error() -> sqlx::Error {
        sqlx::Error::Database(Box::new(StubDatabaseError {
            code: "23000",
            message: "Duplicate entry 'x' for key 'uq'",
            unique_violation: true,
        }))
    }

    #[tokio::test]
    async fn simple_variants_map_status_type_and_instance() {
        let cases = [
            (AppError::NotFound, StatusCode::NOT_FOUND, "not-found"),
            (
                AppError::Unauthorized,
                StatusCode::UNAUTHORIZED,
                "unauthorized",
            ),
            (AppError::Forbidden, StatusCode::FORBIDDEN, "forbidden"),
            (AppError::Conflict, StatusCode::CONFLICT, "conflict"),
            (
                AppError::Internal(Internal),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
        ];
        for (error, status, slug) in cases {
            let (actual_status, _, body) = render(error).await;
            assert_eq!(actual_status, status);
            assert_eq!(body["type"], format!("{PROBLEM_BASE}/{slug}"));
            assert_eq!(body["status"], status.as_u16());
            assert_eq!(body["instance"], "GET /probe#test-id");
            assert!(body.get("errors").is_none(), "no errors[] expected: {body}");
        }
    }

    #[tokio::test]
    async fn timeout_renders_503_with_retry_after() {
        let (status, headers, body) = render(AppError::Timeout).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/service-unavailable"));
        assert_eq!(
            headers.get(header::RETRY_AFTER).unwrap(),
            TIMEOUT_RETRY_AFTER
        );
    }

    #[tokio::test]
    async fn validation_report_renders_errors_array() {
        let mut report = garde::Report::new();
        report.append(
            garde::Path::new("title"),
            garde::Error::new("length: must be at most 255"),
        );
        let (status, _, body) = render(AppError::from(report)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/validation-error"));
        assert_eq!(
            body["errors"],
            serde_json::json!([{"field": "title", "message": "length: must be at most 255"}])
        );
        assert!(body["detail"].as_str().unwrap().contains("title"));
    }

    #[tokio::test]
    async fn json_arms_map_missing_syntax_and_data() {
        let missing = Json::<Value>::from_request(
            Request::builder().uri("/").body(Body::empty()).unwrap(),
            &(),
        )
        .await
        .unwrap_err();
        let (status, _, body) = render(AppError::Json(missing)).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/invalid-json"));

        let syntax = Json::<Value>::from_bytes(b"{\"name\": ").unwrap_err();
        let (status, _, body) = render(AppError::Json(syntax)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/invalid-json"));

        let data = Json::<Vec<u8>>::from_bytes(b"{\"name\":1}").unwrap_err();
        let (status, _, body) = render(AppError::Json(data)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/invalid-json"));
    }

    #[tokio::test]
    async fn path_and_query_rejections_render_400_problem() {
        #[derive(Debug, Deserialize)]
        struct Flag {
            #[allow(dead_code)]
            active: bool,
        }

        let (mut parts, _) = Request::builder()
            .uri("/t/flag?active=notabool")
            .body(Body::empty())
            .unwrap()
            .into_parts();
        let rejection = Query::<Flag>::from_request_parts(&mut parts, &())
            .await
            .unwrap_err();
        let (status, _, body) = render(AppError::Query(rejection)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/invalid-query"));

        let (mut parts, _) = Request::builder()
            .uri("/t/items")
            .body(Body::empty())
            .unwrap()
            .into_parts();
        let rejection = axum::extract::Path::<i32>::from_request_parts(&mut parts, &())
            .await
            .unwrap_err();
        let (status, _, body) = render(AppError::Path(rejection)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/invalid-path"));
    }

    #[tokio::test]
    async fn db_inner_match_maps_row_not_found_duplicate_and_pool() {
        let (status, _, body) = render(AppError::Db(sqlx::Error::RowNotFound)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/not-found"));

        let (status, _, body) = render(AppError::Db(stub_db_error("1062"))).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/conflict"));

        // Real MySQL dup-key shape (SQLSTATE `23000`, identity in `kind()`).
        let (status, _, body) = render(AppError::Db(stub_unique_violation_error())).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["type"], format!("{PROBLEM_BASE}/conflict"));

        for error in [
            sqlx::Error::PoolTimedOut,
            sqlx::Error::PoolClosed,
            sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "refused",
            )),
        ] {
            let (status, _, body) = render(AppError::Db(error)).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(body["type"], format!("{PROBLEM_BASE}/service-unavailable"));
        }
    }

    #[tokio::test]
    async fn unexpected_db_errors_and_internal_are_generic_500() {
        for error in [
            AppError::Db(stub_db_error("2003")),
            AppError::Db(sqlx::Error::WorkerCrashed),
            AppError::Internal(Internal),
        ] {
            let (status, _, body) = render(error).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(body["type"], format!("{PROBLEM_BASE}/internal"));
            assert_eq!(body["detail"], "An unexpected error occurred.");
            let raw = serde_json::to_string(&body).unwrap();
            assert!(!raw.contains("sqlx"), "driver text leaked: {raw}");
            assert!(!raw.contains("Duplicate"), "sql text leaked: {raw}");
        }
    }
}
