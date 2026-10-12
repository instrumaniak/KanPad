//! checklists store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership is deep (`checklists` carries no `user_id`): single-checklist
//! lookups join `cards` + `columns` + `boards` with `b.user_id = ?`, so a
//! foreign or missing id matches zero rows → [`AppError::NotFound`] (via
//! `RowNotFound`), never a 403 oracle. Card-scoped writes first prove the
//! card via [`check_card_owned`] inside the caller's tx, so the check and the
//! write that follows are atomic. Delete fans items out via the DB
//! `ON DELETE CASCADE` FK — no manual cleanup loop.
//!
//! `query!` `DATETIME` selects carry explicit
//! `AS "col: chrono::NaiveDateTime"` overrides (the lockfile `time` 0.3.55
//! renames `PrimitiveDateTime` → `PlainDateTime`, so bare `DATETIME` maps to
//! the `time` crate under `query!`); `tinyint` decodes as `i8` (compare
//! `!= 0` in Rust).
//!
//! One query per fetch fn (an executor value moves, so multi-select flows
//! live in the caller, which orchestrates pool/tx borrows).

use chrono::NaiveDateTime;
use sqlx::MySqlExecutor;

use crate::{
    common::ids::{CardId, ChecklistId, ChecklistItemId},
    error::{AppError, Internal},
};

use super::types::{format_timestamp, ChecklistItemResponse, ChecklistResponse};

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

/// One `checklist_items` row for a checklist body.
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
    /// Zero-based position.
    pub position: i32,
    /// `checklist_items.created_at`.
    pub created_at: NaiveDateTime,
    /// `checklist_items.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Assert a card belongs to the caller (via `column → board`). Foreign/missing
/// id → `RowNotFound` → 404 (no oracle, never 403). Runs inside the caller's
/// tx so the check and the write that follows are atomic.
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
           cl.`created_at` AS \"created_at: chrono::NaiveDateTime\", \
           cl.`updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
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
/// caller proves card ownership first via [`check_card_owned`].
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
           `created_at` AS \"created_at: chrono::NaiveDateTime\", \
           `updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
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

/// Items for one checklist (`position ASC`).
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
           `position`, \
           `created_at` AS \"created_at: chrono::NaiveDateTime\", \
           `updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
           FROM `checklist_items` WHERE `checklist_id` = ? \
           ORDER BY `position` ASC",
        checklist_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| ChecklistItemRow {
            id: ChecklistItemId::from(row.id),
            text: row.text,
            is_completed: row.is_completed,
            checklist_id: ChecklistId::from(row.checklist_id),
            position: row.position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Items for all of one card's checklists (`checklist_id, position ASC`) via
/// a subselect — one query, no dynamic `IN` needed (mirrors the cards detail
/// items fetch).
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
           i.`checklist_id` AS \"checklist_id: i32\", i.`position`, \
           i.`created_at` AS \"created_at: chrono::NaiveDateTime\", \
           i.`updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
           FROM `checklist_items` i \
           WHERE i.`checklist_id` IN (SELECT c.`id` FROM `checklists` c WHERE c.`card_id` = ?) \
           ORDER BY i.`checklist_id` ASC, i.`position` ASC",
        card_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| ChecklistItemRow {
            id: ChecklistItemId::from(row.id),
            text: row.text,
            is_completed: row.is_completed,
            checklist_id: ChecklistId::from(row.checklist_id),
            position: row.position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Insert a checklist row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast). The caller proves card
/// ownership first via [`check_card_owned`] in the same tx.
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
