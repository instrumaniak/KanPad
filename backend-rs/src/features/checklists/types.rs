//! checklists DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Item `position` is int-validated `>= 0`: negatives 400 (current Nest
//! `CreateChecklistItemDto.position` is bare `@IsNumber()`, accepting
//! negatives/floats); floats never reach garde — serde rejects them first.

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{validate_not_blank, validate_optional_not_blank};

/// `POST /api/v1/cards/{cardId}/checklists` body (T26a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateChecklistInput {
    /// Checklist title: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub title: String,
}

/// `PATCH /api/v1/checklists/{id}` body (T26a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateChecklistInput {
    /// Checklist title: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub title: Option<String>,
}

/// `POST /api/v1/checklists/{id}/items` body (T26b handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateChecklistItemInput {
    /// Item text: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub text: String,
    /// Item position (`>= 0`; absent → `MAX+1` in store).
    #[garde(range(min = 0))]
    pub position: Option<i32>,
}

/// `PATCH /api/v1/checklist-items/{itemId}` body (T26b handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateChecklistItemInput {
    /// Item text: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub text: Option<String>,
    /// Completion flag (bool needs no rule).
    #[garde(skip)]
    pub is_completed: Option<bool>,
}
