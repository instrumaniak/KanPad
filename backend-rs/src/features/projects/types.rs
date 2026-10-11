//! projects DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Update requires a non-blank `name` at the DTO layer (retires the
//! service-level `BadRequestException('At least one field…')` with a 400 +
//! `errors[]` field map instead).

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{validate_not_blank, validate_optional_not_blank};

/// `POST /api/v1/projects` body (T20 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateProjectInput {
    /// Project name: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub name: String,
}

/// `PATCH /api/v1/projects/{id}` body (T20 handler): `name` is required
/// (missing/null/blank → 400 with `errors[]`).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateProjectInput {
    /// Project name (required on update).
    #[garde(required, length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
}
