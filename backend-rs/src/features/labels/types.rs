//! labels DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Names are 1..=50 chars with whitespace-only → 400 (same status as Nest's
//! service-level `BadRequestException`, new `errors[]` shape). `color` is
//! the 6-value enum. Per-user uniqueness → 409 stays store logic (T25).

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{
    validate_label_color, validate_not_blank, validate_optional_label_color,
    validate_optional_not_blank,
};

/// `POST /api/v1/labels` body (T25 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateLabelInput {
    /// Label name: 1..=50 chars, non-blank after trim.
    #[garde(length(min = 1, max = 50), custom(validate_not_blank))]
    pub name: String,
    /// Label color enum (6 values).
    #[garde(custom(validate_label_color))]
    pub color: String,
}

/// `PATCH /api/v1/labels/{id}` body (T25 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateLabelInput {
    /// Label name: present means 1..=50 chars and non-blank.
    #[garde(length(min = 1, max = 50), custom(validate_optional_not_blank))]
    pub name: Option<String>,
    /// Label color enum (6 values).
    #[garde(custom(validate_optional_label_color))]
    pub color: Option<String>,
}
