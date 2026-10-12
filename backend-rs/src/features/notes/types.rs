//! notes DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `content` is `<= 65_535` chars (chars, both DTOs — units documented).
//! `type` is the `general|board|project|card` enum derived from which FK is
//! set (there is no `type` column). The at-most-one-FK rule (multiple set →
//! 400, none = `general`) is enforced in the T27 handler, not the DTO.
//! Unknown `tag_ids` → 404 (T27 store aligns create with update).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::{
    ids::{BoardId, CardId, NoteId, ProjectId, TagId, UserId},
    validation::{
        validate_not_blank, validate_optional_not_blank, validate_optional_note_type,
        validate_tag_ids,
    },
};

/// `POST /api/v1/notes` body (T27a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct CreateNoteInput {
    /// Note title: non-blank after trim, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_not_blank))]
    pub title: String,
    /// Note content: 1..=65_535 chars (empty → 400, Nest `IsNotEmpty`
    /// parity; whitespace-only stays allowed — content is outside the
    /// catalogue's trim list).
    #[garde(length(min = 1, max = 65_535))]
    pub content: String,
    /// Owning board id (`>= 1`), optional.
    #[garde(range(min = 1))]
    pub board_id: Option<i32>,
    /// Owning project id (`>= 1`), optional.
    #[garde(range(min = 1))]
    pub project_id: Option<i32>,
    /// Owning card id (`>= 1`), optional.
    #[garde(range(min = 1))]
    pub card_id: Option<i32>,
    /// Linked tag ids (each `>= 1`).
    #[garde(custom(validate_tag_ids))]
    pub tag_ids: Option<Vec<i32>>,
}

/// `PATCH /api/v1/notes/{id}` body (T27a handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct UpdateNoteInput {
    /// Note title: present means non-blank, `<= 255` chars.
    #[garde(length(max = 255), custom(validate_optional_not_blank))]
    pub title: Option<String>,
    /// Note content (`<= 65_535` chars).
    #[garde(length(max = 65_535))]
    pub content: Option<String>,
    /// Owning board id (`>= 1`); explicit `null` detaches.
    #[serde(default, deserialize_with = "deserialize_some_id")]
    #[garde(custom(validate_optional_link))]
    pub board_id: Option<Option<i32>>,
    /// Owning project id (`>= 1`); explicit `null` detaches.
    #[serde(default, deserialize_with = "deserialize_some_id")]
    #[garde(custom(validate_optional_link))]
    pub project_id: Option<Option<i32>>,
    /// Owning card id (`>= 1`); explicit `null` detaches.
    #[serde(default, deserialize_with = "deserialize_some_id")]
    #[garde(custom(validate_optional_link))]
    pub card_id: Option<Option<i32>>,
    /// Linked tag ids (each `>= 1`); present replaces the link set.
    #[garde(custom(validate_tag_ids))]
    pub tag_ids: Option<Vec<i32>>,
}

/// `Option<Option<i32>>` field decoder (same pattern as boards `project_id`):
/// plain serde maps both a missing field and explicit `null` to outer `None`,
/// so detach (`null`) would be indistinguishable from untouched (missing).
/// With `#[serde(default, deserialize_with)]`, missing → `None` via the
/// default, while any present value (incl. `null`) runs the decoder: `null` →
/// `Some(None)`, int → `Some(Some(..))`.
fn deserialize_some_id<'de, D>(deserializer: D) -> Result<Option<Option<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i32>::deserialize(deserializer).map(Some)
}

/// Link rule for the update FKs: untouched (`None`) and detach (`Some(None)`)
/// always pass; a set id must be `>= 1` (ownership itself is store logic →
/// 404, never 403).
fn validate_optional_link(value: &Option<Option<i32>>, _context: &()) -> garde::Result {
    if let Some(Some(link)) = value {
        if *link < 1 {
            return Err(garde::Error::new("must be at least 1"));
        }
    }
    Ok(())
}

/// `GET /api/v1/notes` query (T27a handler; pagination rides the shared
/// strict [`crate::extract::PaginationParams`] extractor).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NoteListQuery {
    /// Title substring filter (unconstrained by the catalogue; `%`/`_`/`\`
    /// escaping is T27 store logic).
    #[garde(skip)]
    pub search: Option<String>,
    /// Note type filter on the wire (`?type=`): the Rust name avoids the
    /// `r#type` raw identifier because garde 0.22 renders Rust idents into
    /// `errors[]` paths (its `rename` attribute is parsed but ignored), so
    /// `r#type` would leak a Rust-ism to clients.
    #[serde(rename = "type")]
    #[garde(custom(validate_optional_note_type))]
    pub note_type: Option<String>,
    /// Tag filter (`>= 1`); garbage never reaches garde — axum rejects it
    /// as 400 `invalid-query` first.
    #[garde(range(min = 1))]
    pub tag_id: Option<i32>,
    /// Board filter (`>= 1`).
    #[garde(range(min = 1))]
    pub board_id: Option<i32>,
}

/// One tag link inside a note body (`id` order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct TagRef {
    /// `tags.id` (`INT` ↔ `i32` newtype).
    pub id: TagId,
    /// Tag name.
    pub name: String,
    /// Tag color.
    pub color: String,
}

/// Wire note summary: the list shape (TEXT-free — no `content` key, mirroring
/// the cards-list `description` rule; detail `GET /:id` only selects it).
/// `type` is NOT a field (there is no column): clients derive it from which
/// FK is set, same as the Nest `getNoteType` helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct NoteSummaryResponse {
    /// `notes.id` (`INT` ↔ `i32` newtype).
    pub id: NoteId,
    /// Note title.
    pub title: String,
    /// Owning board id (`null` = unlinked).
    pub board_id: Option<BoardId>,
    /// Owning project id (`null` = unlinked).
    pub project_id: Option<ProjectId>,
    /// Owning card id (`null` = unlinked).
    pub card_id: Option<CardId>,
    /// Owning user id.
    pub user_id: UserId,
    /// Linked tags (`id` order).
    pub tags: Vec<TagRef>,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
}

/// Wire note detail: the summary shape plus `content` (explicitly selected —
/// the source entity marks it `select:false`, so only this path reads it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct NoteDetailResponse {
    /// `notes.id` (`INT` ↔ `i32` newtype).
    pub id: NoteId,
    /// Note title.
    pub title: String,
    /// Note content (`null` when absent — emitted, never omitted).
    pub content: Option<String>,
    /// Owning board id (`null` = unlinked).
    pub board_id: Option<BoardId>,
    /// Owning project id (`null` = unlinked).
    pub project_id: Option<ProjectId>,
    /// Owning card id (`null` = unlinked).
    pub card_id: Option<CardId>,
    /// Owning user id.
    pub user_id: UserId,
    /// Linked tags (`id` order).
    pub tags: Vec<TagRef>,
    /// `created_at` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (server pinned UTC).
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
}

/// Success envelope for create/detail/update (`201 {data} + Location`,
/// `200 {data}`; no `message` key — dropped everywhere per
/// `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NoteBody {
    /// Created/fetched/updated note.
    pub data: NoteDetailResponse,
}

/// Render a `DATETIME(6)` (server pinned UTC per `30-db-schema.md`) as a
/// fixed-shape UTC ISO-8601 string (micros always present — same shape as the
/// boards/cards helpers, kept per-feature so features never import each
/// other's stores).
pub fn format_timestamp(value: &chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}
