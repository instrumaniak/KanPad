//! boards store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership is baked into every board `WHERE` (`user_id = ?`): a foreign or
//! missing id matches zero rows → [`AppError::NotFound`] (via `RowNotFound`),
//! never a 403 oracle. The optional `project_id` link is ownership-checked
//! the same way (`projects` row must belong to the caller, else 404) before
//! the insert runs — the DB FK (`FK_boards_project_id ... ON DELETE SET
//! NULL`) guards integrity, not isolation, so the check cannot be skipped.
//!
//! Create runs board + 3 default columns in ONE transaction (the Nest
//! two-step save is non-atomic). `DATETIME` selects carry explicit
//! `AS "col: chrono::NaiveDateTime"` overrides: the lockfile `time` 0.3.55
//! renames `PrimitiveDateTime` → `PlainDateTime`, so `query!` maps bare
//! `DATETIME` to the `time` crate otherwise. Nullable selects override with
//! the INNER type (`AS "project_id: i32"`, never `Option<i32>`): sqlx wraps
//! nullable columns in `Option` itself, so an `Option` override double-wraps
//! to `Option<Option<_>>`. `is_archived` (`tinyint`) decodes as `i8`, never
//! `bool` — compare `!= 0` in Rust.

use chrono::NaiveDateTime;
use sqlx::MySqlExecutor;

use crate::{
    common::ids::{BoardId, ColumnId, ProjectId},
    error::{AppError, Internal},
};

use super::types::{format_timestamp, BoardDetailResponse, ColumnSummary, ProjectRef};

/// Default columns created with every board (`name`, `position` pairs —
/// `To Do/In Progress/Done` 0/1/2, matching the Nest create flow).
pub const DEFAULT_COLUMNS: [(&str, i32); 3] = [("To Do", 0), ("In Progress", 1), ("Done", 2)];

/// One board row plus its optional owning-project ref (assembled by
/// [`fetch_board`]; columns ride separately via [`fetch_board_columns`] so
/// both stay `position ASC`-ordered without a join fanout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRow {
    /// `boards.id`.
    pub id: BoardId,
    /// `boards.name`.
    pub name: String,
    /// `boards.background_color`.
    pub background_color: String,
    /// `boards.project_id` (standalone boards carry `NULL`).
    pub project_id: Option<ProjectId>,
    /// Owning project `{id,name}` (`None` = standalone).
    pub project: Option<ProjectRef>,
    /// `boards.is_archived`.
    pub is_archived: bool,
    /// `boards.view_mode`.
    pub view_mode: String,
    /// `boards.created_at`.
    pub created_at: NaiveDateTime,
    /// `boards.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Assert a project belongs to the caller. Foreign/missing id →
/// `RowNotFound` → 404 (no oracle, never 403). Runs inside the create tx so
/// the link check and the insert are atomic.
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

/// Insert a board row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast per `30-db-schema.md`).
/// A `1452` FK failure (project deleted between the ownership check and the
/// insert) maps to 404, matching the check's outcome — never a 500 oracle.
#[tracing::instrument(skip(ex))]
pub async fn insert_board<'e, E>(
    ex: E,
    user_id: i32,
    name: &str,
    background_color: &str,
    project_id: Option<i32>,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `boards` (`name`, `background_color`, `user_id`, `project_id`) \
         VALUES (?, ?, ?, ?)",
        name,
        background_color,
        user_id,
        project_id
    )
    .execute(ex)
    .await
    .map_err(|error| match &error {
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("1452") =>
        {
            AppError::NotFound
        }
        _ => AppError::Db(error),
    })?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Insert the 3 default columns for a fresh board (single multi-row
/// `INSERT` — static names only, the id rides `push_bind`-style `?`
/// placeholders, never interpolated).
#[tracing::instrument(skip(ex))]
pub async fn insert_default_columns<'e, E>(ex: E, board_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "INSERT INTO `columns` (`name`, `position`, `board_id`) \
         VALUES ('To Do', 0, ?), ('In Progress', 1, ?), ('Done', 2, ?)",
        board_id,
        board_id,
        board_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Fetch one board with its owning-project ref. The project join is
/// user-scoped (`p.user_id = b.user_id`, same pattern as the projects
/// `board_count` join) so a cross-user project link can never leak a name.
/// Foreign/missing board id → `RowNotFound` → 404 (no oracle).
#[tracing::instrument(skip(ex))]
pub async fn fetch_board<'e, E>(ex: E, user_id: i32, id: i32) -> Result<BoardRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT b.`id`, b.`name`, b.`background_color`, \
          b.`project_id` AS \"project_id: i32\", \
          b.`is_archived`, b.`view_mode`, \
          b.`created_at` AS \"created_at: chrono::NaiveDateTime\", \
          b.`updated_at` AS \"updated_at: chrono::NaiveDateTime\", \
          p.`name` AS \"project_name: String\" \
          FROM `boards` b \
          LEFT JOIN `projects` p ON p.`id` = b.`project_id` AND p.`user_id` = b.`user_id` \
          WHERE b.`id` = ? AND b.`user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    let project = match (row.project_id, row.project_name) {
        (Some(project_id), Some(name)) => Some(ProjectRef {
            id: ProjectId::from(project_id),
            name,
        }),
        _ => None,
    };
    Ok(BoardRow {
        id: BoardId::from(row.id),
        name: row.name,
        background_color: row.background_color,
        project_id: row.project_id.map(ProjectId::from),
        project,
        is_archived: row.is_archived != 0,
        view_mode: row.view_mode,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Fetch a board's columns, `position ASC` (nested unpaginated in the detail
/// body — no cards; the column-cards list is a separate T24a endpoint).
#[tracing::instrument(skip(ex))]
pub async fn fetch_board_columns<'e, E>(
    ex: E,
    board_id: i32,
) -> Result<Vec<ColumnSummary>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `name`, `position` FROM `columns` \
         WHERE `board_id` = ? ORDER BY `position` ASC",
        board_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| ColumnSummary {
            id: ColumnId::from(row.id),
            name: row.name,
            position: row.position,
        })
        .collect())
}

/// Render a [`BoardRow`] + its columns for the wire (single owner — see
/// [`super::types::format_timestamp`]).
pub fn to_response(row: BoardRow, columns: Vec<ColumnSummary>) -> BoardDetailResponse {
    BoardDetailResponse {
        id: row.id,
        name: row.name,
        background_color: row.background_color,
        project_id: row.project_id,
        project: row.project,
        created_at: format_timestamp(&row.created_at),
        updated_at: format_timestamp(&row.updated_at),
        columns,
        is_archived: row.is_archived,
        view_mode: row.view_mode,
    }
}
