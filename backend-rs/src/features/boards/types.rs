//! boards DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `name` is non-blank after trim (new strictness: Nest `CreateBoardDto`
//! lacks `IsNotEmpty`, so `""` creates today). `project_id` is int-validated
//! (`>= 1`) with null allowed; ownership (foreign id → 404) is T21 store
//! logic, not validation.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    ids::{BoardId, ColumnId, ProjectId},
    validation::{
        validate_not_blank, validate_optional_hex_color, validate_optional_not_blank,
        validate_optional_view_mode,
    },
};

/// Default `background_color` when the create body omits it (mirrors Nest
/// `dto.background_color || '#0079BF'`).
pub const DEFAULT_BOARD_COLOR: &str = "#0079BF";

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

/// One nested column in a board detail body (`id`, `name`, `position` only —
/// never cards; the column-cards list lives under `GET /columns/:col/cards`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ColumnSummary {
    /// `columns.id` (`INT` ↔ `i32` newtype).
    pub id: ColumnId,
    /// Column name.
    pub name: String,
    /// Zero-based position (`position ASC` order on the wire).
    pub position: i32,
}

/// Owning-project ref embedded in a board detail body (`{id,name} | null`,
/// mirroring the Nest `BoardResponse.project` shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ProjectRef {
    /// `projects.id` (`INT` ↔ `i32` newtype).
    pub id: ProjectId,
    /// Project name.
    pub name: String,
}

/// Wire board detail: the board plus its nested unpaginated `columns[]`
/// (`position ASC`). Carries no `description`/`content` keys (boards have
/// neither column); timestamps render as UTC ISO-8601 strings (same fixed
/// shape as projects — `NaiveDateTime` has no utoipa `ToSchema` impl with
/// the `chrono` feature off, so the store formats once here instead of
/// leaking the representation into every handler).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BoardDetailResponse {
    /// `boards.id` (`INT` ↔ `i32` newtype).
    pub id: BoardId,
    /// Board name.
    pub name: String,
    /// Background hex color.
    pub background_color: String,
    /// Owning project id (`null` = standalone).
    pub project_id: Option<ProjectId>,
    /// Owning project ref (`null` = standalone).
    pub project: Option<ProjectRef>,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
    /// Nested columns, `position ASC` (no cards).
    pub columns: Vec<ColumnSummary>,
    /// Archive flag.
    pub is_archived: bool,
    /// View mode (`board` | `list`).
    pub view_mode: String,
}

/// Success envelope for create/detail (`201 {data} + Location`, `200
/// {data}`; no `message` key — dropped everywhere per `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BoardDetailBody {
    /// Created/fetched board with nested columns.
    pub data: BoardDetailResponse,
}

/// Render a `DATETIME(6)` (server pinned UTC per `30-db-schema.md`) as a
/// fixed-shape UTC ISO-8601 string (micros always present, lexicographically
/// sortable). Boards-local owner (same shape as the projects helper; kept
/// per-feature so neither feature imports the other's store).
pub fn format_timestamp(value: &chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_renders_fixed_shape_utc() {
        let parsed = chrono::NaiveDateTime::parse_from_str(
            "2026-10-11 12:34:56.123456",
            "%Y-%m-%d %H:%M:%S%.f",
        )
        .expect("fixture datetime parses");
        assert_eq!(format_timestamp(&parsed), "2026-10-11T12:34:56.123456Z");
    }

    #[test]
    fn default_board_color_is_nest_parity() {
        assert_eq!(DEFAULT_BOARD_COLOR, "#0079BF");
    }
}
