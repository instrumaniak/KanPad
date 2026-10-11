//! columns handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, patch},
    Json,
};

use crate::{
    common::card_summary::{
        assemble_summaries, fetch_board_card_labels, fetch_board_card_progress, fetch_board_cards,
        CardSummary,
    },
    error::AppError,
    extract::{AuthSession, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        check_board_owned, close_column_gap, delete_column_row, fetch_column_owned, fetch_columns,
        insert_column, max_position_for_update, rename_column, to_response,
    },
    types::{
        ColumnBody, ColumnListBody, CreateColumnInput, UpdateColumnInput, DEFAULT_COLUMN_NAME,
    },
};

/// Load a board's card summaries grouped by column id (three set-based
/// selects + pure assembly — no N+1, no TEXT). Single owner for the
/// fetch-assemble sequence so handlers stay one call.
async fn load_summaries(
    pool: &sqlx::MySqlPool,
    board_id: i32,
) -> Result<std::collections::HashMap<i32, Vec<CardSummary>>, AppError> {
    let rows = fetch_board_cards(pool, board_id).await?;
    let labels = fetch_board_card_labels(pool, board_id).await?;
    let progress = fetch_board_card_progress(pool, board_id).await?;
    assemble_summaries(rows, labels, progress)
}

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
/// CRUD only: sort (`{sort_order}`, T23b) and move-cards (T23c) land in their
/// own tasks, so those paths 404 from the router until then.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route(
            "/boards/{boardId}/columns",
            get(list_columns).post(create_column),
        )
        .route(
            "/columns/{id}",
            patch(rename_column_handler).delete(delete_column),
        )
}

/// `GET /api/v1/boards/{boardId}/columns → 200 {data[]}` (nested unpaginated,
/// `position ASC`, embeds card summaries — never `description` TEXT).
/// Foreign/missing board id → 404 (no oracle, never 403); garbage id → 400
/// `invalid-path` via `ValidPath`.
#[utoipa::path(
    get,
    path = "/api/v1/boards/{boardId}/columns",
    responses(
        (status = 200, description = "Board columns with card summaries"),
        (status = 400, description = "Invalid board id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Board not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn list_columns(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(board_id): ValidPath<crate::common::ids::BoardId>,
) -> Result<Json<ColumnListBody>, AppError> {
    let user_id = auth.user_id.get();
    let board_id = board_id.get();
    check_board_owned(&state.pool, user_id, board_id).await?;
    let rows = fetch_columns(&state.pool, board_id).await?;
    let summaries = load_summaries(&state.pool, board_id).await?;
    Ok(Json(ColumnListBody {
        data: to_response(rows, &summaries),
    }))
}

/// `POST /api/v1/boards/{boardId}/columns{name?} → 201 {data} + Location`.
/// The position is `MAX(position)+1` taken under `SELECT ... FOR UPDATE` in
/// the create tx; an absent name defaults to `New Column` (a present blank
/// still 400s via the DTO). Foreign/missing board id rolls back to 404 (no
/// oracle, never 403).
#[utoipa::path(
    post,
    path = "/api/v1/boards/{boardId}/columns",
    request_body = CreateColumnInput,
    responses(
        (status = 201, description = "Column created", body = ColumnBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Board not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create_column(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(board_id): ValidPath<crate::common::ids::BoardId>,
    ValidJson(input): ValidJson<CreateColumnInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let board_id = board_id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    check_board_owned(&mut *tx, user_id, board_id).await?;
    let position = max_position_for_update(&mut *tx, board_id).await? + 1;
    let name = input.name.as_deref().unwrap_or(DEFAULT_COLUMN_NAME);
    let id = insert_column(&mut *tx, board_id, name, position).await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_column_owned(&state.pool, user_id, id).await?;
    let summaries = load_summaries(&state.pool, board_id).await?;
    let location = format!("/api/v1/columns/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(ColumnBody {
            data: to_response(Vec::from([row]), &summaries)
                .pop()
                .expect("created column renders"),
        }),
    ))
}

/// `PATCH /api/v1/columns/{id}{name?} → 200 {data}` (present names validate
/// like create). The column is pre-read for ownership (foreign/missing id →
/// 404, never 403); an absent name is a no-op 200. The response re-reads the
/// stored row (never echoes).
#[utoipa::path(
    patch,
    path = "/api/v1/columns/{id}",
    request_body = UpdateColumnInput,
    responses(
        (status = 200, description = "Column updated", body = ColumnBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Column not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn rename_column_handler(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ColumnId>,
    ValidJson(input): ValidJson<UpdateColumnInput>,
) -> Result<Json<ColumnBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    let owned = fetch_column_owned(&mut *tx, user_id, id).await?;
    if let Some(name) = input.name.as_deref() {
        rename_column(&mut *tx, id, name).await?;
    }
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_column_owned(&state.pool, user_id, id).await?;
    let summaries = load_summaries(&state.pool, owned.board_id.get()).await?;
    Ok(Json(ColumnBody {
        data: to_response(Vec::from([row]), &summaries)
            .pop()
            .expect("updated column renders"),
    }))
}

/// `DELETE /api/v1/columns/{id} → 204` (hard delete, empty body). Cards
/// cascade via `ON DELETE CASCADE` FKs and the position gap closes with a
/// set-based range `UPDATE` in the SAME tx. Foreign/missing id → 404 (no
/// oracle, never 403); garbage id → 400 `invalid-path`.
#[utoipa::path(
    delete,
    path = "/api/v1/columns/{id}",
    responses(
        (status = 204, description = "Column deleted"),
        (status = 400, description = "Invalid column id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Column not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn delete_column(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ColumnId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    let owned = fetch_column_owned(&mut *tx, user_id, id).await?;
    delete_column_row(&mut *tx, id).await?;
    close_column_gap(&mut *tx, owned.board_id.get(), owned.position).await?;
    tx.commit().await.map_err(AppError::Db)?;
    Ok(StatusCode::NO_CONTENT)
}
