//! Shared application state (Clone; pool + session store handle + instance id).
//!
//! The pool is created **lazy** (`connect_lazy`): boot never fails on a DB
//! outage, so `GET /health` stays `200` while `GET /ready` reports `503`
//! (see `10-architecture.md` Health). Boot connection errors surface through
//! readiness, never through panics, and never with credentials attached.

use std::sync::Arc;

use sqlx::MySqlPool;

use crate::{common::session_store::MySqlSessionStore, config::Settings, db};

/// Composition-root state. Handlers take `State<AppState>`.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Lazy MySQL pool (already `Arc`-interned; `Clone` is cheap).
    pub pool: MySqlPool,
    /// Validated boot settings.
    pub settings: Arc<Settings>,
    /// Session store handle (backed by the same pool; the
    /// `SessionManagerLayer` + expiry sweeper are wired in T11/`main.rs`).
    pub sessions: MySqlSessionStore,
    /// Per-replica id (`rust-<pid>-<epoch-ms>`), logged at boot/shutdown and
    /// available for log correlation.
    pub instance_id: String,
}

impl AppState {
    /// Build state from settings without I/O (test-friendly): pool connect
    /// and store migration never run here.
    pub fn from_settings(settings: Settings) -> Self {
        let pool = match settings.database_url().parse() {
            Ok(options) => db::pool_options().connect_lazy_with(options),
            // Unreachable via `Settings::load` (a stricter parse check could
            // live in `validate`, but URL validity is the pool's decision):
            // fall back to an unroutable lazy pool so construction stays
            // total and the failure surfaces as `503` at `/ready`.
            Err(_) => db::pool_options().connect_lazy_with(unroutable_options()),
        };
        Self::with_pool(settings, pool)
    }

    /// Re-point state at `pool`, binding the session store to THAT pool's
    /// database (never a store-owned schema — see `30-db-schema.md`
    /// deviation 11).
    ///
    /// `#[sqlx::test]` mints its per-test database AFTER settings are built,
    /// so the harness cannot pass the name to [`Self::from_settings`]. It
    /// needs no round-trip query either: sqlx constructs the test pool with
    /// `.database(&db_name)` (`sqlx-mysql-0.8.6/src/testing/mod.rs`), so the
    /// name is already present in the pool's connect options. Production
    /// boots through [`Self::from_settings`]; this constructor exists so the
    /// test suites never wire the store by hand.
    ///
    /// **The store is bound by the POOL, not by a name** (`54` Part C §C4,
    /// T65): the in-tree store takes the pool and therefore reaches the
    /// `sessions` table in the pool's own database *by construction*. The
    /// previous store interpolated a schema name into every statement and
    /// defaulted to a `tower_sessions` schema — which is how it ended up
    /// pointing at the wrong database until the 2026-10-19 fix. With no
    /// name to interpolate, there is no identifier to validate here, and the
    /// boot-time screen in [`Settings::validate`] remains the single place
    /// that rejects an unusable database name.
    pub fn with_pool(settings: Settings, pool: MySqlPool) -> Self {
        let sessions = MySqlSessionStore::new(pool.clone());
        Self {
            pool,
            settings: Arc::new(settings),
            sessions,
            instance_id: new_instance_id(),
        }
    }
}

/// Store-owned session table, created by the baseline in the app database.
///
/// The store's own default is `tower_sessions`.`session`; the baseline uses
/// this name in the app database instead (see `30-db-schema.md` deviation
/// 11 and `31-live-unicode-migration.md`).
pub use crate::common::session_store::SESSION_TABLE;

/// Lazy-pool fallback for an unparseable URL: valid options pointing at an
/// unroutable address, so `/ready` reports `503` instead of boot panicking.
fn unroutable_options() -> sqlx::mysql::MySqlConnectOptions {
    sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(1)
        .username("kanpad")
        .password("invalid")
        .database("kanpad")
}

/// `rust-<pid>-<epoch-millis>`; monotonic enough to disambiguate replicas in
/// logs (distinct from problem+json `instance`, which is per-request).
fn new_instance_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    format!("rust-{}-{millis}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn from_settings_builds_without_io() {
        let state = AppState::from_settings(Settings::default());
        assert!(state.instance_id.starts_with("rust-"));
        assert_eq!(state.pool.size(), 0);
    }

    #[tokio::test]
    async fn unparseable_url_stays_total() {
        let settings = Settings {
            database_url: Some("://not a url".to_string()),
            ..Default::default()
        };
        let state = AppState::from_settings(settings);
        assert!(state.instance_id.starts_with("rust-"));
    }

    /// `#[sqlx::test]` binds the app to a per-test database whose name only
    /// the pool knows; `with_pool` must keep that pool as-is (no query, no
    /// hand-wired store) — and the session store must be bound to THAT pool,
    /// so it reaches the `sessions` table in the per-test database.
    #[tokio::test]
    async fn with_pool_reads_database_from_pool_options() {
        let options = sqlx::mysql::MySqlConnectOptions::new()
            .host("127.0.0.1")
            .port(1)
            .username("kanpad")
            .password("invalid")
            .database("_sqlx_test_deadbeef");
        let pool = db::pool_options().connect_lazy_with(options);
        let state = AppState::with_pool(Settings::default(), pool);

        assert_eq!(
            state.pool.connect_options().get_database(),
            Some("_sqlx_test_deadbeef"),
            "the pool is kept as-is"
        );
        assert!(state.instance_id.starts_with("rust-"));
        assert_eq!(state.pool.size(), 0, "construction stays I/O-free");
        assert_eq!(
            state.sessions.pool().connect_options().get_database(),
            Some("_sqlx_test_deadbeef"),
            "the session store is bound to the SAME pool — so it reaches the \
             sessions table in the per-test database, not in a store-owned schema"
        );
    }

    /// The session table name is the app-database one, not a store-owned
    /// `tower_sessions`.`session`.
    #[test]
    fn session_table_lives_in_the_app_database() {
        assert_eq!(SESSION_TABLE, "sessions");
        assert!(db::is_valid_db_name(SESSION_TABLE));
    }
}
