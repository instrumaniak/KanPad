//! boards DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `name` is non-blank after trim (new strictness: Nest `CreateBoardDto`
//! lacks `IsNotEmpty`, so `""` creates today). `project_id` is int-validated
//! (`>= 1`) with null allowed; ownership (foreign id → 404) is T21 store
//! logic, not validation.

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{
    validate_not_blank, validate_optional_hex_color, validate_optional_not_blank,
    validate_optional_view_mode,
};

/// `POST /api/v1/boards` body (T21a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateBoardInput {
    /// Board name: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub name: String,
    /// Background hex color (`#0079BF` shape), optional.
    #[garde(custom(validate_optional_hex_color))]
    pub background_color: Option<String>,
    /// Owning project id (`>= 1`), optional.
    #[garde(range(min = 1))]
    pub project_id: Option<i32>,
}

/// `PATCH /api/v1/boards/{id}` body (T22 handler): every field optional;
/// present values validate the same rules as create.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateBoardInput {
    /// Board name: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
    /// Background hex color, optional.
    #[garde(custom(validate_optional_hex_color))]
    pub background_color: Option<String>,
    /// Owning project id (`>= 1`); null detaches.
    #[garde(range(min = 1))]
    pub project_id: Option<i32>,
    /// View mode enum (`board` | `list`).
    #[garde(custom(validate_optional_view_mode))]
    pub view_mode: Option<String>,
    /// Archive flag flip (idempotent archive/restore; bool needs no rule).
    #[garde(skip)]
    pub is_archived: Option<bool>,
}

/// `GET /api/v1/boards` query (T21b handler; pagination rides the shared
/// strict [`crate::extract::PaginationParams`] extractor).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct BoardListQuery {
    /// Project filter (`>= 1`); garbage (`?project_id=abc`) never reaches
    /// garde — axum rejects it as 400 `invalid-query` first.
    #[garde(range(min = 1))]
    pub project_id: Option<i32>,
    /// `archived=true` includes archived boards, else hidden.
    #[garde(skip)]
    pub archived: Option<bool>,
}
