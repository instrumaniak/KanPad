//! labels handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json,
};

use crate::{
    common::{label_seed::ensure_default_labels, pagination::Page},
    error::AppError,
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        apply_label_patch, count_labels, delete_label_row, fetch_label, fetch_label_page,
        insert_label, wire_timestamps, LabelPatch, LabelRow,
    },
    types::{CreateLabelInput, LabelBody, LabelResponse, UpdateLabelInput},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/labels", get(list_labels).post(create_label))
        .route(
            "/labels/{id}",
            axum::routing::patch(update_label).delete(delete_label),
        )
}

fn to_response(row: LabelRow) -> LabelResponse {
    let (created_at, updated_at) = wire_timestamps(&row);
    LabelResponse {
        id: row.id,
        name: row.name,
        color: row.color,
        created_at,
        updated_at,
    }
}

/// `GET /api/v1/labels?page&limit → 200 Page<Label>` (`id ASC`). When the
/// user owns zero rows, the six prod defaults are lazy-seeded first via
/// [`ensure_default_labels`] (single multi-row `INSERT IGNORE` — a
/// concurrent seed race is a no-op, and a `409` from the helper is swallowed
/// so the list still returns 200).
#[utoipa::path(
    get,
    path = "/api/v1/labels",
    responses(
        (status = 200, description = "Label page"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn list_labels(
    State(state): State<AppState>,
    auth: AuthSession,
    pagination: PaginationParams,
) -> Result<Json<Page<LabelResponse>>, AppError> {
    let user_id = auth.user_id.get();
    let mut total = count_labels(&state.pool, user_id).await?;
    if total == 0 {
        // Lazy-seed the prod defaults. `INSERT IGNORE` turns a concurrent
        // re-seed into a no-op; a `Conflict` surfacing here anyway (seed race
        // through another path) is swallowed — the recount below is the
        // source of truth, never the helper's inserted-count.
        match ensure_default_labels(&state.pool, user_id).await {
            Ok(_) => {}
            Err(error) => {
                let seed_error = AppError::Db(error);
                if !matches!(seed_error, AppError::Conflict) {
                    return Err(seed_error);
                }
            }
        }
        total = count_labels(&state.pool, user_id).await?;
    }
    let rows = fetch_label_page(&state.pool, user_id, pagination.page, pagination.limit).await?;
    Ok(Json(Page::new(
        rows.into_iter().map(to_response).collect(),
        pagination.page,
        pagination.limit,
        total,
    )))
}

/// `POST /api/v1/labels{name,color} → 201 {data} + Location`.
/// The name is trimmed before storage (`"  Side  "` stores `"Side"`, so
/// padded dups collide → 409 like exact and case-variant ones).
/// Blank/overlong/missing → 400 `errors[]` via the DTO; bad color → 400;
/// per-user dup (incl. case-variant under `utf8mb4_unicode_ci`) → 409 via
/// the shared 1062 arm.
#[utoipa::path(
    post,
    path = "/api/v1/labels",
    request_body = CreateLabelInput,
    responses(
        (status = 201, description = "Label created", body = LabelBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 409, description = "Label name already exists")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create_label(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<CreateLabelInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let name = input.name.trim().to_string();
    let id = insert_label(&state.pool, user_id, &name, &input.color).await?;
    // DB re-read: the response carries the stored row, never echoes.
    let row = fetch_label(&state.pool, user_id, id).await?;
    let location = format!("/api/v1/labels/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(LabelBody {
            data: to_response(row),
        }),
    ))
}

/// `PATCH /api/v1/labels/{id}{name?,color?} → 200 {data}`. Empty-body patch
/// is a no-op 200 (row untouched); same-name patch stays 200; rename onto
/// another owned name (incl. case-variant) → 409; foreign id → 404.
#[utoipa::path(
    patch,
    path = "/api/v1/labels/{id}",
    request_body = UpdateLabelInput,
    responses(
        (status = 200, description = "Label updated", body = LabelBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Label not found"),
        (status = 409, description = "Label name already exists")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn update_label(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::LabelId>,
    ValidJson(input): ValidJson<UpdateLabelInput>,
) -> Result<Json<LabelBody>, AppError> {
    let user_id = auth.user_id.get();
    // Existence + ownership first: a same-name `PATCH` must stay 200 (an
    // affected-rows check alone would false-404 when nothing changed).
    fetch_label(&state.pool, user_id, id.get()).await?;
    let trimmed_name;
    let patch = LabelPatch {
        name: match input.name.as_deref() {
            Some(raw) => {
                trimmed_name = raw.trim().to_string();
                Some(trimmed_name.as_str())
            }
            None => None,
        },
        color: input.color.as_deref(),
    };
    apply_label_patch(&state.pool, user_id, id.get(), patch).await?;
    // DB re-read: the response reflects the stored row.
    let row = fetch_label(&state.pool, user_id, id.get()).await?;
    Ok(Json(LabelBody {
        data: to_response(row),
    }))
}

/// `DELETE /api/v1/labels/{id} → 204` (no body). `card_labels` rows cascade
/// via `ON DELETE CASCADE`. Foreign/missing id → 404 (no oracle).
#[utoipa::path(
    delete,
    path = "/api/v1/labels/{id}",
    responses(
        (status = 204, description = "Label deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Label not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn delete_label(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::LabelId>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    if !delete_label_row(&state.pool, user_id, id.get()).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
