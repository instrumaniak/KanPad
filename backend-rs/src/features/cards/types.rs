//! cards DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `description` is `<= 10_000` chars (chars, both DTOs) with null allowed
//! and no not-blank rule (mirrors Nest). `due_date` is ISO-8601
//! valid-date-or-null. Card `position` is intentionally range-free: out-of
//! range values clamp in the store (only checklist-item positions 400).
//!
//! `PATCH` scalar nullability rides a double `Option` for `description` and
//! `due_date` (same pattern as boards `project_id`): missing → `None`
//! (untouched), explicit `null` → `Some(None)` (clear), value →
//! `Some(Some(..))` (set). `title` stays a single `Option` (Nest rejects
//! `null` titles via `IsString`). Move fields (`column_id`, `position`) are
//! accepted on the DTO but IGNORED by the T24a scalars-only handler — moves
//! land in T24b.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::common::{
    card_summary::{CardLabelRef, ChecklistProgress},
    ids::{CardId, ChecklistId, ColumnId},
    validation::{SORT_ORDERS, validate_due_date, validate_not_blank, validate_optional_not_blank},
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
    /// Explicit position (between-neighbours key; clamped in store, never 400).
    #[garde(skip)]
    pub position: Option<i32>,
    /// Card description (`<= 10_000` chars); null allowed.
    #[garde(length(max = 10_000))]
    pub description: Option<String>,
    /// Due date: ISO-8601 valid-date-or-null.
    #[garde(custom(validate_due_date))]
    pub due_date: Option<String>,
}

/// `PATCH /api/v1/cards/{id}` body (T24a scalars-only handler; T24b adds
/// move semantics for `column_id`/`position`).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateCardInput {
    /// Card title: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub title: Option<String>,
    /// Target column id for cross-column moves (`>= 1`; T24b owns it —
    /// the T24a handler ignores it).
    #[garde(range(min = 1))]
    pub column_id: Option<i32>,
    /// Target position (clamped in store, never 400; T24b owns it — the T24a
    /// handler ignores it).
    #[garde(skip)]
    pub position: Option<i32>,
    /// Card description (`<= 10_000` chars); explicit `null` clears.
    #[serde(default, deserialize_with = "deserialize_some_string")]
    #[garde(custom(validate_nullable_description))]
    pub description: Option<Option<String>>,
    /// Due date; explicit `null` clears.
    #[serde(default, deserialize_with = "deserialize_some_string")]
    #[garde(custom(validate_nullable_due_date))]
    pub due_date: Option<Option<String>>,
}

/// `Option<Option<String>>` field decoder (same pattern as boards
/// `project_id`): plain serde maps both a missing field and explicit `null`
/// to outer `None`, so clear (`null`) would be indistinguishable from
/// untouched (missing). With `#[serde(default, deserialize_with)]`, missing
/// → `None` via the default, while any present value (incl. `null`) runs the
/// decoder: `null` → `Some(None)`, string → `Some(Some(..))`.
fn deserialize_some_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

/// Nullable description rule: untouched (`None`) and clear (`Some(None)`)
/// always pass; a set value must fit `<= 10_000` chars (no not-blank rule —
/// whitespace-only descriptions are stored, mirroring Nest).
fn validate_nullable_description(value: &Option<Option<String>>, _context: &()) -> garde::Result {
    if let Some(Some(description)) = value
        && description.chars().count() > 10_000
    {
        return Err(garde::Error::new(
            "length: must be at most 10000 characters",
        ));
    }
    Ok(())
}

/// Nullable due-date rule: untouched (`None`) and clear (`Some(None)`)
/// always pass; a set value must be ISO-8601 valid-date (same catalogue rule
/// as create).
fn validate_nullable_due_date(value: &Option<Option<String>>, _context: &()) -> garde::Result {
    match value {
        None | Some(None) => Ok(()),
        Some(Some(raw)) => validate_due_date(&Some(raw.clone()), _context),
    }
}

/// `POST /api/v1/cards/{id}/labels` deprecated alias body (T24c): snake_case
/// `{label_id}` with legacy camelCase `{labelId}` accepted for one release
/// (serde alias — both names decode into the same field). Missing → the
/// handler 400s `validation-error` with `errors[]` on `label_id`; `0` or
/// negative → 400 via the range rule (same `>= 1` shape as move `column_id`).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct AttachLabelInput {
    /// Label id to attach (`>= 1`).
    #[serde(default, alias = "labelId")]
    #[garde(range(min = 1))]
    pub label_id: Option<i32>,
}

/// One checklist item inside a card detail body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChecklistItemResponse {
    /// `checklist_items.id` (`INT` ↔ `i32` newtype).
    pub id: crate::common::ids::ChecklistItemId,
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

/// One checklist inside a card detail body.
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
    /// Items, sort-key order.
    pub items: Vec<ChecklistItemResponse>,
}

/// Wire card detail: scalars (TEXT selected here — never in lists) + label
/// refs + nested checklists{items} + optional progress rollup (present only
/// when the card owns ≥ 1 checklist, mirroring Nest `toCardResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CardDetailResponse {
    /// `cards.id` (`INT` ↔ `i32` newtype).
    pub id: CardId,
    /// Card title.
    pub title: String,
    /// Owning column id.
    pub column_id: ColumnId,
    /// Zero-based position.
    pub position: i32,
    /// Card description (`null` when unset — emitted, never omitted).
    pub description: Option<String>,
    /// `due_date` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (`null` when unset —
    /// emitted, never omitted).
    pub due_date: Option<String>,
    /// `created_at`, same shape.
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
    /// Attached labels (`id` order).
    pub labels: Vec<CardLabelRef>,
    /// Nested checklists (`id` order) with items (sort-key order).
    pub checklists: Vec<ChecklistResponse>,
    /// Checklist rollup (`None` = the card owns no checklist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checklist_progress: Option<ChecklistProgress>,
}

/// Success envelope for create/detail/update (`201 {data} + Location`,
/// `200 {data}`; no `message` key — dropped everywhere per
/// `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CardBody {
    /// Created/fetched/updated card.
    pub data: CardDetailResponse,
}

pub use crate::common::time::format_timestamp;

/// Parse a validated `due_date` input into the `DATETIME(6)` naive UTC value
/// stored in `cards.due_date`. Accepts the same shapes as
/// [`validate_due_date`]: RFC 3339 datetimes (offset → UTC), calendar dates
/// (`YYYY-MM-DD` = midnight UTC), and naive `YYYY-MM-DDTHH:MM:SS[.f]`.
/// Returns `None` only for inputs validation already rejects.
pub fn parse_due_date(raw: &str) -> Option<chrono::NaiveDateTime> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&chrono::Utc).naive_utc());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return date.and_hms_opt(0, 0, 0);
    }
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").ok()
}

/// Allowed `?sort=` keys for `GET /api/v1/boards/{boardId}/cards`.
pub const BOARD_CARD_SORTS: [&str; 4] = ["created_at", "updated_at", "due_date", "title"];
/// Default board cards list sort.
pub const DEFAULT_BOARD_CARD_SORT: &str = "updated_at";
/// Default board cards list order.
pub const DEFAULT_BOARD_CARD_ORDER: &str = "desc";
/// Default page for board cards list.
pub const DEFAULT_BOARD_CARD_PAGE: u32 = 1;
/// Default limit for board cards list.
pub const DEFAULT_BOARD_CARD_LIMIT: u32 = 20;

fn validate_optional_board_card_sort(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(sort) = value
        && !BOARD_CARD_SORTS.contains(&sort.as_str())
    {
        return Err(garde::Error::new(format!(
            "sort must be one of: {}",
            BOARD_CARD_SORTS.join(", ")
        )));
    }
    Ok(())
}

fn validate_optional_board_card_order(value: &Option<String>, _context: &()) -> garde::Result {
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

fn parse_u32_query_field(
    raw: &str,
    field: &'static str,
    min: u32,
    max: u32,
) -> Result<u32, garde::Report> {
    let mut report = garde::Report::new();
    let parsed = match raw.parse::<u32>() {
        Ok(value) => value,
        Err(_) => {
            report.append(
                garde::Path::new(field),
                garde::Error::new("must be an integer"),
            );
            return Err(report);
        }
    };
    if parsed < min {
        report.append(
            garde::Path::new(field),
            garde::Error::new(format!("must be at least {min}")),
        );
        return Err(report);
    }
    if parsed > max {
        report.append(
            garde::Path::new(field),
            garde::Error::new(format!("must be at most {max}")),
        );
        return Err(report);
    }
    Ok(parsed)
}

fn validate_optional_board_card_page(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(raw) = value {
        parse_u32_query_field(raw, "page", 1, u32::MAX).map_err(|report| {
            garde::Error::new(
                report
                    .iter()
                    .next()
                    .map(|(_, err)| err.to_string())
                    .unwrap_or_else(|| "invalid page".to_string()),
            )
        })?;
    }
    Ok(())
}

fn validate_optional_board_card_limit(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(raw) = value {
        parse_u32_query_field(raw, "limit", 1, crate::extract::PaginationParams::MAX_LIMIT)
            .map_err(|report| {
                garde::Error::new(
                    report
                        .iter()
                        .next()
                        .map(|(_, err)| err.to_string())
                        .unwrap_or_else(|| "invalid limit".to_string()),
                )
            })?;
    }
    Ok(())
}

/// `GET /api/v1/boards/{boardId}/cards` query. Single validated DTO so T049 can
/// extend with filters without extractor composition friction.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BoardCardsListQuery {
    /// 1-based page number (default 1).
    #[garde(custom(validate_optional_board_card_page))]
    pub page: Option<String>,
    /// Items per page (default 20, min 1, max 100).
    #[garde(custom(validate_optional_board_card_limit))]
    pub limit: Option<String>,
    /// Sort key allowlist (`created_at` | `updated_at` | `due_date` | `title`).
    #[garde(custom(validate_optional_board_card_sort))]
    pub sort: Option<String>,
    /// Sort direction (`asc` | `desc`).
    #[garde(custom(validate_optional_board_card_order))]
    pub order: Option<String>,
}

impl BoardCardsListQuery {
    /// Resolved page/limit with shared window ceiling validation.
    pub fn resolved_pagination(&self) -> Result<(u32, u32), garde::Report> {
        let mut report = garde::Report::new();
        let page = match self.page.as_deref() {
            None => DEFAULT_BOARD_CARD_PAGE,
            Some(raw) => match parse_u32_query_field(raw, "page", 1, u32::MAX) {
                Ok(value) => value,
                Err(field_report) => {
                    for (path, err) in field_report.iter() {
                        report.append(path.clone(), err.clone());
                    }
                    DEFAULT_BOARD_CARD_PAGE
                }
            },
        };
        let limit = match self.limit.as_deref() {
            None => DEFAULT_BOARD_CARD_LIMIT,
            Some(raw) => match parse_u32_query_field(
                raw,
                "limit",
                1,
                crate::extract::PaginationParams::MAX_LIMIT,
            ) {
                Ok(value) => value,
                Err(field_report) => {
                    for (path, err) in field_report.iter() {
                        report.append(path.clone(), err.clone());
                    }
                    DEFAULT_BOARD_CARD_LIMIT
                }
            },
        };

        if report.is_empty()
            && u64::from(page).saturating_mul(u64::from(limit))
                > crate::extract::PaginationParams::MAX_PAGE_WINDOW
        {
            report.append(
                garde::Path::new("page"),
                garde::Error::new("page is outside the allowed window"),
            );
        }

        if report.is_empty() {
            Ok((page, limit))
        } else {
            Err(report)
        }
    }

    /// Resolved sort key (validated allowlist; defaults to `updated_at`).
    pub fn resolved_sort(&self) -> &str {
        self.sort.as_deref().unwrap_or(DEFAULT_BOARD_CARD_SORT)
    }

    /// Resolved sort direction (validated enum; defaults to `desc`).
    pub fn resolved_order(&self) -> &str {
        self.order.as_deref().unwrap_or(DEFAULT_BOARD_CARD_ORDER)
    }
}

/// Wire board-card list item: card summary fields plus `column_name`.
/// `position` is the per-column computed position in stable column sort-key
/// order (0-based), matching column-embed `CardSummary.position` semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BoardCardListItem {
    /// `cards.id` (`INT` ↔ `i32` newtype).
    pub id: CardId,
    /// Card title.
    pub title: String,
    /// Whether the card has a non-blank description (SQL `CASE`, never TEXT).
    pub has_description: bool,
    /// Owning column id.
    pub column_id: ColumnId,
    /// Owning column name.
    pub column_name: String,
    /// Zero-based per-column position in stable column sort-key order.
    pub position: i32,
    /// `due_date` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (`null` when unset).
    pub due_date: Option<String>,
    /// `created_at`, same shape.
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
    /// Attached labels (`id` order).
    pub labels: Vec<CardLabelRef>,
    /// Checklist rollup (`None` = the card owns no checklist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checklist_progress: Option<ChecklistProgress>,
}
