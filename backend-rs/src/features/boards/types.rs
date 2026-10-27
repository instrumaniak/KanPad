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
        SORT_ORDERS, validate_not_blank, validate_optional_hex_color, validate_optional_not_blank,
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
/// present values validate the same rules as create. `project_id` is a
/// double `Option` so explicit `null` (detach) stays observable: missing →
/// `None` (untouched), `null` → `Some(None)` (detach), int →
/// `Some(Some(..))` (relink, ownership-checked to 404).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateBoardInput {
    /// Board name: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
    /// Background hex color, optional.
    #[garde(custom(validate_optional_hex_color))]
    pub background_color: Option<String>,
    /// Owning project link (`>= 1`); explicit `null` detaches.
    #[serde(default, deserialize_with = "deserialize_some")]
    #[garde(custom(validate_optional_project_link))]
    pub project_id: Option<Option<i32>>,
    /// View mode enum (`board` | `list`).
    #[garde(custom(validate_optional_view_mode))]
    pub view_mode: Option<String>,
    /// Archive flag flip (idempotent archive/restore; bool needs no rule).
    #[garde(skip)]
    pub is_archived: Option<bool>,
}

/// `Option<Option<T>>` field decoder: plain serde maps both a missing field
/// and explicit `null` to outer `None`, so detach (`null`) would be
/// indistinguishable from untouched (missing). With
/// `#[serde(default, deserialize_with)]`, missing → `None` via the default,
/// while any present value (incl. `null`) runs the decoder: `null` →
/// `Some(None)`, int → `Some(Some(..))`.
fn deserialize_some<'de, D>(deserializer: D) -> Result<Option<Option<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i32>::deserialize(deserializer).map(Some)
}

/// Project-link rule for [`UpdateBoardInput::project_id`]: untouched (`None`)
/// and detach (`Some(None)`) always pass; a relink id must be `>= 1`
/// (ownership itself is store logic → 404, never 403).
fn validate_optional_project_link(value: &Option<Option<i32>>, _context: &()) -> garde::Result {
    if let Some(Some(project_id)) = value
        && *project_id < 1
    {
        return Err(garde::Error::new("project_id must be at least 1"));
    }
    Ok(())
}

/// Allowed `?sort=` keys for `GET /api/v1/boards` (whitelisted `match` in
/// the store — never interpolated, so violations 400 here instead).
pub const BOARD_SORTS: [&str; 3] = ["name", "created_at", "updated_at"];

/// Default list sort (matches Nest `findAllByUserId` `updated_at DESC`).
pub const DEFAULT_BOARD_SORT: &str = "updated_at";

/// Default list order.
pub const DEFAULT_BOARD_ORDER: &str = "desc";

/// `?sort=` allowlist: `name` | `created_at` | `updated_at`.
fn validate_optional_board_sort(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(sort) = value
        && !BOARD_SORTS.contains(&sort.as_str())
    {
        return Err(garde::Error::new(format!(
            "sort must be one of: {}",
            BOARD_SORTS.join(", ")
        )));
    }
    Ok(())
}

/// `?order=` allowlist: `asc` | `desc` (mirrors the column `sort_order` enum).
fn validate_optional_list_order(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(order) = value
        && !SORT_ORDERS.contains(&order.as_str())
    {
        return Err(garde::Error::new(format!(
            "order must be one of: {}",
            SORT_ORDERS.join(", ")
        )));
    }
    Ok(())
}

/// `GET /api/v1/boards` query (T21b handler; pagination rides the shared
/// strict [`crate::extract::PaginationParams`] extractor).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BoardListQuery {
    /// Project filter (`>= 1`); garbage (`?project_id=abc`) never reaches
    /// garde — axum rejects it as 400 `invalid-query` first. A foreign or
    /// missing id matches zero rows (empty page, never a 403/404 oracle).
    #[garde(range(min = 1))]
    pub project_id: Option<i32>,
    /// `archived=true` includes archived boards, else hidden.
    #[garde(skip)]
    pub archived: Option<bool>,
    /// Sort key allowlist (`name` | `created_at` | `updated_at`).
    #[garde(custom(validate_optional_board_sort))]
    pub sort: Option<String>,
    /// Sort direction (`asc` | `desc`).
    #[garde(custom(validate_optional_list_order))]
    pub order: Option<String>,
}

impl BoardListQuery {
    /// Whether archived boards are included (`archived=true` only).
    pub fn include_archived(&self) -> bool {
        self.archived.unwrap_or(false)
    }

    /// Resolved sort key (validated allowlist; defaults to `updated_at`).
    pub fn resolved_sort(&self) -> &str {
        self.sort.as_deref().unwrap_or(DEFAULT_BOARD_SORT)
    }

    /// Resolved sort direction (validated enum; defaults to `desc`).
    pub fn resolved_order(&self) -> &str {
        self.order.as_deref().unwrap_or(DEFAULT_BOARD_ORDER)
    }
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

/// Wire board list item: the flat board shape (mirrors the Nest
/// `findAll`/`findArchived` mapping — no nested `columns[]`, no
/// `description`/`content` keys). Timestamps render as UTC ISO-8601 strings
/// (same fixed shape as detail — `NaiveDateTime` has no utoipa `ToSchema`
/// impl with the `chrono` feature off, so the store formats once instead of
/// leaking the representation into every handler).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BoardListResponse {
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
    /// Archive flag.
    pub is_archived: bool,
    /// View mode (`board` | `list`).
    pub view_mode: String,
}

pub use crate::common::time::format_timestamp;

#[cfg(test)]
mod tests {
    use super::*;
    use garde::Validate;

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

    #[test]
    fn list_query_defaults_match_nest_ordering() {
        let query = BoardListQuery {
            project_id: None,
            archived: None,
            sort: None,
            order: None,
        };
        assert!(query.validate().is_ok());
        assert!(!query.include_archived());
        assert_eq!(query.resolved_sort(), "updated_at");
        assert_eq!(query.resolved_order(), "desc");
    }

    #[test]
    fn list_query_rejects_sort_and_order_violations() {
        for (sort, order) in [
            (Some("evil".to_string()), None),
            (Some("name;DROP TABLE boards".to_string()), None),
            (None, Some("sideways".to_string())),
            (None, Some("name".to_string())),
        ] {
            let query = BoardListQuery {
                project_id: None,
                archived: None,
                sort,
                order,
            };
            assert!(
                query.validate().is_err(),
                "must reject sort/order violation"
            );
        }
    }
}
