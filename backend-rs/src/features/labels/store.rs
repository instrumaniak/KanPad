//! labels store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership is baked into every `WHERE` (`user_id = ?`): a foreign or
//! missing id matches zero rows → [`AppError::NotFound`] (via `RowNotFound`
//! or an explicit affected-rows check), never a 403 oracle (the Nest source
//! threw `ForbiddenException` for foreign labels — retired per the global
//! 403-vs-404 rule). Per-user uniqueness (`UQ_labels_user_name`, case-
//! insensitive under `utf8mb4_unicode_ci`) surfaces as 1062 → 409 through
//! the shared `Db` arm — no pre-check that could race. Seeding itself lives
//! in [`crate::common::label_seed`] (single owner; this module never copies
//! the defaults).

use chrono::NaiveDateTime;
use sqlx::{MySqlExecutor, QueryBuilder};

use crate::{
    common::ids::LabelId,
    error::{AppError, Internal},
};

use super::types::format_timestamp;

/// One list/detail row: the label plus its timestamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelRow {
    /// `labels.id`.
    pub id: LabelId,
    /// `labels.name` (stored trimmed).
    pub name: String,
    /// `labels.color` (one of the 6 enum values).
    pub color: String,
    /// `labels.created_at`.
    pub created_at: NaiveDateTime,
    /// `labels.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Format a [`LabelRow`] timestamp pair for the wire (single owner — see
/// [`super::types::format_timestamp`]).
pub fn wire_timestamps(row: &LabelRow) -> (String, String) {
    (
        format_timestamp(&row.created_at),
        format_timestamp(&row.updated_at),
    )
}

/// Total labels for a user (the list `Page.total` — plain `COUNT(*)`, no
/// join fanout). Callers lazy-seed first when this returns 0.
#[tracing::instrument(skip(ex))]
pub async fn count_labels<'e, E>(ex: E, user_id: i32) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let total = sqlx::query!(
        "SELECT COUNT(*) AS \"total: i64\" FROM `labels` WHERE `user_id` = ?",
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    u64::try_from(total.total).map_err(|_| AppError::Internal(Internal))
}

/// Page `SELECT` for a user (`id ASC`, matching Nest `findAll` ordering).
/// `page`/`limit` arrive pre-validated from the strict
/// [`crate::extract::PaginationParams`] extractor.
#[tracing::instrument(skip(ex))]
pub async fn fetch_label_page<'e, E>(
    ex: E,
    user_id: i32,
    page: u32,
    limit: u32,
) -> Result<Vec<LabelRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `name`, `color`, \
          `created_at`, \
          `updated_at` \
          FROM `labels` WHERE `user_id` = ? \
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
        .map(|row| LabelRow {
            id: LabelId::from(row.id),
            name: row.name,
            color: row.color,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Fetch one owned label. Foreign/missing id → `RowNotFound` → 404.
#[tracing::instrument(skip(ex))]
pub async fn fetch_label<'e, E>(ex: E, user_id: i32, id: i32) -> Result<LabelRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `id`, `name`, `color`, \
          `created_at`, \
          `updated_at` \
          FROM `labels` WHERE `id` = ? AND `user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(LabelRow {
        id: LabelId::from(row.id),
        name: row.name,
        color: row.color,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Insert a label row (name arrives pre-trimmed from the route); returns the
/// new id (`last_insert_id` as `u64` → `i32::try_from`, never a silent `as`
/// cast). Dup per-user name (incl. case-variant under
/// `utf8mb4_unicode_ci`) → 1062 → 409 via the shared `Db` arm.
#[tracing::instrument(skip(ex))]
pub async fn insert_label<'e, E>(
    ex: E,
    user_id: i32,
    name: &str,
    color: &str,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `labels` (`name`, `color`, `user_id`) VALUES (?, ?, ?)",
        name,
        color,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Partial label update: every field optional (`None` = untouched). An
/// all-`None` patch is a no-op (returns without emitting `UPDATE`, so empty
/// `{}` bodies stay 200 without touching the row).
#[derive(Debug, Clone, Copy, Default)]
pub struct LabelPatch<'a> {
    /// New `labels.name`, pre-trimmed (`None` = untouched).
    pub name: Option<&'a str>,
    /// New `labels.color` (`None` = untouched).
    pub color: Option<&'a str>,
}

impl LabelPatch<'_> {
    /// Whether every field is untouched (no `UPDATE` to emit).
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.color.is_none()
    }
}

/// Apply a [`LabelPatch`] to an owned label (`WHERE id + user_id`, so a
/// foreign id matches zero rows — the caller pre-reads via [`fetch_label`]
/// for the 404 instead of trusting affected-rows, which would misreport an
/// idempotent no-change write as missing). Rename onto another owned name
/// (incl. case-variant) → 1062 → 409 via the shared `Db` arm; renaming a row
/// onto itself (even case-only) touches only its own row — no false 409.
/// `updated_at` bumps via the column's `ON UPDATE CURRENT_TIMESTAMP(6)`;
/// `color` is CHECK-guarded at the DB level too (defense in depth behind the
/// DTO enum).
#[tracing::instrument(skip(ex))]
pub async fn apply_label_patch<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
    patch: LabelPatch<'_>,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if patch.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("UPDATE `labels` SET ");
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

/// Delete an owned label (`WHERE id + user_id`). `card_labels` rows follow
/// via `ON DELETE CASCADE` FKs — no manual cleanup loop. Zero affected rows
/// (foreign/missing id) → `false`; the route returns 404.
#[tracing::instrument(skip(ex))]
pub async fn delete_label_row<'e, E>(ex: E, user_id: i32, id: i32) -> Result<bool, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "DELETE FROM `labels` WHERE `id` = ? AND `user_id` = ?",
        id,
        user_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(result.rows_affected() > 0)
}
