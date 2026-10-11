//! Shared application state (Clone; pool + session store handle + instance id).
//!
//! The pool is created **lazy** (`connect_lazy`): boot never fails on a DB
//! outage, so `GET /health` stays `200` while `GET /ready` reports `503`
//! (see `10-architecture.md` Health). Boot connection errors surface through
//! readiness, never through panics, and never with credentials attached.

use std::sync::Arc;

use sqlx::MySqlPool;
use tower_sessions_sqlx_store::MySqlStore;

use crate::{config::Settings, db};

/// Composition-root state. Handlers take `State<AppState>`.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Lazy MySQL pool (already `Arc`-interned; `Clone` is cheap).
    pub pool: MySqlPool,
    /// Validated boot settings.
    pub settings: Arc<Settings>,
    /// Session store handle (backed by the same pool; the
    /// `SessionManagerLayer` + expiry sweeper are wired in T11/`main.rs`).
    pub sessions: MySqlStore,
    /// Per-replica id (`rust-<pid>-<epoch-ms>`), logged at boot/shutdown and
    /// available for log correlation.
    pub instance_id: String,
}

impl AppState {
    /// Build state from settings without I/O (test-friendly): pool connect
    /// and store migration never run here.
    pub fn from_settings(settings: Settings) -> Self {
        let url = settings.database_url();
        let pool = match url.parse() {
            Ok(options) => db::pool_options().connect_lazy_with(options),
            // Unreachable via `Settings::load` (a stricter parse check could
            // live in `validate`, but URL validity is the pool's decision):
            // fall back to an unroutable lazy pool so construction stays
            // total and the failure surfaces as `503` at `/ready`.
            Err(_) => db::pool_options().connect_lazy_with(unroutable_options()),
        };
        let sessions = MySqlStore::new(pool.clone());
        let instance_id = new_instance_id();
        Self {
            pool,
            settings: Arc::new(settings),
            sessions,
            instance_id,
        }
    }
}

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
}
