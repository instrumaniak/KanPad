//! OpenAPI surface. Per-handler `#[utoipa::path]` annotations and DTO
//! `#[aliases]` for `Page<T>` land with the resource tasks (T40a owns the
//! export snapshot); T04 registers the shared problem schemas so the error
//! shape is in the spec from the start.

use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{FieldError, ProblemDetails};

/// Liveness response body `{status:"ok"}`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Always `"ok"`.
    pub status: String,
}

/// API documentation (health + shared problem schemas until resource tasks
/// annotate DTOs).
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(crate::health),
    components(schemas(HealthResponse, ProblemDetails, FieldError))
)]
pub struct ApiDoc;
