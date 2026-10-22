//! notes handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};

use crate::{
    common::pagination::Page,
    error::AppError,
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath, ValidQuery},
    state::AppState,
};

use super::{
    store::{
        NoteFilter, NotePatch, apply_note_patch, check_board_owned, check_card_owned,
        check_project_owned, check_single_link, check_tags_owned, clear_note_tags, count_notes,
        dedup_tag_ids, delete_note_row, fetch_note_ids, fetch_note_owned, fetch_note_summaries,
        fetch_note_tags, fetch_notes_tags, insert_note, link_note_tags, to_detail, to_summaries,
    },
    types::{NoteBody, NoteListQuery, NoteSummaryResponse},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
/// There is deliberately NO `GET /boards/:boardId/notes` duplicate (deleted
/// per `21-api-standard-shapes.md` — use `GET /api/v1/notes?board_id=`): that
/// path 404s from the router.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/notes", post(create_note).get(list_notes))
        .route(
            "/notes/{id}",
            get(get_note).patch(patch_note).delete(delete_note),
        )
}

/// Load a note's tag links and render the wire body. Single owner for the
/// fetch-assemble sequence so handlers stay one call; each select is one query
/// (the executor value moves).
async fn load_detail(
    pool: &sqlx::MySqlPool,
    row: super::store::NoteRow,
) -> Result<NoteBody, AppError> {
    let note_id = row.id.get();
    let tags = fetch_note_tags(pool, note_id).await?;
    Ok(NoteBody {
        data: to_detail(row, tags),
    })
}

/// `POST /api/v1/notes{title,content,board_id?|project_id?|card_id?,tag_ids?}
/// → 201 {data} + Location`. At most one FK may be set (multiple → 400; none
/// = `general`); linked targets must belong to the caller and every tag must
/// exist + belong to the caller (missing/foreign → 404, never 403 — create
/// aligns with update per `20-api-contract.md` notes). `tag_ids` are deduped
/// before linking.
#[utoipa::path(
    post,
    path = "/api/v1/notes",
    request_body = super::types::CreateNoteInput,
    responses(
        (status = 201, description = "Note created", body = NoteBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Linked target or tag not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create_note(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<super::types::CreateNoteInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    check_single_link(input.board_id, input.project_id, input.card_id)?;
    let tag_ids = dedup_tag_ids(input.tag_ids.as_deref().unwrap_or_default());
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    if let Some(board_id) = input.board_id {
        check_board_owned(&mut *tx, user_id, board_id).await?;
    }
    if let Some(project_id) = input.project_id {
        check_project_owned(&mut *tx, user_id, project_id).await?;
    }
    if let Some(card_id) = input.card_id {
        check_card_owned(&mut *tx, user_id, card_id).await?;
    }
    check_tags_owned(&mut *tx, user_id, &tag_ids).await?;
    let id = insert_note(
        &mut *tx,
        user_id,
        &input.title,
        &input.content,
        input.board_id,
        input.project_id,
        input.card_id,
    )
    .await?;
    link_note_tags(&mut *tx, id, &tag_ids).await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored rows, never echoes.
    let row = fetch_note_owned(&state.pool, user_id, id).await?;
    let body = load_detail(&state.pool, row).await?;
    let location = format!("/api/v1/notes/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(body),
    ))
}

/// `GET /api/v1/notes?search&type&tag_id&board_id&page&limit → 200
/// Page<NoteSummary>` (`updated_at DESC`; never selects TEXT — no `content`
/// key, only the summary shape). `type` is derived from which FK is set
/// (there is no column). Garbage `tag_id`/`board_id` never reach garde — axum
/// rejects them as 400 `invalid-query` first; a bad `type` enum and range
/// violations 400 with `errors[]`. Two-step IDs-then-enrich under one shared
/// `WHERE` builder (count via `COUNT(DISTINCT n.id)`, never `COUNT(*)` over
/// the tags join).
#[utoipa::path(
    get,
    path = "/api/v1/notes",
    params(NoteListQuery),
    responses(
        (status = 200, description = "Note page"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn list_notes(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidQuery(filters): ValidQuery<NoteListQuery>,
    pagination: PaginationParams,
) -> Result<Json<Page<NoteSummaryResponse>>, AppError> {
    let user_id = auth.user_id.get();
    let filter = NoteFilter {
        search: filters.search,
        note_type: filters.note_type,
        tag_id: filters.tag_id,
        board_id: filters.board_id,
    };
    let total = count_notes(&state.pool, user_id, &filter).await?;
    if total == 0 {
        return Ok(Json(Page::empty(pagination.page, pagination.limit)));
    }
    let offset = pagination
        .page
        .saturating_sub(1)
        .saturating_mul(pagination.limit);
    let ordered_ids =
        fetch_note_ids(&state.pool, user_id, &filter, pagination.limit, offset).await?;
    let rows = fetch_note_summaries(&state.pool, &ordered_ids).await?;
    let tags = fetch_notes_tags(&state.pool, &ordered_ids).await?;
    Ok(Json(Page::new(
        to_summaries(&ordered_ids, rows, tags),
        pagination.page,
        pagination.limit,
        total,
    )))
}

/// `GET /api/v1/notes/{id} → 200 {data}` (explicit `content` select — the
/// only path that reads TEXT). Foreign/missing id → 404 (no oracle, never
/// 403); garbage id → 400 `invalid-path` via `ValidPath`.
#[utoipa::path(
    get,
    path = "/api/v1/notes/{id}",
    responses(
        (status = 200, description = "Note detail"),
        (status = 400, description = "Invalid note id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Note not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_note(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::NoteId>,
) -> Result<Json<NoteBody>, AppError> {
    let user_id = auth.user_id.get();
    let row = fetch_note_owned(&state.pool, user_id, id.get()).await?;
    Ok(Json(load_detail(&state.pool, row).await?))
}

/// `PATCH /api/v1/notes/{id} → 200 {data}` (every field optional; explicit
/// `null` on an FK detaches, `tag_ids` present replaces the link set). The
/// merged FK set keeps the at-most-one rule (a merge leaving two links → 400
/// before any write); newly set links and tags are ownership-checked like
/// create (foreign/missing → 404). Empty `{}` is a no-op 200. The response
/// re-reads the stored rows (never echoes).
#[utoipa::path(
    patch,
    path = "/api/v1/notes/{id}",
    request_body = super::types::UpdateNoteInput,
    responses(
        (status = 200, description = "Note updated", body = NoteBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Note, linked target, or tag not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn patch_note(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::NoteId>,
    ValidJson(input): ValidJson<super::types::UpdateNoteInput>,
) -> Result<Json<NoteBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let tag_ids = input.tag_ids.as_deref().map(dedup_tag_ids);
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    let owned = fetch_note_owned(&mut *tx, user_id, id).await?;
    // Merged link set (existing row + patch): at most one may stand.
    let board_id = input
        .board_id
        .unwrap_or_else(|| owned.board_id.map(|link| link.get()));
    let project_id = input
        .project_id
        .unwrap_or_else(|| owned.project_id.map(|link| link.get()));
    let card_id = input
        .card_id
        .unwrap_or_else(|| owned.card_id.map(|link| link.get()));
    check_single_link(board_id, project_id, card_id)?;
    if let Some(Some(board_id)) = input.board_id {
        check_board_owned(&mut *tx, user_id, board_id).await?;
    }
    if let Some(Some(project_id)) = input.project_id {
        check_project_owned(&mut *tx, user_id, project_id).await?;
    }
    if let Some(Some(card_id)) = input.card_id {
        check_card_owned(&mut *tx, user_id, card_id).await?;
    }
    if let Some(tag_ids) = tag_ids.as_deref() {
        check_tags_owned(&mut *tx, user_id, tag_ids).await?;
    }
    apply_note_patch(
        &mut *tx,
        id,
        NotePatch {
            title: input.title.as_deref(),
            content: input.content.as_deref(),
            board_id: input.board_id,
            project_id: input.project_id,
            card_id: input.card_id,
        },
    )
    .await?;
    if let Some(tag_ids) = tag_ids.as_deref() {
        clear_note_tags(&mut *tx, id).await?;
        link_note_tags(&mut *tx, id, tag_ids).await?;
    }
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored rows, never echoes.
    let row = fetch_note_owned(&state.pool, user_id, id).await?;
    Ok(Json(load_detail(&state.pool, row).await?))
}

/// `DELETE /api/v1/notes/{id} → 204` (hard delete, empty body). Tag links fan
/// out via `ON DELETE CASCADE` FKs. Foreign/missing id → 404 (no oracle,
/// never 403); garbage id → 400 `invalid-path`.
#[utoipa::path(
    delete,
    path = "/api/v1/notes/{id}",
    responses(
        (status = 204, description = "Note deleted"),
        (status = 400, description = "Invalid note id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Note not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn delete_note(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::NoteId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    fetch_note_owned(&mut *tx, user_id, id).await?;
    delete_note_row(&mut *tx, id).await?;
    tx.commit().await.map_err(AppError::Db)?;
    Ok(StatusCode::NO_CONTENT)
}
