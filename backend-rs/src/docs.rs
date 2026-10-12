//! OpenAPI surface: the single `ApiDoc` behind `export-openapi`, the
//! committed `openapi.json`, `GET /api-docs/openapi.json`, and `/swagger-ui`.
//!
//! Per-handler `#[utoipa::path]` annotations live with the resource tasks;
//! this module only aggregates them into one spec. Generic envelopes
//! (`Page<T>`) are registered once per concrete item type — utoipa composes
//! per-item component names (`Page_BoardListResponse`), and every `T` already
//! implements `ToSchema`, which is the bound the derive requires. Nested
//! DTOs (`BoardDetailResponse`, `CardLabelRef`, …) are pulled into
//! `components.schemas` transitively by the generated `ToSchema::schemas`
//! recursion, so only top-level request/response bodies are listed here.
//!
//! Known wart: `cards::types` and `checklists::types` both declare
//! `ChecklistResponse` / `ChecklistItemResponse` with identical shapes (only
//! doc-comment descriptions differ). Both map to one component name, so the
//! later registration wins deterministically (checklists is listed after
//! cards below, as the canonical owner of the checklist routes). A follow-up
//! may rename one pair via `#[schema(as = …)]` (types.rs attrs only).

use serde::Serialize;
use utoipa::ToSchema;

use crate::common::pagination::Page;
use crate::error::{FieldError, ProblemDetails};

/// Liveness response body `{status:"ok"}`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Always `"ok"`.
    pub status: String,
}

/// Aggregated API documentation: every served handler path, every top-level
/// DTO, the `Page<T>` list envelopes, and the shared problem+json error
/// schemas. Rendered by `export-openapi` (boots no pool, no sessions) and
/// diffed against the committed `openapi.json` by the snapshot test.
#[derive(utoipa::OpenApi)]
#[openapi(
    info(title = "KanPad backend-rs", version = "0.1.0"),
    paths(
        crate::health,
        crate::features::auth::routes::register,
        crate::features::auth::routes::login,
        crate::features::auth::routes::logout,
        crate::features::auth::routes::me,
        crate::features::projects::routes::list_projects,
        crate::features::projects::routes::create_project,
        crate::features::projects::routes::update_project,
        crate::features::projects::routes::delete_project,
        crate::features::boards::routes::create_board,
        crate::features::boards::routes::list_boards,
        crate::features::boards::routes::get_board,
        crate::features::boards::routes::patch_board,
        crate::features::boards::routes::delete_board,
        crate::features::columns::routes::list_columns,
        crate::features::columns::routes::create_column,
        crate::features::columns::routes::rename_column_handler,
        crate::features::columns::routes::delete_column,
        crate::features::columns::routes::move_cards,
        crate::features::cards::routes::create_card,
        crate::features::cards::routes::get_card,
        crate::features::cards::routes::update_card,
        crate::features::cards::routes::attach_card_label,
        crate::features::cards::routes::attach_card_label_alias,
        crate::features::cards::routes::detach_card_label,
        crate::features::cards::routes::delete_card,
        crate::features::cards::routes::list_cards,
        crate::features::labels::routes::list_labels,
        crate::features::labels::routes::create_label,
        crate::features::labels::routes::update_label,
        crate::features::labels::routes::delete_label,
        crate::features::checklists::routes::create_checklist,
        crate::features::checklists::routes::list_checklists,
        crate::features::checklists::routes::get_checklist,
        crate::features::checklists::routes::update_checklist,
        crate::features::checklists::routes::delete_checklist,
        crate::features::checklists::routes::create_item,
        crate::features::checklists::routes::update_item,
        crate::features::checklists::routes::delete_item,
        crate::features::notes::routes::create_note,
        crate::features::notes::routes::list_notes,
        crate::features::notes::routes::get_note,
        crate::features::notes::routes::patch_note,
        crate::features::notes::routes::delete_note,
        crate::features::tags::routes::list_tags,
        crate::features::tags::routes::create_tag,
        crate::features::tags::routes::update_tag,
        crate::features::tags::routes::delete_tag,
    ),
    components(schemas(
        HealthResponse,
        ProblemDetails,
        FieldError,
        crate::features::auth::types::RegisterInput,
        crate::features::auth::types::LoginInput,
        crate::features::auth::types::UserData,
        crate::features::auth::types::RegisterResponse,
        crate::features::auth::types::LoginResponse,
        crate::features::auth::types::MeResponse,
        crate::features::projects::types::CreateProjectInput,
        crate::features::projects::types::UpdateProjectInput,
        crate::features::projects::types::ProjectResponse,
        crate::features::projects::types::ProjectBody,
        Page<crate::features::projects::types::ProjectResponse>,
        crate::features::boards::types::CreateBoardInput,
        crate::features::boards::types::UpdateBoardInput,
        crate::features::boards::types::BoardDetailBody,
        crate::features::boards::types::BoardListResponse,
        Page<crate::features::boards::types::BoardListResponse>,
        crate::features::columns::types::CreateColumnInput,
        crate::features::columns::types::UpdateColumnInput,
        crate::features::columns::types::MoveCardsInput,
        crate::features::columns::types::MoveCardsBody,
        crate::features::columns::types::ColumnBody,
        crate::features::columns::types::ColumnListBody,
        crate::features::cards::types::CreateCardInput,
        crate::features::cards::types::UpdateCardInput,
        crate::features::cards::types::AttachLabelInput,
        crate::features::cards::types::CardBody,
        Page<crate::common::card_summary::CardSummary>,
        crate::features::labels::types::CreateLabelInput,
        crate::features::labels::types::UpdateLabelInput,
        crate::features::labels::types::LabelBody,
        Page<crate::features::labels::types::LabelResponse>,
        crate::features::checklists::types::CreateChecklistInput,
        crate::features::checklists::types::UpdateChecklistInput,
        crate::features::checklists::types::CreateChecklistItemInput,
        crate::features::checklists::types::UpdateChecklistItemInput,
        crate::features::checklists::types::ChecklistBody,
        crate::features::checklists::types::ChecklistListBody,
        crate::features::checklists::types::ChecklistItemBody,
        crate::features::notes::types::CreateNoteInput,
        crate::features::notes::types::UpdateNoteInput,
        crate::features::notes::types::NoteBody,
        Page<crate::features::notes::types::NoteSummaryResponse>,
        crate::features::tags::types::CreateTagInput,
        crate::features::tags::types::UpdateTagInput,
        crate::features::tags::types::TagBody,
        Page<crate::features::tags::types::TagResponse>,
    ))
)]
pub struct ApiDoc;
