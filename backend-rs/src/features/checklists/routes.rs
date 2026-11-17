//! checklists handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use sqlx::Connection;

use crate::{
    common::ownership::check_card_owned,
    error::{AppError, Internal},
    extract::{AuthSession, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        ItemPatch, apply_item_patch, delete_checklist_row, delete_item_row,
        fetch_card_checklist_items, fetch_checklist_items, fetch_checklist_owned,
        fetch_checklists_for_card, fetch_item_keys_in_checklist, fetch_item_owned,
        fetch_last_sort_key_for_update, insert_checklist, insert_item, point_move_item_sort_key,
        rename_checklist, to_item_response, to_list, to_response,
    },
    types::{
        ChecklistBody, ChecklistItemBody, ChecklistListBody, CreateChecklistInput,
        CreateChecklistItemInput, UpdateChecklistInput, UpdateChecklistItemInput,
    },
};
use crate::common::{
    order_key::{clamp_position, key_between, key_for_index, needs_rebalance},
    rebalance::{RebalanceScope, rebalance_sort_keys},
    retry::{MAX_ATTEMPTS, is_transient_lock_error},
};
use std::time::Duration;

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
/// sort-key order). Foreign/missing card → 404 (no oracle, never 403).
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
/// Location`. An absent `position` appends via a serialized `ORDER BY
/// sort_key DESC LIMIT 1 FOR UPDATE` read + `key_between` in the create tx;
/// an explicit `position` clamps to `[0, N]` and takes the
/// between-neighbours sort key. A foreign/missing checklist id rolls back to
/// 404 (no oracle, never 403); garbage id → 400 `invalid-path` via `ValidPath`;
/// blank/overlong text or negative `position` → 400 `errors[]` via the DTO
/// (floats never reach garde — serde rejects them first as `invalid-json`).
/// The transaction runs under `READ COMMITTED` and retries the whole create
/// (≤ `MAX_ATTEMPTS` attempts, linear backoff) on a `1213`/`1205` or a
/// `UQ_checklist_items_checklist_sort_key`-scoped `1062` — two concurrent
/// explicit-position inserts can pick the same between-neighbours key (F2,
/// 2026-11-15); unrelated unique violations never retry. The serialized
/// append path cannot 1062, but shares the same loop.
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
    let mut created_id = None;
    for attempt in 0..MAX_ATTEMPTS {
        // Same READ COMMITTED + retry scoping as the card paths (F2).
        let mut connection = state.pool.acquire().await.map_err(AppError::Db)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *connection)
            .await
            .map_err(AppError::Db)?;
        let mut tx = connection.begin().await.map_err(AppError::Db)?;
        let step: Result<i32, AppError> = async {
            fetch_checklist_owned(&mut *tx, user_id, checklist_id).await?;
            let sort_key = match input.position {
                Some(requested) => {
                    let others = fetch_item_keys_in_checklist(&mut *tx, checklist_id, None).await?;
                    let target = clamp_position(
                        requested,
                        i32::try_from(others.len()).map_err(|_| AppError::Internal(Internal))?,
                    ) as usize;
                    let keys: Vec<&str> = others.iter().map(|(_, key)| key.as_str()).collect();
                    key_for_index(&keys, target)?
                }
                None => match fetch_last_sort_key_for_update(&mut *tx, checklist_id).await? {
                    Some(last) => key_between(Some(&last), None)?,
                    None => key_between(None, None)?,
                },
            };
            let id = insert_item(&mut *tx, checklist_id, &input.text, &sort_key).await?;
            if needs_rebalance(&sort_key) {
                rebalance_sort_keys(&mut tx, RebalanceScope::ChecklistItems { checklist_id })
                    .await?;
            }
            Ok(id)
        }
        .await;
        match step {
            Ok(id) => {
                tx.commit().await.map_err(AppError::Db)?;
                created_id = Some(id);
                break;
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if is_transient_lock_error(&error) && attempt + 1 < MAX_ATTEMPTS {
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = MAX_ATTEMPTS,
                        error = %error,
                        "retrying a checklist-item create after a transient failure"
                    );
                    tokio::time::sleep(Duration::from_millis(10 * u64::from(attempt + 1))).await;
                    continue;
                }
                return Err(error);
            }
        }
    }
    let Some(id) = created_id else {
        return Err(AppError::Internal(Internal));
    };
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

/// `PATCH /api/v1/checklist-items/{id}{text?,is_completed?,position?} → 200
/// {data}`. An empty `{}` body is a no-op 200. `position` reorders the item
/// with a single between-neighbours sort-key write (clamped to `[0, N-1]`,
/// never 400). The response re-reads the stored row (never echoes).
/// Foreign/missing id → 404, never 403. The transaction runs under `READ
/// COMMITTED` and retries the whole PATCH (≤ `MAX_ATTEMPTS` attempts, linear
/// backoff) on a `1213`/`1205` or a `UQ_checklist_items_checklist_sort_key`-
/// scoped `1062` — two concurrent moves can pick the same between-neighbours
/// key (F2, 2026-11-15); unrelated unique violations never retry.
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
    for attempt in 0..MAX_ATTEMPTS {
        // Same READ COMMITTED + retry scoping as the card paths (F2).
        let mut connection = state.pool.acquire().await.map_err(AppError::Db)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *connection)
            .await
            .map_err(AppError::Db)?;
        let mut tx = connection.begin().await.map_err(AppError::Db)?;
        let step: Result<(), AppError> = async {
            // Existence + ownership first: an idempotent no-change `PATCH`
            // must stay 200 (an affected-rows check alone would false-404
            // when nothing changed).
            let owned = fetch_item_owned(&mut *tx, user_id, id).await?;
            if let Some(requested) = input.position {
                // Move: clamp to `[0, N-1]` and write ONE between-neighbours
                // sort key (never a sibling rewrite). Neighbour reads stay
                // unlocked (D3).
                let checklist_id = owned.checklist_id.get();
                let others = fetch_item_keys_in_checklist(&mut *tx, checklist_id, Some(id)).await?;
                let upper =
                    i32::try_from(others.len()).map_err(|_| AppError::Internal(Internal))?;
                let target = clamp_position(requested, upper) as usize;
                let keys: Vec<&str> = others.iter().map(|(_, key)| key.as_str()).collect();
                let sort_key = key_for_index(&keys, target)?;
                point_move_item_sort_key(&mut *tx, id, &sort_key).await?;
                if needs_rebalance(&sort_key) {
                    rebalance_sort_keys(&mut tx, RebalanceScope::ChecklistItems { checklist_id })
                        .await?;
                }
            }
            apply_item_patch(
                &mut *tx,
                id,
                ItemPatch {
                    text: input.text.as_deref(),
                    is_completed: input.is_completed,
                },
            )
            .await?;
            Ok(())
        }
        .await;
        match step {
            Ok(()) => {
                tx.commit().await.map_err(AppError::Db)?;
                break;
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if is_transient_lock_error(&error) && attempt + 1 < MAX_ATTEMPTS {
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = MAX_ATTEMPTS,
                        error = %error,
                        "retrying a checklist-item update after a transient failure"
                    );
                    tokio::time::sleep(Duration::from_millis(10 * u64::from(attempt + 1))).await;
                    continue;
                }
                return Err(error);
            }
        }
    }
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
