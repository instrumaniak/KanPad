//! Shared application state (Clone; pool + session store handle + instance id).
//!
//! The pool is created **lazy** (`connect_lazy`): boot never fails on a DB
//! outage, so `GET /health` stays `200` while `GET /ready` reports `503`
//! (see `10-architecture.md` Health). Boot connection errors surface through
//! readiness, never through panics, and never with credentials attached.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use sqlx::MySqlPool;

use crate::{common::session_store::MySqlSessionStore, config::Settings, db, metrics::Metrics};

/// [`AppState::db_state`] — no probe has completed yet. The FIRST observed
/// failure is therefore a real transition (`Unknown → Down`) and logs at
/// `warn`: a `warn`-mode server that comes up with the DB already down
/// must still get ONE log line.
pub const DB_PROBE_UNKNOWN: u8 = 0;
/// [`AppState::db_state`] — the last completed probe reached the database.
pub const DB_PROBE_UP: u8 = 1;
/// [`AppState::db_state`] — the last completed probe failed.
pub const DB_PROBE_DOWN: u8 = 2;

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
    /// Per-state Prometheus registry (`/metrics`, T60).
    pub metrics: Arc<Metrics>,
    /// Tri-state last-probe outcome (`DB_PROBE_*`), shared by every clone
    /// of this state. Written by [`Self::record_db_probe`] — the SINGLE
    /// transition logger (`Unknown→Down` = `warn` bootstrap,
    /// `Up→Down` = `error`, `Down→Up` = `info` recovery) — so a flapping
    /// database logs on TRANSITION, not on every probe.
    pub db_state: Arc<AtomicU8>,
}

impl AppState {
    /// Build state from settings without I/O (test-friendly): pool connect
    /// and store migration never run here.
    pub fn from_settings(settings: Settings) -> Self {
        // `connect_options` is the SAME parse the boot-config log reports
        // from, so "what the console says" and "what the pool dials" cannot
        // drift apart.
        let pool = match settings.connect_options() {
            Some(options) => db::pool_options(&settings).connect_lazy_with(options),
            // Unreachable via `Settings::load` (a stricter parse check could
            // live in `validate`, but URL validity is the pool's decision):
            // fall back to an unroutable lazy pool so construction stays
            // total and the failure surfaces as `503` at `/ready`.
            None => db::pool_options(&settings).connect_lazy_with(unroutable_options()),
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
            metrics: Arc::new(Metrics::new()),
            db_state: Arc::new(AtomicU8::new(DB_PROBE_UNKNOWN)),
        }
    }

    /// Record one COMPLETED DB probe — the single owner of the
    /// transition log, the `kanpad_db_up` gauge and the
    /// `kanpad_db_connect_failures_total{class}` counter.
    ///
    /// Transitions (no per-probe spam — a steady DOWN logs nothing):
    /// * first observed failure (`Unknown→Down`): `warn` — the bootstrap
    ///   case a `DB_STARTUP_CHECK=warn` server specifically cares about;
    /// * `Up→Down`: `error` with the full cause;
    /// * `Down→Up`: `info` recovery.
    ///
    /// The log carries the SAME field set as the diagnostics kernel (`log_at`)
    /// — host, port, database, class, mysql_number, hint — and NEVER the
    /// password. Two concurrent probes cannot double-log a transition: only
    /// the compare-exchange winner logs (the loser's outcome is recorded in
    /// the metrics by its own call).
    pub fn record_db_probe(
        &self,
        outcome: Result<&db::ProbeReport, &db::Diagnostics>,
        target: &db::Target,
        elapsed: std::time::Duration,
    ) {
        let (new_state, log): (u8, Box<dyn FnOnce(u8)>) = match outcome {
            Ok(report) => {
                self.metrics.set_db_up(true);
                (
                    DB_PROBE_UP,
                    Box::new(move |previous: u8| {
                        if previous == DB_PROBE_DOWN {
                            tracing::info!(
                                database = target.database.as_deref().unwrap_or("<none>"),
                                server_version = %report.server_version,
                                elapsed_ms = %elapsed.as_millis(),
                                "database recovered"
                            );
                        }
                    }),
                )
            }
            Err(diagnostics) => {
                self.metrics.set_db_up(false);
                self.metrics
                    .record_db_connect_failure(diagnostics.class.label());
                let level = match self.db_state.load(Ordering::SeqCst) {
                    DB_PROBE_UP => tracing::Level::ERROR,
                    _ => tracing::Level::WARN,
                };
                (
                    DB_PROBE_DOWN,
                    Box::new(move |previous: u8| {
                        // Unknown→Down = warn (bootstrap), Up→Down = error,
                        // Down→Down = silent.
                        if previous != DB_PROBE_DOWN {
                            diagnostics.log_at(level, target, elapsed);
                        }
                    }),
                )
            }
        };
        // CAS: only the probe that flips the flag logs the transition (the
        // flag's load inside the closures is the PRE-transition value —
        // the same one the CAS compares against, so a racing winner's
        // store cannot produce a second, stale-transition log).
        let previous = self.db_state.load(Ordering::SeqCst);
        if self
            .db_state
            .compare_exchange(previous, new_state, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            log(previous);
        }
    }

    /// Acquire a pooled connection, recording the wait in
    /// `kanpad_db_pool_acquire_duration_seconds`.
    ///
    /// sqlx exposes NO pool metrics type (verified on 0.8.6 and 0.9: there is
    /// no `PoolMetrics` and no `metrics` feature), so the series is our own.
    /// `db::pool_options()` pairs this with `acquire_time_level(INFO)`.
    pub async fn acquire_timed(
        &self,
    ) -> Result<sqlx::pool::PoolConnection<sqlx::MySql>, sqlx::Error> {
        let started = std::time::Instant::now();
        let connection = self.pool.acquire().await?;
        self.metrics.record_acquire(started.elapsed());
        Ok(connection)
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
        let pool = db::pool_options(&Settings::default()).connect_lazy_with(options);
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

    /// T005: `record_db_probe` is the SINGLE transition logger —
    /// `Unknown→Down` warns ONCE (bootstrap), a steady DOWN is silent,
    /// `Down→Up` infos a recovery, `Up→Down` errors with the cause — and
    /// never logs the password. The flag and the metrics move with it.
    /// (`#[tokio::test]`: `from_settings` builds a lazy pool, which needs a
    /// runtime context.)
    #[tokio::test]
    async fn record_db_probe_logs_transitions_without_per_probe_spam() {
        let state = AppState::from_settings(Settings::default());
        let options: sqlx::mysql::MySqlConnectOptions =
            "mysql://kanpad:sekrit-do-not-log@127.0.0.1:1/kanpad?charset=utf8mb4"
                .parse()
                .expect("fixture URL parses");
        let target = db::Target::from_options(&options, db::DbSource::DatabaseUrl);
        let refused = db::diagnose(&sqlx::Error::Io(std::io::Error::from_raw_os_error(111)));
        let elapsed = std::time::Duration::from_millis(7);

        // 1. FIRST failure (Unknown→Down): warn + cause, password never
        //    logged, flag + metrics set.
        let logs = crate::test_support::capture_logs(|| {
            state.record_db_probe(Err(&refused), &target, elapsed)
        });
        assert_eq!(state.db_state.load(Ordering::SeqCst), DB_PROBE_DOWN);
        assert!(logs.contains("WARN"), "bootstrap warn: {logs}");
        assert!(logs.contains("Connection refused"), "cause: {logs}");
        assert!(logs.contains("class=\"refused\""), "{logs}");
        assert!(!logs.contains("sekrit-do-not-log"), "{logs}");
        let text = state.metrics.render(0, 0);
        assert!(text.contains("kanpad_db_up 0"), "{text}");
        assert!(
            text.contains("kanpad_db_connect_failures_total{class=\"refused\"} 1"),
            "{text}"
        );

        // 2. Steady DOWN: NO new log line (no per-probe spam), counter only.
        let logs = crate::test_support::capture_logs(|| {
            state.record_db_probe(Err(&refused), &target, elapsed)
        });
        assert!(logs.is_empty(), "steady DOWN must be silent: {logs:?}");
        assert_eq!(state.db_state.load(Ordering::SeqCst), DB_PROBE_DOWN);
        assert!(
            state
                .metrics
                .render(0, 0)
                .contains("kanpad_db_connect_failures_total{class=\"refused\"} 2"),
            "the counter still moves"
        );

        // 3. Recovery (Down→Up): info, gauge flips, flag moves.
        let report = db::ProbeReport {
            server_version: "8.4.11".to_string(),
            selected_database: Some("kanpad".to_string()),
            elapsed,
        };
        let logs = crate::test_support::capture_logs(|| {
            state.record_db_probe(Ok(&report), &target, elapsed)
        });
        assert_eq!(state.db_state.load(Ordering::SeqCst), DB_PROBE_UP);
        assert!(logs.contains("INFO"), "recovery info: {logs}");
        assert!(logs.contains("database recovered"), "{logs}");
        assert!(
            state.metrics.render(0, 0).contains("kanpad_db_up 1"),
            "gauge flips on recovery"
        );

        // 4. Up→Down: ERROR with the full cause.
        let logs = crate::test_support::capture_logs(|| {
            state.record_db_probe(Err(&refused), &target, elapsed)
        });
        assert_eq!(state.db_state.load(Ordering::SeqCst), DB_PROBE_DOWN);
        assert!(logs.contains("ERROR"), "outage error: {logs}");
        assert!(logs.contains("Connection refused"), "{logs}");
        assert!(!logs.contains("sekrit-do-not-log"), "{logs}");
    }
}
