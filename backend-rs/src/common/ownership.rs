//! Single owner of user-scoping SQL helpers.
//!
//! Ownership checks are not a feature's own store: the same "does this id
//! belong to the caller?" question is asked from cards, columns, notes,
//! checklists and boards. Foreign and missing ids both yield `RowNotFound`
//! → 404, never a 403 oracle.

use sqlx::{MySqlExecutor, QueryBuilder};

use crate::error::{AppError, Internal};

/// Assert a project belongs to the caller. Foreign/missing id → `RowNotFound`
/// → 404 (no oracle, never 403). Runs inside the caller's tx so the link
/// check and the insert are atomic.
#[tracing::instrument(skip(ex))]
pub async fn check_project_owned<'e, E>(
    ex: E,
    user_id: i32,
    project_id: i32,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT `id` FROM `projects` WHERE `id` = ? AND `user_id` = ?",
        project_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert a column belongs to the caller (via `board → user`) and return the
/// column's `board_id`. Foreign/missing id → 404, never 403.
#[tracing::instrument(skip(ex))]
pub async fn check_column_owned<'e, E>(ex: E, user_id: i32, column_id: i32) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT c.`board_id` AS \"board_id: i32\" \
          FROM `columns` c JOIN `boards` b ON b.`id` = c.`board_id` \
          WHERE c.`id` = ? AND b.`user_id` = ?",
        column_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.board_id)
}

/// Assert a label belongs to the caller. Foreign/missing id → 404.
#[tracing::instrument(skip(ex))]
pub async fn check_label_owned<'e, E>(ex: E, user_id: i32, label_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT `id` FROM `labels` WHERE `id` = ? AND `user_id` = ?",
        label_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert a board belongs to the caller. Foreign/missing id → `RowNotFound`
/// → 404 (no oracle, never 403). Runs inside the caller's tx so the check and
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

/// Assert a card belongs to the caller via its grandparent board (cards carry
/// no `user_id`; same 404 rule as [`check_board_owned`]).
#[tracing::instrument(skip(ex))]
pub async fn check_card_owned<'e, E>(ex: E, user_id: i32, card_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT k.`id` FROM `cards` k \
          JOIN `columns` c ON c.`id` = k.`column_id` \
          JOIN `boards` b ON b.`id` = c.`board_id` \
          WHERE k.`id` = ? AND b.`user_id` = ?",
        card_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert every requested tag exists AND belongs to the caller: the count of
/// matching rows must equal the (already deduped) request count, so a missing
/// OR foreign tag → 404 (no oracle, never 403). `COUNT(CASE…)` is unnecessary
/// here — plain `COUNT(*)` over the `IN` set. Empty input short-circuits (no
/// `IN ()` query emitted).
#[tracing::instrument(skip(ex))]
pub async fn check_tags_owned<'e, E>(ex: E, user_id: i32, tag_ids: &[i32]) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if tag_ids.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM `tags` WHERE `user_id` = ");
    builder.push_bind(user_id);
    builder.push(" AND `id` IN (");
    {
        let mut separated = builder.separated(", ");
        for id in tag_ids {
            separated.push_bind(*id);
        }
    }
    builder.push(")");
    let matched: i64 = builder
        .build_query_scalar()
        .fetch_one(ex)
        .await
        .map_err(AppError::Db)?;
    let expected = i64::try_from(tag_ids.len()).map_err(|_| AppError::Internal(Internal))?;
    if matched != expected {
        return Err(AppError::NotFound);
    }
    Ok(())
}
