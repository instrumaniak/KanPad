//! columns handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, patch, post},
};

use crate::{
    common::{
        card_summary::{
            CardSummary, assemble_summaries, fetch_board_card_labels, fetch_board_card_progress,
            fetch_board_cards,
        },
        ownership::check_board_owned,
    },
    error::{AppError, Internal},
    extract::{AuthSession, ValidJson, ValidPath},
    state::AppState,
};

use super::{
    store::{
        delete_column_row, fetch_card_ids_for_sort, fetch_column_keys_in_board, fetch_column_owned,
        fetch_columns, fetch_last_card_sort_key_for_update, fetch_last_sort_key_for_update,
        fetch_source_card_ids_for_update, insert_column, move_cards_into,
        point_move_column_sort_key, rename_column, rewrite_card_sort_keys, to_response,
        to_response_one,
    },
    types::{
        ColumnBody, ColumnListBody, CreateColumnInput, DEFAULT_COLUMN_NAME, MoveCardsBody,
        MoveCardsData, MoveCardsInput, UpdateColumnInput,
    },
};
use crate::common::{
    order_key::{clamp_position, key_between, key_for_index, keys_between, needs_rebalance},
    rebalance::{RebalanceScope, rebalance_sort_keys},
    tx_retry::{TxRetryOpts, with_tx_retry},
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
/// Sort rides `PATCH /columns/{id}{sort_order}` inline (T23b) and bulk move
/// rides `POST /columns/{id}/move-cards` (T23c). The deleted verb paths
/// `/columns/{id}/sort` and `/columns/{id}/move-all` stay unregistered
/// (→ 404 from the API fallback).
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
        .route("/columns/{id}/move-cards", post(move_cards))
}

/// `GET /api/v1/boards/{boardId}/columns → 200 {data[]}` (nested unpaginated,
/// sort-key order, embeds card summaries — never `description` TEXT).
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
#[tracing::instrument(skip(state, auth))]
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
/// The sort key is `key_between(last_key, None)` from a serialized
/// `ORDER BY sort_key DESC LIMIT 1 FOR UPDATE` read in the create tx (the
/// serialized append contract — no `1062` path there); an over-64-char key
/// fires the same in-tx two-phase rebalance as the other append paths. This
/// tx is the ONLY un-retried position-rewriting path pre-T020; transient
/// lock failures now retry inside the shared kernel (`READ COMMITTED` +
/// bounded attempts; exhaustion → 503 + `Retry-After: 2`). An absent name
/// defaults to `New Column` (a present blank still 400s via the DTO).
/// Foreign/missing board id rolls back to 404 (no oracle, never 403).
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
#[tracing::instrument(skip(state, auth, input))]
pub async fn create_column(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(board_id): ValidPath<crate::common::ids::BoardId>,
    ValidJson(input): ValidJson<CreateColumnInput>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = auth.user_id.get();
    let board_id = board_id.get();
    let input = &input;
    let id = with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
        check_board_owned(&mut *tx, user_id, board_id).await?;
        let sort_key = match fetch_last_sort_key_for_update(&mut *tx, board_id).await? {
            Some(last) => key_between(Some(&last), None)?,
            None => key_between(None, None)?,
        };
        let name = input.name.as_deref().unwrap_or(DEFAULT_COLUMN_NAME);
        let id = insert_column(&mut *tx, board_id, name, &sort_key).await?;
        if needs_rebalance(&sort_key) {
            rebalance_sort_keys(&mut tx, RebalanceScope::Columns { board_id }).await?;
        }
        Ok((tx, id))
    })
    .await?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_column_owned(&state.pool, user_id, id).await?;
    let summaries = load_summaries(&state.pool, board_id).await?;
    let location = format!("/api/v1/columns/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(ColumnBody {
            data: to_response_one(row, &summaries),
        }),
    ))
}

/// `PATCH /api/v1/columns/{id}{name?,sort_order?,position?} → 200 {data}`
/// (present names validate like create; present `sort_order` rewrites card
/// sort keys by `created_at` via a two-phase park-then-assign rewrite;
/// present `position` moves the column between siblings with ONE single-row
/// between-neighbours `sort_key` write, clamped to `[0, N-1]`, never 400).
/// The column is pre-read for ownership (foreign/missing id → 404, never
/// 403); absent keys are a no-op 200. The response re-reads the stored row
/// (never echoes). The transaction runs under `READ COMMITTED` and retries
/// the whole PATCH inside the shared `with_tx_retry` kernel (≤ MAX_ATTEMPTS
/// attempts, jittered backoff) when the matcher sees `1213`/`1205` or a
/// `UQ_*_sort_key`-scoped `1062` — the
/// `sort_order` CASE rewrite and the `position` between-neighbours write can
/// both collide with a concurrent transaction committing inside the gap
/// (F2, 2026-11-15); unrelated unique violations never retry.
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
#[tracing::instrument(skip(state, auth, input))]
pub async fn rename_column_handler(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ColumnId>,
    ValidJson(input): ValidJson<UpdateColumnInput>,
) -> Result<Json<ColumnBody>, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    let input = &input;
    with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
        let owned = fetch_column_owned(&mut *tx, user_id, id).await?;
        if let Some(name) = input.name.as_deref() {
            rename_column(&mut *tx, id, name).await?;
        }
        if let Some(order) = input.sort_order.as_deref() {
            // Whitelisted direction (garde already rejected anything
            // else, so the fallthrough is unreachable-but-total, never
            // `format!`).
            let desc = match order {
                "asc" => false,
                "desc" => true,
                _ => return Err(AppError::Internal(Internal)),
            };
            let ordered = fetch_card_ids_for_sort(&mut *tx, id, desc).await?;
            let keys = keys_between(None, None, ordered.len())?;
            let assignments: Vec<(i32, String)> = ordered.into_iter().zip(keys).collect();
            rewrite_card_sort_keys(&mut tx, id, &assignments).await?;
        }
        if let Some(requested) = input.position {
            // Move: clamp to `[0, N-1]` and write ONE between-neighbours
            // sort key. Neighbour reads stay unlocked (D3).
            let board_id = owned.board_id.get();
            let others = fetch_column_keys_in_board(&mut *tx, board_id, Some(id)).await?;
            let upper = i32::try_from(others.len()).map_err(|_| AppError::Internal(Internal))?;
            let target = clamp_position(requested, upper) as usize;
            let keys: Vec<&str> = others.iter().map(|(_, key)| key.as_str()).collect();
            let sort_key = key_for_index(&keys, target)?;
            point_move_column_sort_key(&mut *tx, id, &sort_key).await?;
            if needs_rebalance(&sort_key) {
                rebalance_sort_keys(&mut tx, RebalanceScope::Columns { board_id }).await?;
            }
        }
        Ok((tx, ()))
    })
    .await?;
    // DB re-read: the response reflects the stored row, never echoes.
    let row = fetch_column_owned(&state.pool, user_id, id).await?;
    let summaries = load_summaries(&state.pool, row.board_id.get()).await?;
    Ok(Json(ColumnBody {
        data: to_response_one(row, &summaries),
    }))
}

/// `DELETE /api/v1/columns/{id} → 204` (hard delete, empty body). Cards
/// cascade via `ON DELETE CASCADE` FKs; sibling `sort_key`s are untouched.
/// Foreign/missing id → 404 (no oracle, never 403); garbage id → 400
/// `invalid-path`.
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
#[tracing::instrument(skip(state, auth))]
pub async fn delete_column(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ColumnId>,
) -> Result<StatusCode, AppError> {
    let user_id = auth.user_id.get();
    let id = id.get();
    with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
        fetch_column_owned(&mut *tx, user_id, id).await?;
        delete_column_row(&mut *tx, id).await?;
        Ok((tx, ()))
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// State-class 400 for move-cards (`error.rs` is outside T23c touch scope,
/// so no new `AppError` variant): a hand-built garde report renders the
/// shared 400 `validation-error` problem+json with `errors[]`.
fn move_state_error(field: &'static str, message: &'static str) -> AppError {
    let mut report = garde::Report::new();
    report.append(garde::Path::new(field), garde::Error::new(message));
    AppError::from(report)
}

/// `POST /api/v1/columns/{id}/move-cards{target_column_id} → 200
/// {data: {moved_count}}` (moves every card out of the source column,
/// appended after the target's last `sort_key` in ONE tx; the source select
/// is `sort_key ASC, id ASC` so the target mirrors source order). Same-id
/// and cross-board targets are state errors → 400 `errors[]`; an empty
/// source is a success (`{moved_count: 0}`), never an error.
/// Foreign/missing source or target id → 404 (no oracle, never 403);
/// garbage id → 400 `invalid-path`; bad body → 400 via `ValidJson`. The
/// transaction runs under `READ COMMITTED` and retries the whole move
/// inside the shared `with_tx_retry` kernel (≤ MAX_ATTEMPTS attempts,
/// jittered backoff) on a `1213`/`1205` or a
/// `UQ_cards_column_sort_key`-scoped `1062` — the target-side `CASE` key
/// assignment can collide with a row committed past the locked last key
/// (F2, 2026-11-15); unrelated unique violations never retry.
#[utoipa::path(
    post,
    path = "/api/v1/columns/{id}/move-cards",
    request_body = MoveCardsInput,
    responses(
        (status = 200, description = "Cards moved", body = MoveCardsBody),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Column not found")
    )
)]
#[tracing::instrument(skip(state, auth, input))]
pub async fn move_cards(
    State(state): State<AppState>,
    auth: AuthSession,
    ValidPath(id): ValidPath<crate::common::ids::ColumnId>,
    ValidJson(input): ValidJson<MoveCardsInput>,
) -> Result<Json<MoveCardsBody>, AppError> {
    use crate::error::Internal;
    let user_id = auth.user_id.get();
    let source_id = id.get();
    let target_id = input.target_column_id;
    if source_id == target_id {
        return Err(move_state_error(
            "target_column_id",
            "Cards already in this column",
        ));
    }
    let moved_count = with_tx_retry(&state.pool, TxRetryOpts::default(), |mut tx| async move {
        let source = fetch_column_owned(&mut *tx, user_id, source_id).await?;
        let target = fetch_column_owned(&mut *tx, user_id, target_id).await?;
        if source.board_id.get() != target.board_id.get() {
            return Err(move_state_error(
                "target_column_id",
                "Cannot move cards between different boards",
            ));
        }
        // Locking selects run in ascending column-id order (stable lock
        // order, never a reload+save loop downstream).
        let (source_ids, target_last) = if source_id < target_id {
            let ids = fetch_source_card_ids_for_update(&mut *tx, source_id).await?;
            let last = fetch_last_card_sort_key_for_update(&mut *tx, target_id).await?;
            (ids, last)
        } else {
            let last = fetch_last_card_sort_key_for_update(&mut *tx, target_id).await?;
            let ids = fetch_source_card_ids_for_update(&mut *tx, source_id).await?;
            (ids, last)
        };
        let moved_count =
            i64::try_from(source_ids.len()).map_err(|_| AppError::Internal(Internal))?;
        if !source_ids.is_empty() {
            let keys = keys_between(target_last.as_deref(), None, source_ids.len())?;
            let assignments: Vec<(i32, String)> = source_ids.into_iter().zip(keys).collect();
            move_cards_into(&mut *tx, source_id, target_id, &assignments).await?;
        }
        Ok((tx, moved_count))
    })
    .await?;
    Ok(Json(MoveCardsBody {
        data: MoveCardsData { moved_count },
    }))
}
