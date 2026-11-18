//! Two-phase rebalance of a parent's `sort_key`s (T010, D2).
//!
//! `rebalance_sort_keys` is a library called INSIDE the enclosing
//! transaction; the retry policy therefore belongs to the enclosing
//! `with_tx_retry` (see `common/tx_retry.rs`), never to this module. Phase 0
//! takes `FOR UPDATE` on the parent's rows and holds the locks to the
//! enclosing commit — this code has no `tx.begin()` of its own.
//!
//! A freshly generated key longer than [`crate::common::order_key::REBALANCE_THRESHOLD`]
//! means the neighbouring keys have bloated; every row of the parent is then
//! re-keyed to evenly distributed even keys. The rewrite is **two-phase**
//! because a single-phase even rewrite transiently violates the unique index
//! when row *i*'s new key equals an un-rewritten row's old key:
//!
//!   0. Lock the parent's rows (`SELECT … FOR UPDATE`) and read their ids in
//!      `sort_key` order — the rewrite is deterministic.
//!   1. Park exactly the rows captured in phase 0 at a unique temporary key
//!      outside the base62 key space (`tmp-<id>`), scoped by `id IN (…)` —
//!      unique per parent by construction. Rows that appeared AFTER phase 0
//!      (a concurrent transaction inserted them mid-rebalance) are never
//!      parked, so phase 2 can never strand one at a non-base62 key.
//!   2. Assign final even keys (`keys_between(None, None, n)`) to exactly the
//!      captured ids, one row at a time; parked rows never collide with
//!      base62 finals (`-` is not in the alphabet) and finals are distinct by
//!      construction.
//!
//! Phases 0/1/2 are public so the regression test can interleave a concurrent
//! `INSERT` between them (the F1 post-mortem, `specs/tasks/T010-*.md`);
//! [`rebalance_sort_keys`] composes them for the caller. The caller runs this
//! inside the transaction that generated the long key, so the parent is never
//! observably half-rebalanced. The executor is `&mut MySqlConnection` (the
//! caller's transaction derefs to it) so the locks taken in phase 0 are held
//! to commit.
//!
//! `sort_key` values never carry user input; the per-table statements are
//! literal SQL selected by the [`RebalanceScope`] match — no dynamic string
//! is ever pushed into the wire. Dynamic values travel via `push_bind` only.

use sqlx::{MySql, MySqlConnection, QueryBuilder, Row};

use crate::{
    common::order_key::keys_between,
    error::{AppError, Internal},
};

/// Chunk width for the phase-1 `IN (…)` list (well under placeholder limits).
const PARK_CHUNK: usize = 1000;

/// The parent whose keys to rebalance.
#[derive(Debug, Clone, Copy)]
pub enum RebalanceScope {
    /// A board's columns (`board_id = ?`).
    Columns {
        /// Owning board id.
        board_id: i32,
    },
    /// A column's cards (`column_id = ?`).
    Cards {
        /// Owning column id.
        column_id: i32,
    },
    /// A checklist's items (`checklist_id = ?`).
    ChecklistItems {
        /// Owning checklist id.
        checklist_id: i32,
    },
}

/// Phase 0 — read the parent's ids in key order, locking every row
/// (`SELECT … FOR UPDATE`). Returns the ids in `sort_key ASC, id ASC` order.
#[tracing::instrument(skip(ex))]
pub async fn phase_read(
    ex: &mut MySqlConnection,
    scope: RebalanceScope,
) -> Result<Vec<i32>, AppError> {
    let rows = match scope {
        RebalanceScope::Columns { board_id } => {
            sqlx::query(
                "SELECT `id` FROM `columns` WHERE `board_id` = ? \
                 ORDER BY `sort_key` ASC, `id` ASC FOR UPDATE",
            )
            .bind(board_id)
            .fetch_all(&mut *ex)
            .await
        }
        RebalanceScope::Cards { column_id } => {
            sqlx::query(
                "SELECT `id` FROM `cards` WHERE `column_id` = ? \
                 ORDER BY `sort_key` ASC, `id` ASC FOR UPDATE",
            )
            .bind(column_id)
            .fetch_all(&mut *ex)
            .await
        }
        RebalanceScope::ChecklistItems { checklist_id } => {
            sqlx::query(
                "SELECT `id` FROM `checklist_items` WHERE `checklist_id` = ? \
                 ORDER BY `sort_key` ASC, `id` ASC FOR UPDATE",
            )
            .bind(checklist_id)
            .fetch_all(&mut *ex)
            .await
        }
    }
    .map_err(AppError::Db)?;
    Ok(rows.into_iter().map(|row| row.get("id")).collect())
}

/// Phase 1 — park exactly `ids` (the phase-0 capture) at unique temporary
/// keys outside the base62 key space (`tmp-<id>`; `-` never appears in a
/// base62 key, and ids are unique). Rows that are NOT in `ids` — e.g.
/// inserted by a concurrent transaction after phase 0 — are left untouched.
#[tracing::instrument(skip(ex))]
pub async fn phase_park(
    ex: &mut MySqlConnection,
    scope: RebalanceScope,
    ids: &[i32],
) -> Result<(), AppError> {
    for chunk in ids.chunks(PARK_CHUNK) {
        let mut builder: QueryBuilder<MySql> = match scope {
            RebalanceScope::Columns { board_id } => {
                let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
                    "UPDATE `columns` SET `sort_key` = CONCAT('tmp-', `id`) WHERE `board_id` = ",
                );
                builder.push_bind(board_id);
                builder
            }
            RebalanceScope::Cards { column_id } => {
                let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
                    "UPDATE `cards` SET `sort_key` = CONCAT('tmp-', `id`) WHERE `column_id` = ",
                );
                builder.push_bind(column_id);
                builder
            }
            RebalanceScope::ChecklistItems { checklist_id } => {
                let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
                    "UPDATE `checklist_items` SET `sort_key` = CONCAT('tmp-', `id`) \
                     WHERE `checklist_id` = ",
                );
                builder.push_bind(checklist_id);
                builder
            }
        };
        builder.push(" AND `id` IN (");
        {
            let mut separated = builder.separated(", ");
            for id in chunk {
                separated.push_bind(*id);
            }
        }
        builder.push(")");
        builder
            .build()
            .execute(&mut *ex)
            .await
            .map_err(AppError::Db)?;
    }
    Ok(())
}

/// Phase 2 — assign final even keys (`keys_between(None, None, n)`) to
/// exactly `ids`, in the captured order, one row at a time.
#[tracing::instrument(skip(ex))]
pub async fn phase_assign(
    ex: &mut MySqlConnection,
    scope: RebalanceScope,
    ids: &[i32],
) -> Result<(), AppError> {
    let keys = keys_between(None, None, ids.len()).map_err(|_| AppError::Internal(Internal))?;
    for (id, key) in ids.iter().zip(keys.iter()) {
        let affected = match scope {
            RebalanceScope::Columns { board_id } => {
                sqlx::query("UPDATE `columns` SET `sort_key` = ? WHERE `id` = ? AND `board_id` = ?")
                    .bind(key)
                    .bind(id)
                    .bind(board_id)
                    .execute(&mut *ex)
                    .await
            }
            RebalanceScope::Cards { column_id } => {
                sqlx::query("UPDATE `cards` SET `sort_key` = ? WHERE `id` = ? AND `column_id` = ?")
                    .bind(key)
                    .bind(id)
                    .bind(column_id)
                    .execute(&mut *ex)
                    .await
            }
            RebalanceScope::ChecklistItems { checklist_id } => sqlx::query(
                "UPDATE `checklist_items` SET `sort_key` = ? WHERE `id` = ? AND `checklist_id` = ?",
            )
            .bind(key)
            .bind(id)
            .bind(checklist_id)
            .execute(&mut *ex)
            .await,
        }
        .map_err(AppError::Db)?;
        if affected.rows_affected() != 1 {
            // Phase 0 locked these rows, so a missing row means a bug or a
            // schema anomaly — fail loudly inside the caller's transaction.
            return Err(AppError::Internal(Internal));
        }
    }
    Ok(())
}

/// Rewrite every row of `scope`'s parent to evenly distributed keys, in the
/// two phases described in the module docs. All statements run on the
/// caller's connection (pass the caller's transaction).
#[tracing::instrument(skip(ex))]
pub async fn rebalance_sort_keys(
    ex: &mut MySqlConnection,
    scope: RebalanceScope,
) -> Result<(), AppError> {
    let ids = phase_read(ex, scope).await?;
    if ids.is_empty() {
        return Ok(());
    }
    phase_park(ex, scope, &ids).await?;
    phase_assign(ex, scope, &ids).await
}
