//! OpenAPI stub (T03). Full `ApiDoc` + per-handler annotations land in T04/T40a.

use serde::Serialize;
use utoipa::ToSchema;

/// Liveness response body `{status:"ok"}`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Always `"ok"`.
    pub status: String,
}

/// Stub API documentation (health only until resource tasks annotate DTOs).
#[derive(utoipa::OpenApi)]
#[openapi(paths(crate::health), components(schemas(HealthResponse)))]
pub struct ApiDoc;
