//! checklists handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};

use crate::{
    error::AppError,
    extract::{AuthSession, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        ItemPatch, apply_item_patch, check_card_owned, delete_checklist_row, delete_item_row,
        fetch_card_checklist_items, fetch_checklist_items, fetch_checklist_owned,
        fetch_checklists_for_card, fetch_item_owned, insert_checklist, insert_item,
        max_item_position_for_update, rename_checklist, to_item_response, to_list, to_response,
    },
    types::{
        ChecklistBody, ChecklistItemBody, ChecklistListBody, CreateChecklistInput,
        CreateChecklistItemInput, UpdateChecklistInput, UpdateChecklistItemInput,
    },
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route(
            "/cards/{cardId}/checklists",
            post(create_checklist).get(list_checklists),
        )
        .route(
            "/checklists/{id}",
            get(get_checklist)
                .patch(update_checklist)
                .delete(delete_checklist),
        )
        .route("/checklists/{id}/items", post(create_item))
        .route(
            "/checklist-items/{id}",
            axum::routing::patch(update_item).delete(delete_item),
        )
}

/// `POST /api/v1/cards/{cardId}/checklists{title} → 201 {data} + Location`.
/// A foreign/missing `cardId` rolls back to 404 (no oracle, never 403);
/// garbage id → 400 `invalid-path` via `ValidPath`; blank/overlong titles →
/// 400 `errors[]` via the DTO.
#[utoipa::path(
    post,
    path = "/api/v1/cards/{cardId}/checklists",
    request_body = CreateChecklistInput,
    responses(
        (status = 201, description = "Checklist created", body = ChecklistBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card not found")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn create_checklist(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(card_id): ValidPath<crate::common::ids::CardId>,
    ValidJson(input): ValidJson<CreateChecklistInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let card_id = card_id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    check_card_owned(&mut *tx, user_id, card_id).await?;
    let id = insert_checklist(&mut *tx, card_id, &input.title).await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_checklist_owned(&state.pool, user_id, id).await?;
    let items = fetch_checklist_items(&state.pool, id).await?;
    let location = format!("/api/v1/checklists/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(ChecklistBody {
            data: to_response(row, items),
        }),
    ))
}

/// `GET /api/v1/cards/{cardId}/checklists → 200 {data[]}` (small-N,
/// unpaginated — added route per `21 §checklists`; `id ASC`, items embedded
/// `position ASC`). Foreign/missing card → 404 (no oracle, never 403).
#[utoipa::path(
    get,
    path = "/api/v1/cards/{cardId}/checklists",
    responses(
        (status = 200, description = "Card checklists", body = ChecklistListBody),
        (status = 400, description = "Invalid card id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card not found")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn list_checklists(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(card_id): ValidPath<crate::common::ids::CardId>,
) -> Result<Json<ChecklistListBody>, AppError> {
    let user_id = auth.user_id.get();
    let card_id = card_id.get();
    check_card_owned(&state.pool, user_id, card_id).await?;
    let rows = fetch_checklists_for_card(&state.pool, card_id).await?;
    let items = fetch_card_checklist_items(&state.pool, card_id).await?;
    Ok(Json(ChecklistListBody {
        data: to_list(rows, items),
    }))
}

/// `GET /api/v1/checklists/{id} → 200 {data}` (items embedded `position
/// ASC`). Foreign/missing id → 404 (no oracle, never 403); garbage id → 400
/// `invalid-path` via `ValidPath`.
#[utoipa::path(
    get,
    path = "/api/v1/checklists/{id}",
    responses(
        (status = 200, description = "Checklist detail", body = ChecklistBody),
        (status = 400, description = "Invalid checklist id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Checklist not found")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn get_checklist(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ChecklistId>,
) -> Result<Json<ChecklistBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let row = fetch_checklist_owned(&state.pool, user_id, id).await?;
    let items = fetch_checklist_items(&state.pool, id).await?;
    Ok(Json(ChecklistBody {
        data: to_response(row, items),
    }))
}

/// `PATCH /api/v1/checklists/{id}{title?} → 200 {data}`. An empty `{}` body
/// is a no-op 200. The response re-reads the stored row (never echoes).
/// Foreign/missing id → 404, never 403.
#[utoipa::path(
    patch,
    path = "/api/v1/checklists/{id}",
    request_body = UpdateChecklistInput,
    responses(
        (status = 200, description = "Checklist updated", body = ChecklistBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Checklist not found")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn update_checklist(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ChecklistId>,
    ValidJson(input): ValidJson<UpdateChecklistInput>,
) -> Result<Json<ChecklistBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    // Existence + ownership first: a same-title `PATCH` must stay 200 (an
    // affected-rows check alone would false-404 when nothing changed).
    fetch_checklist_owned(&mut *tx, user_id, id).await?;
    if let Some(title) = input.title.as_deref() {
        rename_checklist(&mut *tx, id, title).await?;
    }
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_checklist_owned(&state.pool, user_id, id).await?;
    let items = fetch_checklist_items(&state.pool, id).await?;
    Ok(Json(ChecklistBody {
        data: to_response(row, items),
    }))
}

/// `DELETE /api/v1/checklists/{id} → 204` (hard delete, empty body). Items
/// cascade via the DB `ON DELETE CASCADE` FK. Foreign/missing id → 404 (no
/// oracle, never 403); garbage id → 400 `invalid-path`.
#[utoipa::path(
    delete,
    path = "/api/v1/checklists/{id}",
    responses(
        (status = 204, description = "Checklist deleted"),
        (status = 400, description = "Invalid checklist id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Checklist not found")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn delete_checklist(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ChecklistId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    fetch_checklist_owned(&mut *tx, user_id, id).await?;
    delete_checklist_row(&mut *tx, id).await?;
    tx.commit().await.map_err(AppError::Db)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/checklists/{id}/items{text,position?} → 201 {data} +
/// Location`. An absent `position` appends at `MAX(position) + 1` taken under
/// `SELECT … FOR UPDATE` in the create tx; an explicit `position` writes
/// verbatim. A foreign/missing checklist id rolls back to 404 (no oracle,
/// never 403); garbage id → 400 `invalid-path` via `ValidPath`; blank/overlong
/// text or negative `position` → 400 `errors[]` via the DTO (floats never reach
/// garde — serde rejects them first as `invalid-json`).
#[utoipa::path(
    post,
    path = "/api/v1/checklists/{id}/items",
    request_body = CreateChecklistItemInput,
    responses(
        (status = 201, description = "Checklist item created", body = ChecklistItemBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Checklist not found")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn create_item(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ChecklistId>,
    ValidJson(input): ValidJson<CreateChecklistItemInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let checklist_id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    fetch_checklist_owned(&mut *tx, user_id, checklist_id).await?;
    let position = match input.position {
        Some(position) => position,
        None => max_item_position_for_update(&mut *tx, checklist_id).await? + 1,
    };
    let id = insert_item(&mut *tx, checklist_id, &input.text, position).await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_item_owned(&state.pool, user_id, id).await?;
    let location = format!("/api/v1/checklist-items/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(ChecklistItemBody {
            data: to_item_response(row),
        }),
    ))
}

/// `PATCH /api/v1/checklist-items/{id}{text?,is_completed?} → 200 {data}`. An
/// empty `{}` body is a no-op 200. The response re-reads the stored row (never
/// echoes). Foreign/missing id → 404, never 403.
#[utoipa::path(
    patch,
    path = "/api/v1/checklist-items/{id}",
    request_body = UpdateChecklistItemInput,
    responses(
        (status = 200, description = "Checklist item updated", body = ChecklistItemBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Checklist item not found")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn update_item(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ChecklistItemId>,
    ValidJson(input): ValidJson<UpdateChecklistItemInput>,
) -> Result<Json<ChecklistItemBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    // Existence + ownership first: an idempotent no-change `PATCH` must stay
    // 200 (an affected-rows check alone would false-404 when nothing changed).
    fetch_item_owned(&mut *tx, user_id, id).await?;
    apply_item_patch(
        &mut *tx,
        id,
        ItemPatch {
            text: input.text.as_deref(),
            is_completed: input.is_completed,
        },
    )
    .await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_item_owned(&state.pool, user_id, id).await?;
    Ok(Json(ChecklistItemBody {
        data: to_item_response(row),
    }))
}

/// `DELETE /api/v1/checklist-items/{id} → 204` (hard delete, empty body).
/// Foreign/missing id → 404 (no oracle, never 403); garbage id → 400
/// `invalid-path`.
#[utoipa::path(
    delete,
    path = "/api/v1/checklist-items/{id}",
    responses(
        (status = 204, description = "Checklist item deleted"),
        (status = 400, description = "Invalid item id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Checklist item not found")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn delete_item(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ChecklistItemId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    fetch_item_owned(&mut *tx, user_id, id).await?;
    delete_item_row(&mut *tx, id).await?;
    tx.commit().await.map_err(AppError::Db)?;
    Ok(StatusCode::NO_CONTENT)
}
