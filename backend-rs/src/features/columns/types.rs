//! columns DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `target_column_id >= 1` (same-board/same-id conflicts stay 400 state
//! checks in the T23 store, not validation). `sort_order` is `asc` | `desc`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    card_summary::CardSummary,
    ids::{BoardId, ColumnId},
    validation::{validate_optional_not_blank, validate_sort_order},
};

/// `POST /api/v1/boards/{boardId}/columns` body (T23a handler): absent name
/// defaults to `New Column` in the store.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateColumnInput {
    /// Column name (`<= 255` chars); present means non-blank.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
}

/// `PATCH /api/v1/columns/{id}` body (T23a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateColumnInput {
    /// Column name (`<= 255` chars); present means non-blank.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
}

/// `PATCH /api/v1/columns/{id}` sort body (T23b handler): rewrites positions
/// by `created_at`. The wire field is `sort_order` (Nest's `order` renamed
/// per `20-api-contract.md`).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct SortColumnInput {
    /// Sort direction enum (`asc` | `desc`).
    #[garde(custom(validate_sort_order))]
    pub sort_order: String,
}

/// `POST /api/v1/columns/{id}/move-cards` body (T23c handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct MoveCardsInput {
    /// Target column id (`>= 1`).
    #[garde(range(min = 1))]
    pub target_column_id: i32,
}

/// Default name when the create body omits `name` (mirrors Nest
/// `dto.name || 'New Column'`).
pub const DEFAULT_COLUMN_NAME: &str = "New Column";

/// Wire column: scalars + nested card summaries (`position ASC`, never
/// `description` TEXT — see [`crate::common::card_summary`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ColumnResponse {
    /// `columns.id` (`INT` ↔ `i32` newtype).
    pub id: ColumnId,
    /// Column name.
    pub name: String,
    /// Zero-based position (`position ASC` order on the wire).
    pub position: i32,
    /// Owning board id.
    pub board_id: BoardId,
    /// Nested card summaries (`position ASC`).
    pub cards: Vec<CardSummary>,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
}

/// Success envelope for create/update (`201 {data} + Location`, `200
/// {data}`; no `message` key — dropped everywhere per `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ColumnBody {
    /// Created/updated column with nested cards.
    pub data: ColumnResponse,
}

/// Success envelope for the nested list (`200 {data[]}`, unpaginated —
/// paginating board columns would break board rendering).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ColumnListBody {
    /// Board columns, `position ASC`.
    pub data: Vec<ColumnResponse>,
}

/// Render a `DATETIME(6)` (server pinned UTC per `30-db-schema.md`) as a
/// fixed-shape UTC ISO-8601 string (same shape as the boards helper; kept
/// per-feature so neither feature imports the other's store).
pub fn format_timestamp(value: &chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}
