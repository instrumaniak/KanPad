//! tags store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership is baked into every `WHERE` (`user_id = ?`): a foreign or
//! missing id matches zero rows → [`AppError::NotFound`] (via `RowNotFound`
//! or an explicit affected-rows check), never a 403 oracle. Per-user
//! uniqueness (`UQ_tags_user_name`, case-insensitive under
//! `utf8mb4_unicode_ci`) surfaces as 409 through the shared `Db` arm — no
//! pre-check that could race. Tags carry
//! `created_at` ONLY (no `updated_at` by design per `30-db-schema.md`), so
//! patches never touch a timestamp column. An omitted create `color` defaults
//! to teal server-side (`dto.color ?? 'teal'` Nest parity).

use chrono::NaiveDateTime;
use sqlx::{MySqlExecutor, QueryBuilder};

use crate::{
    common::ids::TagId,
    error::{AppError, Internal},
};

use super::types::format_timestamp;

/// Default `tags.color` when the create body omits it (Nest
/// `dto.color ?? 'teal'` parity; also the column default).
pub const DEFAULT_TAG_COLOR: &str = "teal";

/// One list/detail row: the tag plus its creation timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRow {
    /// `tags.id`.
    pub id: TagId,
    /// `tags.name` (stored trimmed).
    pub name: String,
    /// `tags.color` (free-form, `<= 20` chars).
    pub color: String,
    /// `tags.created_at`.
    pub created_at: NaiveDateTime,
}

/// Format a [`TagRow`] timestamp for the wire (single owner — see
/// [`super::types::format_timestamp`]).
pub fn wire_created_at(row: &TagRow) -> String {
    format_timestamp(&row.created_at)
}

/// Total tags for a user (the list `Page.total` — plain `COUNT(*)`, no
/// join fanout).
#[tracing::instrument(skip(ex))]
pub async fn count_tags<'e, E>(ex: E, user_id: i32) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let total = sqlx::query!(
        "SELECT COUNT(*) AS \"total: i64\" FROM `tags` WHERE `user_id` = ?",
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    u64::try_from(total.total).map_err(|_| AppError::Internal(Internal))
}

/// Page `SELECT` for a user (`id ASC`). `page`/`limit` arrive pre-validated
/// from the strict [`crate::extract::PaginationParams`] extractor.
#[tracing::instrument(skip(ex))]
pub async fn fetch_tag_page<'e, E>(
    ex: E,
    user_id: i32,
    page: u32,
    limit: u32,
) -> Result<Vec<TagRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `name`, `color`, \
          `created_at` AS \"created_at: chrono::NaiveDateTime\" \
          FROM `tags` WHERE `user_id` = ? \
          ORDER BY `id` ASC LIMIT ? OFFSET ?",
        user_id,
        limit,
        page.saturating_sub(1).saturating_mul(limit)
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| TagRow {
            id: TagId::from(row.id),
            name: row.name,
            color: row.color,
            created_at: row.created_at,
        })
        .collect())
}

/// Fetch one owned tag. Foreign/missing id → `RowNotFound` → 404.
#[tracing::instrument(skip(ex))]
pub async fn fetch_tag<'e, E>(ex: E, user_id: i32, id: i32) -> Result<TagRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `id`, `name`, `color`, \
          `created_at` AS \"created_at: chrono::NaiveDateTime\" \
          FROM `tags` WHERE `id` = ? AND `user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(TagRow {
        id: TagId::from(row.id),
        name: row.name,
        color: row.color,
        created_at: row.created_at,
    })
}

/// Insert a tag row (name arrives pre-trimmed from the route; `None` color
/// stores [`DEFAULT_TAG_COLOR`]); returns the new id (`last_insert_id` as
/// `u64` → `i32::try_from`, never a silent `as` cast). Dup per-user name
/// (incl. case-variant under `utf8mb4_unicode_ci`) → 409 via the shared
/// `Db` arm.
#[tracing::instrument(skip(ex))]
pub async fn insert_tag<'e, E>(
    ex: E,
    user_id: i32,
    name: &str,
    color: Option<&str>,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let color = color.unwrap_or(DEFAULT_TAG_COLOR);
    let result = sqlx::query!(
        "INSERT INTO `tags` (`name`, `color`, `user_id`) VALUES (?, ?, ?)",
        name,
        color,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Partial tag update: every field optional (`None` = untouched). An
/// all-`None` patch is a no-op (returns without emitting `UPDATE`, so empty
/// `{}` bodies stay 200 without touching the row). There is no `updated_at`
/// column to bump (by design — tags are rename-light).
#[derive(Debug, Clone, Copy, Default)]
pub struct TagPatch<'a> {
    /// New `tags.name`, pre-trimmed (`None` = untouched).
    pub name: Option<&'a str>,
    /// New `tags.color` (`None` = untouched).
    pub color: Option<&'a str>,
}

impl TagPatch<'_> {
    /// Whether every field is untouched (no `UPDATE` to emit).
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.color.is_none()
    }
}

/// Apply a [`TagPatch`] to an owned tag (`WHERE id + user_id`, so a
/// foreign id matches zero rows — the caller pre-reads via [`fetch_tag`]
/// for the 404 instead of trusting affected-rows, which would misreport an
/// idempotent no-change write as missing). Rename onto another owned name
/// (incl. case-variant) → 409 via the shared `Db` arm; renaming a row
/// onto itself (even case-only) touches only its own row — no false 409.
#[tracing::instrument(skip(ex))]
pub async fn apply_tag_patch<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
    patch: TagPatch<'_>,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if patch.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("UPDATE `tags` SET ");
    {
        // `Separated` inserts `", "` between EVERY push — including between a
        // `` `col` = `` prefix and its placeholder — so each value rides
        // `push_bind_unseparated` (the `push` prefix already consumed the
        // separator slot). Plain `push_bind` here emits `` `col` = , ? ``
        // (1064 at runtime; see T22 probe).
        let mut set = builder.separated(", ");
        if let Some(name) = patch.name {
            set.push("`name` = ");
            set.push_bind_unseparated(name);
        }
        if let Some(color) = patch.color {
            set.push("`color` = ");
            set.push_bind_unseparated(color);
        }
    }
    builder.push(" WHERE `id` = ");
    builder.push_bind(id);
    builder.push(" AND `user_id` = ");
    builder.push_bind(user_id);
    builder.build().execute(ex).await.map_err(AppError::Db)?;
    Ok(())
}

/// Delete an owned tag (`WHERE id + user_id`). `note_tags` rows follow
/// via `ON DELETE CASCADE` FKs — no manual cleanup loop. Zero affected rows
/// (foreign/missing id) → `false`; the route returns 404.
#[tracing::instrument(skip(ex))]
pub async fn delete_tag_row<'e, E>(ex: E, user_id: i32, id: i32) -> Result<bool, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "DELETE FROM `tags` WHERE `id` = ? AND `user_id` = ?",
        id,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(result.rows_affected() > 0)
}
