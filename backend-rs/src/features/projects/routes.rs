//! projects handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, patch},
};

use crate::{
    common::pagination::Page,
    error::AppError,
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath, ValidQuery},
    state::AppState,
};

use super::{
    store::{
        count_projects, delete_project_boards, delete_project_row, fetch_project,
        fetch_project_page, insert_project, update_project_name, wire_timestamps,
    },
    types::{
        CreateProjectInput, ProjectBody, ProjectListQuery, ProjectResponse, UpdateProjectInput,
    },
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/projects", get(list_projects).post(create_project))
        .route(
            "/projects/{id}",
            patch(update_project).delete(delete_project),
        )
}

fn to_response(row: super::store::ProjectRow) -> ProjectResponse {
    let (created_at, updated_at) = wire_timestamps(&row);
    ProjectResponse {
        id: row.id,
        name: row.name,
        board_count: row.board_count,
        created_at,
        updated_at,
    }
}

/// `GET /api/v1/projects?page&limit&q&sort&order → 200 Page<Project>`
/// (`board_count` real via a single `LEFT JOIN ... GROUP BY`; the count
/// query reuses the same `WHERE` without the join — exactly 2 statements
/// for any P, never N+1).
#[utoipa::path(
    get,
    path = "/api/v1/projects",
    params(ProjectListQuery),
    responses(
        (status = 200, description = "Project page"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated")
    )
)]
// `filters` carries the user's SEARCH TEXT (`q` / `search`) — the
// same redaction rule as a request body (`54` B4). `pagination` holds
// two integers and is fine to record.
#[tracing::instrument(skip(state, auth, filters))]
pub async fn list_projects(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidQuery(filters): ValidQuery<ProjectListQuery>,
    pagination: PaginationParams,
) -> Result<Json<Page<ProjectResponse>>, AppError> {
    let user_id = auth.user_id.get();
    let query = filters.q.as_deref();
    let total = count_projects(&state.pool, user_id, query).await?;
    let rows = fetch_project_page(
        &state.pool,
        user_id,
        query,
        filters.resolved_sort(),
        filters.resolved_order(),
        pagination.page,
        pagination.limit,
    )
    .await?;
    Ok(Json(Page::new(
        rows.into_iter().map(to_response).collect(),
        pagination.page,
        pagination.limit,
        total,
    )))
}

/// `POST /api/v1/projects{name} → 201 {data} + Location`.
/// Empty/blank/overlong names → 400 `errors[]` via the DTO.
#[utoipa::path(
    post,
    path = "/api/v1/projects",
    request_body = CreateProjectInput,
    responses(
        (status = 201, description = "Project created", body = ProjectBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn create_project(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<CreateProjectInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let id = insert_project(&state.pool, user_id, &input.name).await?;
    // DB re-read: the response carries stored timestamps, never echoes.
    let row = fetch_project(&state.pool, user_id, id).await?;
    let location = format!("/api/v1/projects/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(ProjectBody {
            data: to_response(row),
        }),
    ))
}

/// `PATCH /api/v1/projects/{id}{name} → 200 {data}`. `name` is required
/// (missing/null/blank → 400 `errors[]`); foreign id → 404, never 403.
#[utoipa::path(
    patch,
    path = "/api/v1/projects/{id}",
    request_body = UpdateProjectInput,
    responses(
        (status = 200, description = "Project updated", body = ProjectBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Project not found")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn update_project(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ProjectId>,
    ValidJson(input): ValidJson<UpdateProjectInput>,
) -> Result<Json<ProjectBody>, AppError> {
    let user_id = auth.user_id.get();
    // `name` is always `Some` here: `ValidJson` ran the garde `required`
    // rule first (missing/null → 400); the default arm is unreachable.
    let name = input.name.unwrap_or_default();
    // Existence + ownership first: a same-name `PATCH` must stay 200 (an
    // affected-rows check alone would false-404 when nothing changed).
    fetch_project(&state.pool, user_id, id.get()).await?;
    update_project_name(&state.pool, user_id, id.get(), &name).await?;
    // DB re-read: the response reflects the stored row.
    let row = fetch_project(&state.pool, user_id, id.get()).await?;
    Ok(Json(ProjectBody {
        data: to_response(row),
    }))
}

/// `DELETE /api/v1/projects/{id} → 204` (no body). Boards cascade in-tx
/// (explicit delete — the DB FK would `SET NULL`); foreign id → 404.
#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}",
    responses(
        (status = 204, description = "Project deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Project not found")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn delete_project(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ProjectId>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    delete_project_boards(&mut *tx, user_id, id.get()).await?;
    if !delete_project_row(&mut *tx, user_id, id.get()).await? {
        tx.rollback().await.map_err(AppError::Db)?;
        return Err(AppError::NotFound);
    }
    tx.commit().await.map_err(AppError::Db)?;
    Ok(StatusCode::NO_CONTENT)
}
