//! checklists handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json,
};

use crate::{
    error::AppError,
    extract::{AuthSession, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        check_card_owned, delete_checklist_row, fetch_card_checklist_items, fetch_checklist_items,
        fetch_checklist_owned, fetch_checklists_for_card, insert_checklist, rename_checklist,
        to_list, to_response,
    },
    types::{ChecklistBody, ChecklistListBody, CreateChecklistInput, UpdateChecklistInput},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
/// Checklist items (T26b) land in their own task, so those paths 404 from the
/// router until then.
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
#[tracing::instrument(skip(state))]
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
#[tracing::instrument(skip(state))]
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
#[tracing::instrument(skip(state))]
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
#[tracing::instrument(skip(state))]
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
#[tracing::instrument(skip(state))]
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
