//! projects DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Update requires a non-blank `name` at the DTO layer (retires the
//! service-level `BadRequestException('At least one field…')` with a 400 +
//! `errors[]` field map instead).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    ids::ProjectId,
    validation::{SORT_ORDERS, validate_not_blank, validate_optional_not_blank},
};

/// Allowed `?sort=` keys for `GET /api/v1/projects` (whitelisted `match` in
/// the store — never interpolated, so violations 400 here instead).
pub const PROJECT_SORTS: [&str; 3] = ["name", "created_at", "updated_at"];

/// Default list sort (matches Nest `findAllByUserId` `created_at DESC`).
pub const DEFAULT_PROJECT_SORT: &str = "created_at";

/// Default list order.
pub const DEFAULT_PROJECT_ORDER: &str = "desc";

/// `?sort=` allowlist: `name` | `created_at` | `updated_at`.
fn validate_optional_project_sort(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(sort) = value
        && !PROJECT_SORTS.contains(&sort.as_str())
    {
        return Err(garde::Error::new(format!(
            "sort must be one of: {}",
            PROJECT_SORTS.join(", ")
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

/// `POST /api/v1/projects` body (T20 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateProjectInput {
    /// Project name: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub name: String,
}

/// `PATCH /api/v1/projects/{id}` body (T20 handler): `name` is required
/// (missing/null/blank → 400 with `errors[]`).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateProjectInput {
    /// Project name (required on update).
    #[garde(required, length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
}

/// `GET /api/v1/projects` query (T20 handler; pagination rides the shared
/// strict [`crate::extract::PaginationParams`] extractor).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProjectListQuery {
    /// Name substring filter (`LIKE %q%` with `%`/`_`/`\` escaped in Rust +
    /// `push_bind` only — never interpolated).
    #[garde(skip)]
    #[param(nullable = false)]
    pub q: Option<String>,
    /// Sort key allowlist (`name` | `created_at` | `updated_at`).
    #[garde(custom(validate_optional_project_sort))]
    pub sort: Option<String>,
    /// Sort direction (`asc` | `desc`).
    #[garde(custom(validate_optional_list_order))]
    pub order: Option<String>,
}

impl ProjectListQuery {
    /// Resolved sort key (validated allowlist; defaults to `created_at`).
    pub fn resolved_sort(&self) -> &str {
        self.sort.as_deref().unwrap_or(DEFAULT_PROJECT_SORT)
    }

    /// Resolved sort direction (validated enum; defaults to `desc`).
    pub fn resolved_order(&self) -> &str {
        self.order.as_deref().unwrap_or(DEFAULT_PROJECT_ORDER)
    }
}

/// Wire project item: `board_count` is real (single `LEFT JOIN ... GROUP BY`
/// aggregation — the old Nest `boardCount: 0` stub is dropped) and
/// timestamps render as UTC ISO-8601 strings (`NaiveDateTime` has no utoipa
/// `ToSchema` impl with the `chrono` feature off, so the store formats once
/// here instead of leaking the representation into every handler).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ProjectResponse {
    /// `projects.id` (`INT` ↔ `i32` newtype).
    pub id: ProjectId,
    /// Project name.
    pub name: String,
    /// Boards linked to this project (user-scoped join, never N+1).
    pub board_count: i64,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
}

/// Success envelope for create/update (`201 {data} + Location`, `200 {data}`;
/// no `message` key — dropped everywhere per `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProjectBody {
    /// Created/updated project.
    pub data: ProjectResponse,
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
    fn list_query_defaults_match_nest_ordering() {
        let query = ProjectListQuery {
            q: None,
            sort: None,
            order: None,
        };
        assert!(query.validate().is_ok());
        assert_eq!(query.resolved_sort(), "created_at");
        assert_eq!(query.resolved_order(), "desc");
    }

    #[test]
    fn list_query_rejects_sort_and_order_violations() {
        for (sort, order) in [
            (Some("evil".to_string()), None),
            (Some("name;DROP TABLE projects".to_string()), None),
            (None, Some("sideways".to_string())),
            (None, Some("name".to_string())),
        ] {
            let query = ProjectListQuery {
                q: None,
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
