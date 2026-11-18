//! Shared transaction-retry kernel (T020, R4).
//!
//! The six inline retry loops that predated this helper (card create, card
//! move, checklist-item create/update, column update, bulk `move_cards`)
//! generalize into [`with_tx_retry`], and the previously SINGLE-SHOT write
//! transactions (deletes, scalar patches, `register`, notes, boards,
//! projects) gain the same protection through it. One matcher — the scoped
//! [`crate::common::retry::is_transient_lock_error`] (`1213` | `1205` |
//! `1062` on `UQ_*_sort_key`) — bounded attempts, `READ COMMITTED` scoped to
//! the NEXT transaction (never `SET SESSION`, which would leak onto every
//! later pooled transaction), and rollback-before-retry queued before the
//! retry decision (see §3 below).
//!
//! The kernel is built on `retry.rs` (matcher + `MAX_ATTEMPTS`), never on a
//! duplicated matcher. Design decisions locked 2026-11-18 (T020 §Design
//! decisions):
//!
//! 1. **Exhaustion surfaces at the boundary.** A lock-class error that
//!    survived all attempts flows out of the closure, is NOT retried, and
//!    reaches the `Db` arm of [`crate::error::AppError`], which renders
//!    **503 `service-unavailable` + `Retry-After: 2`**. Scoped-`1062`
//!    exhaustion stays **409** there (class 23, not a serialization failure).
//! 2. **A failed `commit()` is never retried.** It has an unknown outcome —
//!    it may have applied — so replaying the transaction could double-apply
//!    in-tx effects. The commit happens INSIDE the wrapper; its failure
//!    returns directly (`Db` arm: 503 if lock-class, else generic 500).
//! 3. **1213 already unwound the whole transaction; 1205 rolls back only the
//!    current statement** — so every non-success path leads to a rollback
//!    before the retry decision. On the `Restart` path the kernel rolls back
//!    explicitly; on the error path the body closure drops the transaction,
//!    which queues sqlx's rollback-before-next-use (`Transaction::drop` →
//!    `start_rollback`); the error only reaches the matcher AFTER the body's
//!    future returned, so the retry decision never observes a half-applied
//!    transaction. (A by-value body that hands its transaction back could
//!    not also propagate an error with it — the error path is therefore
//!    drop-queued by construction. The `Restart` and `Done` paths DO get
//!    explicit rollback/commit.)
//! 4. **The backoff sleep never holds a pool connection.** The connection is
//!    released to the pool before sleeping; each attempt re-acquires.
//! 5. **Argon2 (and any out-of-band effect) stays outside the retried
//!    closure.** Retries wrap in-tx effects only.
//!
//! Same-card race (the move worker's `Restart` short-circuit): a LOGICAL
//! conflict must not burn the lock-retry budget, so the move path uses
//! [`with_tx_retry_restartable`] — `Restart` passes through **unretried**
//! (after an explicit rollback) and the caller bounds its own restart loop.

use std::future::Future;
use std::time::Duration;

use sqlx::MySqlPool;

use crate::{
    common::retry::{MAX_ATTEMPTS, is_transient_lock_error},
    error::{AppError, Internal},
};

/// What the transaction body signals once its work is done.
///
/// Used through [`with_tx_retry_restartable`]; [`with_tx_retry`] wraps a
/// plain `Result<T, AppError>` and produces `Done` itself.
// `Restart` is constructed only by the move worker; both variants are part
// of the contract the whole workspace compiles against.
#[derive(Debug)]
pub enum TxOutcome<T> {
    /// The body's work is complete; the kernel commits and returns the value.
    Done(T),
    /// The body detected a same-parent race (the row changed between its
    /// read and its locks) and bailed BEFORE writing. The kernel rolls the
    /// transaction back explicitly and hands the signal back **unretried** —
    /// the caller decides whether to restart the transaction (the move worker
    /// does, bounded by [`MAX_ATTEMPTS`]).
    Restart,
}

/// Options for [`with_tx_retry`].
#[derive(Debug, Clone, Copy)]
pub struct TxRetryOpts {
    /// Run `SET TRANSACTION ISOLATION LEVEL READ COMMITTED` before `BEGIN`
    /// (MySQL rejects it inside a transaction — error 1568 — and the
    /// `SET SESSION` variant would leak onto later pooled transactions).
    /// Default: on, matching the post-T010 paths.
    pub read_committed: bool,
}

impl Default for TxRetryOpts {
    fn default() -> Self {
        Self {
            read_committed: true,
        }
    }
}

impl TxRetryOpts {
    /// Opt out of `READ COMMITTED` (recorded per call site; never the default).
    pub fn skip_read_committed(mut self) -> Self {
        self.read_committed = false;
        self
    }
}

/// Backoff before retry `retry` (1-based: the first retry passes `1`).
///
/// Jittered full-ish sleep `∈ [5, 20 · retry)` ms (AWS/SRE backoff guidance):
/// `5..20`, `5..40`, `5..60`, `5..80` — the whole retry budget stays
/// ≤ 200 ms and comfortably inside the 30 s request `TimeoutLayer`.
/// `sleep` runs with no pool connection held.
fn backoff(retry: u32) -> Duration {
    let upper = 20 * u64::from(retry);
    Duration::from_millis(rand::random_range(5..upper))
}

/// Retry the transaction body up to [`MAX_ATTEMPTS`] on transient
/// lock-class failures (`1213` | `1205` | scoped `1062` on `UQ_*_sort_key`).
///
/// Each attempt re-acquires a pooled connection, optionally scopes
/// `READ COMMITTED` to the upcoming transaction, and runs `f` inside one
/// transaction. On success the transaction commits; an error that is NOT
/// matcher-transient is returned as-is; a matcher-transient error sleeps
/// with no connection held, and retries. When the budget is exhausted the
/// LAST matcher error is returned — the [`AppError::Db`] arm then renders
/// 503 + `Retry-After: 2` (or 409 for a scoped `1062`). Commit is never
/// retried (see module docs, decision 2).
///
/// The body receives the transaction BY VALUE and returns it alongside its
/// output (`Ok((tx, value))`) — the lifetime of each attempt's transaction is
/// per-attempt, so it cannot be borrowed from a stack frame on stable Rust;
/// the kernel commits (or rolls back) the transaction it gets back. The body
/// is any `FnMut(Transaction) -> Fut` whose future is `Send` — typically
/// `|tx| async { ... }`. Re-derive everything from fresh reads inside `f`;
/// the transaction from a previous attempt is discarded. Every store
/// function and [`crate::common::rebalance::rebalance_sort_keys`] takes
/// `&mut MySqlConnection` — deref the transaction as `&mut *tx`.
pub async fn with_tx_retry<'pool, T, F, Fut>(
    pool: &'pool MySqlPool,
    opts: TxRetryOpts,
    mut f: F,
) -> Result<T, AppError>
where
    F: FnMut(sqlx::Transaction<'pool, sqlx::MySql>) -> Fut,
    Fut: Future<Output = Result<(sqlx::Transaction<'pool, sqlx::MySql>, T), AppError>> + Send,
{
    // Wrap the plain-Result body so the shared kernel sees one shape; the
    // wrapper can only ever produce `Done` (never `Restart`).
    match kernel(pool, opts, move |tx| {
        let inner = f(tx);
        async move { inner.await.map(|(tx, value)| (tx, TxOutcome::Done(value))) }
    })
    .await?
    {
        TxOutcome::Done(value) => Ok(value),
        TxOutcome::Restart => Err(AppError::Internal(Internal)),
    }
}

/// The `Restart`-aware variant for the move worker's same-card race:
/// identical to [`with_tx_retry`] except a [`TxOutcome::Restart`] from the
/// body passes through **unretried** (after an explicit rollback) instead of
/// committing or erroring. The caller bounds its own restart loop.
pub async fn with_tx_retry_restartable<'pool, T, F, Fut>(
    pool: &'pool MySqlPool,
    opts: TxRetryOpts,
    f: F,
) -> Result<TxOutcome<T>, AppError>
where
    F: FnMut(sqlx::Transaction<'pool, sqlx::MySql>) -> Fut,
    Fut: Future<Output = Result<(sqlx::Transaction<'pool, sqlx::MySql>, TxOutcome<T>), AppError>>
        + Send,
{
    kernel(pool, opts, f).await
}

/// Shared kernel behind both public entry points.
#[allow(clippy::too_many_lines)]
async fn kernel<'pool, T, F, Fut>(
    pool: &'pool MySqlPool,
    opts: TxRetryOpts,
    mut f: F,
) -> Result<TxOutcome<T>, AppError>
where
    F: FnMut(sqlx::Transaction<'pool, sqlx::MySql>) -> Fut,
    Fut: Future<Output = Result<(sqlx::Transaction<'pool, sqlx::MySql>, TxOutcome<T>), AppError>>
        + Send,
{
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let mut connection = pool.acquire().await.map_err(AppError::Db)?;
        if opts.read_committed {
            // `SET TRANSACTION` is rejected INSIDE a transaction (MySQL 1568)
            // and `SET SESSION` would leak onto every later transaction on
            // this pooled connection — so scope it to the NEXT transaction.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *connection)
                .await
                .map_err(AppError::Db)?;
        }
        // Transaction::begin and MaybePoolConnection::PoolConnection are
        // #[doc(hidden)] in sqlx 0.9 — the only way to a pool-owned
        // `Transaction<'pool>` (the public `Pool::begin` attaches its own
        // lifetime). Pin-and-review on any sqlx minor upgrade.
        let tx = sqlx::Transaction::begin(
            sqlx::pool::MaybePoolConnection::PoolConnection(connection),
            None,
        )
        .await
        .map_err(AppError::Db)?;
        match f(tx).await {
            Ok((tx, TxOutcome::Done(value))) => {
                // A failed commit has an UNKNOWN outcome (it may have applied) —
                // it is never retried and MUST NOT be routed back into
                // `is_transient_lock_error`; the Db arm renders its 503 or 500.
                tx.commit().await.map_err(AppError::Db)?;
                return Ok(TxOutcome::Done(value));
            }
            Ok((tx, TxOutcome::Restart)) => {
                // A logical conflict, not a lock conflict — rolled back and
                // passed through unretried; the caller decides whether to restart.
                if let Err(error) = tx.rollback().await {
                    // The drop path re-queues start_rollback as a safety net,
                    // so this is loud-but-recoverable — worth a signal line.
                    tracing::warn!(error = %error, "rollback after Restart signal failed");
                }
                return Ok(TxOutcome::Restart);
            }
            Err(error) => {
                // The transaction is consumed by the body closure — on the error
                // path it is dropped inside the body's future, which queues
                // sqlx's rollback-before-next-use. That is still BEFORE the
                // matcher decision and any retry sleep here (the error reaches
                // this arm only after the body's future returned), so the retry
                // never observes a half-applied transaction. (1213 has already
                // unwound the tx server-side; 1205 rolls back only the current
                // statement, which the queued rollback then covers.)
                if is_transient_lock_error(&error) {
                    if attempt < MAX_ATTEMPTS {
                        tracing::warn!(
                            attempt,
                            max_attempts = MAX_ATTEMPTS,
                            error = %error,
                            "retrying a transaction after a transient lock failure"
                        );
                        // Sleep WITHOUT holding a pool connection: the
                        // connection returned to the pool with the rollback
                        // queued (see the error arm), so this attempt owns
                        // nothing while sleeping.
                        tokio::time::sleep(backoff(attempt)).await;
                        continue;
                    }
                    // The `error!` (not `warn!`) marks budget exhaustion — the
                    // saturation signal for the capacity/alerting work (T024/T026).
                    tracing::error!(
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        error = %error,
                        "transaction retry budget exhausted"
                    );
                }
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TxOutcome, TxRetryOpts, backoff, with_tx_retry, with_tx_retry_restartable};
    use crate::common::retry::MAX_ATTEMPTS;
    use crate::error::AppError;
    use sqlx::MySqlPool;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    // `1213`/`1205` cannot be fabricated in-process — the matcher only sees
    // real driver errors — so the transient-`1062` arms below use a scratch
    // table with unique indexes (real driver errors), and the `1213` path
    // is proven by the always-fire SIGNAL-trigger test at the end.

    /// Create the scratch table (two unique indexes) and seed the collision
    /// row. Every test gets a fresh per-test database, so plain CREATEs are
    /// fine (no IF EXISTS, no cleanup).
    async fn scratch(pool: &MySqlPool) {
        sqlx::query(
            "CREATE TABLE `tx_retry_unit_scratch` ( \
               `uq_sort_key` INT NOT NULL, \
               `other_key` INT NOT NULL, \
               UNIQUE KEY `UQ_txretry_unit_sort_key` (`uq_sort_key`), \
               UNIQUE KEY `UQ_txretry_unit_other` (`other_key`) \
             ) ENGINE=InnoDB",
        )
        .execute(pool)
        .await
        .expect("scratch table");
        sqlx::query(
            "INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (7, 7)",
        )
        .execute(pool)
        .await
        .expect("scratch seed");
    }

    fn scoped_1062_query() -> sqlx::query::Query<'static, sqlx::MySql, sqlx::mysql::MySqlArguments>
    {
        sqlx::query(
            "INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (7, 8)",
        )
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn scoped_1062_is_retried_to_the_attempt_budget(pool: MySqlPool) {
        scratch(&pool).await;
        let calls = AtomicUsize::new(0);
        let calls = &calls;
        let result = with_tx_retry(&pool, TxRetryOpts::default(), |mut tx| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            scoped_1062_query()
                .execute(&mut *tx)
                .await
                .map_err(AppError::Db)
                .map(|_| (tx, ()))
        })
        .await;
        assert!(
            matches!(result, Err(AppError::Db(sqlx::Error::Database(_)))),
            "scoped 1062 must surface as Db, got {result:?}"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAX_ATTEMPTS as usize,
            "a scoped 1062 burns the full budget before surfacing"
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn unrelated_1062_is_not_retried(pool: MySqlPool) {
        scratch(&pool).await;
        let calls = AtomicUsize::new(0);
        let calls = &calls;
        let result = with_tx_retry(&pool, TxRetryOpts::default(), |mut tx| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            sqlx::query(
                "INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (8, 7)",
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)
            .map(|_| (tx, ()))
        })
        .await;
        assert!(
            matches!(result, Err(AppError::Db(sqlx::Error::Database(_)))),
            "unrelated 1062 must surface as Db, got {result:?}"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "an unrelated unique violation never burns the retry budget"
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn transient_failures_then_recovery_commit_once(pool: MySqlPool) {
        scratch(&pool).await;
        let calls = AtomicUsize::new(0);
        let calls = &calls;
        // Fail the first two attempts with a scoped 1062, then insert a fresh
        // row that succeeds — the helper must retry and the final attempt
        // must COMMIT.
        let result = with_tx_retry(&pool, TxRetryOpts::default(), |mut tx| async move {
            if calls.fetch_add(1, Ordering::SeqCst) < 2 {
                // The duplicate insert must fail with the scoped 1062.
                scoped_1062_query().execute(&mut *tx).await.map_err(AppError::Db).map(|_| (tx, 42))
            } else {
                sqlx::query("INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (9, 9)")
                    .execute(&mut *tx)
                    .await
                    .expect("fresh insert succeeds");
                Ok((tx, 42))
            }
    })
        .await;
        assert_eq!(result.expect("recovery to Ok"), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let rows: i64 = sqlx::query_scalar(
            "SELECT CAST(COUNT(*) AS SIGNED) FROM `tx_retry_unit_scratch` WHERE `uq_sort_key` = 9",
        )
        .fetch_one(&pool)
        .await
        .expect("persisted row");
        assert_eq!(rows, 1, "the recovered transaction committed its insert");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn done_outcome_returns_value_and_commits(pool: MySqlPool) {
        scratch(&pool).await;
        let calls = AtomicUsize::new(0);
        let calls = &calls;
        let result = with_tx_retry(&pool, TxRetryOpts::default(), |mut tx| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            sqlx::query(
                "INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (10, 10)",
            )
            .execute(&mut *tx)
            .await
            .expect("insert");
            Ok((tx, 7))
        })
        .await;
        assert_eq!(result.expect("done"), 7);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a successful closure runs exactly once"
        );
        let rows: i64 = sqlx::query_scalar(
            "SELECT CAST(COUNT(*) AS SIGNED) FROM `tx_retry_unit_scratch` WHERE `uq_sort_key` = 10",
        )
        .fetch_one(&pool)
        .await
        .expect("persisted row");
        assert_eq!(rows, 1, "the Done outcome committed");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn restart_passes_through_unretried_with_rollback(pool: MySqlPool) {
        scratch(&pool).await;
        let calls = AtomicUsize::new(0);
        let calls = &calls;
        let result = with_tx_retry_restartable(&pool, TxRetryOpts::default(), |mut tx| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            sqlx::query("INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (11, 11)")
                .execute(&mut *tx)
                .await
                .expect("insert");
            Ok((tx, TxOutcome::<()>::Restart))
    })
        .await;
        assert!(
            matches!(result, Ok(TxOutcome::Restart)),
            "Restart passes through unretried, got {result:?}"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "Restart never burns the lock-retry budget"
        );
        let rows: i64 = sqlx::query_scalar(
            "SELECT CAST(COUNT(*) AS SIGNED) FROM `tx_retry_unit_scratch` WHERE `uq_sort_key` = 11",
        )
        .fetch_one(&pool)
        .await
        .expect("re-read");
        assert_eq!(rows, 0, "the rolled-back Restart left no rows");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn restartable_done_commits(pool: MySqlPool) {
        scratch(&pool).await;
        let result = with_tx_retry_restartable(&pool, TxRetryOpts::default(), |mut tx| async move {
            sqlx::query("INSERT INTO `tx_retry_unit_scratch` (`uq_sort_key`, `other_key`) VALUES (12, 12)")
                .execute(&mut *tx)
                .await
                .expect("insert");
            Ok((tx, TxOutcome::Done(13)))
    })
        .await;
        assert!(matches!(result, Ok(TxOutcome::Done(13))));
        let rows: i64 = sqlx::query_scalar(
            "SELECT CAST(COUNT(*) AS SIGNED) FROM `tx_retry_unit_scratch` WHERE `uq_sort_key` = 12",
        )
        .fetch_one(&pool)
        .await
        .expect("re-read");
        assert_eq!(rows, 1, "Done commits exactly once");
    }

    #[test]
    fn backoff_bounds_hold() {
        for retry in 1..=4u32 {
            for _ in 0..500 {
                let sleep = backoff(retry);
                assert!(
                    sleep >= Duration::from_millis(5),
                    "floor violated: {sleep:?}"
                );
                assert!(
                    sleep < Duration::from_millis(20 * u64::from(retry)),
                    "ceiling violated for retry {retry}: {sleep:?}"
                );
            }
        }
    }

    #[test]
    fn backoff_is_jittered() {
        // Not a statistical proof — just that the sequence is not constant.
        let samples: Vec<Duration> = (0..32).map(|_| backoff(2)).collect();
        let unique: std::collections::HashSet<u64> =
            samples.iter().map(|d| d.as_nanos() as u64).collect();
        assert!(unique.len() > 1, "backoff must jitter, not repeat");
    }

    /// A pool with ONE connection to THIS test's fresh database proves the
    /// backoff sleep happens with no connection checked out: the second
    /// attempt can only acquire if the first connection was released before
    /// sleeping (otherwise the acquire times out with `PoolTimedOut` long
    /// before the fault counter reaches `MAX_ATTEMPTS`). Built as a sibling
    /// pool on the per-test database so nothing shared is written.
    #[sqlx::test(migrations = "./migrations")]
    async fn backoff_sleep_holds_no_pool_connection(pool: MySqlPool) {
        let base = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = pool
            .connect_options()
            .get_database()
            .expect("the #[sqlx::test] pool names a fresh database")
            .to_string();
        // mysql://user:pass@host:port/db?query → swap the database component.
        let (scheme, rest) = base.split_once("://").expect("DATABASE_URL has a scheme");
        let (creds, after_at) = rest.split_once('@').expect("DATABASE_URL has userinfo");
        let hostport = after_at
            .split('/')
            .next()
            .expect("DATABASE_URL names a host");
        let query = after_at
            .split_once('?')
            .map(|(_, query)| format!("?{query}"))
            .unwrap_or_default();
        let one_conn = format!("{scheme}://{creds}@{hostport}/{database}{query}");
        let one = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&one_conn)
            .await
            .expect("connect");
        sqlx::query(
            "CREATE TABLE `_tx_retry_unit_1213_scratch` (`id` INT PRIMARY KEY) ENGINE=InnoDB",
        )
        .execute(&one)
        .await
        .expect("scratch");
        sqlx::query(
            "CREATE TABLE `_tx_retry_unit_1213_fault` (`fired` BIGINT NOT NULL) ENGINE=MyISAM",
        )
        .execute(&one)
        .await
        .expect("fault table");
        sqlx::query("INSERT INTO `_tx_retry_unit_1213_fault` VALUES (0)")
            .execute(&one)
            .await
            .expect("arm fault");
        // CREATE TRIGGER is not preparable; raw_sql sends one COM_QUERY.
        sqlx::raw_sql(
            "CREATE TRIGGER `_tx_retry_unit_1213_trigger` BEFORE INSERT ON `_tx_retry_unit_1213_scratch` FOR EACH ROW \
             BEGIN \
               UPDATE `_tx_retry_unit_1213_fault` SET `fired` = `fired` + 1; \
               SIGNAL SQLSTATE '40001' SET MYSQL_ERRNO = 1213, MESSAGE_TEXT = 'injected deadlock'; \
             END"
        )
        .execute(&one)
        .await
        .expect("fault trigger");

        let result = with_tx_retry(&one, TxRetryOpts::default(), |mut tx| async move {
            sqlx::query("INSERT INTO `_tx_retry_unit_1213_scratch` (`id`) VALUES (1)")
                .execute(&mut *tx)
                .await
                .map_err(AppError::Db)
                .map(|_| (tx, ()))
        })
        .await;
        assert!(
            matches!(result, Err(AppError::Db(sqlx::Error::Database(_)))),
            "the injected 1213 must survive the budget and surface, got {result:?}"
        );
        let fires: i64 =
            sqlx::query_scalar("SELECT CAST(`fired` AS SIGNED) FROM `_tx_retry_unit_1213_fault`")
                .fetch_one(&pool)
                .await
                .expect("fault counter");
        assert_eq!(
            fires, MAX_ATTEMPTS as i64,
            "every attempt fired; the pool of ONE would have deadlocked if the sleep held the connection"
        );
    }
}

#[cfg(test)]
mod rc_probe_tests {
    use super::{TxRetryOpts, with_tx_retry};
    use crate::error::AppError;
    use sqlx::MySqlPool;

    /// The opt-out works: the tx runs at the SERVER DEFAULT isolation (not
    /// READ COMMITTED), and the insert lands. The isolation level is only
    /// observable per-transaction on MySQL 8
    /// (`performance_schema.events_transactions_current`); MariaDB has no
    /// such view, so the probe records a constant there and the level
    /// assertion is MySQL-only.
    #[sqlx::test(migrations = "./migrations")]
    async fn skip_read_committed_runs_at_server_default_isolation(pool: MySqlPool) {
        sqlx::query("CREATE TABLE `tx_retry_unit_rc_probe` (`isolation` VARCHAR(32) NOT NULL) ENGINE=MyISAM")
            .execute(&pool)
            .await
            .expect("probe table");
        sqlx::query("CREATE TABLE `tx_retry_unit_rc_scratch` (`id` INT PRIMARY KEY) ENGINE=InnoDB")
            .execute(&pool)
            .await
            .expect("scratch");
        let version: String = sqlx::query_scalar("SELECT VERSION()")
            .fetch_one(&pool)
            .await
            .expect("server version");
        let isolation_expr = if version.contains("MariaDB") {
            "SELECT 'READ-COMMITTED'"
        } else {
            "SELECT ISOLATION_LEVEL FROM performance_schema.events_transactions_current \
             WHERE THREAD_ID = PS_CURRENT_THREAD_ID()"
        };
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE TRIGGER `tx_retry_unit_rc_trigger` BEFORE INSERT ON `tx_retry_unit_rc_scratch` FOR EACH ROW \
             BEGIN \
               INSERT INTO `tx_retry_unit_rc_probe` (`isolation`) {isolation_expr}; \
             END"
        )))
        .execute(&pool)
        .await
        .expect("probe trigger");

        let result = with_tx_retry(
            &pool,
            TxRetryOpts::default().skip_read_committed(),
            |mut tx| async move {
                sqlx::query("INSERT INTO `tx_retry_unit_rc_scratch` (`id`) VALUES (1)")
                    .execute(&mut *tx)
                    .await
                    .map_err(AppError::Db)
                    .map(|_| (tx, ()))
            },
        )
        .await;
        assert!(matches!(result, Ok(())), "opt-out tx must succeed");
        let rows: i64 =
            sqlx::query_scalar("SELECT CAST(COUNT(*) AS SIGNED) FROM `tx_retry_unit_rc_scratch`")
                .fetch_one(&pool)
                .await
                .expect("re-read");
        assert_eq!(rows, 1, "the opt-out tx committed");
        let levels: Vec<String> =
            sqlx::query_scalar("SELECT `isolation` FROM `tx_retry_unit_rc_probe`")
                .fetch_all(&pool)
                .await
                .expect("probe rows");
        assert_eq!(levels.len(), 1, "the trigger recorded the tx");
        if !version.contains("MariaDB") {
            let normalised = levels[0].replace('-', " ").to_uppercase();
            assert_eq!(
                normalised, "REPEATABLE READ",
                "the opt-out must NOT set READ COMMITTED"
            );
        }
    }
}
