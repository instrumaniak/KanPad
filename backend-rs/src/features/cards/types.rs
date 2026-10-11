//! cards DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `description` is `<= 10_000` chars (chars, both DTOs) with null allowed
//! and no not-blank rule (mirrors Nest). `due_date` is ISO-8601
//! valid-date-or-null. Card `position` is intentionally range-free: out-of
//! range values clamp in the store (only checklist-item positions 400).

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{
    validate_due_date, validate_not_blank, validate_optional_not_blank,
};

/// `POST /api/v1/cards` body (T24a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateCardInput {
    /// Card title: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub title: String,
    /// Owning column id (`>= 1`).
    #[garde(range(min = 1))]
    pub column_id: i32,
    /// Explicit position (insert-with-open-gap; clamped in store, never 400).
    #[garde(skip)]
    pub position: Option<i32>,
    /// Card description (`<= 10_000` chars); null allowed.
    #[garde(length(max = 10_000))]
    pub description: Option<String>,
    /// Due date: ISO-8601 valid-date-or-null.
    #[garde(custom(validate_due_date))]
    pub due_date: Option<String>,
}

/// `PATCH /api/v1/cards/{id}` body (T24a/T24b handlers): scalar + move fields.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateCardInput {
    /// Card title: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub title: Option<String>,
    /// Target column id for cross-column moves (`>= 1`).
    #[garde(range(min = 1))]
    pub column_id: Option<i32>,
    /// Target position (clamped in store, never 400).
    #[garde(skip)]
    pub position: Option<i32>,
    /// Card description (`<= 10_000` chars); null allowed.
    #[garde(length(max = 10_000))]
    pub description: Option<String>,
    /// Due date: ISO-8601 valid-date-or-null.
    #[garde(custom(validate_due_date))]
    pub due_date: Option<String>,
}
