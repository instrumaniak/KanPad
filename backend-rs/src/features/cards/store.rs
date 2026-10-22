//! cards store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership rides the grandparent board (`cards` carries no `user_id`):
//! single-card lookups join `columns` + `boards` with `b.user_id = ?`, so a
//! foreign or missing id matches zero rows → [`AppError::NotFound`] (via
//! `RowNotFound`), never a 403 oracle. Column-scoped writes first prove the
//! column via [`check_column_owned`] inside the caller's tx, so the check and
//! the write that follows are atomic.
//!
//! Create with an explicit `position` clamps to `[0, max + 1]` then opens a
//! gap with a single set-based range `UPDATE` (`position + 1` where
//! `position >= ?`); the default is `MAX(position) + 1` taken under `SELECT
//! ... FOR UPDATE` in the create tx (an unlocked `MAX` races duplicate
//! positions). Delete runs the row delete + a set-based gap-close
//! (`position - 1` range `UPDATE`) in ONE tx — never a reload+save loop.
//!
//! Move/reorder (`column_id`/`position` on `PATCH`, T24b): same-column = one
//! range shift + one point update; cross-column = close-gap + open-gap +
//! point update. All range writes are set-based (`position ± 1` with a
//! `WHERE` band — never find-all-then-save-loop). The tx locks columns in
//! stable ascending `column_id` order (`min` first) via
//! [`lock_cards_for_update`], then re-reads the moving card's spot under
//! `FOR UPDATE` ([`fetch_card_spot_for_update`]) so concurrent moves of
//! *other* cards cannot stale the shift bands; a card that changed columns
//! under us (same-card race) restarts the tx instead of writing against an
//! unlocked column. Requested positions clamp via [`clamp_position`] and the
//! CLAMPED value is written (same-column upper = `N - 1`, cross-column upper
//! = `M`, missing cross-column position appends `M = max + 1`).
//!
//! List queries never `SELECT` the `description` TEXT: `has_description`
//! comes from a SQL `CASE` (boolean only), `checklist_progress` from a batch
//! aggregation (present only when the card owns at least one checklist).
//! `query!` `DATETIME` selects carry explicit
//! `AS "col: chrono::NaiveDateTime"` overrides (the lockfile `time` 0.3.55
//! renames `PrimitiveDateTime` → `PlainDateTime`, so bare `DATETIME` maps to
//! the `time` crate under `query!`); `QueryBuilder` page selects decode
//! straight into `chrono` (the quirk only affects the macros). Nullable
//! selects override with the INNER type (sqlx wraps nullable columns in
//! `Option` itself). `COUNT()` decodes as `i64` (`LONGLONG`); `tinyint`
//! decodes as `i8` (compare `!= 0` in Rust); `COUNT(CASE …)` — never `SUM()`
//! — because `SUM()` of an exact-value argument returns `DECIMAL`, which has
//! no `i64` decoder.
//!
//! One query per fetch fn (an executor value moves, so multi-select flows
//! live in the caller, which orchestrates pool/tx borrows).

use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlExecutor, QueryBuilder};

use crate::{
    common::{
        card_summary::{
            CardLabelRef, CardSummary, ChecklistProgress, format_timestamp as summary_timestamp,
        },
        ids::{CardId, ColumnId, LabelId},
    },
    error::{AppError, Internal},
};

use super::types::{
    CardDetailResponse, ChecklistItemResponse, ChecklistResponse, format_timestamp,
};

/// One `cards` row (assembled by [`fetch_card_owned`]; labels/checklists ride
/// separately so the card select stays single-row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRow {
    /// `cards.id`.
    pub id: CardId,
    /// Card title.
    pub title: String,
    /// `cards.column_id`.
    pub column_id: ColumnId,
    /// Zero-based position.
    pub position: i32,
    /// `cards.description` (`None` = unset; TEXT selected here — never in
    /// lists).
    pub description: Option<String>,
    /// `cards.due_date` (`None` = unset).
    pub due_date: Option<NaiveDateTime>,
    /// `cards.created_at`.
    pub created_at: NaiveDateTime,
    /// `cards.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Assert a column belongs to the caller (via its board). Foreign/missing id
/// → `RowNotFound` → 404 (no oracle, never 403). Returns the owning board id
/// for callers that need it. Runs inside the caller's tx so the check and the
/// write that follows are atomic.
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

/// Fetch one owned card (join proves ownership: foreign/missing id →
/// `RowNotFound` → 404, never 403).
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_owned<'e, E>(ex: E, user_id: i32, id: i32) -> Result<CardRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT k.`id`, k.`title`, k.`column_id` AS \"column_id: i32\", k.`position`, \
          k.`description` AS \"description: String\", \
          k.`due_date` AS \"due_date: chrono::NaiveDateTime\", \
          k.`created_at` AS \"created_at: chrono::NaiveDateTime\", \
          k.`updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
          FROM `cards` k \
          JOIN `columns` c ON c.`id` = k.`column_id` \
          JOIN `boards` b ON b.`id` = c.`board_id` \
          WHERE k.`id` = ? AND b.`user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(CardRow {
        id: CardId::from(row.id),
        title: row.title,
        column_id: ColumnId::from(row.column_id),
        position: row.position,
        description: row.description,
        due_date: row.due_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Current `MAX(position)` in a column (`-1` when empty) locking the range
/// for the insert that follows. MUST run inside the create tx (`FOR UPDATE`
/// outside a tx locks nothing); `MAX(INT)` decodes as `i64` (`LONGLONG`),
/// narrowed via `try_from` (never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn max_position_for_update<'e, E>(ex: E, column_id: i32) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT COALESCE(MAX(`position`), -1) AS \"max_pos: i64\" \
          FROM `cards` WHERE `column_id` = ? FOR UPDATE",
        column_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(row.max_pos).map_err(|_| AppError::Internal(Internal))
}

/// Total cards in a column (the list `Page.total` — plain `COUNT(*)`, no join
/// fanout; the caller proves ownership first via [`check_column_owned`]).
#[tracing::instrument(skip(ex))]
pub async fn count_cards_in_column<'e, E>(ex: E, column_id: i32) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let total = sqlx::query!(
        "SELECT COUNT(*) AS \"total: i64\" FROM `cards` WHERE `column_id` = ?",
        column_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    u64::try_from(total.total).map_err(|_| AppError::Internal(Internal))
}

/// Open a gap for an explicit-position insert: one set-based range `UPDATE`
/// shifts every card at or after `position` up by one (never a reload+save
/// loop).
#[tracing::instrument(skip(ex))]
pub async fn open_card_gap<'e, E>(ex: E, column_id: i32, position: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `cards` SET `position` = `position` + 1 \
          WHERE `column_id` = ? AND `position` >= ?",
        column_id,
        position
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Insert a card row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn insert_card<'e, E>(
    ex: E,
    column_id: i32,
    title: &str,
    position: i32,
    description: Option<&str>,
    due_date: Option<NaiveDateTime>,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `cards` (`title`, `column_id`, `position`, `description`, `due_date`) \
          VALUES (?, ?, ?, ?, ?)",
        title,
        column_id,
        position,
        description,
        due_date
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Partial card update (T24a `PATCH`, scalars only): every field optional.
/// `description`/`due_date` mirror the DTO double `Option` — `None` leaves
/// the column untouched, `Some(None)` clears (`NULL`), `Some(Some(..))`
/// sets. An all-`None` patch is a no-op (returns without emitting `UPDATE`,
/// so empty `{}` bodies stay 200 without touching the row).
#[derive(Debug, Clone, Copy, Default)]
pub struct CardPatch<'a> {
    /// New `cards.title` (`None` = untouched).
    pub title: Option<&'a str>,
    /// Description change (`None` = untouched).
    pub description: Option<Option<&'a str>>,
    /// Due-date change (`None` = untouched).
    pub due_date: Option<Option<NaiveDateTime>>,
}

impl CardPatch<'_> {
    /// Whether every field is untouched (no `UPDATE` to emit).
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none() && self.due_date.is_none()
    }
}

/// Apply a [`CardPatch`] to a card row. The caller pre-reads via
/// [`fetch_card_owned`] for the 404 instead of trusting affected-rows (which
/// would misreport an idempotent no-change write as missing). `updated_at`
/// bumps via the column's `ON UPDATE CURRENT_TIMESTAMP(6)`.
#[tracing::instrument(skip(ex))]
pub async fn apply_card_patch<'e, E>(ex: E, id: i32, patch: CardPatch<'_>) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if patch.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("UPDATE `cards` SET ");
    {
        // `Separated` inserts `", "` between EVERY push — including between a
        // `` `col` = `` prefix and its placeholder — so each value rides
        // `push_bind_unseparated` (the `push` prefix already consumed the
        // separator slot). Plain `push_bind` here emits `` `col` = , ? ``
        // (1064 at runtime; see T22 probe).
        let mut set = builder.separated(", ");
        if let Some(title) = patch.title {
            set.push("`title` = ");
            set.push_bind_unseparated(title);
        }
        if let Some(description) = patch.description {
            set.push("`description` = ");
            set.push_bind_unseparated(description);
        }
        if let Some(due_date) = patch.due_date {
            set.push("`due_date` = ");
            set.push_bind_unseparated(due_date);
        }
    }
    builder.push(" WHERE `id` = ");
    builder.push_bind(id);
    builder.build().execute(ex).await.map_err(AppError::Db)?;
    Ok(())
}

/// Delete a card row. Label links/checklists/items/notes fan out via DB
/// `ON DELETE CASCADE` FKs — no manual cleanup loop. The caller closes the
/// position gap next (same tx) and pre-read the row ([`fetch_card_owned`])
/// for ownership + the deleted position.
#[tracing::instrument(skip(ex))]
pub async fn delete_card_row<'e, E>(ex: E, id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("DELETE FROM `cards` WHERE `id` = ?", id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Close the position gap left by a delete: one set-based range `UPDATE`
/// shifts every later sibling down by one (never a reload+save loop).
#[tracing::instrument(skip(ex))]
pub async fn close_card_gap<'e, E>(ex: E, column_id: i32, position: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `cards` SET `position` = `position` - 1 \
          WHERE `column_id` = ? AND `position` > ?",
        column_id,
        position
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Clamp a requested card position into `[0, upper]` (pure: the unit subject
/// for move clamping). Callers pass the max WRITABLE index — `N - 1` for a
/// same-column reorder of `N` cards, `M` for a cross-column insert into `M`
/// cards (so `M` appends at `max + 1`) — and write the returned value (never
/// the raw request).
pub fn clamp_position(requested: i32, upper: i32) -> i32 {
    requested.clamp(0, upper.max(0))
}

/// Lock every card row of one column in `(column_id, position)` order
/// (`ORDER BY position ASC ... FOR UPDATE`) — MUST run inside the move tx.
/// Returns the locked card ids (the caller derives the count); the locks are
/// held until the tx commits, so the count and every shift band computed from
/// it are stable. Cross-column moves call this once per column in ascending
/// `column_id` order (`min` first) so opposite-direction moves serialize
/// instead of deadlocking.
#[tracing::instrument(skip(ex))]
pub async fn lock_cards_for_update<'e, E>(ex: E, column_id: i32) -> Result<Vec<i32>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC FOR UPDATE",
        column_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

/// Re-read the moving card's `(column_id, position)` under `FOR UPDATE`
/// (ownership join included: foreign/missing id → `RowNotFound` → 404, never
/// 403). MUST run inside the move tx AFTER [`lock_cards_for_update`]: the
/// spot is fresh (a concurrent move that committed before our locks is
/// visible) and the row lock serializes concurrent moves of the SAME card.
/// The caller restarts the tx when the column differs from the pre-lock read
/// (same-card race — writing against an unlocked column is never allowed).
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_spot_for_update<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
) -> Result<(i32, i32), AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT k.`column_id` AS \"column_id: i32\", k.`position` \
          FROM `cards` k \
          JOIN `columns` c ON c.`id` = k.`column_id` \
          JOIN `boards` b ON b.`id` = c.`board_id` \
          WHERE k.`id` = ? AND b.`user_id` = ? FOR UPDATE",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok((row.column_id, row.position))
}

/// Same-column shift down (moving to a HIGHER index): one set-based range
/// `UPDATE` moves every card in `(old, new]` down by one (never a
/// reload+save loop). No-op when the band is empty — callers skip it when
/// `new == old`, the `UPDATE` itself is safe either way.
#[tracing::instrument(skip(ex))]
pub async fn shift_cards_down<'e, E>(
    ex: E,
    column_id: i32,
    old_position: i32,
    new_position: i32,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `cards` SET `position` = `position` - 1 \
          WHERE `column_id` = ? AND `position` > ? AND `position` <= ?",
        column_id,
        old_position,
        new_position
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Same-column shift up (moving to a LOWER index): one set-based range
/// `UPDATE` moves every card in `[new, old)` up by one (never a reload+save
/// loop).
#[tracing::instrument(skip(ex))]
pub async fn shift_cards_up<'e, E>(
    ex: E,
    column_id: i32,
    new_position: i32,
    old_position: i32,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `cards` SET `position` = `position` + 1 \
          WHERE `column_id` = ? AND `position` >= ? AND `position` < ?",
        column_id,
        new_position,
        old_position
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Point-write the moving card after its gap work (same-column shift or
/// cross-column close+open): sets both `column_id` and `position` in one
/// row write. The caller passes the CLAMPED position (never the raw
/// request).
#[tracing::instrument(skip(ex))]
pub async fn point_move_card<'e, E>(
    ex: E,
    id: i32,
    column_id: i32,
    position: i32,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `cards` SET `column_id` = ?, `position` = ? WHERE `id` = ?",
        column_id,
        position,
        id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert a label belongs to the caller (`WHERE id + user_id` — the same
/// ownership semantics as the labels feature's `fetch_label`: a foreign or
/// missing id matches zero rows → `RowNotFound` → 404, never a 403 oracle).
/// Lives here (not imported from the labels store — features never import
/// each other's stores per `10-architecture.md`).
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

/// Idempotent label attach: single `INSERT IGNORE` into the composite-PK
/// `card_labels` M2M table (a re-attach is a no-op, never a 409 — mirrors the
/// `INSERT IGNORE` idempotency of `common::label_seed`). The caller proves
/// card + label ownership first (foreign ids → 404, never 403).
#[tracing::instrument(skip(ex))]
pub async fn insert_card_label_link<'e, E>(
    ex: E,
    card_id: i32,
    label_id: i32,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "INSERT IGNORE INTO `card_labels` (`card_id`, `label_id`) VALUES (?, ?)",
        card_id,
        label_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Detach a label link. Returns `true` when a row was removed; `false` (stale
/// or never-attached link) → the route renders 404, never 409. The caller
/// proves card + label ownership first, so a foreign label 404s on ownership
/// (never 403) even when no link exists.
#[tracing::instrument(skip(ex))]
pub async fn delete_card_label_link<'e, E>(
    ex: E,
    card_id: i32,
    label_id: i32,
) -> Result<bool, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "DELETE FROM `card_labels` WHERE `card_id` = ? AND `label_id` = ?",
        card_id,
        label_id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(result.rows_affected() > 0)
}

/// One label link for a card detail body.
#[derive(Debug, Clone)]
pub struct DetailLabelRow {
    /// `labels.id`.
    pub label_id: i32,
    /// Label name.
    pub name: String,
    /// Label color.
    pub color: String,
}

/// Label links for one card (`label id` order).
#[tracing::instrument(skip(ex))]
pub async fn fetch_detail_labels<'e, E>(
    ex: E,
    card_id: i32,
) -> Result<Vec<DetailLabelRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT l.`id` AS \"label_id: i32\", l.`name`, l.`color` \
          FROM `card_labels` cl JOIN `labels` l ON l.`id` = cl.`label_id` \
          WHERE cl.`card_id` = ? ORDER BY l.`id` ASC",
        card_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| DetailLabelRow {
            label_id: row.label_id,
            name: row.name,
            color: row.color,
        })
        .collect())
}

/// One checklist row for a card detail body.
#[derive(Debug, Clone)]
pub struct DetailChecklistRow {
    /// `checklists.id`.
    pub id: i32,
    /// Checklist title.
    pub title: String,
    /// `checklists.created_at`.
    pub created_at: NaiveDateTime,
    /// `checklists.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Checklists for one card (`id` order).
#[tracing::instrument(skip(ex))]
pub async fn fetch_detail_checklists<'e, E>(
    ex: E,
    card_id: i32,
) -> Result<Vec<DetailChecklistRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT `id`, `title`, \
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
        .map(|row| DetailChecklistRow {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// One checklist-item row for a card detail body.
#[derive(Debug, Clone)]
pub struct DetailItemRow {
    /// `checklist_items.id`.
    pub id: i32,
    /// Item text.
    pub text: String,
    /// `is_completed` (`tinyint` → `i8`, never `bool`).
    pub is_completed: i8,
    /// `checklist_items.checklist_id`.
    pub checklist_id: i32,
    /// Zero-based position.
    pub position: i32,
    /// `checklist_items.created_at`.
    pub created_at: NaiveDateTime,
    /// `checklist_items.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Items for all of one card's checklists (`checklist_id, position ASC`) via
/// a subselect — one query, no dynamic `IN` needed.
#[tracing::instrument(skip(ex))]
pub async fn fetch_detail_items<'e, E>(ex: E, card_id: i32) -> Result<Vec<DetailItemRow>, AppError>
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
        .map(|row| DetailItemRow {
            id: row.id,
            text: row.text,
            is_completed: row.is_completed,
            checklist_id: row.checklist_id,
            position: row.position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Render a [`CardRow`] + its nests for the wire. Progress rolls up from the
/// loaded items (present only when the card owns ≥ 1 checklist, mirroring
/// Nest `toCardResponse`).
pub fn to_detail(
    row: CardRow,
    labels: Vec<DetailLabelRow>,
    checklists: Vec<DetailChecklistRow>,
    items: Vec<DetailItemRow>,
) -> CardDetailResponse {
    use std::collections::HashMap;
    let mut items_by_checklist: HashMap<i32, Vec<ChecklistItemResponse>> = HashMap::new();
    for item in items {
        items_by_checklist
            .entry(item.checklist_id)
            .or_default()
            .push(ChecklistItemResponse {
                id: crate::common::ids::ChecklistItemId::from(item.id),
                text: item.text,
                is_completed: item.is_completed != 0,
                checklist_id: crate::common::ids::ChecklistId::from(item.checklist_id),
                position: item.position,
                created_at: format_timestamp(&item.created_at),
                updated_at: format_timestamp(&item.updated_at),
            });
    }
    let total: i32 = items_by_checklist.values().map(Vec::len).sum::<usize>() as i32;
    let completed: i32 = items_by_checklist
        .values()
        .flatten()
        .filter(|item| item.is_completed)
        .count() as i32;
    let checklist_responses: Vec<ChecklistResponse> = checklists
        .into_iter()
        .map(|checklist| {
            let list_items = items_by_checklist.remove(&checklist.id).unwrap_or_default();
            ChecklistResponse {
                id: crate::common::ids::ChecklistId::from(checklist.id),
                title: checklist.title,
                card_id: row.id,
                created_at: format_timestamp(&checklist.created_at),
                updated_at: format_timestamp(&checklist.updated_at),
                items: list_items,
            }
        })
        .collect();
    let checklist_progress = if checklist_responses.is_empty() {
        None
    } else {
        Some(ChecklistProgress {
            completed,
            total,
            percent: if total == 0 {
                0
            } else {
                (f64::from(completed) / f64::from(total) * 100.0).round() as i32
            },
        })
    };
    CardDetailResponse {
        id: row.id,
        title: row.title,
        column_id: row.column_id,
        position: row.position,
        description: row.description,
        due_date: row.due_date.as_ref().map(format_timestamp),
        created_at: format_timestamp(&row.created_at),
        updated_at: format_timestamp(&row.updated_at),
        labels: labels
            .into_iter()
            .map(|label| CardLabelRef {
                id: LabelId::from(label.label_id),
                name: label.name,
                color: label.color,
            })
            .collect(),
        checklists: checklist_responses,
        checklist_progress,
    }
}

/// One TEXT-free page row (decodes the [`fetch_card_page`] select).
#[derive(Debug, Clone, sqlx::FromRow)]
struct CardPageRow {
    /// `cards.id`.
    id: i32,
    /// `cards.title`.
    title: String,
    /// `has_description` flag (`CASE` → integer).
    has_description: i64,
    /// `cards.column_id`.
    column_id: i32,
    /// Zero-based position.
    position: i32,
    /// `cards.due_date` (`None` = unset).
    due_date: Option<NaiveDateTime>,
    /// `cards.created_at`.
    created_at: NaiveDateTime,
    /// `cards.updated_at`.
    updated_at: NaiveDateTime,
}

/// One TEXT-free page row: `(id, title, has_description, column_id, position,
/// due_date, created_at, updated_at)`.
pub type CardPageTuple = (
    i32,
    String,
    bool,
    i32,
    i32,
    Option<NaiveDateTime>,
    NaiveDateTime,
    NaiveDateTime,
);

/// One label link for a page: `(card_id, label_id, name, color)`.
pub type PageLabelTuple = (i32, i32, String, String);

/// One progress rollup for a page: `(card_id, total, completed)`.
pub type PageProgressTuple = (i32, i64, i64);

/// Page `SELECT` for a column's cards (`position ASC`, TEXT-free). The caller
/// proves ownership first via [`check_column_owned`]. Built with
/// `QueryBuilder` (shared-filter discipline per `30-db-schema.md`; user text
/// is never `push`ed — only bound ids/limits).
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_page<'e, E>(
    ex: E,
    column_id: i32,
    limit: u32,
    offset: u32,
) -> Result<Vec<CardPageTuple>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let mut builder = QueryBuilder::new(
        "SELECT k.`id`, k.`title`, \
          (CASE WHEN k.`description` IS NOT NULL AND k.`description` REGEXP '[^[:space:]]' \
            THEN 1 ELSE 0 END) AS `has_description`, \
          k.`column_id`, k.`position`, k.`due_date`, k.`created_at`, k.`updated_at` \
          FROM `cards` k WHERE k.`column_id` = ",
    );
    builder.push_bind(column_id);
    builder.push(" ORDER BY k.`position` ASC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    // `QueryBuilder` (not `query!`): pagination binds ride dynamic decoding.
    // Types mirror the `query!` overrides in [`fetch_card_owned`]: the
    // `CASE` flag arrives integer (`!= 0` in Rust), `DATETIME` decodes
    // straight into `chrono` (the `time`-crate mapping quirk only affects the
    // `query!` macros).
    let rows: Vec<CardPageRow> = builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.id,
                row.title,
                row.has_description != 0,
                row.column_id,
                row.position,
                row.due_date,
                row.created_at,
                row.updated_at,
            )
        })
        .collect())
}

/// Label links for a page of cards (`card_id, label id` order). Empty input
/// short-circuits (no `IN ()` query emitted).
#[tracing::instrument(skip(ex))]
pub async fn fetch_page_labels<'e, E>(
    ex: E,
    card_ids: &[i32],
) -> Result<Vec<PageLabelTuple>, AppError>
where
    E: MySqlExecutor<'e>,
{
    if card_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::new(
        "SELECT cl.`card_id`, l.`id`, l.`name`, l.`color` \
          FROM `card_labels` cl JOIN `labels` l ON l.`id` = cl.`label_id` \
          WHERE cl.`card_id` IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for id in card_ids {
            separated.push_bind(*id);
        }
    }
    builder.push(") ORDER BY cl.`card_id` ASC, l.`id` ASC");
    let rows: Vec<PageLabelTuple> = builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(rows)
}

/// Checklist rollups for a page of cards (one row per card owning ≥ 1
/// checklist). Empty input short-circuits. `completed` counts via
/// `COUNT(CASE …)` — never `SUM()` (see module docs).
#[tracing::instrument(skip(ex))]
pub async fn fetch_page_progress<'e, E>(
    ex: E,
    card_ids: &[i32],
) -> Result<Vec<PageProgressTuple>, AppError>
where
    E: MySqlExecutor<'e>,
{
    if card_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<'_, MySql> = QueryBuilder::new(
        "SELECT cl.`card_id`, COUNT(ci.`id`), \
          COUNT(CASE WHEN ci.`is_completed` <> 0 THEN 1 END) \
          FROM `checklists` cl \
          LEFT JOIN `checklist_items` ci ON ci.`checklist_id` = cl.`id` \
          WHERE cl.`card_id` IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for id in card_ids {
            separated.push_bind(*id);
        }
    }
    builder.push(") GROUP BY cl.`card_id`");
    let rows: Vec<PageProgressTuple> = builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(rows)
}

/// Assemble a page of TEXT-free rows + label links + progress rollups into
/// wire [`CardSummary`]s, preserving the `position ASC` page order. Pure: no
/// SQL.
#[allow(clippy::too_many_arguments)]
pub fn to_summaries(
    rows: Vec<CardPageTuple>,
    labels: Vec<PageLabelTuple>,
    progress: Vec<PageProgressTuple>,
) -> Result<Vec<CardSummary>, AppError> {
    use std::collections::HashMap;
    let mut labels_by_card: HashMap<i32, Vec<CardLabelRef>> = HashMap::new();
    for (card_id, label_id, name, color) in labels {
        labels_by_card
            .entry(card_id)
            .or_default()
            .push(CardLabelRef {
                id: LabelId::from(label_id),
                name,
                color,
            });
    }
    let mut progress_by_card: HashMap<i32, ChecklistProgress> = HashMap::new();
    for (card_id, total, completed) in progress {
        let total = i32::try_from(total).map_err(|_| AppError::Internal(Internal))?;
        let completed = i32::try_from(completed).map_err(|_| AppError::Internal(Internal))?;
        let percent = if total == 0 {
            0
        } else {
            (f64::from(completed) / f64::from(total) * 100.0).round() as i32
        };
        progress_by_card.insert(
            card_id,
            ChecklistProgress {
                completed,
                total,
                percent,
            },
        );
    }
    let mut summaries = Vec::with_capacity(rows.len());
    for (id, title, has_description, column_id, position, due_date, created_at, updated_at) in rows
    {
        summaries.push(CardSummary {
            id: CardId::from(id),
            title,
            has_description,
            column_id: ColumnId::from(column_id),
            position,
            due_date: due_date.as_ref().map(summary_timestamp),
            created_at: summary_timestamp(&created_at),
            updated_at: summary_timestamp(&updated_at),
            labels: labels_by_card.remove(&id).unwrap_or_default(),
            checklist_progress: progress_by_card.get(&id).copied(),
        });
    }
    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::clamp_position;

    /// `clamp_position` unit: in-range passes through, negatives pin to 0,
    /// over-large pins to `upper` (the caller passes `N - 1` same-column /
    /// `M` cross-column and WRITES the result — see `cards_move.rs` for the
    /// written-value integration proof).
    #[test]
    fn clamp_position_pins_to_closed_range() {
        assert_eq!(clamp_position(2, 5), 2);
        assert_eq!(clamp_position(0, 5), 0);
        assert_eq!(clamp_position(5, 5), 5);
        assert_eq!(clamp_position(-1, 5), 0);
        assert_eq!(clamp_position(-50, 3), 0);
        assert_eq!(clamp_position(6, 5), 5);
        assert_eq!(clamp_position(999, 3), 3);
        // Degenerate upper (empty/singleton column): still `[0, 0]`.
        assert_eq!(clamp_position(0, 0), 0);
        assert_eq!(clamp_position(7, 0), 0);
        assert_eq!(clamp_position(-7, 0), 0);
        // Negative upper is defensive-only (callers never pass one): pins 0.
        assert_eq!(clamp_position(3, -4), 0);
    }
}
