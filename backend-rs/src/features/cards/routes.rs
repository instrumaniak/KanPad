//! cards handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post, put},
};

use crate::{
    common::{
        card_summary::CardSummary,
        ownership::{check_board_owned, check_column_owned, check_label_owned},
        pagination::Page,
    },
    error::{AppError, Internal},
    extract::{AuthSession, PaginationParams, ValidJson, ValidPath, ValidQuery},
    state::AppState,
};

use super::{
    store::{
        CardPatch, apply_card_patch, count_board_cards, count_cards_in_column,
        delete_card_label_link, delete_card_row, fetch_board_card_page, fetch_card_keys_in_column,
        fetch_card_owned, fetch_card_page, fetch_card_spot_for_update, fetch_detail_checklists,
        fetch_detail_items, fetch_detail_labels, fetch_last_sort_key_for_update, fetch_page_labels,
        fetch_page_progress, hydrate_board_card_items, insert_card, insert_card_label_link,
        point_move_card, to_detail, to_summaries,
    },
    types::{
        AttachLabelInput, BoardCardListItem, BoardCardsListQuery, CardBody, CreateCardInput,
        UpdateCardInput, parse_due_date,
    },
};
use crate::common::{
    order_key::{clamp_position, key_between, key_for_index, needs_rebalance},
    rebalance::{RebalanceScope, rebalance_sort_keys},
    retry::MAX_ATTEMPTS,
    tx_retry::{TxOutcome, TxRetryOpts, with_tx_retry, with_tx_retry_restartable},
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
        .route("/boards/{boardId}/cards", get(list_board_cards))
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
/// 201 {data} + Location`. An explicit `position` clamps to `[0, N]` and gets
/// the between-neighbours sort key (never a verbatim write); the default
/// appends via a serialized `ORDER BY sort_key DESC LIMIT 1 FOR UPDATE` read
/// + `key_between`. A foreign/missing `column_id` rolls back to 404 (no
/// oracle, never 403). Duplicate-key races on the unique
/// `(column_id, sort_key)` index retry (`1213`/`1205`/scoped `1062`), like a
/// move.
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
#[tracing::instrument(skip(state, auth, input))]
pub async fn create_card(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidJson(input): ValidJson<CreateCardInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let due_date = input
        .due_date
        .as_deref()
        .map(|raw| parse_due_date(raw).ok_or(AppError::Internal(Internal)))
        .transpose()?;
    // Transient lock failures (`1213`/`1205`/scoped `1062`) retry inside the
    // shared kernel; exhaustion surfaces at the boundary as 503 + Retry-After.
    let input = &input;
    let id = with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
        check_column_owned(&mut *tx, user_id, input.column_id).await?;
        let sort_key = match input.position {
            Some(requested) => {
                let others = fetch_card_keys_in_column(&mut *tx, input.column_id, None).await?;
                let target = clamp_position(
                    requested,
                    i32::try_from(others.len()).map_err(|_| AppError::Internal(Internal))?,
                ) as usize;
                let keys: Vec<&str> = others.iter().map(|(_, key)| key.as_str()).collect();
                key_for_index(&keys, target)?
            }
            None => match fetch_last_sort_key_for_update(&mut *tx, input.column_id).await? {
                Some(last) => key_between(Some(&last), None)?,
                None => key_between(None, None)?,
            },
        };
        let id = insert_card(
            &mut *tx,
            input.column_id,
            &input.title,
            &sort_key,
            input.description.as_deref(),
            due_date,
        )
        .await?;
        if needs_rebalance(&sort_key) {
            rebalance_sort_keys(
                &mut tx,
                RebalanceScope::Cards {
                    column_id: input.column_id,
                },
            )
            .await?;
        }
        Ok((tx, id))
    })
    .await?;
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
#[tracing::instrument(skip(state, auth))]
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
/// and/or `position` trigger a single-row sort-key move in the SAME tx —
/// same-column and cross-column are the same between-neighbours key write.
/// Requested positions clamp ([0, N-1] same-column, [0, M] cross-column,
/// missing cross-column position appends) and the CLAMPED index is written.
/// The moving row locks `FOR UPDATE`; a same-card race restarts the tx
/// instead of writing unlocked. A foreign/missing `column_id` rolls back to
/// 404 (no oracle, never 403). The response re-reads the stored row (never
/// echoes).
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
#[tracing::instrument(skip(state, auth, input))]
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
        with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
            fetch_card_owned(&mut *tx, user_id, id).await?;
            apply_card_patch(&mut *tx, id, patch).await?;
            Ok((tx, ()))
        })
        .await?;
    }
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_card_owned(&state.pool, user_id, id).await?;
    Ok(Json(load_detail(&state.pool, row).await?))
}

/// Move/reorder worker for [`update_card`]: single-row writes + point update
/// in a short tx (the moving row re-reads under `FOR UPDATE` so a same-card
/// race restarts the tx instead of writing unlocked — see store docs).
/// `target_column` is `None` for same-column reorders; `requested` is `None`
/// for cross-column appends (the end of the target). Scalar `patch` applies
/// in the same tx after the move. The tx runs at `READ COMMITTED` (set
/// before `BEGIN` — MySQL rejects the statement inside a transaction with
/// error 1568) and transient lock failures (`1213` deadlock / `1205`
/// lock-wait timeout / scoped `1062` on a `UQ_*_sort_key` unique index) are
/// retried up to [`MAX_ATTEMPTS`] inside the shared `with_tx_retry` kernel —
/// exhaustion surfaces at the boundary as 503 + `Retry-After: 2`. A
/// same-card race returns `Restart`, which the kernel passes through
/// UNRETRIED (after rollback); the bounded worker loop restarts the tx with
/// fresh reads instead of burning the lock-retry budget on a logical
/// conflict.
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
    for _attempt in 0..MAX_ATTEMPTS {
        match with_tx_retry_restartable(pool, TxRetryOpts::default(), |mut tx| async move {
            let owned = fetch_card_owned(&mut *tx, user_id, id).await?;
            let source = owned.column_id.get();
            let target = target_column.unwrap_or(source);
            if target != source {
                check_column_owned(&mut *tx, user_id, target).await?;
            }
            let (fresh_column, fresh_sort_key) =
                fetch_card_spot_for_update(&mut *tx, user_id, id).await?;
            if fresh_column != source {
                // Same-card race: the card left `source` after our read, so
                // our lock covers the wrong column — roll back and restart
                // with fresh values rather than writing against an unlocked
                // column.
                return Ok((tx, TxOutcome::Restart));
            }
            // Sibling keys of the TARGET (excluding the moving card). Neighbor
            // reads stay unlocked (D3: last-write-wins for concurrent same-gap
            // drops; the unique index breaks true same-key races).
            let others = fetch_card_keys_in_column(&mut *tx, target, Some(id)).await?;
            let others_len =
                i32::try_from(others.len()).map_err(|_| AppError::Internal(Internal))?;
            let others_keys: Vec<&str> = others.iter().map(|(_, key)| key.as_str()).collect();
            let fresh_index = others_keys
                .iter()
                .filter(|key| **key < fresh_sort_key.as_str())
                .count();
            let target_index = match requested {
                Some(requested) => clamp_position(requested, others_len) as usize,
                None if target == source => fresh_index,
                None => others.len(),
            };
            let sort_key = key_for_index(&others_keys, target_index)?;
            point_move_card(&mut *tx, id, target, &sort_key).await?;
            if needs_rebalance(&sort_key) {
                rebalance_sort_keys(&mut tx, RebalanceScope::Cards { column_id: target }).await?;
            }
            apply_card_patch(&mut *tx, id, patch).await?;
            Ok((tx, TxOutcome::Done(())))
        })
        .await?
        {
            TxOutcome::Done(()) => return Ok(()),
            TxOutcome::Restart => {
                // The kernel already rolled the tx back. A logical conflict
                // is not a lock conflict — no backoff, no lock-retry budget;
                // just restart with fresh values.
                continue;
            }
        }
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
#[tracing::instrument(skip(state, auth))]
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
#[tracing::instrument(skip(state, auth, input))]
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
            return Err(AppError::from(report));
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
#[tracing::instrument(skip(state, auth))]
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

/// `DELETE /api/v1/cards/{id} → 204` (hard delete, empty body). Sibling
/// `sort_key`s are untouched (no gap to close). Foreign/missing id → 404 (no
/// oracle, never 403); garbage id → 400 `invalid-path`.
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
#[tracing::instrument(skip(state, auth))]
pub async fn delete_card(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::CardId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
        fetch_card_owned(&mut *tx, user_id, id).await?;
        delete_card_row(&mut *tx, id).await?;
        Ok((tx, ()))
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/columns/{colId}/cards?page&limit → 200 Page<CardSummary>`
/// (sort-key order; never selects TEXT — no `description` key, only the
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
#[tracing::instrument(skip(state, auth))]
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

/// `GET /api/v1/boards/{boardId}/cards?page&limit&sort&order → 200 Page<BoardCardListItem>`
/// (board-scoped list view; never selects TEXT — no `description` key, only
/// `has_description`; `checklist_progress` only when the card owns checklists).
/// `position` is the per-column computed position in stable column sort-key
/// order (0-based). `due_date` NULLs sort last in both directions (locked).
/// Foreign/missing board → 404 (no oracle, never 403).
#[utoipa::path(
    get,
    path = "/api/v1/boards/{boardId}/cards",
    params(
        ("boardId" = i32, Path, description = "Board id"),
        BoardCardsListQuery
    ),
    responses(
        (status = 200, description = "Board cards page", body = Page<BoardCardListItem>),
        (status = 400, description = "Invalid board id, pagination, or query"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Board not found")
    )
)]
#[tracing::instrument(skip(state, auth, filters))]
pub async fn list_board_cards(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(board_id): ValidPath<crate::common::ids::BoardId>,
    ValidQuery(filters): ValidQuery<BoardCardsListQuery>,
) -> Result<Json<Page<BoardCardListItem>>, AppError> {
    let user_id = auth.user_id.get();
    let board_id = board_id.get();
    let (page, limit) = filters.resolved_pagination().map_err(AppError::from)?;
    check_board_owned(&state.pool, user_id, board_id).await?;

    let total = count_board_cards(&state.pool, board_id).await?;
    if total == 0 {
        return Ok(Json(Page::empty(page, limit)));
    }

    let offset = page.saturating_sub(1).saturating_mul(limit);
    let mut items = fetch_board_card_page(
        &state.pool,
        board_id,
        filters.resolved_sort(),
        filters.resolved_order(),
        limit,
        offset,
    )
    .await?;
    hydrate_board_card_items(&state.pool, &mut items).await?;

    Ok(Json(Page::new(items, page, limit, total)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        sync::{Arc, Mutex},
    };
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::SubscriberExt as _;

    /// Captures the RENDERED value of every field of every new span.
    ///
    /// `#[instrument]` records every argument it is not told to skip, via
    /// that argument's `Debug` impl — so the DTO body (card title +
    /// description) and the session's email used to land in the log sink.
    /// This capture is what proves they no longer do.
    #[derive(Clone, Default)]
    struct SpanFieldCapture(Arc<Mutex<Vec<String>>>);

    impl Visit for SpanFieldCapture {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.0
                .lock()
                .expect("capture lock")
                .push(format!("{}={:?}", field.name(), value));
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.0
                .lock()
                .expect("capture lock")
                .push(format!("{}={}", field.name(), value));
        }
    }

    impl<S> tracing_subscriber::Layer<S> for SpanFieldCapture
    where
        S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    {
        fn on_new_span(
            &self,
            attributes: &tracing::span::Attributes<'_>,
            _id: &tracing::Id,
            _context: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut visitor = self.clone();
            attributes.record(&mut visitor);
        }
    }

    /// AC (T56, `54` B4): the card-create span must carry NEITHER the card
    /// title NOR its description NOR the session's email.
    ///
    /// The instrumented handler creates its span when its future is first
    /// POLLED, so one poll is enough to record the span's fields; the
    /// handler's body never completes (the pool is lazy and the future is
    /// dropped right after the poll).
    #[tokio::test]
    async fn card_create_span_records_neither_the_body_nor_the_email() {
        const TITLE: &str = "TOP-SECRET-CARD-TITLE";
        const DESCRIPTION: &str = "TOP-SECRET-CARD-DESCRIPTION";
        const EMAIL: &str = "top-secret@example.com";

        let capture = SpanFieldCapture::default();
        let subscriber = tracing_subscriber::registry().with(capture.clone());
        let guard = tracing::subscriber::set_default(subscriber);

        // An unroutable DB URL: the pool is lazy and the handler is never
        // awaited to completion, so nothing ever talks to a database.
        let settings = crate::config::Settings {
            database_url: Some("://not a url".to_string()),
            ..Default::default()
        };
        let state = crate::state::AppState::from_settings(settings);
        let auth = AuthSession {
            user_id: crate::common::ids::UserId::from(1),
            email: EMAIL.to_string(),
            role: crate::common::ids::Role::User,
        };
        let input = ValidJson(CreateCardInput {
            title: TITLE.to_string(),
            column_id: 1,
            position: None,
            description: Some(DESCRIPTION.to_string()),
            due_date: None,
        });

        let mut future = Box::pin(create_card(State(state), auth, input));
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        let _ = future.as_mut().poll(&mut context);
        drop(future);
        drop(guard);

        let rendered = capture.0.lock().expect("capture lock").join("\n");
        assert!(
            !rendered.contains(TITLE),
            "the card title leaked into the span: {rendered}"
        );
        assert!(
            !rendered.contains(DESCRIPTION),
            "the card description leaked into the span: {rendered}"
        );
        assert!(
            !rendered.contains(EMAIL),
            "the session email leaked into the span: {rendered}"
        );
    }
}
