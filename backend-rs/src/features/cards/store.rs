//! cards store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership rides the grandparent board (`cards` carries no `user_id`):
//! single-card lookups join `columns` + `boards` with `b.user_id = ?`, so a
//! foreign or missing id matches zero rows → [`AppError::NotFound`] (via
//! `RowNotFound`), never a 403 oracle. Column-scoped writes first prove the
//! column via [`crate::common::ownership::check_column_owned`] inside the
//! caller's tx, so the check and the write that follows are atomic.
//!
//! Ordering rides fractional `sort_key`s (T010): every card carries an
//! `ascii_bin` base62 key and every create/move/delete is a **single-row
//! write** — no sibling is ever renumbered. Wire `position` (integer,
//! 0-based) is COMPUTED at read: `enumerate` for lists/pages (page =
//! `offset + i`), a correlated `COUNT(*) … sort_key < ?` for single-row
//! reads, per-column counters for grouped fetches. Default-create appends via
//! a serialized `ORDER BY sort_key DESC LIMIT 1 FOR UPDATE` read +
//! `key_between`; an explicit `position` clamps to `[0, N]` and takes the
//! between-neighbours key. Move/reorder locks ONLY the moving row
//! (`fetch_card_spot_for_update`, `FOR UPDATE`) preserving the same-card-race
//! `Restart`; neighbour reads stay unlocked (last-write-wins for concurrent
//! same-gap drops — the unique `(column_id, sort_key)` index decides).
//! Requested positions clamp via [`crate::common::order_key::clamp_position`]
//! and the CLAMPED value is written (same-column upper = `others`, cross-
//! column upper = `M`, missing cross-column position appends).
//!
//! List queries never `SELECT` the `description` TEXT: `has_description`
//! comes from a SQL `CASE` (boolean only), `checklist_progress` from a batch
//! aggregation (present only when the card owns at least one checklist).
//! `query!` `DATETIME` selects need NO type override: with the in-tree
//! session store (T65) the `sqlx` `time` feature is no longer enabled, so
//! `query!` maps a bare `DATETIME` to `chrono::NaiveDateTime` natively.
//! Nullable
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
    BoardCardListItem, CardDetailResponse, ChecklistItemResponse, ChecklistResponse,
    format_timestamp,
};

/// One `cards` row (assembled by [`fetch_card_owned`]; labels/checklists ride
/// separately so the card select stays single-row). `position` is COMPUTED
/// at read (T010) — never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRow {
    /// `cards.id`.
    pub id: CardId,
    /// Card title.
    pub title: String,
    /// `cards.column_id`.
    pub column_id: ColumnId,
    /// Wire position (0-based, computed).
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

/// Fetch one owned card (join proves ownership: foreign/missing id →
/// `RowNotFound` → 404, never 403). The wire position is a correlated
/// `COUNT(*) … sort_key < ?` over the unique `(column_id, sort_key)` index.
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_owned<'e, E>(ex: E, user_id: i32, id: i32) -> Result<CardRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT k.`id` AS \"id!\", k.`title` AS \"title!\", k.`column_id` AS \"column_id!: i32\", \
          (SELECT COUNT(*) FROM `cards` p \
            WHERE p.`column_id` = k.`column_id` AND p.`sort_key` < k.`sort_key`) AS \"position!: i64\", \
          k.`description` AS \"description: String\", \
          k.`due_date`, \
          k.`created_at` AS \"created_at!\", \
          k.`updated_at` AS \"updated_at!\" \
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
        position: i32::try_from(row.position).map_err(|_| AppError::Internal(Internal))?,
        description: row.description,
        due_date: row.due_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// The highest `sort_key` currently in a column (`None` when empty), locking
/// the last index record for the insert that follows. MUST run inside the
/// create tx (`FOR UPDATE` outside a tx locks nothing). The last-key read +
/// `key_between` + INSERT in one tx is the serialized append contract (the
/// pre-T010 `MAX(position) FOR UPDATE` analog, still effectively
/// single-record — the win is single-row writes with zero sibling rewrites).
#[tracing::instrument(skip(ex))]
pub async fn fetch_last_sort_key_for_update<'e, E>(
    ex: E,
    column_id: i32,
) -> Result<Option<String>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `sort_key` FROM `cards` WHERE `column_id` = ? \
         ORDER BY `sort_key` DESC LIMIT 1 FOR UPDATE",
        column_id
    )
    .fetch_optional(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|row| row.sort_key))
}

/// A column's sibling `sort_key`s, ascending, excluding `exclude_id` (the
/// moving card) when set — the between-neighbours computation for moves and
/// explicit-position creates. Neighbour reads are deliberately UNLOCKED
/// (D3: last-write-wins for concurrent same-gap drops; uniqueness rides
/// the unique index, turning same-key races into a scoped `1062`).
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_keys_in_column<'e, E>(
    ex: E,
    column_id: i32,
    exclude_id: Option<i32>,
) -> Result<Vec<(i32, String)>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let pairs: Vec<(i32, String)> = match exclude_id {
        Some(id) => sqlx::query!(
            "SELECT `id`, `sort_key` FROM `cards` \
             WHERE `column_id` = ? AND `id` != ? ORDER BY `sort_key` ASC, `id` ASC",
            column_id,
            id
        )
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .map(|row| (row.id, row.sort_key))
        .collect(),
        None => sqlx::query!(
            "SELECT `id`, `sort_key` FROM `cards` \
             WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
            column_id
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

/// Total cards in a column (the list `Page.total` — plain `COUNT(*)`, no join
/// fanout; the caller proves ownership first via
/// [`crate::common::ownership::check_column_owned`]).
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

/// Insert a card row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn insert_card<'e, E>(
    ex: E,
    column_id: i32,
    title: &str,
    sort_key: &str,
    description: Option<&str>,
    due_date: Option<NaiveDateTime>,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `cards` (`title`, `column_id`, `sort_key`, `description`, `due_date`) \
          VALUES (?, ?, ?, ?, ?)",
        title,
        column_id,
        sort_key,
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
/// `ON DELETE CASCADE` FKs — no manual cleanup loop. No gap is closed —
/// sibling `sort_key`s are untouched (T010). The caller pre-read the row
/// ([`fetch_card_owned`]) for ownership.
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

/// Re-read the moving card's `(column_id, sort_key)` under `FOR UPDATE`
/// (ownership join included: foreign/missing id → `RowNotFound` → 404, never
/// 403). MUST run inside the move tx AFTER any sibling reads: the row lock
/// serializes concurrent moves of the SAME card; the caller restarts the tx
/// when the column differs from the pre-lock read (same-card race — writing
/// against an unlocked column is never allowed). `sort_key` is returned for
/// fresh-index computation by the move worker.
#[tracing::instrument(skip(ex))]
pub async fn fetch_card_spot_for_update<'e, E>(
    ex: E,
    user_id: i32,
    id: i32,
) -> Result<(i32, String), AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT k.`column_id` AS \"column_id: i32\", k.`sort_key` \
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
    Ok((row.column_id, row.sort_key))
}

/// Point-write the moving card: sets `column_id` and `sort_key` in one
/// row write (never a neighbour-touching rewrite). The caller passes the
/// between-neighbours key for the requested (clamped) index.
#[tracing::instrument(skip(ex))]
pub async fn point_move_card<'e, E>(
    ex: E,
    id: i32,
    column_id: i32,
    sort_key: &str,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "UPDATE `cards` SET `column_id` = ?, `sort_key` = ? WHERE `id` = ?",
        column_id,
        sort_key,
        id
    )
    .execute(ex)
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
    /// Wire position (0-based, computed per checklist at read).
    pub position: i32,
    /// `checklist_items.created_at`.
    pub created_at: NaiveDateTime,
    /// `checklist_items.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// Items for all of one card's checklists (`checklist_id, sort_key ASC`) via
/// a subselect — one query, no dynamic `IN` needed. Positions are per-group
/// enumerate indexes assigned in Rust (the fetch is group-ordered).
#[tracing::instrument(skip(ex))]
pub async fn fetch_detail_items<'e, E>(ex: E, card_id: i32) -> Result<Vec<DetailItemRow>, AppError>
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
        items.push(DetailItemRow {
            id: row.id,
            text: row.text,
            is_completed: row.is_completed,
            checklist_id: row.checklist_id,
            position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(items)
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

/// Page `SELECT` for a column's cards (`sort_key ASC, id ASC`, TEXT-free).
/// Wire positions are `offset + index` (computed). The caller proves
/// ownership first via [`crate::common::ownership::check_column_owned`].
/// Built with
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
          k.`column_id`, k.`due_date`, k.`created_at`, k.`updated_at` \
          FROM `cards` k WHERE k.`column_id` = ",
    );
    builder.push_bind(column_id);
    builder.push(" ORDER BY k.`sort_key` ASC, k.`id` ASC LIMIT ");
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
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let position = offset
                .checked_add(u32::try_from(index).map_err(|_| AppError::Internal(Internal))?)
                .and_then(|position| i32::try_from(position).ok())
                .ok_or(AppError::Internal(Internal))?;
            Ok((
                row.id,
                row.title,
                row.has_description != 0,
                row.column_id,
                position,
                row.due_date,
                row.created_at,
                row.updated_at,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
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
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
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
/// wire [`CardSummary`]s, preserving the page order. Pure: no
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

/// One TEXT-free board cards list row.
#[derive(Debug, Clone, sqlx::FromRow)]
struct BoardCardPageRow {
    id: i32,
    title: String,
    has_description: i64,
    column_id: i32,
    column_name: String,
    position: i64,
    due_date: Option<NaiveDateTime>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

/// Shared board-scope WHERE for board card count/page queries. T049 extends
/// this builder with filter/search clauses while keeping count/page drift-free.
fn push_board_cards_filter(builder: &mut QueryBuilder<MySql>, board_id: i32) {
    builder.push(" WHERE c.`board_id` = ");
    builder.push_bind(board_id);
}

/// Static ORDER BY fragment for board cards list. User input is never
/// interpolated; callers pass validated allowlist values only.
fn board_cards_ordering(sort: &str, order: &str) -> &'static str {
    match (sort, order) {
        ("created_at", "asc") => "k.`created_at` ASC, k.`id` ASC",
        ("created_at", "desc") => "k.`created_at` DESC, k.`id` DESC",
        ("updated_at", "asc") => "k.`updated_at` ASC, k.`id` ASC",
        ("updated_at", "desc") => "k.`updated_at` DESC, k.`id` DESC",
        ("title", "asc") => "k.`title` ASC, k.`id` ASC",
        ("title", "desc") => "k.`title` DESC, k.`id` DESC",
        // due_date: NULLs last in both directions (locked T048 contract).
        ("due_date", "asc") => "(k.`due_date` IS NULL) ASC, k.`due_date` ASC, k.`id` ASC",
        ("due_date", "desc") => "(k.`due_date` IS NULL) ASC, k.`due_date` DESC, k.`id` DESC",
        _ => "k.`updated_at` DESC, k.`id` DESC",
    }
}

/// `SELECT COUNT(*)` for board-scoped card list `Page.total`.
#[tracing::instrument(skip(ex))]
pub async fn count_board_cards<'e, E>(ex: E, board_id: i32) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let mut builder = QueryBuilder::new(
        "SELECT COUNT(*) FROM `cards` k JOIN `columns` c ON c.`id` = k.`column_id`",
    );
    push_board_cards_filter(&mut builder, board_id);
    let total: i64 = builder
        .build_query_scalar()
        .fetch_one(ex)
        .await
        .map_err(AppError::Db)?;
    u64::try_from(total).map_err(|_| AppError::Internal(Internal))
}

/// Page `SELECT` for board-scoped card list. TEXT-free; `position` is the
/// per-column computed position in stable column sort-key order (0-based).
#[tracing::instrument(skip(ex))]
#[allow(clippy::too_many_arguments)]
pub async fn fetch_board_card_page<'e, E>(
    ex: E,
    board_id: i32,
    sort: &str,
    order: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<BoardCardListItem>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let ordering = board_cards_ordering(sort, order);
    let mut builder = QueryBuilder::new(
        "SELECT k.`id`, k.`title`, \
          (CASE WHEN k.`description` IS NOT NULL AND k.`description` REGEXP '[^[:space:]]' \
            THEN 1 ELSE 0 END) AS `has_description`, \
          k.`column_id`, c.`name` AS `column_name`, \
          (SELECT COUNT(*) FROM `cards` p2 \
            WHERE p2.`column_id` = k.`column_id` AND p2.`sort_key` < k.`sort_key`) AS `position`, \
          k.`due_date`, k.`created_at`, k.`updated_at` \
          FROM `cards` k JOIN `columns` c ON c.`id` = k.`column_id`",
    );
    push_board_cards_filter(&mut builder, board_id);
    builder.push(" ORDER BY ");
    builder.push(ordering);
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows: Vec<BoardCardPageRow> = builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(BoardCardListItem {
            id: CardId::from(row.id),
            title: row.title,
            has_description: row.has_description != 0,
            column_id: ColumnId::from(row.column_id),
            column_name: row.column_name,
            position: i32::try_from(row.position).map_err(|_| AppError::Internal(Internal))?,
            due_date: row.due_date.as_ref().map(summary_timestamp),
            created_at: summary_timestamp(&row.created_at),
            updated_at: summary_timestamp(&row.updated_at),
            labels: Vec::new(),
            checklist_progress: None,
        });
    }
    Ok(out)
}

/// Attach labels and checklist progress to board card list items (in place).
/// Reuses page label/progress batch queries; empty input short-circuits.
#[tracing::instrument(skip(pool, items))]
pub async fn hydrate_board_card_items(
    pool: &sqlx::MySqlPool,
    items: &mut [BoardCardListItem],
) -> Result<(), AppError> {
    use std::collections::HashMap;

    if items.is_empty() {
        return Ok(());
    }
    let card_ids: Vec<i32> = items.iter().map(|item| item.id.get()).collect();
    let (labels, progress) = tokio::try_join!(
        fetch_page_labels(pool, &card_ids),
        fetch_page_progress(pool, &card_ids),
    )?;

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
    for item in items {
        let id = item.id.get();
        item.labels = labels_by_card.remove(&id).unwrap_or_default();
        item.checklist_progress = progress_by_card.get(&id).copied();
    }
    Ok(())
}
