//! tags handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};

use crate::{
    common::pagination::Page,
    error::AppError,
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        TagPatch, TagRow, apply_tag_patch, count_tags, delete_tag_row, fetch_tag, fetch_tag_page,
        insert_tag, wire_created_at,
    },
    types::{CreateTagInput, TagBody, TagResponse, UpdateTagInput},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/tags", get(list_tags).post(create_tag))
        .route(
            "/tags/{id}",
            axum::routing::patch(update_tag).delete(delete_tag),
        )
}

fn to_response(row: TagRow) -> TagResponse {
    let created_at = wire_created_at(&row);
    TagResponse {
        id: row.id,
        name: row.name,
        color: row.color,
        created_at,
    }
}

/// `GET /api/v1/tags?page&limit → 200 Page<Tag>` (`id ASC`). Tags have no
/// lazy seed (unlike labels): a fresh user lists `total == 0`.
#[utoipa::path(
    get,
    path = "/api/v1/tags",
    responses(
        (status = 200, description = "Tag page"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn list_tags(
    State(state): State<AppState>,
    auth: AuthSession,
    pagination: PaginationParams,
) -> Result<Json<Page<TagResponse>>, AppError> {
    let user_id = auth.user_id.get();
    let total = count_tags(&state.pool, user_id).await?;
    let rows = fetch_tag_page(&state.pool, user_id, pagination.page, pagination.limit).await?;
    Ok(Json(Page::new(
        rows.into_iter().map(to_response).collect(),
        pagination.page,
        pagination.limit,
        total,
    )))
}

/// `POST /api/v1/tags{name,color?} → 201 {data} + Location`.
/// The name is trimmed before storage (`"  Side  "` stores `"Side"`, so
/// padded dups collide → 409 like exact and case-variant ones). An omitted
/// color defaults to teal server-side (never echoed as missing).
/// Blank/overlong/missing → 400 `errors[]` via the DTO; overlong color →
/// 400; per-user dup (incl. case-variant under `utf8mb4_unicode_ci`) → 409
/// via the local write-error map.
#[utoipa::path(
    post,
    path = "/api/v1/tags",
    request_body = CreateTagInput,
    responses(
        (status = 201, description = "Tag created", body = TagBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 409, description = "Tag name already exists")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn create_tag(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<CreateTagInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let name = input.name.trim().to_string();
    let id = insert_tag(&state.pool, user_id, &name, input.color.as_deref()).await?;
    // DB re-read: the response carries the stored row, never echoes.
    let row = fetch_tag(&state.pool, user_id, id).await?;
    let location = format!("/api/v1/tags/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(TagBody {
            data: to_response(row),
        }),
    ))
}

/// `PATCH /api/v1/tags/{id}{name?,color?} → 200 {data}`. Empty-body patch
/// is a no-op 200 (row untouched); same-name patch stays 200; rename onto
/// another owned name (incl. case-variant) → 409; foreign id → 404.
#[utoipa::path(
    patch,
    path = "/api/v1/tags/{id}",
    request_body = UpdateTagInput,
    responses(
        (status = 200, description = "Tag updated", body = TagBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Tag not found"),
        (status = 409, description = "Tag name already exists")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn update_tag(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::TagId>,
    ValidJson(input): ValidJson<UpdateTagInput>,
) -> Result<Json<TagBody>, AppError> {
    let user_id = auth.user_id.get();
    // Existence + ownership first: a same-name `PATCH` must stay 200 (an
    // affected-rows check alone would false-404 when nothing changed).
    fetch_tag(&state.pool, user_id, id.get()).await?;
    let trimmed_name;
    let patch = TagPatch {
        name: match input.name.as_deref() {
            Some(raw) => {
                trimmed_name = raw.trim().to_string();
                Some(trimmed_name.as_str())
            }
            None => None,
        },
        color: input.color.as_deref(),
    };
    apply_tag_patch(&state.pool, user_id, id.get(), patch).await?;
    // DB re-read: the response reflects the stored row.
    let row = fetch_tag(&state.pool, user_id, id.get()).await?;
    Ok(Json(TagBody {
        data: to_response(row),
    }))
}

/// `DELETE /api/v1/tags/{id} → 204` (no body). `note_tags` rows cascade
/// via `ON DELETE CASCADE`. Foreign/missing id → 404 (no oracle).
#[utoipa::path(
    delete,
    path = "/api/v1/tags/{id}",
    responses(
        (status = 204, description = "Tag deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Tag not found")
    )
)]
#[tracing::instrument(skip(state, auth))]
pub async fn delete_tag(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::TagId>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    if !delete_tag_row(&state.pool, user_id, id.get()).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
