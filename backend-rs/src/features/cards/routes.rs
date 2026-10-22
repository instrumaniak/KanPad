//! cards handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post, put},
};

use crate::{
    common::{card_summary::CardSummary, pagination::Page},
    error::{AppError, Internal},
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        CardPatch, apply_card_patch, check_column_owned, check_label_owned, clamp_position,
        close_card_gap, count_cards_in_column, delete_card_label_link, delete_card_row,
        fetch_card_owned, fetch_card_page, fetch_card_spot_for_update, fetch_detail_checklists,
        fetch_detail_items, fetch_detail_labels, fetch_page_labels, fetch_page_progress,
        insert_card, insert_card_label_link, lock_cards_for_update, max_position_for_update,
        open_card_gap, point_move_card, shift_cards_down, shift_cards_up, to_detail, to_summaries,
    },
    types::{AttachLabelInput, CardBody, CreateCardInput, UpdateCardInput, parse_due_date},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
/// CRUD + list + move/reorder (T24a/T24b) + label M2M (T24c): `PUT
/// /cards/{id}/labels/{label_id}` attaches idempotently, the deprecated `POST
/// /cards/{id}/labels` alias accepts `{label_id}` (legacy `{labelId}` too)
/// for one release, and `DELETE /cards/{id}/labels/{label_id}` detaches.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/cards", post(create_card))
        .route(
            "/cards/{id}",
            get(get_card).patch(update_card).delete(delete_card),
        )
        .route("/cards/{id}/labels", post(attach_card_label_alias))
        .route(
            "/cards/{id}/labels/{label_id}",
            put(attach_card_label).delete(detach_card_label),
        )
        .route("/columns/{colId}/cards", get(list_cards))
}

/// Load a card's detail nests (labels + checklists + items) and render the
/// wire body. Single owner for the fetch-assemble sequence so handlers stay
/// one call; each select is one query (the executor value moves).
async fn load_detail(
    pool: &sqlx::MySqlPool,
    row: super::store::CardRow,
) -> Result<CardBody, AppError> {
    let card_id = row.id.get();
    let labels = fetch_detail_labels(pool, card_id).await?;
    let checklists = fetch_detail_checklists(pool, card_id).await?;
    let items = fetch_detail_items(pool, card_id).await?;
    Ok(CardBody {
        data: to_detail(row, labels, checklists, items),
    })
}

/// `POST /api/v1/cards{title,column_id,position?,description?,due_date?} →
/// 201 {data} + Location`. An explicit `position` clamps to `[0, max + 1]`
/// then opens a gap with a single range shift (never a verbatim duplicate
/// write); the default appends `MAX(position) + 1` taken under `SELECT ...
/// FOR UPDATE` in the create tx. A foreign/missing `column_id` rolls back to
/// 404 (no oracle, never 403).
#[utoipa::path(
    post,
    path = "/api/v1/cards",
    request_body = CreateCardInput,
    responses(
        (status = 201, description = "Card created", body = CardBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Column not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn create_card(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<CreateCardInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    check_column_owned(&mut *tx, user_id, input.column_id).await?;
    let max = max_position_for_update(&mut *tx, input.column_id).await?;
    let position = match input.position {
        Some(requested) => requested.clamp(0, max + 1),
        None => max + 1,
    };
    if input.position.is_some() {
        open_card_gap(&mut *tx, input.column_id, position).await?;
    }
    let due_date = input
        .due_date
        .as_deref()
        .map(|raw| parse_due_date(raw).ok_or(AppError::Internal(Internal)))
        .transpose()?;
    let id = insert_card(
        &mut *tx,
        input.column_id,
        &input.title,
        position,
        input.description.as_deref(),
        due_date,
    )
    .await?;
    tx.commit().await.map_err(AppError::Db)?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_card_owned(&state.pool, user_id, id).await?;
    let body = load_detail(&state.pool, row).await?;
    let location = format!("/api/v1/cards/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(body),
    ))
}

/// `GET /api/v1/cards/{id} → 200 {data:CardDetail}` (checklists{items},
/// labels, `checklist_progress` only when the card owns checklists).
/// Foreign/missing id → 404 (no oracle, never 403); garbage id → 400
/// `invalid-path` via `ValidPath`.
#[utoipa::path(
    get,
    path = "/api/v1/cards/{id}",
    responses(
        (status = 200, description = "Card detail"),
        (status = 400, description = "Invalid card id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_card(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::CardId>,
) -> Result<Json<CardBody>, AppError> {
    let user_id = auth.user_id.get();
    let row = fetch_card_owned(&state.pool, user_id, id.get()).await?;
    Ok(Json(load_detail(&state.pool, row).await?))
}

/// `PATCH /api/v1/cards/{id} → 200 {data:CardDetail}` (scalars +
/// move/reorder, T24b): `title`/`description`/`due_date` patch as in T24a
/// (explicit `null` clears; empty `{}` is a no-op 200), while `column_id`
/// and/or `position` trigger a set-based move in the SAME tx — same-column =
/// one range shift + point update, cross-column = close-gap + open-gap +
/// point update. Requested positions clamp ([0, N-1] same-column, [0, M]
/// cross-column, missing cross-column position appends `M = max+1`) and the
/// CLAMPED value is written. Columns lock ascending (`min` first) via
/// `SELECT … FOR UPDATE`; the moving spot re-reads under `FOR UPDATE` so a
/// same-card race restarts the tx instead of writing unlocked. A
/// foreign/missing `column_id` rolls back to 404 (no oracle, never 403).
/// The response re-reads the stored row (never echoes).
#[utoipa::path(
    patch,
    path = "/api/v1/cards/{id}",
    request_body = UpdateCardInput,
    responses(
        (status = 200, description = "Card updated", body = CardBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn update_card(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::CardId>,
    ValidJson(input): ValidJson<UpdateCardInput>,
) -> Result<Json<CardBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let due_date: Option<Option<chrono::NaiveDateTime>> = input
        .due_date
        .as_ref()
        .map(|nullable| {
            nullable
                .as_deref()
                .map(|raw| parse_due_date(raw).ok_or(AppError::Internal(Internal)))
                .transpose()
        })
        .transpose()?;
    let patch = CardPatch {
        title: input.title.as_deref(),
        description: input
            .description
            .as_ref()
            .map(|nullable| nullable.as_deref()),
        due_date,
    };
    // Any `column_id`/`position` key (even explicit same-column/same-position)
    // takes the move path; scalars-only bodies keep the T24a short path.
    if input.column_id.is_some() || input.position.is_some() {
        move_card(
            &state.pool,
            user_id,
            id,
            input.column_id,
            input.position,
            patch,
        )
        .await?;
    } else {
        let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
        fetch_card_owned(&mut *tx, user_id, id).await?;
        apply_card_patch(&mut *tx, id, patch).await?;
        tx.commit().await.map_err(AppError::Db)?;
    }
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_card_owned(&state.pool, user_id, id).await?;
    Ok(Json(load_detail(&state.pool, row).await?))
}

/// Move/reorder worker for [`update_card`]: set-based range `UPDATE`s + point
/// update in a short tx (columns locked ascending, spot re-read locked, tx
/// restarted when the card changed columns under us — see store docs).
/// `target_column` is `None` for same-column reorders; `requested` is `None`
/// for cross-column appends (`max + 1`). Scalar `patch` applies in the same
/// tx after the move.
async fn move_card(
    pool: &sqlx::MySqlPool,
    user_id: i32,
    id: i32,
    target_column: Option<i32>,
    requested: Option<i32>,
    patch: CardPatch<'_>,
) -> Result<(), AppError> {
    // A same-card race (the card changed columns between our read and our
    // locks) restarts the whole tx; bounded — exhaustion is unreachable in
    // practice (it needs a concurrent move of the SAME card on every try).
    for _ in 0..5 {
        let mut tx = pool.begin().await.map_err(AppError::Db)?;
        let owned = fetch_card_owned(&mut *tx, user_id, id).await?;
        let source = owned.column_id.get();
        let target = target_column.unwrap_or(source);
        if target != source {
            check_column_owned(&mut *tx, user_id, target).await?;
        }
        // Stable lock order: `min` column first (opposite-direction moves
        // serialize instead of deadlocking).
        let (first, second) = if source <= target {
            (source, target)
        } else {
            (target, source)
        };
        let first_locked = lock_cards_for_update(&mut *tx, first).await?;
        let second_locked = if second == first {
            Vec::new()
        } else {
            lock_cards_for_update(&mut *tx, second).await?
        };
        let (fresh_column, fresh_position) =
            fetch_card_spot_for_update(&mut *tx, user_id, id).await?;
        if fresh_column != source {
            // Same-card race: the card left `source` after our read, so our
            // locks cover the wrong column — roll back and retry with fresh
            // values rather than writing against an unlocked column.
            tx.rollback().await.map_err(AppError::Db)?;
            continue;
        }
        if target == source {
            let upper = i32::try_from(first_locked.len())
                .map_err(|_| AppError::Internal(Internal))?
                .saturating_sub(1);
            let new_position = clamp_position(requested.unwrap_or(fresh_position), upper);
            if new_position < fresh_position {
                shift_cards_up(&mut *tx, target, new_position, fresh_position).await?;
            } else if new_position > fresh_position {
                shift_cards_down(&mut *tx, target, fresh_position, new_position).await?;
            }
            point_move_card(&mut *tx, id, target, new_position).await?;
        } else {
            let target_count = if target == first {
                first_locked.len()
            } else {
                second_locked.len()
            };
            let target_count =
                i32::try_from(target_count).map_err(|_| AppError::Internal(Internal))?;
            close_card_gap(&mut *tx, source, fresh_position).await?;
            let new_position = requested
                .map(|position| clamp_position(position, target_count))
                .unwrap_or(target_count);
            // Appending (`new == M`) shifts zero rows — the same `UPDATE`
            // covers both cases, no branch needed.
            open_card_gap(&mut *tx, target, new_position).await?;
            point_move_card(&mut *tx, id, target, new_position).await?;
        }
        apply_card_patch(&mut *tx, id, patch).await?;
        tx.commit().await.map_err(AppError::Db)?;
        return Ok(());
    }
    Err(AppError::Internal(Internal))
}

/// Two-id path for the label M2M routes (`/cards/{id}/labels/{label_id}`).
/// Field names match the route placeholders; garbage ids fail int-parse →
/// 400 `invalid-path` via `ValidPath`. `pub` (like the handlers) so the
/// `pub` attach/detach signatures don't leak a private type.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct CardLabelPath {
    /// Owning card id.
    pub id: crate::common::ids::CardId,
    /// Label id to attach/detach.
    pub label_id: crate::common::ids::LabelId,
}

/// Attach worker shared by [`attach_card_label`] (PUT) and
/// [`attach_card_label_alias`] (deprecated POST): proves card + label
/// ownership (foreign/missing id → 404, never 403), inserts idempotently
/// (`INSERT IGNORE` — a re-attach is a no-op, never 409), then re-reads the
/// stored row (the response reflects the DB, never echoes).
async fn attach_and_reload(
    pool: &sqlx::MySqlPool,
    user_id: i32,
    card_id: i32,
    label_id: i32,
) -> Result<CardBody, AppError> {
    fetch_card_owned(pool, user_id, card_id).await?;
    check_label_owned(pool, user_id, label_id).await?;
    insert_card_label_link(pool, card_id, label_id).await?;
    let row = fetch_card_owned(pool, user_id, card_id).await?;
    load_detail(pool, row).await
}

/// `PUT /api/v1/cards/{id}/labels/{label_id} → 200 {data:CardDetail}`
/// (idempotent: double attach keeps one link row). Foreign/missing card or
/// label → 404 (no oracle, never 403); garbage id → 400 `invalid-path`.
#[utoipa::path(
    put,
    path = "/api/v1/cards/{id}/labels/{label_id}",
    responses(
        (status = 200, description = "Label attached", body = CardBody),
        (status = 400, description = "Invalid card or label id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card or label not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn attach_card_label(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(path): ValidPath<CardLabelPath>,
) -> Result<Json<CardBody>, AppError> {
    Ok(Json(
        attach_and_reload(
            &state.pool,
            auth.user_id.get(),
            path.id.get(),
            path.label_id.get(),
        )
        .await?,
    ))
}

/// `POST /api/v1/cards/{id}/labels{label_id} → 200 {data:CardDetail}`
/// (deprecated alias, kept one release; legacy camelCase `{labelId}` also
/// accepted via the DTO serde alias). Missing key → 400 `validation-error`
/// with `errors[]`; `0`/negative → 400 via the DTO range rule; ownership and
/// idempotency match [`attach_card_label`].
#[utoipa::path(
    post,
    path = "/api/v1/cards/{id}/labels",
    request_body = AttachLabelInput,
    responses(
        (status = 200, description = "Label attached", body = CardBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card or label not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn attach_card_label_alias(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::CardId>,
    ValidJson(input): ValidJson<AttachLabelInput>,
) -> Result<Json<CardBody>, AppError> {
    let label_id = match input.label_id {
        Some(label_id) => label_id,
        None => {
            let mut report = garde::Report::new();
            report.append(
                garde::Path::new("label_id"),
                garde::Error::new("must be present"),
            );
            return Err(AppError::Validation(report));
        }
    };
    Ok(Json(
        attach_and_reload(&state.pool, auth.user_id.get(), id.get(), label_id).await?,
    ))
}

/// `DELETE /api/v1/cards/{id}/labels/{label_id} → 204` (hard detach, empty
/// body). Unassigned (stale or never-attached) link → 404 (not 409);
/// foreign/missing card or label → 404 (no oracle, never 403 — ownership is
/// proven before the link check); garbage id → 400 `invalid-path`.
#[utoipa::path(
    delete,
    path = "/api/v1/cards/{id}/labels/{label_id}",
    responses(
        (status = 204, description = "Label detached"),
        (status = 400, description = "Invalid card or label id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card, label, or assignment not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn detach_card_label(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(path): ValidPath<CardLabelPath>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let (card_id, label_id) = (path.id.get(), path.label_id.get());
    fetch_card_owned(&state.pool, user_id, card_id).await?;
    check_label_owned(&state.pool, user_id, label_id).await?;
    if !delete_card_label_link(&state.pool, card_id, label_id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/v1/cards/{id} → 204` (hard delete, empty body). The position
/// gap closes with a set-based range `UPDATE` in the SAME tx. Foreign/missing
/// id → 404 (no oracle, never 403); garbage id → 400 `invalid-path`.
#[utoipa::path(
    delete,
    path = "/api/v1/cards/{id}",
    responses(
        (status = 204, description = "Card deleted"),
        (status = 400, description = "Invalid card id"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Card not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn delete_card(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::CardId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    let owned = fetch_card_owned(&mut *tx, user_id, id).await?;
    delete_card_row(&mut *tx, id).await?;
    close_card_gap(&mut *tx, owned.column_id.get(), owned.position).await?;
    tx.commit().await.map_err(AppError::Db)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/columns/{colId}/cards?page&limit → 200 Page<CardSummary>`
/// (`position ASC`; never selects TEXT — no `description` key, only the
/// `has_description` flag; `checklist_progress` only when the card owns
/// checklists). Foreign/missing column → 404 (no oracle, never 403).
#[utoipa::path(
    get,
    path = "/api/v1/columns/{colId}/cards",
    responses(
        (status = 200, description = "Column cards page"),
        (status = 400, description = "Invalid column id or pagination"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Column not found")
    )
)]
#[tracing::instrument(skip(state))]
pub async fn list_cards(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(column_id): ValidPath<crate::common::ids::ColumnId>,
    pagination: PaginationParams,
) -> Result<Json<Page<CardSummary>>, AppError> {
    let user_id = auth.user_id.get();
    let column_id = column_id.get();
    check_column_owned(&state.pool, user_id, column_id).await?;
    let total = count_cards_in_column(&state.pool, column_id).await?;
    if total == 0 {
        return Ok(Json(Page::empty(pagination.page, pagination.limit)));
    }
    let offset = pagination
        .page
        .saturating_sub(1)
        .saturating_mul(pagination.limit);
    let rows = fetch_card_page(&state.pool, column_id, pagination.limit, offset).await?;
    let card_ids: Vec<i32> = rows.iter().map(|row| row.0).collect();
    let labels = fetch_page_labels(&state.pool, &card_ids).await?;
    let progress = fetch_page_progress(&state.pool, &card_ids).await?;
    Ok(Json(Page::new(
        to_summaries(rows, labels, progress)?,
        pagination.page,
        pagination.limit,
        total,
    )))
}
