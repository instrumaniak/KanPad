//! columns store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership rides the parent board (`columns` carries no `user_id`): every
//! single-column lookup joins `boards` with `b.user_id = ?`, so a foreign or
//! missing id matches zero rows → [`AppError::NotFound`] (via `RowNotFound`),
//! never a 403 oracle. The nested list checks the board first, then reads its
//! columns unscoped (the check already proved ownership).
//!
//! Create takes `MAX(position)+1` under `SELECT ... FOR UPDATE` in the
//! caller's tx (an unlocked `MAX` races duplicate positions). Delete runs the
//! row delete + a set-based gap-close (`position - 1` range `UPDATE`) in ONE
//! tx — never a reload+save loop. Cards cascade via the DB FK
//! (`FK_cards_column_id ... ON DELETE CASCADE`, which fans out to
//! `card_labels`/`checklists`/items/notes the same way). `columns` is a
//! reserved word: always backticked.
//!
//! `DATETIME` selects carry explicit `AS "col: chrono::NaiveDateTime"`
//! overrides (the lockfile `time` 0.3.55 renames `PrimitiveDateTime` →
//! `PlainDateTime`, so `query!` maps bare `DATETIME` to the `time` crate
//! otherwise).

use chrono::NaiveDateTime;
use sqlx::MySqlExecutor;

use crate::{
    common::ids::{BoardId, ColumnId},
    error::{AppError, Internal},
};

use super::types::{format_timestamp, ColumnResponse};

/// One `columns` row (assembled by [`fetch_column_owned`]/[`fetch_columns`];
/// cards ride separately via `common::card_summary` so the column select stays
/// TEXT-free).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnRow {
    /// `columns.id`.
    pub id: ColumnId,
    /// `columns.board_id`.
    pub board_id: BoardId,
    /// Column name.
    pub name: String,
    /// Zero-based position.
    pub position: i32,
    /// `columns.created_at`.
    pub created_at: NaiveDateTime,
    /// `columns.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Assert a board belongs to the caller. Foreign/missing id → `RowNotFound` →
/// 404 (no oracle, never 403). Runs inside the caller's tx so the check and
/// the write that follows are atomic.
#[tracing::instrument(skip(ex))]
pub async fn check_board_owned<'e, E>(ex: E, user_id: i32, board_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT `id` FROM `boards` WHERE `id` = ? AND `user_id` = ?",
        board_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Fetch one owned column (join proves ownership: foreign/missing id →
/// `RowNotFound` → 404, never 403).
#[tracing::instrument(skip(ex))]
pub async fn fetch_column_owned<'e, E>(ex: E, user_id: i32, id: i32) -> Result<ColumnRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT c.`id`, c.`board_id` AS \"board_id: i32\", c.`name`, c.`position`, \
          c.`created_at` AS \"created_at: chrono::NaiveDateTime\", \
          c.`updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
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
        position: row.position,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Fetch a board's columns, `position ASC` (nested unpaginated in the list
/// body — cards attach separately). The caller proves board ownership first
/// via [`check_board_owned`].
#[tracing::instrument(skip(ex))]
pub async fn fetch_columns<'e, E>(ex: E, board_id: i32) -> Result<Vec<ColumnRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `board_id` AS \"board_id: i32\", `name`, `position`, \
          `created_at` AS \"created_at: chrono::NaiveDateTime\", \
          `updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
          FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC",
        board_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| ColumnRow {
            id: ColumnId::from(row.id),
            board_id: BoardId::from(row.board_id),
            name: row.name,
            position: row.position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Current `MAX(position)` on a board (`-1` when empty) locking the range for
/// the insert that follows. MUST run inside the create tx (`FOR UPDATE` outside
/// a tx locks nothing); `MAX(INT)` decodes as `i64` (`LONGLONG`), narrowed via
/// `try_from` (never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn max_position_for_update<'e, E>(ex: E, board_id: i32) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT COALESCE(MAX(`position`), -1) AS \"max_pos: i64\" \
          FROM `columns` WHERE `board_id` = ? FOR UPDATE",
        board_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(row.max_pos).map_err(|_| AppError::Internal(Internal))
}

/// Insert a column row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn insert_column<'e, E>(
    ex: E,
    board_id: i32,
    name: &str,
    position: i32,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `columns` (`name`, `position`, `board_id`) VALUES (?, ?, ?)",
        name,
        position,
        board_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
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
/// DB-side). The caller closes the position gap next (same tx) and pre-read
/// the row ([`fetch_column_owned`]) for ownership + the deleted position.
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

/// Close the position gap left by a delete: one set-based range `UPDATE`
/// shifts every later sibling down by one (never a reload+save loop).
#[tracing::instrument(skip(ex))]
pub async fn close_column_gap<'e, E>(ex: E, board_id: i32, position: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `columns` SET `position` = `position` - 1 \
          WHERE `board_id` = ? AND `position` > ?",
        board_id,
        position
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Source-column card ids in `position ASC` (the target mirrors source order
/// on move). Locks the rows (`FOR UPDATE`) — MUST run inside the move tx.
/// One query: the caller assigns positions and writes via
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
        "SELECT `id` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC FOR UPDATE",
        source_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

/// Current `MAX(position)` on the target column (`-1` when empty), locking
/// the range (`FOR UPDATE`) — MUST run inside the move tx. `MAX(INT)`
/// decodes as `i64` (`LONGLONG`), narrowed via `try_from` (never silent `as`).
#[tracing::instrument(skip(ex))]
pub async fn max_card_position_for_update<'e, E>(ex: E, target_id: i32) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT COALESCE(MAX(`position`), -1) AS \"max_pos: i64\" \
          FROM `cards` WHERE `column_id` = ? FOR UPDATE",
        target_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(row.max_pos).map_err(|_| AppError::Internal(Internal))
}

/// Move `assignments` (`(card_id, position)`, already `position ASC`) into
/// `target_id` with ONE set-based `UPDATE ... CASE` (never N saves). The
/// `WHERE` re-checks `column_id = source_id` so only source rows move.
/// Skipped by the caller when empty (an empty `IN ()` is invalid SQL).
/// Dynamic `CASE` arity rules out `query!` — ids/positions are `i32`s bound
/// server-side (never interpolated), so there is no injection surface.
#[tracing::instrument(skip(ex))]
pub async fn move_cards_into<'e, E>(
    ex: E,
    source_id: i32,
    target_id: i32,
    assignments: &[(i32, i32)],
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    debug_assert!(!assignments.is_empty(), "empty move must skip the write");
    let mut builder: sqlx::QueryBuilder<sqlx::MySql> =
        sqlx::QueryBuilder::new("UPDATE `cards` SET `column_id` = ");
    builder.push_bind(target_id);
    builder.push(", `position` = CASE `id`");
    for (id, position) in assignments {
        builder.push(" WHEN ");
        builder.push_bind(*id);
        builder.push(" THEN ");
        builder.push_bind(*position);
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
pub fn to_response(
    rows: Vec<ColumnRow>,
    summaries: &std::collections::HashMap<i32, Vec<crate::common::card_summary::CardSummary>>,
) -> Vec<ColumnResponse> {
    rows.into_iter()
        .map(|row| {
            let cards = summaries.get(&row.id.get()).cloned().unwrap_or_default();
            ColumnResponse {
                id: row.id,
                name: row.name,
                position: row.position,
                board_id: row.board_id,
                cards,
                created_at: format_timestamp(&row.created_at),
                updated_at: format_timestamp(&row.updated_at),
            }
        })
        .collect()
}
