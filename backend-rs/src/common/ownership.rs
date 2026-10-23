//! Single owner of user-scoping SQL helpers.
//!
//! Ownership checks are not a feature's own store: the same "does this id
//! belong to the caller?" question is asked from cards, columns, notes,
//! checklists and boards. Foreign and missing ids both yield `RowNotFound`
//! → 404, never a 403 oracle.

use sqlx::MySqlExecutor;

use crate::error::AppError;

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
