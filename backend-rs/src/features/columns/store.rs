//! columns store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership rides the parent board (`columns` carries no `user_id`): every
//! single-column lookup joins `boards` with `b.user_id = ?`, so a foreign or
//! missing id matches zero rows → [`AppError::NotFound`] (via `RowNotFound`),
//! never a 403 oracle. The nested list checks the board first, then reads its
//! columns unscoped (the check already proved ownership).
//!
//! Ordering rides fractional `sort_key`s (T010): every column carries an
//! `ascii_bin` base62 key and every **move is a single-row write** — no
//! sibling is ever renumbered. Wire `position` (integer, 0-based) is
//! COMPUTED at read: `enumerate` for lists/pages, a correlated
//! `COUNT(*) … sort_key < ?` for single-row reads. Neighbour reads for
//! moves are unlocked; the append path (`fetch_last_sort_key_for_update` +
//! `key_between` + INSERT in one tx) keeps the serialized contract today's
//! `MAX(position) FOR UPDATE` provided. `columns` is a reserved word:
//! always backticked.
//!
//! `DATETIME` selects need NO type override: with the in-tree session store
//! (T65) the `sqlx` `time` feature is no longer enabled, so `query!` maps a
//! bare `DATETIME` to `chrono::NaiveDateTime` natively.

use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlExecutor, QueryBuilder};

use crate::{
    common::ids::{BoardId, ColumnId},
    error::{AppError, Internal},
};

use super::types::{ColumnResponse, format_timestamp};

/// One `columns` row (assembled by [`fetch_column_owned`]/[`fetch_columns`];
/// cards ride separately via `common::card_summary` so the column select stays
/// TEXT-free). `position` is COMPUTED at read (T010) — never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnRow {
    /// `columns.id`.
    pub id: ColumnId,
    /// `columns.board_id`.
    pub board_id: BoardId,
    /// Column name.
    pub name: String,
    /// Wire position (0-based, computed).
    pub position: i32,
    /// `columns.created_at`.
    pub created_at: NaiveDateTime,
    /// `columns.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Fetch one owned column (join proves ownership: foreign/missing id →
/// `RowNotFound` → 404, never 403). The wire position is a correlated
/// `COUNT(*) … sort_key < ?` over the unique `(board_id, sort_key)` index.
#[tracing::instrument(skip(ex))]
pub async fn fetch_column_owned<'e, E>(ex: E, user_id: i32, id: i32) -> Result<ColumnRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT c.`id` AS \"id!\", c.`board_id` AS \"board_id!: i32\", c.`name` AS \"name!\", \
          (SELECT COUNT(*) FROM `columns` p \
            WHERE p.`board_id` = c.`board_id` AND p.`sort_key` < c.`sort_key`) AS \"position!: i64\", \
          c.`created_at` AS \"created_at!\", \
          c.`updated_at` AS \"updated_at!\" \
          FROM `columns` c JOIN `boards` b ON b.`id` = c.`board_id` \
          WHERE c.`id` = ? AND b.`user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(ColumnRow {
        id: ColumnId::from(row.id),
        board_id: BoardId::from(row.board_id),
        name: row.name,
        position: i32::try_from(row.position).map_err(|_| AppError::Internal(Internal))?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Fetch a board's columns, `sort_key ASC, id ASC` (nested unpaginated in the
/// list body — cards attach separately). Positions are the enumerate index.
/// The caller proves board ownership first via [`crate::common::ownership::check_board_owned`].
#[tracing::instrument(skip(ex))]
pub async fn fetch_columns<'e, E>(ex: E, board_id: i32) -> Result<Vec<ColumnRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `board_id` AS \"board_id: i32\", `name`, \
          `created_at`, \
          `updated_at` \
          FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
        board_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        out.push(ColumnRow {
            id: ColumnId::from(row.id),
            board_id: BoardId::from(row.board_id),
            name: row.name,
            position: i32::try_from(index).map_err(|_| AppError::Internal(Internal))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(out)
}

/// A board's sibling `sort_key`s, ascending, excluding `exclude_id` (the
/// moving column) when set — the between-neighbours computation for moves.
/// Neighbor reads are deliberately UNLOCKED (D3: last-write-wins for
/// concurrent same-gap drops; uniqueness rides the unique index).
#[tracing::instrument(skip(ex))]
pub async fn fetch_column_keys_in_board<'e, E>(
    ex: E,
    board_id: i32,
    exclude_id: Option<i32>,
) -> Result<Vec<(i32, String)>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let pairs: Vec<(i32, String)> = match exclude_id {
        Some(id) => sqlx::query!(
            "SELECT `id`, `sort_key` FROM `columns` \
             WHERE `board_id` = ? AND `id` != ? ORDER BY `sort_key` ASC, `id` ASC",
            board_id,
            id
        )
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .map(|row| (row.id, row.sort_key))
        .collect(),
        None => sqlx::query!(
            "SELECT `id`, `sort_key` FROM `columns` \
             WHERE `board_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
            board_id
        )
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .map(|row| (row.id, row.sort_key))
        .collect(),
    };
    Ok(pairs)
}

/// The highest `sort_key` currently on a board (`None` when the board has no
/// columns), locking the last index record for the insert that follows. MUST
/// run inside the create tx (`FOR UPDATE` outside a tx locks nothing). The
/// last-key read + `key_between` + INSERT in one tx is the serialized append
/// contract (same as the pre-T010 `MAX(position) FOR UPDATE`).
#[tracing::instrument(skip(ex))]
pub async fn fetch_last_sort_key_for_update<'e, E>(
    ex: E,
    board_id: i32,
) -> Result<Option<String>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `sort_key` FROM `columns` WHERE `board_id` = ? \
         ORDER BY `sort_key` DESC LIMIT 1 FOR UPDATE",
        board_id
    )
    .fetch_optional(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|row| row.sort_key))
}

/// Insert a column row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn insert_column<'e, E>(
    ex: E,
    board_id: i32,
    name: &str,
    sort_key: &str,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `columns` (`name`, `sort_key`, `board_id`) VALUES (?, ?, ?)",
        name,
        sort_key,
        board_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Point-write a column's `sort_key` (the single-row move).
#[tracing::instrument(skip(ex))]
pub async fn point_move_column_sort_key<'e, E>(
    ex: E,
    id: i32,
    sort_key: &str,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `columns` SET `sort_key` = ? WHERE `id` = ?",
        sort_key,
        id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Rename an owned column (the caller pre-reads via [`fetch_column_owned`] for
/// the 404 instead of trusting affected-rows).
#[tracing::instrument(skip(ex))]
pub async fn rename_column<'e, E>(ex: E, id: i32, name: &str) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("UPDATE `columns` SET `name` = ? WHERE `id` = ?", name, id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Delete a column row. Cards cascade via `FK_cards_column_id`
/// (`ON DELETE CASCADE`, fanning out to label links, checklists, and notes
/// DB-side). No gap is closed — sibling `sort_key`s are untouched (T010).
/// The caller pre-read the row ([`fetch_column_owned`]) for ownership.
#[tracing::instrument(skip(ex))]
pub async fn delete_column_row<'e, E>(ex: E, id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("DELETE FROM `columns` WHERE `id` = ?", id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Card ids of one column in `created_at` order (`desc` = newest first; `id
/// ASC` tiebreaks equal timestamps so the rewrite is deterministic). One
/// query per fn (T20 style): the direction rides a whitelisted `match`,
/// never `format!` of input. Runs inside the caller's sort tx (`FOR UPDATE`
/// locks exactly the rows the `CASE` rewrite below will move).
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_ids_for_sort<'e, E>(
    ex: E,
    column_id: i32,
    desc: bool,
) -> Result<Vec<i32>, AppError>
where
    E: MySqlExecutor<'e>,
{
    // Each `query!` site expands to its own anonymous `Record` type, so the
    // branches map to `Vec<i32>` independently (they never unify).
    let ordered: Vec<i32> = if desc {
        sqlx::query!(
            "SELECT `id` FROM `cards` WHERE `column_id` = ? \
              ORDER BY `created_at` DESC, `id` ASC FOR UPDATE",
            column_id
        )
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .map(|row| row.id)
        .collect()
    } else {
        sqlx::query!(
            "SELECT `id` FROM `cards` WHERE `column_id` = ? \
              ORDER BY `created_at` ASC, `id` ASC FOR UPDATE",
            column_id
        )
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .map(|row| row.id)
        .collect()
    };
    Ok(ordered)
}

/// Rewrite card sort keys to the given `(id, sort_key)` assignments in
/// **two phases** (park, then assign): phase 1 parks every assigned row at a
/// unique temporary key outside the base62 key space (`tmp-<id>`), phase 2
/// assigns the final keys with one set-based `UPDATE ... CASE`. A
/// single-phase even rewrite transiently violates the unique
/// `(column_id, sort_key)` index when one row's new key equals an
/// un-rewritten row's old key (the same reason the rebalance is two-phase).
/// Empty input skips the statement (a `CASE` with no `WHEN` is invalid SQL).
/// Dynamic arity defeats `query!` literals, so this rides `QueryBuilder`;
/// ids/keys ride `push_bind` only (never `push` of values). `push_bind`
/// emits a bare `?` with no surrounding spaces, so every `push` fragment that
/// follows a bind carries a leading space (see `30-db-schema.md`).
#[tracing::instrument(skip(ex))]
pub async fn rewrite_card_sort_keys(
    ex: &mut sqlx::MySqlConnection,
    column_id: i32,
    assignments: &[(i32, String)],
) -> Result<(), AppError> {
    if assignments.is_empty() {
        return Ok(());
    }
    // Phase 1 — park the assigned rows at unique temporary keys.
    {
        let mut park: QueryBuilder<MySql> = QueryBuilder::new(
            "UPDATE `cards` SET `sort_key` = CONCAT('tmp-', `id`) WHERE `column_id` = ",
        );
        park.push_bind(column_id);
        park.push(" AND `id` IN (");
        {
            let mut ids = park.separated(", ");
            for (id, _) in assignments {
                ids.push_bind(*id);
            }
        }
        park.push(")");
        park.build().execute(&mut *ex).await.map_err(AppError::Db)?;
    }
    // Phase 2 — assign the final keys with one set-based CASE.
    let mut builder: QueryBuilder<MySql> =
        QueryBuilder::new("UPDATE `cards` SET `sort_key` = CASE `id` ");
    for (id, sort_key) in assignments {
        builder.push(" WHEN ");
        builder.push_bind(*id);
        builder.push(" THEN ");
        builder.push_bind(sort_key.as_str());
    }
    builder.push(" ELSE `sort_key` END WHERE `column_id` = ");
    builder.push_bind(column_id);
    builder
        .build()
        .execute(&mut *ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Source-column card ids in `sort_key ASC` (the target mirrors source order
/// on move). Locks the rows (`FOR UPDATE`) — MUST run inside the move tx.
/// One query: the caller assigns keys and writes via
/// [`move_cards_into`] next.
#[tracing::instrument(skip(ex))]
pub async fn fetch_source_card_ids_for_update<'e, E>(
    ex: E,
    source_id: i32,
) -> Result<Vec<i32>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC FOR UPDATE",
        source_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

/// The highest `sort_key` currently on the target column (`None` when the
/// target is empty), locking the last index record (`FOR UPDATE`) — MUST run
/// inside the move tx. The last-key read + `keys_between(Some(last), None, n)`
/// + one `UPDATE ... CASE` is the serialized append contract (the source
/// select in `sort_key` order carries `id` tiebreaks).
#[tracing::instrument(skip(ex))]
pub async fn fetch_last_card_sort_key_for_update<'e, E>(
    ex: E,
    target_id: i32,
) -> Result<Option<String>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `sort_key` FROM `cards` WHERE `column_id` = ? \
         ORDER BY `sort_key` DESC LIMIT 1 FOR UPDATE",
        target_id
    )
    .fetch_optional(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|row| row.sort_key))
}

/// Move `assignments` (`(card_id, sort_key)`, already `sort_key ASC`) into
/// `target_id` with ONE set-based `UPDATE ... CASE` (never N saves). The
/// `WHERE` re-checks `column_id = source_id` so only source rows move.
/// Skipped by the caller when empty (an empty `IN ()` is invalid SQL).
/// Dynamic `CASE` arity rules out `query!` — ids/keys are bound server-side
/// (never interpolated), so there is no injection surface.
#[tracing::instrument(skip(ex))]
pub async fn move_cards_into<'e, E>(
    ex: E,
    source_id: i32,
    target_id: i32,
    assignments: &[(i32, String)],
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    debug_assert!(!assignments.is_empty(), "empty move must skip the write");
    let mut builder: sqlx::QueryBuilder<sqlx::MySql> =
        sqlx::QueryBuilder::new("UPDATE `cards` SET `column_id` = ");
    builder.push_bind(target_id);
    builder.push(", `sort_key` = CASE `id`");
    for (id, sort_key) in assignments {
        builder.push(" WHEN ");
        builder.push_bind(*id);
        builder.push(" THEN ");
        builder.push_bind(sort_key.as_str());
    }
    builder.push(" END WHERE `column_id` = ");
    builder.push_bind(source_id);
    builder.push(" AND `id` IN (");
    {
        let mut ids = builder.separated(", ");
        for (id, _) in assignments {
            ids.push_bind(*id);
        }
    }
    builder.push(")");
    builder.build().execute(ex).await.map_err(AppError::Db)?;
    Ok(())
}

/// Render [`ColumnRow`]s + their card summaries for the wire (single owner —
/// see [`super::types::format_timestamp`]). `summaries` maps
/// `column_id → cards[]` (missing key = no cards, never an error).
/// `totals` maps `column_id → total_cards` (authoritative count; missing key = 0).
pub fn to_response(
    rows: Vec<ColumnRow>,
    summaries: &std::collections::HashMap<i32, Vec<crate::common::card_summary::CardSummary>>,
    totals: &std::collections::HashMap<i32, u64>,
) -> Vec<ColumnResponse> {
    rows.into_iter()
        .map(|row| to_response_one(row, summaries, totals))
        .collect()
}

/// Render ONE column for the wire.
///
/// The caller knows there is exactly one row (a create/update returns a
/// single column), so the cardinality is a **compile-time** fact and must not
/// be a runtime assertion: the list case used to build a one-element `Vec`,
/// pop it, and `expect(...)` that it was there — a runtime check of something
/// the compiler already knew.
pub fn to_response_one(
    row: ColumnRow,
    summaries: &std::collections::HashMap<i32, Vec<crate::common::card_summary::CardSummary>>,
    totals: &std::collections::HashMap<i32, u64>,
) -> ColumnResponse {
    let col_id = row.id.get();
    let cards = summaries.get(&col_id).cloned().unwrap_or_default();
    let total_cards = totals.get(&col_id).copied().unwrap_or(cards.len() as u64);
    ColumnResponse {
        id: row.id,
        name: row.name,
        position: row.position,
        board_id: row.board_id,
        cards,
        total_cards,
        created_at: format_timestamp(&row.created_at),
        updated_at: format_timestamp(&row.updated_at),
    }
}
