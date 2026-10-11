//! Pool defaults + canonical executor bound (T03 skeleton).
//! Real pool wiring + health check land in T05; no `query!` here, so no `.sqlx/` entries.

use std::time::Duration;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlExecutor;

/// Canonical pool defaults per 10-architecture.md DB pool.
pub fn pool_options() -> MySqlPoolOptions {
    MySqlPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(30 * 60)))
        .test_before_acquire(true)
}

/// Compile-time proof of the canonical executor bound: every feature store
/// fn is generic over `sqlx::MySqlExecutor<'e>` (one form everywhere), so the
/// same fn accepts `&MySqlPool` and `&mut Transaction<'_, MySql>`.
pub async fn with_executor<'e, E>(_ex: E) -> Result<(), sqlx::Error>
where
    E: MySqlExecutor<'e>,
{
    Ok(())
}
