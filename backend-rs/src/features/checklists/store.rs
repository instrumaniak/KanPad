//! checklists store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership is deep (`checklists` carries no `user_id`): single-checklist
//! lookups join `cards` + `columns` + `boards` with `b.user_id = ?`, so a
//! foreign or missing id matches zero rows → [`AppError::NotFound`] (via
//! `RowNotFound`), never a 403 oracle. Card-scoped writes first prove the
//! card via [`crate::common::ownership::check_card_owned`] inside the caller's tx, so the check and the
//! write that follows are atomic. Delete fans items out via the DB
//! `ON DELETE CASCADE` FK — no manual cleanup loop.
//!
//! Ordering rides fractional `sort_key`s (T010): item moves and
//! explicit-position creates are **single-row writes** — no sibling is ever
//! renumbered. Wire `position` (integer, 0-based) is COMPUTED at read:
//! `enumerate` for lists (per-checklist counters), a correlated
//! `COUNT(*) … sort_key < ?` for single-row reads. The default item create
//! appends via a serialized `ORDER BY sort_key DESC LIMIT 1 FOR UPDATE`
//! read + `key_between`; an explicit `position` clamps to `[0, N]` and takes
//! the between-neighbours key (it no longer writes verbatim duplicates).
//! `query!` `DATETIME` selects need NO type override: with the in-tree
//! session store (T65) the `sqlx` `time` feature is no longer enabled, so
//! `query!` maps a bare `DATETIME` to `chrono::NaiveDateTime` natively;
//! `tinyint` decodes as `i8` (compare
//! `!= 0` in Rust).
//!
//! One query per fetch fn (an executor value moves, so multi-select flows
//! live in the caller, which orchestrates pool/tx borrows).

use chrono::NaiveDateTime;
use sqlx::{MySqlExecutor, QueryBuilder};

use crate::{
    common::ids::{CardId, ChecklistId, ChecklistItemId},
    error::{AppError, Internal},
};

use super::types::{ChecklistItemResponse, ChecklistResponse, format_timestamp};

/// One `checklists` row (assembled by [`fetch_checklist_owned`] /
/// [`fetch_checklists_for_card`]; items ride separately so the checklist
/// select stays single-purpose).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistRow {
    /// `checklists.id`.
    pub id: ChecklistId,
    /// Checklist title.
    pub title: String,
    /// `checklists.card_id`.
    pub card_id: CardId,
    /// `checklists.created_at`.
    pub created_at: NaiveDateTime,
    /// `checklists.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// One `checklist_items` row for a checklist body. `position` is COMPUTED
/// at read (T010) — never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistItemRow {
    /// `checklist_items.id`.
    pub id: ChecklistItemId,
    /// Item text.
    pub text: String,
    /// `is_completed` (`tinyint` → `i8`, never `bool`).
    pub is_completed: i8,
    /// `checklist_items.checklist_id`.
    pub checklist_id: ChecklistId,
    /// Wire position (0-based, computed).
    pub position: i32,
    /// `checklist_items.created_at`.
    pub created_at: NaiveDateTime,
    /// `checklist_items.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Fetch one owned checklist (deep join proves ownership: foreign/missing id
/// → `RowNotFound` → 404, never 403).
#[tracing::instrument(skip(ex))]
pub async fn fetch_checklist_owned<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
) -> Result<ChecklistRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT cl.`id`, cl.`title`, cl.`card_id` AS \"card_id: i32\", \
           cl.`created_at`, \
           cl.`updated_at` \
           FROM `checklists` cl \
           JOIN `cards` k ON k.`id` = cl.`card_id` \
           JOIN `columns` c ON c.`id` = k.`column_id` \
           JOIN `boards` b ON b.`id` = c.`board_id` \
           WHERE cl.`id` = ? AND b.`user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(ChecklistRow {
        id: ChecklistId::from(row.id),
        title: row.title,
        card_id: CardId::from(row.card_id),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Fetch a card's checklists, `id ASC` (small-N unpaginated list body). The
/// caller proves card ownership first via [`crate::common::ownership::check_card_owned`].
#[tracing::instrument(skip(ex))]
pub async fn fetch_checklists_for_card<'e, E>(
    ex: E,
    card_id: i32,
) -> Result<Vec<ChecklistRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `title`, `card_id` AS \"card_id: i32\", \
           `created_at`, \
           `updated_at` \
           FROM `checklists` WHERE `card_id` = ? ORDER BY `id` ASC",
        card_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| ChecklistRow {
            id: ChecklistId::from(row.id),
            title: row.title,
            card_id: CardId::from(row.card_id),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Items for one checklist (`sort_key ASC, id ASC`). Positions are the
/// enumerate index.
#[tracing::instrument(skip(ex))]
pub async fn fetch_checklist_items<'e, E>(
    ex: E,
    checklist_id: i32,
) -> Result<Vec<ChecklistItemRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `text`, `is_completed`, `checklist_id` AS \"checklist_id: i32\", \
           `created_at`, \
           `updated_at` \
           FROM `checklist_items` WHERE `checklist_id` = ? \
           ORDER BY `sort_key` ASC, `id` ASC",
        checklist_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    let mut items = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        items.push(ChecklistItemRow {
            id: ChecklistItemId::from(row.id),
            text: row.text,
            is_completed: row.is_completed,
            checklist_id: ChecklistId::from(row.checklist_id),
            position: i32::try_from(index).map_err(|_| AppError::Internal(Internal))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(items)
}

/// Items for all of one card's checklists (`checklist_id, sort_key ASC`) via
/// a subselect — one query, no dynamic `IN` needed (mirrors the cards detail
/// items fetch). Positions are per-checklist enumerate indexes assigned in
/// Rust (the fetch is group-ordered).
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_checklist_items<'e, E>(
    ex: E,
    card_id: i32,
) -> Result<Vec<ChecklistItemRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT i.`id`, i.`text`, i.`is_completed`, \
           i.`checklist_id` AS \"checklist_id: i32\", \
           i.`created_at`, \
           i.`updated_at` \
           FROM `checklist_items` i \
           WHERE i.`checklist_id` IN (SELECT c.`id` FROM `checklists` c WHERE c.`card_id` = ?) \
           ORDER BY i.`checklist_id` ASC, i.`sort_key` ASC, i.`id` ASC",
        card_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    let mut items = Vec::with_capacity(rows.len());
    let mut current_checklist = -1_i32;
    let mut next_position = 0_i32;
    for row in rows {
        if row.checklist_id != current_checklist {
            current_checklist = row.checklist_id;
            next_position = 0;
        }
        let position = next_position;
        next_position += 1;
        items.push(ChecklistItemRow {
            id: ChecklistItemId::from(row.id),
            text: row.text,
            is_completed: row.is_completed,
            checklist_id: ChecklistId::from(row.checklist_id),
            position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(items)
}

/// Insert a checklist row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast). The caller proves card
/// ownership first via [`crate::common::ownership::check_card_owned`] in the same tx.
#[tracing::instrument(skip(ex))]
pub async fn insert_checklist<'e, E>(ex: E, card_id: i32, title: &str) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `checklists` (`title`, `card_id`) VALUES (?, ?)",
        title,
        card_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Rename an owned checklist (the caller pre-reads via
/// [`fetch_checklist_owned`] for the 404 instead of trusting affected-rows).
#[tracing::instrument(skip(ex))]
pub async fn rename_checklist<'e, E>(ex: E, id: i32, title: &str) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `checklists` SET `title` = ? WHERE `id` = ?",
        title,
        id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Delete a checklist row. Items cascade via `FK_checklist_items_checklist_id`
/// (`ON DELETE CASCADE`) — no manual cleanup loop. The caller pre-read the row
/// ([`fetch_checklist_owned`]) for ownership.
#[tracing::instrument(skip(ex))]
pub async fn delete_checklist_row<'e, E>(ex: E, id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("DELETE FROM `checklists` WHERE `id` = ?", id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Fetch one owned checklist item (deep join proves ownership: `item →
/// checklist → card → column → board.user`, mirroring
/// [`fetch_checklist_owned`] one level deeper). Foreign/missing id →
/// `RowNotFound` → 404, never 403. The wire position is a correlated
/// `COUNT(*) … sort_key < ?` over the unique `(checklist_id, sort_key)`
/// index.
#[tracing::instrument(skip(ex))]
pub async fn fetch_item_owned<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
) -> Result<ChecklistItemRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT i.`id` AS \"id!\", i.`text` AS \"text!\", i.`is_completed` AS \"is_completed!\", \
           i.`checklist_id` AS \"checklist_id!: i32\", \
           (SELECT COUNT(*) FROM `checklist_items` p \
             WHERE p.`checklist_id` = i.`checklist_id` AND p.`sort_key` < i.`sort_key`) AS \"position!: i64\", \
           i.`created_at` AS \"created_at!\", \
           i.`updated_at` AS \"updated_at!\" \
           FROM `checklist_items` i \
           JOIN `checklists` cl ON cl.`id` = i.`checklist_id` \
           JOIN `cards` k ON k.`id` = cl.`card_id` \
           JOIN `columns` c ON c.`id` = k.`column_id` \
           JOIN `boards` b ON b.`id` = c.`board_id` \
           WHERE i.`id` = ? AND b.`user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(ChecklistItemRow {
        id: ChecklistItemId::from(row.id),
        text: row.text,
        is_completed: row.is_completed,
        checklist_id: ChecklistId::from(row.checklist_id),
        position: i32::try_from(row.position).map_err(|_| AppError::Internal(Internal))?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// The highest `sort_key` currently in a checklist (`None` when empty),
/// locking the last index record for the insert that follows. MUST run
/// inside the create tx (`FOR UPDATE` outside a tx locks nothing). The
/// last-key read + `key_between` + INSERT in one tx is the serialized
/// append contract (the pre-T010 `MAX(position) FOR UPDATE` analog).
#[tracing::instrument(skip(ex))]
pub async fn fetch_last_sort_key_for_update<'e, E>(
    ex: E,
    checklist_id: i32,
) -> Result<Option<String>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `sort_key` FROM `checklist_items` WHERE `checklist_id` = ? \
         ORDER BY `sort_key` DESC LIMIT 1 FOR UPDATE",
        checklist_id
    )
    .fetch_optional(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|row| row.sort_key))
}

/// A checklist's sibling `sort_key`s, ascending, excluding `exclude_id` (the
/// moving item) when set — the between-neighbours computation for moves and
/// explicit-position creates. Neighbour reads are deliberately UNLOCKED
/// (D3: last-write-wins for concurrent same-gap drops).
#[tracing::instrument(skip(ex))]
pub async fn fetch_item_keys_in_checklist<'e, E>(
    ex: E,
    checklist_id: i32,
    exclude_id: Option<i32>,
) -> Result<Vec<(i32, String)>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let pairs: Vec<(i32, String)> = match exclude_id {
        Some(id) => sqlx::query!(
            "SELECT `id`, `sort_key` FROM `checklist_items` \
             WHERE `checklist_id` = ? AND `id` != ? ORDER BY `sort_key` ASC, `id` ASC",
            checklist_id,
            id
        )
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .map(|row| (row.id, row.sort_key))
        .collect(),
        None => sqlx::query!(
            "SELECT `id`, `sort_key` FROM `checklist_items` \
             WHERE `checklist_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
            checklist_id
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

/// Point-write an item's `sort_key` (the single-row move).
#[tracing::instrument(skip(ex))]
pub async fn point_move_item_sort_key<'e, E>(ex: E, id: i32, sort_key: &str) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `checklist_items` SET `sort_key` = ? WHERE `id` = ?",
        sort_key,
        id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Insert a checklist-item row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast). The caller proves checklist
/// ownership first via [`fetch_checklist_owned`] in the same tx.
#[tracing::instrument(skip(ex))]
pub async fn insert_item<'e, E>(
    ex: E,
    checklist_id: i32,
    text: &str,
    sort_key: &str,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `checklist_items` (`text`, `checklist_id`, `sort_key`) \
          VALUES (?, ?, ?)",
        text,
        checklist_id,
        sort_key
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Partial item update (`PATCH`, scalars only): every field optional. An
/// all-`None` patch is a no-op (returns without emitting `UPDATE`, so empty
/// `{}` bodies stay 200 without touching the row).
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemPatch<'a> {
    /// New item text (`None` = untouched).
    pub text: Option<&'a str>,
    /// Completion flag (`None` = untouched; `bool` → `tinyint` `i8` at bind).
    pub is_completed: Option<bool>,
}

impl ItemPatch<'_> {
    /// Whether every field is untouched (no `UPDATE` to emit).
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.is_completed.is_none()
    }
}

/// Apply an [`ItemPatch`] to an item row. The caller pre-reads via
/// [`fetch_item_owned`] for the 404 instead of trusting affected-rows (which
/// would misreport an idempotent no-change write as missing). `updated_at`
/// bumps via the column's `ON UPDATE CURRENT_TIMESTAMP(6)`.
#[tracing::instrument(skip(ex))]
pub async fn apply_item_patch<'e, E>(ex: E, id: i32, patch: ItemPatch<'_>) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if patch.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("UPDATE `checklist_items` SET ");
    {
        // `Separated` inserts `", "` between EVERY push — including between a
        // `` `col` = `` prefix and its placeholder — so each value rides
        // `push_bind_unseparated` (the `push` prefix already consumed the
        // separator slot). Plain `push_bind` here emits `` `col` = , ? ``
        // (1064 at runtime; see T22 probe).
        let mut set = builder.separated(", ");
        if let Some(text) = patch.text {
            set.push("`text` = ");
            set.push_bind_unseparated(text);
        }
        if let Some(is_completed) = patch.is_completed {
            set.push("`is_completed` = ");
            set.push_bind_unseparated(i8::from(is_completed));
        }
    }
    builder.push(" WHERE `id` = ");
    builder.push_bind(id);
    builder.build().execute(ex).await.map_err(AppError::Db)?;
    Ok(())
}

/// Delete an item row. The caller pre-read the row ([`fetch_item_owned`]) for
/// ownership.
#[tracing::instrument(skip(ex))]
pub async fn delete_item_row<'e, E>(ex: E, id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("DELETE FROM `checklist_items` WHERE `id` = ?", id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Render one [`ChecklistItemRow`] for the wire. Pure: no SQL.
pub fn to_item_response(item: ChecklistItemRow) -> ChecklistItemResponse {
    ChecklistItemResponse {
        id: item.id,
        text: item.text,
        is_completed: item.is_completed != 0,
        checklist_id: item.checklist_id,
        position: item.position,
        created_at: format_timestamp(&item.created_at),
        updated_at: format_timestamp(&item.updated_at),
    }
}

/// Render one [`ChecklistRow`] + its items for the wire. Pure: no SQL.
pub fn to_response(row: ChecklistRow, items: Vec<ChecklistItemRow>) -> ChecklistResponse {
    ChecklistResponse {
        id: row.id,
        title: row.title,
        card_id: row.card_id,
        created_at: format_timestamp(&row.created_at),
        updated_at: format_timestamp(&row.updated_at),
        items: items
            .into_iter()
            .map(|item| ChecklistItemResponse {
                id: item.id,
                text: item.text,
                is_completed: item.is_completed != 0,
                checklist_id: item.checklist_id,
                position: item.position,
                created_at: format_timestamp(&item.created_at),
                updated_at: format_timestamp(&item.updated_at),
            })
            .collect(),
    }
}

/// Assemble card-scoped rows + items into wire bodies, preserving the
/// `id ASC` list order. Pure: no SQL.
pub fn to_list(rows: Vec<ChecklistRow>, items: Vec<ChecklistItemRow>) -> Vec<ChecklistResponse> {
    use std::collections::HashMap;
    let mut items_by_checklist: HashMap<i32, Vec<ChecklistItemRow>> = HashMap::new();
    for item in items {
        items_by_checklist
            .entry(item.checklist_id.get())
            .or_default()
            .push(item);
    }
    rows.into_iter()
        .map(|row| {
            let list_items = items_by_checklist.remove(&row.id.get()).unwrap_or_default();
            to_response(row, list_items)
        })
        .collect()
}
