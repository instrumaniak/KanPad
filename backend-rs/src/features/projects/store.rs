//! projects store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership is baked into every `WHERE` (`user_id = ?`): a foreign or
//! missing id matches zero rows → [`AppError::NotFound`] (via
//! `RowNotFound` or an explicit affected-rows check), never a 403 oracle.
//! `board_count` is aggregated in the page `SELECT` itself (single
//! `LEFT JOIN ... GROUP BY`); the count query reuses the same `WHERE`
//! without the join — the list path issues exactly 2 statements for any P
//! (proven by the `Questions`-counter test in `tests/api/projects.rs`).
//!
//! Delete cascades boards EXPLICITLY in-tx: the baseline FK
//! (`FK_boards_project_id ... ON DELETE SET NULL`) would otherwise orphan
//! boards with `NULL` projects, but the contract (`20-api-contract.md`
//! projects + T20 AC) mandates cascade. Columns/cards/notes under those
//! boards follow via DB-side cascades.

use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlExecutor, QueryBuilder};

use crate::{
    common::ids::ProjectId,
    error::{AppError, Internal},
};

use super::types::format_timestamp;

/// One list/detail row: the project plus its real board count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    /// `projects.id`.
    pub id: ProjectId,
    /// `projects.name`.
    pub name: String,
    /// `COUNT(boards.id)` for this project (user-scoped join).
    pub board_count: i64,
    /// `projects.created_at`.
    pub created_at: NaiveDateTime,
    /// `projects.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Escape a `LIKE` pattern in Rust (`%`/`_`/`\` get a backslash prefix) so
/// user input is matched literally. The pattern is sent via `push_bind`
/// only — never `push`ed into the SQL text.
fn escape_like(raw: &str) -> String {
    let mut pattern = String::with_capacity(raw.len() + 2);
    pattern.push('%');
    for ch in raw.chars() {
        if ch == '\\' || ch == '%' || ch == '_' {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern.push('%');
    pattern
}

/// Shared list `WHERE` builder (one owner for the count + page selects, so
/// the two can never drift apart per `30-db-schema.md`).
fn push_project_filter(builder: &mut QueryBuilder<'_, MySql>, user_id: i32, q: Option<&str>) {
    builder.push(" WHERE p.`user_id` = ");
    builder.push_bind(user_id);
    if let Some(term) = q {
        builder.push(" AND p.`name` LIKE ");
        builder.push_bind(escape_like(term));
    }
}

/// `SELECT COUNT(*)` for the list `Page.total` (same `WHERE` as the page
/// select, minus the boards join — no join fanout, plain `COUNT(*)`).
#[tracing::instrument(skip(ex))]
pub async fn count_projects<'e, E>(ex: E, user_id: i32, q: Option<&str>) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM `projects` p");
    push_project_filter(&mut builder, user_id, q);
    let total: i64 = builder
        .build_query_scalar()
        .fetch_one(ex)
        .await
        .map_err(AppError::Db)?;
    u64::try_from(total).map_err(|_| AppError::Internal(Internal))
}

/// Page `SELECT` with real `board_count` (single `LEFT JOIN ... GROUP BY` —
/// never N+1 per-project queries). `sort`/`order` arrive pre-validated from
/// [`crate::features::projects::types::ProjectListQuery`]; the `match` below
/// maps them to static fragments (defense in depth — user text is never
/// `push`ed), so even a bypassed validator cannot inject SQL.
#[tracing::instrument(skip(ex))]
#[allow(clippy::too_many_arguments)]
pub async fn fetch_project_page<'e, E>(
    ex: E,
    user_id: i32,
    q: Option<&str>,
    sort: &str,
    order: &str,
    page: u32,
    limit: u32,
) -> Result<Vec<ProjectRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let ordering = match (sort, order) {
        ("name", "asc") => "p.`name` ASC",
        ("name", "desc") => "p.`name` DESC",
        ("updated_at", "asc") => "p.`updated_at` ASC",
        ("updated_at", "desc") => "p.`updated_at` DESC",
        ("created_at", "asc") => "p.`created_at` ASC",
        _ => "p.`created_at` DESC",
    };
    let mut builder = QueryBuilder::new(
        "SELECT p.`id`, p.`name`, CAST(COUNT(b.`id`) AS SIGNED) AS `board_count`, \
         p.`created_at`, p.`updated_at` FROM `projects` p \
         LEFT JOIN `boards` b ON b.`project_id` = p.`id` AND b.`user_id` = p.`user_id`",
    );
    push_project_filter(&mut builder, user_id, q);
    builder.push(" GROUP BY p.`id`, p.`name`, p.`created_at`, p.`updated_at` ORDER BY ");
    builder.push(ordering);
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(page.saturating_sub(1).saturating_mul(limit));
    let rows: Vec<(i32, String, i64, NaiveDateTime, NaiveDateTime)> = builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, name, board_count, created_at, updated_at)| ProjectRow {
                id: ProjectId::from(id),
                name,
                board_count,
                created_at,
                updated_at,
            },
        )
        .collect())
}

/// Format a [`ProjectRow`] timestamp pair for the wire (single owner — see
/// [`super::types::format_timestamp`]).
pub fn wire_timestamps(row: &ProjectRow) -> (String, String) {
    (
        format_timestamp(&row.created_at),
        format_timestamp(&row.updated_at),
    )
}

/// Insert a project row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast per `30-db-schema.md`).
#[tracing::instrument(skip(ex))]
pub async fn insert_project<'e, E>(ex: E, user_id: i32, name: &str) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `projects` (`name`, `user_id`) VALUES (?, ?)",
        name,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Fetch one project with its real `board_count` (single-row `LEFT JOIN ...
/// GROUP BY`). Foreign/missing id → `RowNotFound` → 404 (no oracle).
#[tracing::instrument(skip(ex))]
pub async fn fetch_project<'e, E>(ex: E, user_id: i32, id: i32) -> Result<ProjectRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT p.`id`, p.`name`, CAST(COUNT(b.`id`) AS SIGNED) AS `board_count`, \
         p.`created_at`, \
         p.`updated_at` FROM `projects` p \
         LEFT JOIN `boards` b ON b.`project_id` = p.`id` AND b.`user_id` = p.`user_id` \
         WHERE p.`id` = ? AND p.`user_id` = ? \
         GROUP BY p.`id`, p.`name`, p.`created_at`, p.`updated_at`",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(ProjectRow {
        id: ProjectId::from(row.id),
        name: row.name,
        board_count: row.board_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Rename a project (no existence check here — the route pre-reads via
/// [`fetch_project`] so a same-name `PATCH` stays 200 instead of
/// false-404ing on zero affected rows).
#[tracing::instrument(skip(ex))]
pub async fn update_project_name<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
    name: &str,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `projects` SET `name` = ? WHERE `id` = ? AND `user_id` = ?",
        name,
        id,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Delete this project's boards (explicit cascade — the DB FK would
/// `SET NULL`; the route runs this + [`delete_project_row`] in one tx).
#[tracing::instrument(skip(ex))]
pub async fn delete_project_boards<'e, E>(
    ex: E,
    user_id: i32,
    project_id: i32,
) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "DELETE FROM `boards` WHERE `project_id` = ? AND `user_id` = ?",
        project_id,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(result.rows_affected())
}

/// Delete the project row itself. Zero affected rows (foreign/missing id)
/// → `false`; the route rolls back and returns 404.
#[tracing::instrument(skip(ex))]
pub async fn delete_project_row<'e, E>(ex: E, user_id: i32, id: i32) -> Result<bool, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "DELETE FROM `projects` WHERE `id` = ? AND `user_id` = ?",
        id,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_escapes_wildcards_and_backslash() {
        assert_eq!(escape_like("alp"), "%alp%");
        assert_eq!(escape_like("100%"), "%100\\%%");
        assert_eq!(escape_like("a_b"), "%a\\_b%");
        assert_eq!(escape_like("a\\b"), "%a\\\\b%");
        assert_eq!(escape_like("O'Brien"), "%O'Brien%");
        assert_eq!(escape_like(""), "%%");
    }
}
