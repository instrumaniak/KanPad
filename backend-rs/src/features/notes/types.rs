//! notes DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! `content` is `<= 65_535` chars (chars, both DTOs — units documented).
//! `type` is the `general|board|project|card` enum derived from which FK is
//! set (there is no `type` column). The at-most-one-FK rule (multiple set →
//! 400, none = `general`) is enforced in the T27 handler, not the DTO.
//! Unknown `tag_ids` → 404 (T27 store aligns create with update).

use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::validation::{
    validate_not_blank, validate_optional_not_blank, validate_optional_note_type, validate_tag_ids,
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
    /// Owning board id (`>= 1`); null detaches.
    #[garde(range(min = 1))]
    pub board_id: Option<i32>,
    /// Owning project id (`>= 1`); null detaches.
    #[garde(range(min = 1))]
    pub project_id: Option<i32>,
    /// Owning card id (`>= 1`); null detaches.
    #[garde(range(min = 1))]
    pub card_id: Option<i32>,
    /// Linked tag ids (each `>= 1`).
    #[garde(custom(validate_tag_ids))]
    pub tag_ids: Option<Vec<i32>>,
}

/// `GET /api/v1/notes` query (T27a handler; pagination rides the shared
/// strict [`crate::extract::PaginationParams`] extractor).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
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
