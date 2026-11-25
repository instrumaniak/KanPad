//! columns DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `target_column_id >= 1` (same-board/same-id conflicts stay 400 state
//! checks in the T23 store, not validation). `sort_order` is `asc` | `desc`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    card_filters::{
        self, CardFilters, validate_labels_filter, validate_optional_checklist_filter,
        validate_optional_due_date_filter, validate_optional_search,
    },
    card_summary::CardSummary,
    ids::{BoardId, ColumnId},
    validation::{validate_optional_not_blank, validate_sort_order},
};

/// Default number of cards embedded per column when `embed_limit` is omitted.
pub const DEFAULT_EMBED_LIMIT: u32 = 10;
/// Hard ceiling for embedded cards per column (`100 * N columns * summary payload`).
pub const MAX_EMBED_LIMIT: u32 = 100;

/// `GET /api/v1/boards/{boardId}/columns` query.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ColumnListQuery {
    /// Max cards embedded per column (default 10, min 1, max 100).
    #[garde(custom(validate_embed_limit))]
    pub embed_limit: Option<String>,
    /// Case-insensitive title search (trimmed; empty/whitespace = no filter).
    #[garde(custom(validate_optional_search))]
    pub search: Option<String>,
    /// Repeated label IDs (AND semantics). `?labels=1&labels=2`.
    #[serde(
        default,
        deserialize_with = "crate::common::card_filters::deserialize_string_or_seq"
    )]
    #[garde(custom(validate_labels_filter))]
    pub labels: Vec<String>,
    /// Due-date filter (`overdue` | `today` | `this_week` | `no_date`).
    #[garde(custom(validate_optional_due_date_filter))]
    pub due_date: Option<String>,
    /// Checklist filter (`with` | `complete` | `incomplete`).
    #[garde(custom(validate_optional_checklist_filter))]
    pub checklist: Option<String>,
}

impl ColumnListQuery {
    /// Parse and return the validated embed limit (default when omitted).
    pub fn resolved_embed_limit(&self) -> Result<u32, garde::Report> {
        match self.embed_limit.as_deref() {
            None => Ok(DEFAULT_EMBED_LIMIT),
            Some(raw) => parse_embed_limit(raw).map_err(|message| {
                let mut report = garde::Report::new();
                report.append(garde::Path::new("embed_limit"), garde::Error::new(message));
                report
            }),
        }
    }

    /// Resolved/validated card filters.
    pub fn resolved_filters(&self) -> Result<CardFilters, garde::Report> {
        card_filters::parse_card_filters(
            self.search.as_deref(),
            &self.labels,
            self.due_date.as_deref(),
            self.checklist.as_deref(),
        )
    }
}

fn parse_embed_limit(raw: &str) -> Result<u32, &'static str> {
    if raw.is_empty() {
        return Err("must be an integer");
    }
    let parsed: u32 = raw.parse().map_err(|_| "must be an integer")?;
    if parsed < 1 {
        return Err("must be at least 1");
    }
    if parsed > MAX_EMBED_LIMIT {
        return Err("must be at most 100");
    }
    Ok(parsed)
}

fn validate_embed_limit(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(raw) = value {
        parse_embed_limit(raw).map_err(garde::Error::new)?;
    }
    Ok(())
}

/// `POST /api/v1/boards/{boardId}/columns` body (T23a handler): absent name
/// defaults to `New Column` in the store.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateColumnInput {
    /// Column name (`<= 255` chars); present means non-blank.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
}

/// `PATCH /api/v1/columns/{id}` body (T23a rename + T23b sort + T010 move):
/// any key may appear (both are applied, rename first, then move). Absent
/// keys leave that aspect untouched, so `{}` stays a no-op 200.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateColumnInput {
    /// Column name (`<= 255` chars); present means non-blank.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub name: Option<String>,
    /// Card sort direction (`asc` | `desc`); rewrites card sort keys by
    /// `created_at` via one set-based `UPDATE ... CASE`.
    #[garde(custom(validate_optional_sort_order))]
    pub sort_order: Option<String>,
    /// Target position (clamped to `[0, N-1]` in the store, never 400);
    /// reorder lands a single between-neighbours `sort_key` write.
    #[garde(skip)]
    pub position: Option<i32>,
}

/// Optional variant of [`validate_sort_order`] for the PATCH body (`None`
/// means "leave card order alone", `Some` must be `asc` | `desc`). Local to
/// this feature (mirrors the per-feature optional validators in
/// `boards::types`) so the shared catalogue stays untouched.
fn validate_optional_sort_order(value: &Option<String>, context: &()) -> garde::Result {
    if let Some(inner) = value {
        validate_sort_order(inner, context)?;
    }
    Ok(())
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

/// `POST /api/v1/columns/{id}/move-cards` success envelope (T23c handler):
/// `200 {data: {moved_count}}` (no `message` key — dropped everywhere per
/// `20-api-contract.md`; `movedCount→moved_count` per `21 §columns`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct MoveCardsData {
    /// Number of cards moved into the target column (`0` when the source is
    /// empty — an empty source is a success, never an error).
    pub moved_count: i64,
}

/// Success envelope for move-cards (`200 {data: {moved_count}}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct MoveCardsBody {
    /// `{moved_count}` payload.
    pub data: MoveCardsData,
}

/// Default name when the create body omits `name` (mirrors Nest
/// `dto.name || 'New Column'`).
pub const DEFAULT_COLUMN_NAME: &str = "New Column";

/// Wire column: scalars + nested card summaries (sort-key order, never
/// `description` TEXT — see [`crate::common::card_summary`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ColumnResponse {
    /// `columns.id` (`INT` ↔ `i32` newtype).
    pub id: ColumnId,
    /// Column name.
    pub name: String,
    /// Zero-based position (computed; sort-key order on the wire).
    pub position: i32,
    /// Owning board id.
    pub board_id: BoardId,
    /// Nested card summaries (sort-key order; first N embedded, rest via pagination).
    pub cards: Vec<CardSummary>,
    /// Total cards in this column (authoritative, unfiltered; may exceed `cards.len()`).
    pub total_cards: u64,
    /// Matching cards in this column under active filters (equals `total_cards` when no filter).
    pub matching_cards: u64,
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
    /// Board columns, sort-key order.
    pub data: Vec<ColumnResponse>,
}

pub use crate::common::time::format_timestamp;

impl<S> axum::extract::FromRequestParts<S> for ColumnListQuery
where
    S: Send + Sync,
{
    type Rejection = crate::error::AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or("");
        let pairs = crate::common::card_filters::parse_query_pairs(query);
        let mut embed_limit = None;
        let mut search = None;
        let mut labels = Vec::new();
        let mut due_date = None;
        let mut checklist = None;
        for (k, v) in pairs {
            match k.as_str() {
                "embed_limit" => embed_limit = Some(v),
                "search" => search = Some(v),
                "labels" => labels.push(v),
                "due_date" => due_date = Some(v),
                "checklist" => checklist = Some(v),
                other => {
                    let mut report = garde::Report::new();
                    report.append(
                        garde::Path::new(other),
                        garde::Error::new("unknown query parameter"),
                    );
                    return Err(crate::error::AppError::from(report));
                }
            }
        }
        let value = Self {
            embed_limit,
            search,
            labels,
            due_date,
            checklist,
        };
        {
            use garde::Validate;
            value.validate().map_err(crate::error::AppError::from)?;
        }
        Ok(value)
    }
}
