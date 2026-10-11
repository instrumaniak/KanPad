//! columns DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `target_column_id >= 1` (same-board/same-id conflicts stay 400 state
//! checks in the T23 store, not validation). `sort_order` is `asc` | `desc`.

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{validate_optional_not_blank, validate_sort_order};

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
