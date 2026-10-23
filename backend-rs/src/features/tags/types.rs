//! tags DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Names trim + non-empty with dup-name → 409 (update gains the 409/trim/
//! not-empty parity create already had). `color` is free-form `<= 20` chars
//! (NOT the label enum); the server defaults it to teal (T28 store logic).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    ids::TagId,
    validation::{validate_not_blank, validate_optional_not_blank},
};

/// `POST /api/v1/tags` body (T28 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateTagInput {
    /// Tag name: non-blank after trim, `<= 50` chars.
    #[garde(length(max = 50), custom(validate_not_blank))]
    pub name: String,
    /// Tag color, free-form `<= 20` chars (absent → teal in store).
    #[garde(length(max = 20))]
    pub color: Option<String>,
}

/// `PATCH /api/v1/tags/{id}` body (T28 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateTagInput {
    /// Tag name: present means non-blank, `<= 50` chars.
    #[garde(length(max = 50), custom(validate_optional_not_blank))]
    pub name: Option<String>,
    /// Tag color, free-form `<= 20` chars.
    #[garde(length(max = 20))]
    pub color: Option<String>,
}

/// Wire tag item: timestamps render as UTC ISO-8601 strings (`NaiveDateTime`
/// has no utoipa `ToSchema` impl with the `chrono` feature off, so the store
/// formats once here instead of leaking the representation into handlers).
/// Tags carry `created_at` ONLY — no `updated_at` by design (see
/// `30-db-schema.md`; tags are rename-light).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct TagResponse {
    /// `tags.id` (`INT` ↔ `i32` newtype).
    pub id: TagId,
    /// Tag name (stored trimmed).
    pub name: String,
    /// Tag color (free-form, defaults to teal server-side).
    pub color: String,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
}

/// Success envelope for create/update (`201 {data} + Location`, `200 {data}`;
/// no `message` key — dropped everywhere per `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagBody {
    /// Created/updated tag.
    pub data: TagResponse,
}

pub use crate::common::time::format_timestamp;
