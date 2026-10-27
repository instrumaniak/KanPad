//! Shared card-summary fragments (single owner).
//!
//! The nested columns list (T23a) embeds these per card; the card list/detail
//! endpoints (T24a) reuse the same shape — columns must import from here, never
//! keep a local copy and never import another feature's store
//! (`arch-avoid-circular-deps`).
//!
//! List queries never `SELECT` the `description` TEXT: `has_description` comes
//! from a SQL `CASE` (boolean only), `checklist_progress` from a batch
//! aggregation (present only when the card owns at least one checklist).
//! `DATETIME` selects carry explicit `AS "col: chrono::NaiveDateTime"`
//! overrides (the lockfile `time` 0.3.55 renames `PrimitiveDateTime` →
//! `PlainDateTime`, so bare `DATETIME` maps to the `time` crate under
//! `query!`). Nullable selects override with the INNER type (sqlx wraps
//! nullable columns in `Option` itself). `COUNT()` decodes as `i64`
//! (`LONGLONG`); progress math stays in `i64` until the final `i32::try_from`
//! (never a silent `as` cast). `completed` counts via `COUNT(CASE ...)` —
//! never `SUM()` — because `SUM()` of an exact-value argument returns
//! `DECIMAL`, which has no `i64` decoder without extra sqlx features.
//!
//! One query per fetch fn (an executor value moves, so the three selects live
//! in three fns — the caller assembles via [`assemble_summaries`]).

use serde::Serialize;
use sqlx::MySqlExecutor;
use utoipa::ToSchema;

use crate::{
    common::ids::{CardId, ColumnId, LabelId},
    error::{AppError, Internal},
};

/// One label ref inside a card summary (`{id,name,color}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CardLabelRef {
    /// `labels.id` (`INT` ↔ `i32` newtype).
    pub id: LabelId,
    /// Label name.
    pub name: String,
    /// Label color.
    pub color: String,
}

/// Checklist rollup attached only when the card owns ≥ 1 checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChecklistProgress {
    /// Completed items.
    pub completed: i32,
    /// Total items.
    pub total: i32,
    /// `round(completed / total * 100)` (`0` when `total` is `0`).
    pub percent: i32,
}

/// Wire card summary: scalars + label refs + optional progress. Carries no
/// `description` key (TEXT is never selected); `due_date` renders as an
/// ISO-8601 string or `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CardSummary {
    /// `cards.id` (`INT` ↔ `i32` newtype).
    pub id: CardId,
    /// Card title.
    pub title: String,
    /// Whether the card has a non-blank description (SQL `CASE`, never TEXT).
    pub has_description: bool,
    /// Owning column id.
    pub column_id: ColumnId,
    /// Zero-based position.
    pub position: i32,
    /// `due_date` as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (`null` when unset).
    pub due_date: Option<String>,
    /// `created_at`, same shape.
    pub created_at: String,
    /// `updated_at`, same shape.
    pub updated_at: String,
    /// Attached labels (`id` order).
    pub labels: Vec<CardLabelRef>,
    /// Checklist rollup (`None` = the card owns no checklist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checklist_progress: Option<ChecklistProgress>,
}

pub use crate::common::time::format_timestamp;

/// One summary row (decodes the [`fetch_board_cards`] select).
#[derive(Debug, Clone)]
pub struct SummaryRow {
    /// `cards.id`.
    pub id: i32,
    /// `cards.title`.
    pub title: String,
    /// `has_description` flag (`CASE` → `LONGLONG` → `i64`, `!= 0` in Rust).
    pub has_description: i64,
    /// `cards.column_id`.
    pub column_id: i32,
    /// `cards.position`.
    pub position: i32,
    /// `cards.due_date` (`None` = unset).
    pub due_date: Option<chrono::NaiveDateTime>,
    /// `cards.created_at`.
    pub created_at: chrono::NaiveDateTime,
    /// `cards.updated_at`.
    pub updated_at: chrono::NaiveDateTime,
}

/// One label-link row (decodes the [`fetch_board_card_labels`] select).
#[derive(Debug, Clone)]
pub struct CardLabelRow {
    /// `card_labels.card_id`.
    pub card_id: i32,
    /// `labels.id`.
    pub label_id: i32,
    /// `labels.name`.
    pub name: String,
    /// `labels.color`.
    pub color: String,
}

/// One progress row (decodes the [`fetch_board_card_progress`] select).
#[derive(Debug, Clone)]
pub struct ProgressRow {
    /// `checklists.card_id`.
    pub card_id: i32,
    /// `COUNT(checklist_items.id)`.
    pub total: i64,
    /// `COUNT` of completed items (via `CASE`, never `SUM` — see module docs).
    pub completed: i64,
}

/// Board cards for the summary embed (`column_id, position ASC`), TEXT-free.
#[tracing::instrument(skip(ex))]
pub async fn fetch_board_cards<'e, E>(ex: E, board_id: i32) -> Result<Vec<SummaryRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let card_rows = sqlx::query!(
        "SELECT k.`id`, k.`title`, k.`column_id`, k.`position`, \
          (CASE WHEN k.`description` IS NOT NULL AND k.`description` REGEXP '[^[:space:]]' \
            THEN 1 ELSE 0 END) AS \"has_description: i64\", \
          k.`due_date` AS \"due_date: chrono::NaiveDateTime\", \
          k.`created_at` AS \"created_at: chrono::NaiveDateTime\", \
          k.`updated_at` AS \"updated_at: chrono::NaiveDateTime\" \
          FROM `cards` k \
          WHERE k.`column_id` IN (SELECT c.`id` FROM `columns` c WHERE c.`board_id` = ?) \
          ORDER BY k.`column_id` ASC, k.`position` ASC",
        board_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(card_rows
        .into_iter()
        .map(|row| SummaryRow {
            id: row.id,
            title: row.title,
            has_description: row.has_description,
            column_id: row.column_id,
            position: row.position,
            due_date: row.due_date,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Label links for a board's cards (`card_id, label id` order).
#[tracing::instrument(skip(ex))]
pub async fn fetch_board_card_labels<'e, E>(
    ex: E,
    board_id: i32,
) -> Result<Vec<CardLabelRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let label_rows = sqlx::query!(
        "SELECT cl.`card_id` AS \"card_id: i32\", l.`id` AS \"label_id: i32\", \
          l.`name`, l.`color` \
          FROM `card_labels` cl \
          JOIN `labels` l ON l.`id` = cl.`label_id` \
          JOIN `cards` k ON k.`id` = cl.`card_id` \
          WHERE k.`column_id` IN (SELECT c.`id` FROM `columns` c WHERE c.`board_id` = ?) \
          ORDER BY cl.`card_id` ASC, l.`id` ASC",
        board_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(label_rows
        .into_iter()
        .map(|row| CardLabelRow {
            card_id: row.card_id,
            label_id: row.label_id,
            name: row.name,
            color: row.color,
        })
        .collect())
}

/// Checklist rollups for a board's cards (one row per card owning ≥ 1
/// checklist).
#[tracing::instrument(skip(ex))]
pub async fn fetch_board_card_progress<'e, E>(
    ex: E,
    board_id: i32,
) -> Result<Vec<ProgressRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let progress_rows = sqlx::query!(
        "SELECT cl.`card_id` AS \"card_id: i32\", \
          COUNT(ci.`id`) AS \"total: i64\", \
          COUNT(CASE WHEN ci.`is_completed` <> 0 THEN 1 END) AS \"completed: i64\" \
          FROM `checklists` cl \
          LEFT JOIN `checklist_items` ci ON ci.`checklist_id` = cl.`id` \
          WHERE cl.`card_id` IN \
            (SELECT k.`id` FROM `cards` k \
             WHERE k.`column_id` IN (SELECT c.`id` FROM `columns` c WHERE c.`board_id` = ?)) \
          GROUP BY cl.`card_id`",
        board_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(progress_rows
        .into_iter()
        .map(|row| ProgressRow {
            card_id: row.card_id,
            total: row.total,
            completed: row.completed,
        })
        .collect())
}

/// Assemble summary rows + label links + progress rollups into per-column
/// groups (each group keeps the `position ASC` fetch order). Pure: no SQL.
pub fn assemble_summaries(
    rows: Vec<SummaryRow>,
    labels: Vec<CardLabelRow>,
    progress: Vec<ProgressRow>,
) -> Result<std::collections::HashMap<i32, Vec<CardSummary>>, AppError> {
    let mut labels_by_card: std::collections::HashMap<i32, Vec<CardLabelRef>> =
        std::collections::HashMap::new();
    for label in labels {
        labels_by_card
            .entry(label.card_id)
            .or_default()
            .push(CardLabelRef {
                id: LabelId::from(label.label_id),
                name: label.name,
                color: label.color,
            });
    }
    let mut progress_by_card: std::collections::HashMap<i32, ChecklistProgress> =
        std::collections::HashMap::new();
    for row in progress {
        let total = i32::try_from(row.total).map_err(|_| AppError::Internal(Internal))?;
        let completed = i32::try_from(row.completed).map_err(|_| AppError::Internal(Internal))?;
        let percent = if total == 0 {
            0
        } else {
            (f64::from(completed) / f64::from(total) * 100.0).round() as i32
        };
        progress_by_card.insert(
            row.card_id,
            ChecklistProgress {
                completed,
                total,
                percent,
            },
        );
    }

    let mut by_column: std::collections::HashMap<i32, Vec<CardSummary>> =
        std::collections::HashMap::new();
    for row in rows {
        by_column
            .entry(row.column_id)
            .or_default()
            .push(CardSummary {
                id: CardId::from(row.id),
                title: row.title,
                has_description: row.has_description != 0,
                column_id: ColumnId::from(row.column_id),
                position: row.position,
                due_date: row.due_date.as_ref().map(format_timestamp),
                created_at: format_timestamp(&row.created_at),
                updated_at: format_timestamp(&row.updated_at),
                labels: labels_by_card.remove(&row.id).unwrap_or_default(),
                checklist_progress: progress_by_card.get(&row.id).copied(),
            });
    }
    Ok(by_column)
}
