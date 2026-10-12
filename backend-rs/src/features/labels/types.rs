//! labels DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Names are 1..=50 chars with whitespace-only → 400 (same status as Nest's
//! service-level `BadRequestException`, new `errors[]` shape). `color` is
//! the 6-value enum. Per-user uniqueness → 409 stays store logic (T25).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    ids::LabelId,
    validation::{
        validate_label_color, validate_not_blank, validate_optional_label_color,
        validate_optional_not_blank,
    },
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

/// Wire label item: timestamps render as UTC ISO-8601 strings (`NaiveDateTime`
/// has no utoipa `ToSchema` impl with the `chrono` feature off, so the store
/// formats once here instead of leaking the representation into handlers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct LabelResponse {
    /// `labels.id` (`INT` ↔ `i32` newtype).
    pub id: LabelId,
    /// Label name (stored trimmed).
    pub name: String,
    /// Label color (one of the 6 enum values).
    pub color: String,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
}

/// Success envelope for create/update (`201 {data} + Location`, `200 {data}`;
/// no `message` key — dropped everywhere per `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LabelBody {
    /// Created/updated label.
    pub data: LabelResponse,
}

/// Render a `DATETIME(6)` (server pinned UTC per `30-db-schema.md`) as a
/// fixed-shape UTC ISO-8601 string (micros always present, lexicographically
/// sortable). Single owner: the store calls this, handlers never format.
pub fn format_timestamp(value: &chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}
