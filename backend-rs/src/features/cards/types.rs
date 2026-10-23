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
use utoipa::ToSchema;

use crate::common::{
    card_summary::{CardLabelRef, ChecklistProgress},
    ids::{CardId, ChecklistId, ColumnId},
    validation::{validate_due_date, validate_not_blank, validate_optional_not_blank},
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
    /// Explicit position (insert-with-open-gap; clamped in store, never 400).
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
    /// Items, `position ASC`.
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
    /// Nested checklists (`id` order) with items (`position ASC`).
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
