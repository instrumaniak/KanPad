//! checklists DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Item `position` is int-validated `>= 0`: negatives 400 (current Nest
//! `CreateChecklistItemDto.position` is bare `@IsNumber()`, accepting
//! negatives/floats); floats never reach garde — serde rejects them first.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    common::ids::{CardId, ChecklistId, ChecklistItemId},
    common::validation::{validate_not_blank, validate_optional_not_blank},
};

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

/// One checklist item on the wire (T26a nests; T26b owns the item routes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChecklistItemResponse {
    /// `checklist_items.id` (`INT` ↔ `i32` newtype).
    pub id: ChecklistItemId,
    /// Item text.
    pub text: String,
    /// Completion flag.
    pub is_completed: bool,
    /// Owning checklist id.
    pub checklist_id: ChecklistId,
    /// Zero-based position.
    pub position: i32,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
}

/// One checklist on the wire, with items (`position ASC`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChecklistResponse {
    /// `checklists.id` (`INT` ↔ `i32` newtype).
    pub id: ChecklistId,
    /// Checklist title.
    pub title: String,
    /// Owning card id.
    pub card_id: CardId,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
    /// Items, `position ASC`.
    pub items: Vec<ChecklistItemResponse>,
}

/// Success envelope for create/detail/update (`201 {data} + Location`,
/// `200 {data}`; no `message` key — dropped everywhere per
/// `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChecklistBody {
    /// Created/fetched/updated checklist.
    pub data: ChecklistResponse,
}

/// Success envelope for the card-scoped list (`200 {data[]}`, small-N
/// unpaginated per `21 §checklists` — added route).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChecklistListBody {
    /// The card's checklists, `id ASC`.
    pub data: Vec<ChecklistResponse>,
}

/// Render a `DATETIME(6)` (server pinned UTC per `30-db-schema.md`) as a
/// fixed-shape UTC ISO-8601 string (same shape as the cards/columns helpers;
/// kept per-feature so features never import each other's stores).
pub fn format_timestamp(value: &chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}
