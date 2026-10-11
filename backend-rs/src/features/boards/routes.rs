//! boards handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json,
};

use crate::{
    common::pagination::Page,
    error::AppError,
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath, ValidQuery},
    state::AppState,
};

use super::{
    store::{
        check_project_owned, count_boards, fetch_board, fetch_board_columns, fetch_board_page,
        insert_board, insert_default_columns, to_list_response, to_response,
    },
    types::{
        BoardDetailBody, BoardListQuery, BoardListResponse, CreateBoardInput, DEFAULT_BOARD_COLOR,
    },
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
/// Update/delete routes land in T22; this task owns create+detail (T21a)
/// plus list (T21b).
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/boards", post(create_board).get(list_boards))
        .route("/boards/{id}", get(get_board))
}

/// `POST /api/v1/boards{name,background_color?,project_id?} → 201 {data} +
/// Location`. The board + 3 default columns (`To Do/In Progress/Done` 0/1/2)
/// are created in ONE tx; a foreign/missing `project_id` rolls back to 404
/// (no oracle, never 403). Blank/overlong names and bad hex colors → 400
/// `errors[]` via the DTO; `project_id` type garbage never reaches garde —
/// serde rejects it as 400 `invalid-json` first.
#[utoipa::path(
    post,
    path = "/api/v1/boards",
    request_body = CreateBoardInput,
    responses(
        (status = 201, description = "Board created", body = BoardDetailBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Project not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create_board(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<CreateBoardInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    if let Some(project_id) = input.project_id {
        check_project_owned(&mut *tx, user_id, project_id).await?;
    }
    let background_color = input
        .background_color
        .as_deref()
        .unwrap_or(DEFAULT_BOARD_COLOR);
    let id = insert_board(
        &mut *tx,
        user_id,
        &input.name,
        background_color,
        input.project_id,
    )
    .await?;
    insert_default_columns(&mut *tx, id).await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored rows, never echoes.
    let row = fetch_board(&state.pool, user_id, id).await?;
    let columns = fetch_board_columns(&state.pool, id).await?;
    let location = format!("/api/v1/boards/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(BoardDetailBody {
            data: to_response(row, columns),
        }),
    ))
}

/// `GET /api/v1/boards?project_id&archived&sort&order&page&limit → 200
/// Page<Board>` (flat items — no nested `columns[]`). Archived boards are
/// hidden unless `archived=true`; `project_id` scopes to that project (a
/// foreign or missing id matches zero rows — empty page, never a 403/404
/// oracle). There is deliberately NO `GET /boards/archived` duplicate:
/// `archived` in that position fails the `:id` int-parse → 400
/// `invalid-path`, never a 200 list.
#[utoipa::path(
    get,
    path = "/api/v1/boards",
    params(BoardListQuery),
    responses(
        (status = 200, description = "Board page"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn list_boards(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidQuery(filters): ValidQuery<BoardListQuery>,
    pagination: PaginationParams,
) -> Result<Json<Page<BoardListResponse>>, AppError> {
    let user_id = auth.user_id.get();
    let include_archived = filters.include_archived();
    let total = count_boards(&state.pool, user_id, filters.project_id, include_archived).await?;
    let rows = fetch_board_page(
        &state.pool,
        user_id,
        filters.project_id,
        include_archived,
        filters.resolved_sort(),
        filters.resolved_order(),
        pagination.page,
        pagination.limit,
    )
    .await?;
    Ok(Json(Page::new(
        rows.into_iter().map(to_list_response).collect(),
        pagination.page,
        pagination.limit,
        total,
    )))
}

/// `GET /api/v1/boards/{id} → 200 {data:{…board,columns[]}}` (nested
/// unpaginated, `position ASC`, no cards). Foreign/missing id → 404 (no
/// oracle, never 403); garbage id → 400 `invalid-path` via `ValidPath`.
#[utoipa::path(
    get,
    path = "/api/v1/boards/{id}",
    responses(
        (status = 200, description = "Board detail", body = BoardDetailBody),
        (status = 400, description = "Invalid board id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Board not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_board(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::BoardId>,
) -> Result<Json<BoardDetailBody>, AppError> {
    let user_id = auth.user_id.get();
    let row = fetch_board(&state.pool, user_id, id.get()).await?;
    let columns = fetch_board_columns(&state.pool, id.get()).await?;
    Ok(Json(BoardDetailBody {
        data: to_response(row, columns),
    }))
}
