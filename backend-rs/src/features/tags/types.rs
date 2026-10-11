//! tags DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Names trim + non-empty with dup-name → 409 (update gains the 409/trim/
//! not-empty parity create already had). `color` is free-form `<= 20` chars
//! (NOT the label enum); the server defaults it to teal (T28 store logic).

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{validate_not_blank, validate_optional_not_blank};

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
