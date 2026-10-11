//! Default-label seeding (Nest parity: `LabelsService.seedDefaultLabels`).
//!
//! T07 provides this minimal prod implementation as a DOCUMENTED EXCEPTION
//! to the T07 touch scope (`tests/api.rs`, `tests/api/helpers.rs`,
//! `tests/fixtures/*.sql`): the `ensure_default_labels` AC (exactly the 6
//! defaults, idempotent) requires a real prod function for the test-only
//! wrapper in `tests/api/helpers.rs` to call, and T10/T25 import prod
//! directly (never a copy). T10 owns tx-integration (hash-before-tx,
//! register flow) and may extend this file without changing its contract.

use sqlx::MySqlExecutor;

/// The six Nest-parity default labels as `(name, color)` pairs.
/// Colors satisfy `CHK_labels_color`; names satisfy `UQ_labels_user_name`.
pub const DEFAULT_LABELS: [(&str, &str); 6] = [
    ("Urgent", "red"),
    ("Important", "orange"),
    ("In Progress", "yellow"),
    ("Done", "green"),
    ("Bug", "blue"),
    ("Feature", "purple"),
];

/// Seed the six default labels for `user_id`, idempotently.
///
/// Single multi-row `INSERT IGNORE` (one statement; `UQ_labels_user_name`
/// turns a re-seed into a no-op instead of a 1062). Generic over
/// `sqlx::MySqlExecutor` (one form everywhere), so callers pass
/// `&MySqlPool` or `&mut Transaction`. Returns rows inserted
/// (6 on first call, 0 when already seeded). Uses runtime `QueryBuilder`
/// (no `query!` macros), so no `.sqlx/` entries.
pub async fn ensure_default_labels<'e, E>(ex: E, user_id: i32) -> Result<u64, sqlx::Error>
where
    E: MySqlExecutor<'e>,
{
    let mut qb =
        sqlx::QueryBuilder::new("INSERT IGNORE INTO `labels` (`name`, `color`, `user_id`) ");
    qb.push_values(DEFAULT_LABELS, |mut b, (name, color)| {
        b.push_bind(name).push_bind(color).push_bind(user_id);
    });
    qb.build()
        .execute(ex)
        .await
        .map(|result| result.rows_affected())
}
