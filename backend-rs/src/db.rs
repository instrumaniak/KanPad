//! Pool defaults, MySQL identifier rules (T03) and the DB connectivity
//! diagnostics kernel (T005).
//!
//! **No `query!` here, so no `.sqlx/` entries**: every statement in this
//! module is a RUNTIME `sqlx::query`/`query_as` — `.sqlx/` must stay
//! byte-identical. (The T005 kernel needs no SQL beyond a literal
//! `SELECT VERSION(), DATABASE()`.)
//!
//! The kernel answers three questions for every DB-touching entry point
//! (`serve`, `migrate *`, `create-admin`, `/ready`):
//!
//! 1. **Where will we dial?** — [`Target`], derived from the SAME parsed
//!    `MySqlConnectOptions` the pool is built from, never holding a
//!    password.
//! 2. **What went wrong?** — [`diagnose`] / [`diagnose_migrate`] map a raw
//!    `sqlx::Error` / `MigrateError` to a [`Diagnostics`] with a coarse
//!    [`DbClass`], the MySQL error number, and an actionable hint.
//! 3. **Should we retry, and for how long?** — [`wait_until_ready`] is the
//!    SINGLE owner of the retry policy: it retries only `transient`
//!    classes within a bounded budget.
//!
//! **Why the probe must NOT acquire through the pool** (T005, verified in
//! the installed `sqlx-core-0.9.0/src/pool/inner.rs:325-385` and upstream
//! launchbadge/sqlx #3808 / PR #3929): the pool's internal connect loop
//! retries `Io(ConnectionRefused)` with backoff until `acquire_timeout`,
//! then returns `Error::PoolTimedOut` **with the inner cause dropped** —
//! a pooled probe cannot even see the failure it is probing for, and out-
//! sources retry policy to the 5 s `acquire_timeout`. `probe` dials
//! DIRECTLY via `ConnectOptions::connect()` — the exact call the pool
//! itself makes at `inner.rs:350` — so the raw error class is available on
//! attempt 1. When upstream #3582 ships (`connect_timeout` +
//! last-error-in-`PoolTimedOut`), `probe` can be revisited; tracked
//! upstream, not required here.

use std::time::Duration;

use sqlx::ConnectOptions;
use sqlx::Connection;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

use crate::config::Settings;

/// Canonical pool defaults per 10-architecture.md DB pool, with the two
/// connection counts env-driven: `DB_MAX_CONNECTIONS` (default 10) and
/// `DB_MIN_CONNECTIONS` (default 1) — a production server's
/// `max_connections` is not knowable at build time. Validated in
/// `Settings::validate` (max >= 1, min <= max).
pub fn pool_options(settings: &Settings) -> MySqlPoolOptions {
    MySqlPoolOptions::new()
        .max_connections(settings.db_max_connections)
        .min_connections(settings.db_min_connections)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(30 * 60)))
        .test_before_acquire(true)
        // A slow acquire must also be a tracing event; `log::LevelFilter` is
        // the type sqlx expects.
        .acquire_time_level(log::LevelFilter::Info)
}

/// Whether `name` is usable as a plain database identifier.
///
/// The rule is STRICTER than MySQL itself: a database name starting with a
/// digit, or containing `-`, `.`, or a space, is legal SQL but rejected here.
///
/// **Why the rule still exists (it used to be the store's rule).** It was
/// inherited from `tower-sessions-sqlx-store`'s `with_schema_name`, which
/// interpolated the name into every statement and validated it. The
/// in-tree store (T65) takes the pool and interpolates **nothing**, so it no
/// longer needs the name to be identifier-safe — but the screen stays, for
/// two reasons that outlive the store: (a) the database name reaches
/// operator tooling that does not quote identifiers (`mysql -D`, dumps, the
/// runbook's `CREATE DATABASE`), where a name needing quoting is a trap;
/// (b) `Settings::validate` is the place where a bad name becomes a clean
/// boot error instead of a surprise at the first query. Mirrored here so
/// that screen can be expressed in one place.
pub fn is_valid_db_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_alphabetic() || first == '_')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

// ---------------------------------------------------------------------------
// T005 diagnostics kernel: Target / DbClass / Diagnostics / probe
// ---------------------------------------------------------------------------

/// Which configuration input produced a [`Target`].
///
/// A leftover `DB_*` in the environment is inert when `DATABASE_URL` wins —
/// the boot-config log says which input won, and the diagnostics carry the
/// same fact so an operator can see WHICH knob to fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbSource {
    /// An explicit `DATABASE_URL`.
    DatabaseUrl,
    /// The individual `DB_*` parts (no `DATABASE_URL` set).
    Parts,
    /// The POOL's own connect options (`/ready` probes what the pool
    /// actually dials — `AppState::with_pool` may be handed a pool whose
    /// target the settings cannot describe, e.g. a `#[sqlx::test]`
    /// per-test database).
    Pool,
}

impl DbSource {
    /// Human label for logs and CLI output — the exact string
    /// `Settings::log_boot_config` uses for `db_source`.
    pub fn label(self) -> &'static str {
        match self {
            Self::DatabaseUrl => "DATABASE_URL",
            Self::Parts => "DB_* parts",
            Self::Pool => "pool",
        }
    }
}

/// Where the process will actually dial, derived from the SAME parsed
/// [`MySqlConnectOptions`] the pool is built from.
///
/// **Never holds a password** — safe to log and to render in `Debug`. A
/// socket path replaces host:port in the output when the target is a unix
/// socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// MySQL host (or the unix-socket directory when [`Self::socket`] is
    /// set — sqlx treats a leading `/` host as a socket).
    pub host: String,
    /// MySQL port (meaningless for a socket target).
    pub port: u16,
    /// Unix socket path, when the target is one.
    pub socket: Option<std::path::PathBuf>,
    /// Database name, `None` when no default database is selected.
    pub database: Option<String>,
    /// Username (the password is NEVER part of a [`Target`]).
    pub username: String,
    /// Which configuration input produced this target.
    pub source: DbSource,
}

impl Target {
    /// Redact-and-derive: everything except the password, straight from the
    /// parsed options the pool was built from.
    pub fn from_options(options: &MySqlConnectOptions, source: DbSource) -> Self {
        Self {
            host: options.get_host().to_string(),
            port: options.get_port(),
            socket: options.get_socket().cloned(),
            database: options.get_database().map(str::to_string),
            username: options.get_username().to_string(),
            source,
        }
    }

    /// The `at …` half of the CLI target line: socket path when present,
    /// `host:port` otherwise.
    fn at(&self) -> String {
        match &self.socket {
            Some(socket) => socket.display().to_string(),
            None => format!("{}:{}", self.host, self.port),
        }
    }

    /// Derive the parsed connect options + the redacted dial description
    /// from settings — the SAME source the pool is built from
    /// (`config::Settings::connect_options` is the ONE parse site), so the
    /// probe, the `/ready` target line and the pool can never disagree
    /// about which database is in play.
    ///
    /// `None` when the URL does not parse at all (malformed URL or a
    /// percent-decode failure) — callers must treat that as a CONFIG
    /// failure, never dial it. (T005 step 5 additionally rejects such a
    /// URL at boot, after which this arm is defense-in-depth.)
    pub fn for_settings(settings: &Settings) -> Option<(MySqlConnectOptions, Self)> {
        let options = settings.connect_options()?;
        let source = if settings.database_url.is_some() {
            DbSource::DatabaseUrl
        } else {
            DbSource::Parts
        };
        Some((options.clone(), Self::from_options(&options, source)))
    }

    /// One human line naming the inspected database — used by `migrate info`
    /// (stdout, so a `RUST_LOG=error` operator still sees WHICH database was
    /// inspected). Never contains the password.
    pub fn describe_line(&self) -> String {
        format!(
            "database: {} at {} (user {:?}, source {})",
            self.database.as_deref().unwrap_or("<none>"),
            self.at(),
            self.username,
            self.source.label(),
        )
    }
}

/// Coarse, human-meaningful classification of a connection failure.
///
/// The class drives the retry policy (`Diagnostics::transient`), the
/// `kanpad_db_connect_failures_total{class}` metric label and the one-line
/// CLI summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbClass {
    /// MySQL 1045 — the server rejected the credentials.
    AccessDenied,
    /// MySQL 1044 — the user exists but is not granted this database.
    DbAccessDenied,
    /// MySQL 1049 — the database does not exist.
    UnknownDatabase,
    /// MySQL 1129 (blocked after too many failed attempts) / 1130
    /// (host not privileged by the grant tables).
    HostBlocked,
    /// MySQL 1040 / 1203 — server connection budget exhausted.
    TooManyConnections,
    /// MySQL 1251, or a `Protocol` error (sqlx surfaces
    /// `caching_sha2_password`/RSA handshake failures as `Protocol`, NOT as
    /// `Database` 1251).
    AuthPlugin,
    /// MySQL 1146 — table missing; for the migration journal this is the
    /// "never migrated" signal.
    MissingTable,
    /// `Io(ConnectionRefused)` — nothing is listening on host:port.
    Refused,
    /// `Io` timed out / network unreachable / reset, the dial-timeout
    /// wrapper, or `Error::PoolTimedOut`.
    Timeout,
    /// `Io(AddrNotAvailable)` and other `Io` kinds — the host did not
    /// resolve or is unusable.
    Dns,
    /// TLS negotiation failed.
    Tls,
    /// `Configuration` / `InvalidArgument` — the connection options are
    /// invalid (a config error, not an outage).
    Config,
    /// Anything else.
    Other,
}

impl DbClass {
    /// Human phrase for logs, the CLI summary and the metrics `class` label
    /// (a valid Prometheus label value — plain words, no punctuation).
    pub fn label(self) -> &'static str {
        match self {
            Self::AccessDenied => "access denied",
            Self::DbAccessDenied => "database access denied",
            Self::UnknownDatabase => "unknown database",
            Self::HostBlocked => "host blocked",
            Self::TooManyConnections => "too many connections",
            Self::AuthPlugin => "auth plugin",
            Self::MissingTable => "missing table",
            Self::Refused => "refused",
            Self::Timeout => "timeout",
            Self::Dns => "dns",
            Self::Tls => "tls",
            Self::Config => "config",
            Self::Other => "other",
        }
    }
}

/// Operator-facing hint constants — short and actionable.
///
/// **Every fact in these strings is verified against the tree** (the AGENTS
/// rule: a wrong hint compiled into the binary is worse than a generic one).
/// The `:3337` fact is `backend-rs/docker-compose.test.yml`'s publish port.
const HINT_ACCESS_DENIED: &str = "MySQL rejected the credentials: check DB_USERNAME / DB_PASSWORD (the password is never logged).";
const HINT_DB_ACCESS_DENIED: &str =
    "the user is not granted access to this database: GRANT … ON <db>.* TO <user>.";
const HINT_UNKNOWN_DATABASE: &str = "the database does not exist: CREATE DATABASE … CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci.";
const HINT_HOST_BLOCKED_1129: &str = "the server blocked this host after too many failed attempts: wait, or FLUSH HOSTS on the server.";
const HINT_HOST_BLOCKED_1130: &str = "the server does not allow this host: add a matching account (CREATE USER 'user'@'host' + GRANT …).";
const HINT_TOO_MANY_CONNECTIONS: &str =
    "the server is out of connections: raise MySQL max_connections or lower DB_MAX_CONNECTIONS.";
const HINT_AUTH_PLUGIN_1251: &str = "the account uses an auth plugin this client cannot negotiate.";
const HINT_AUTH_PLUGIN_PROTOCOL: &str = "the auth/handshake negotiation failed — sqlx surfaces `caching_sha2_password`/RSA handshake failures here, NOT as `Database` 1251.";
const HINT_MISSING_TABLE: &str =
    "table missing — for the migration journal this means the database has never been migrated.";
const HINT_REFUSED: &str = "nothing is listening on host:port — check DB_HOST/DB_PORT against the compose file / the runbook (the shared test container is :3337).";
const HINT_TIMEOUT: &str =
    "the server did not answer — firewall, wrong host, or MySQL still starting.";
const HINT_POOL_TIMED_OUT: &str = "the connection pool timed out waiting for a connection.";
const HINT_DNS: &str = "the host name did not resolve — check DB_HOST.";
const HINT_TLS: &str = "TLS negotiation failed: check the CA/ssl-mode of DATABASE_URL.";
const HINT_CONFIG: &str = "the connection options are invalid — see the boot configuration line.";
const HINT_OTHER: &str = "unexpected driver error: see `class`/`mysql_number` and the message.";
const HINT_JOURNAL_DIRTY: &str = "the migration journal is dirty: `DELETE FROM _sqlx_migrations WHERE version = <v> AND success = 0` (or drop and re-migrate dev data) — NEVER hand-edit a `success = 1` row (rows carry SHA-384 checksums).";
const HINT_JOURNAL_CHECKSUM: &str = "a migration file was edited in place after it was applied — checksum break; the applied DB must be dropped and re-migrated.";

/// Structured diagnosis of one failed DB connection attempt.
///
/// `message` carries the server text (or the driver text when no server
/// answered) — it is the diagnostic value and is allowed to name the user
/// and host; it never contains the password.
#[derive(Debug, Clone)]
pub struct Diagnostics {
    /// Coarse class — drives retry policy and the metrics label.
    pub class: DbClass,
    /// MySQL error number when the server answered (1045, 1049, …).
    pub mysql_number: Option<u16>,
    /// SQLSTATE when the server answered (28000, 42000, …).
    pub sqlstate: Option<String>,
    /// Server (or driver) text. Never contains the password.
    pub message: String,
    /// Whether `wait_until_ready` should retry this class.
    pub transient: bool,
    /// Actionable, verified hint text.
    pub hint: &'static str,
}

impl Diagnostics {
    /// A short precise code for the one-line CLI summary: the MySQL number
    /// when the server answered, a transport-derived short code otherwise,
    /// and nothing when the class label already says it all.
    pub fn code_label(&self) -> Option<String> {
        if let Some(number) = self.mysql_number {
            return Some(number.to_string());
        }
        match self.class {
            DbClass::Refused => Some("connection refused".to_string()),
            DbClass::Timeout => Some("timed out".to_string()),
            DbClass::Dns => Some("host unresolved".to_string()),
            _ => None,
        }
    }

    /// `{class} ({code})` — the one-line human summary (code omitted when
    /// redundant).
    pub fn describe(&self) -> String {
        match self.code_label() {
            Some(code) => format!("{} ({code})", self.class.label()),
            None => self.class.label().to_string(),
        }
    }

    /// Log the IDENTICAL field set from every call site — host, port,
    /// socket, database, username, source, elapsed_ms, class, mysql_number,
    /// sqlstate, transient, message, hint.
    ///
    /// **Never logs**: the password, `DB_PASSWORD`, `SESSION_SECRET`, or
    /// `Settings::database_url()` (the derived URL embeds the password
    /// verbatim).
    pub fn log(&self, target: &Target, elapsed: Duration) {
        self.log_at(tracing::Level::ERROR, target, elapsed);
    }

    /// [`Self::log`] with a caller-chosen level — the transition logger
    /// (`AppState::record_db_probe`) renders the FIRST observed failure as
    /// `warn` (bootstrap) and `Up→Down` as `error`, from the same field set;
    /// the CLI's `DB_STARTUP_CHECK=warn` path logs the identical fields at
    /// `warn`. Public because the `backend-rs` BINARY is a separate crate
    /// from the library and cannot see `pub(crate)`.
    pub fn log_at(&self, level: tracing::Level, target: &Target, elapsed: Duration) {
        macro_rules! event {
            ($level_macro:ident) => {{
                tracing::$level_macro!(
                    host = %target.host,
                    port = target.port,
                    socket = ?target.socket,
                    database = target.database.as_deref().unwrap_or("<none>"),
                    username = %target.username,
                    source = target.source.label(),
                    elapsed_ms = %elapsed.as_millis(),
                    class = self.class.label(),
                    mysql_number = self.mysql_number,
                    sqlstate = self.sqlstate.as_deref(),
                    transient = self.transient,
                    hint = self.hint,
                    "{}", self.message,
                );
            }};
        }
        match level {
            tracing::Level::ERROR => event!(error),
            tracing::Level::WARN => event!(warn),
            _ => event!(info),
        }
    }

    /// The dial-timeout wrapper diagnosis (no server ever answered).
    fn dial_timeout(elapsed_cap: Duration) -> Self {
        Self {
            class: DbClass::Timeout,
            mysql_number: None,
            sqlstate: None,
            message: format!(
                "the dial did not answer within {} ms",
                elapsed_cap.as_millis()
            ),
            transient: true,
            hint: HINT_TIMEOUT,
        }
    }
}

/// One successful direct-dial probe result.
#[derive(Debug, Clone)]
pub struct ProbeReport {
    /// `SELECT VERSION()` — the server the process actually reached.
    pub server_version: String,
    /// `SELECT DATABASE()` — the default database the server picked (may
    /// differ from the configured name; the boot path warns on mismatch).
    pub selected_database: Option<String>,
    /// Dial + handshake + version-query time.
    pub elapsed: Duration,
}

/// Dials DIRECTLY (one attempt, optionally bounded) and reports the server
/// version + selected database. The handshake is the SAME one the pool
/// makes (`caching_sha2_password` + `mysql-rsa`), so a probe that succeeds
/// here proves the pool can too.
async fn dial_and_report(
    options: &MySqlConnectOptions,
    dial_timeout: Option<Duration>,
) -> Result<ProbeReport, Diagnostics> {
    let started = std::time::Instant::now();
    let connect = options.connect();
    let connection = match dial_timeout {
        Some(per_attempt) => match tokio::time::timeout(per_attempt, connect).await {
            Ok(result) => result,
            Err(_elapsed) => return Err(Diagnostics::dial_timeout(per_attempt)),
        },
        None => connect.await,
    };
    let mut connection = connection.map_err(|err| diagnose(&err))?;
    // `(String, Option<String>)`: VERSION() is never NULL, DATABASE() is
    // NULL when no default database is selected.
    let row: (String, Option<String>) = sqlx::query_as("SELECT VERSION(), DATABASE()")
        .fetch_one(&mut connection)
        .await
        .map_err(|err| diagnose(&err))?;
    let elapsed = started.elapsed();
    connection.close().await.map_err(|err| diagnose(&err))?;
    Ok(ProbeReport {
        server_version: row.0,
        selected_database: row.1,
        elapsed,
    })
}

/// One direct-dial probe WITHOUT a timeout bound — a single attempt that
/// classifies any failure. Callers that need bounds use [`wait_until_ready`]
/// (which wraps each attempt the same way); this public form is for
/// one-shot callers that already run under their own deadline.
pub async fn probe(
    options: &MySqlConnectOptions,
    target: &Target,
) -> Result<ProbeReport, Diagnostics> {
    let _ = target; // carried for signature symmetry and future logging use
    dial_and_report(options, None).await
}

/// Probe until the database answers — retrying ONLY transient classes, with
/// the total spend bounded by `budget`.
///
/// Retry policy (the SINGLE owner — no pool retries hide it, no pool
/// `acquire_timeout` out-sources it):
///
/// - each attempt dials DIRECTLY via `ConnectOptions::connect()`, bounded
///   by `min(dial_timeout, remaining budget)` — so the whole loop finishes
///   within `budget` even on a SYN-hanging unroutable host;
/// - attempt 1 fires immediately; retry ONLY when `diagnostics.transient`;
/// - backoff 250 ms → ×2, capped at 2 s;
/// - a **permanent** error returns after exactly ONE attempt (a wrong
///   password answers in ~ms and must not burn the budget).
///
/// `dial_timeout` is an INDEPENDENT knob (boot callers pass ~2 s): it caps
/// a single attempt, while `budget` caps the total. Returns
/// `(report, attempts)` on success and `(last diagnostics, attempts)` on
/// failure — the attempt count is the evidence for the "wrong password
/// fails in one attempt / refused port retries" ACs.
pub async fn wait_until_ready(
    options: &MySqlConnectOptions,
    target: &Target,
    budget: Duration,
    dial_timeout: Duration,
) -> Result<(ProbeReport, u32), (Diagnostics, u32)> {
    let _ = target; // signature symmetry: callers log the failure with it
    let started = std::time::Instant::now();
    let mut backoff = Duration::from_millis(250);
    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        let remaining = budget.saturating_sub(started.elapsed());
        let per_attempt = dial_timeout.min(remaining);
        let outcome = dial_and_report(options, Some(per_attempt)).await;
        match outcome {
            Ok(report) => return Ok((report, attempts)),
            Err(diagnostics) => {
                if !diagnostics.transient {
                    return Err((diagnostics, attempts));
                }
                // Sleep only when the backoff still fits the budget;
                // otherwise the budget is exhausted — return the last cause.
                let remaining = budget.saturating_sub(started.elapsed());
                if backoff >= remaining {
                    return Err((diagnostics, attempts));
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(2));
            }
        }
    }
}

/// Classify a MySQL error number (the server answered). Split from
/// [`diagnose`] so the number→(class, transient, hint) matrix is testable
/// without a live server — sqlx 0.9's `dyn DatabaseError` downcasts to
/// `MySqlDatabaseError` only for REAL driver errors, so unit-test stubs
/// cannot exercise this branch through the public entry point (the CLI
/// suite covers it end-to-end against the shared test container).
fn mysql_diagnostics(number: u16, sqlstate: Option<&str>, message: String) -> Diagnostics {
    let (class, transient, hint) = match number {
        1045 => (DbClass::AccessDenied, false, HINT_ACCESS_DENIED),
        1044 => (DbClass::DbAccessDenied, false, HINT_DB_ACCESS_DENIED),
        1049 => (DbClass::UnknownDatabase, false, HINT_UNKNOWN_DATABASE),
        1129 => (DbClass::HostBlocked, false, HINT_HOST_BLOCKED_1129),
        1130 => (DbClass::HostBlocked, false, HINT_HOST_BLOCKED_1130),
        1040 | 1203 => (DbClass::TooManyConnections, true, HINT_TOO_MANY_CONNECTIONS),
        1251 => (DbClass::AuthPlugin, false, HINT_AUTH_PLUGIN_1251),
        1146 => (DbClass::MissingTable, false, HINT_MISSING_TABLE),
        _ => (DbClass::Other, false, HINT_OTHER),
    };
    Diagnostics {
        class,
        mysql_number: Some(number),
        sqlstate: sqlstate.map(str::to_string),
        message,
        transient,
        hint,
    }
}

/// Classify a raw `sqlx::Error` into a [`Diagnostics`].
///
/// MySQL numbers verified against the MySQL 8.4 error reference and
/// `sqlx-mysql-0.9.0`: 1045, 1044, 1049, 1129, 1130, 1040, 1203, 1251, 1146.
pub fn diagnose(err: &sqlx::Error) -> Diagnostics {
    match err {
        sqlx::Error::Database(boxed) => {
            match boxed.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>() {
                Some(mysql_err) => mysql_diagnostics(
                    mysql_err.number(),
                    mysql_err.code(),
                    mysql_err.message().to_string(),
                ),
                // A non-MySQL Database error (not reachable in this crate):
                // keep the SQLSTATE + message, classify Other.
                None => Diagnostics {
                    class: DbClass::Other,
                    mysql_number: None,
                    sqlstate: boxed.code().map(|code| code.into_owned()),
                    message: boxed.message().to_string(),
                    transient: false,
                    hint: HINT_OTHER,
                },
            }
        }
        sqlx::Error::Io(io_err) => {
            let message = io_err.to_string();
            match io_err.kind() {
                std::io::ErrorKind::ConnectionRefused => Diagnostics {
                    class: DbClass::Refused,
                    mysql_number: None,
                    sqlstate: None,
                    message,
                    transient: true,
                    hint: HINT_REFUSED,
                },
                std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::HostUnreachable
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted => Diagnostics {
                    class: DbClass::Timeout,
                    mysql_number: None,
                    sqlstate: None,
                    message,
                    transient: true,
                    hint: HINT_TIMEOUT,
                },
                // `AddrNotAvailable` (the host is unusable) and every other
                // Io kind land here — the host did not resolve.
                _ => Diagnostics {
                    class: DbClass::Dns,
                    mysql_number: None,
                    sqlstate: None,
                    message,
                    transient: true,
                    hint: HINT_DNS,
                },
            }
        }
        sqlx::Error::Tls(source) => Diagnostics {
            class: DbClass::Tls,
            mysql_number: None,
            sqlstate: None,
            message: source.to_string(),
            transient: false,
            hint: HINT_TLS,
        },
        // sqlx surfaces `caching_sha2_password`/RSA handshake failures as
        // `Protocol`, NOT as `Database` 1251 — hence the same class with a
        // handshake-specific hint.
        sqlx::Error::Protocol(text) => Diagnostics {
            class: DbClass::AuthPlugin,
            mysql_number: None,
            sqlstate: None,
            message: text.clone(),
            transient: false,
            hint: HINT_AUTH_PLUGIN_PROTOCOL,
        },
        sqlx::Error::Configuration(source) => Diagnostics {
            class: DbClass::Config,
            mysql_number: None,
            sqlstate: None,
            message: source.to_string(),
            transient: false,
            hint: HINT_CONFIG,
        },
        sqlx::Error::InvalidArgument(text) => Diagnostics {
            class: DbClass::Config,
            mysql_number: None,
            sqlstate: None,
            message: text.clone(),
            transient: false,
            hint: HINT_CONFIG,
        },
        sqlx::Error::PoolTimedOut => Diagnostics {
            class: DbClass::Timeout,
            mysql_number: None,
            sqlstate: None,
            message: "the connection pool timed out waiting for a connection".to_string(),
            transient: true,
            hint: HINT_POOL_TIMED_OUT,
        },
        other => Diagnostics {
            class: DbClass::Other,
            mysql_number: None,
            sqlstate: None,
            message: other.to_string(),
            transient: false,
            hint: HINT_OTHER,
        },
    }
}

/// Classify a raw `MigrateError`.
///
/// `Execute`/`ExecuteMigration` HOLD a real `sqlx::Error` (`#[from]` /
/// `#[source]`) — unwrap it directly, no downcast, so connection failures
/// get the full [`diagnose`] treatment. The NON-connection variants are
/// journal-integrity errors: they must NEVER render as "fresh database"
/// and never suggest hand-editing a `success = 1` journal row (rows carry
/// SHA-384 checksums). Only the journal-absent MySQL 1146 path classifies
/// as `MissingTable` (that is what `migrate info` keys the "fresh
/// database" message on — sqlx runs a bare `SELECT … FROM _sqlx_migrations`
/// with no table-existence check, `sqlx-mysql-0.9.0/src/migrate.rs`).
pub fn diagnose_migrate(err: &sqlx::migrate::MigrateError) -> Diagnostics {
    match err {
        sqlx::migrate::MigrateError::Execute(inner) => diagnose(inner),
        sqlx::migrate::MigrateError::ExecuteMigration(inner, _version) => diagnose(inner),
        sqlx::migrate::MigrateError::Dirty(version) => Diagnostics {
            class: DbClass::Other,
            mysql_number: None,
            sqlstate: None,
            message: format!("the migration journal is dirty at version {version}"),
            transient: false,
            hint: HINT_JOURNAL_DIRTY,
        },
        sqlx::migrate::MigrateError::VersionMismatch(version) => Diagnostics {
            class: DbClass::Other,
            mysql_number: None,
            sqlstate: None,
            message: format!("migration {version} was previously applied but has been modified"),
            transient: false,
            hint: HINT_JOURNAL_CHECKSUM,
        },
        sqlx::migrate::MigrateError::VersionMissing(version) => Diagnostics {
            class: DbClass::Other,
            mysql_number: None,
            sqlstate: None,
            message: format!(
                "migration {version} is recorded as applied but is missing from the resolved migrations"
            ),
            transient: false,
            hint: HINT_JOURNAL_CHECKSUM,
        },
        // `Source`, `VersionNotPresent`, `VersionTooOld/TooNew`,
        // `ForceNotSupported`, `CreateSchemasNotSupported`, … — shape
        // errors, not connectivity: carry the real message, no retry.
        other => Diagnostics {
            class: DbClass::Other,
            mysql_number: None,
            sqlstate: None,
            message: other.to_string(),
            transient: false,
            hint: HINT_OTHER,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_db_name;
    use crate::config::Settings;

    #[test]
    fn pool_options_honour_settings() {
        let settings = Settings {
            db_max_connections: 3,
            db_min_connections: 2,
            ..Settings::default()
        };
        let options = super::pool_options(&settings);
        assert_eq!(options.get_max_connections(), 3);
        assert_eq!(options.get_min_connections(), 2);

        let defaults = super::pool_options(&Settings::default());
        assert_eq!(defaults.get_max_connections(), 10, "default max unchanged");
        assert_eq!(defaults.get_min_connections(), 1, "default min unchanged");
    }

    #[test]
    fn accepts_identifiers_the_store_accepts() {
        for name in [
            "kanpad",         // the default DB_NAME
            "_sqlx_test_abc", // #[sqlx::test] per-test databases
            "_leading",
            "with_underscores_123",
            "dollar$sign",
            "kanpad_prod_1",
        ] {
            assert!(is_valid_db_name(name), "{name:?} must be accepted");
        }
    }

    #[test]
    fn rejects_names_mysql_allows_but_the_store_does_not() {
        // These are legal MySQL schema names but unusable as plain
        // identifiers, so `Settings::validate` must reject them — otherwise
        // the first operator tool that does not quote identifiers breaks on
        // a name that the boot screen let through.
        for name in [
            "123kanpad", // leading digit
            "kan-pad",   // hyphen needs quoting
            "kan.pad",   // dot needs quoting
            "kan pad",   // space needs quoting
            "",          // empty
            "kan@pad",
        ] {
            assert!(!is_valid_db_name(name), "{name:?} must be rejected");
        }
    }

    #[test]
    fn non_ascii_letters_count_as_alphabetic_like_the_store() {
        // The crate checks `char::is_alphabetic`, which is Unicode-aware;
        // mirror it exactly rather than restricting to ASCII.
        assert!(is_valid_db_name("kanpäd"));
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use std::time::Duration;

    use super::{DbClass, Target, diagnose, mysql_diagnostics};

    /// Mirrors the `StubDatabaseError` pattern from `src/error.rs` tests —
    /// per-module unit-test stubs are the sanctioned shape (the shared
    /// harness rule governs `tests/api/*` plumbing, not unit-test stubs).
    #[derive(Debug)]
    struct StubDatabaseError {
        sqlstate: Option<&'static str>,
        message: &'static str,
    }

    impl std::fmt::Display for StubDatabaseError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "stub db error: {}", self.message)
        }
    }

    impl std::error::Error for StubDatabaseError {}

    impl sqlx::error::DatabaseError for StubDatabaseError {
        fn message(&self) -> &str {
            self.message
        }

        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.sqlstate.map(std::borrow::Cow::Borrowed)
        }

        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }

        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
    }

    fn stub_db_error(sqlstate: Option<&'static str>, message: &'static str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(StubDatabaseError { sqlstate, message }))
    }

    fn io_error(kind: std::io::ErrorKind) -> sqlx::Error {
        // `ErrorKind` alone cannot be constructed into an io::Error; raw OS
        // error numbers yield the kind (Linux x86: ECONNREFUSED = 111,
        // ETIMEDOUT = 110 — `libc` is only a transitive dep here).
        let err = match kind {
            std::io::ErrorKind::ConnectionRefused => std::io::Error::from_raw_os_error(111),
            std::io::ErrorKind::TimedOut => std::io::Error::from_raw_os_error(110),
            _ => std::io::Error::other("stub io error"),
        };
        sqlx::Error::Io(err)
    }

    /// The number → (class, transient, hint) matrix — the RETRY POLICY table
    /// from the spec, verified number by number. `mysql_diagnostics` is the
    /// pure split of `diagnose` (real `MySqlDatabaseError` downcasts are
    /// covered end-to-end by the `migrate_cli` binary suite).
    #[test]
    fn mysql_number_matrix_classifies_and_pins_retry_policy() {
        // (number, class, transient)
        let cases: &[(u16, DbClass, bool)] = &[
            (1045, DbClass::AccessDenied, false),
            (1044, DbClass::DbAccessDenied, false),
            (1049, DbClass::UnknownDatabase, false),
            (1129, DbClass::HostBlocked, false),
            (1130, DbClass::HostBlocked, false),
            (1040, DbClass::TooManyConnections, true),
            (1203, DbClass::TooManyConnections, true),
            (1251, DbClass::AuthPlugin, false),
            (1146, DbClass::MissingTable, false),
        ];
        for &(number, class, transient) in cases {
            let diagnostics = mysql_diagnostics(number, Some("28000"), "stub message".to_string());
            assert_eq!(diagnostics.class, class, "number {number}");
            assert_eq!(diagnostics.mysql_number, Some(number));
            assert_eq!(diagnostics.sqlstate.as_deref(), Some("28000"));
            assert_eq!(diagnostics.transient, transient, "number {number}");
            assert!(
                !diagnostics.hint.is_empty(),
                "number {number} must carry a hint"
            );
        }
    }

    /// Unknown MySQL numbers classify Other, non-transient, with the number
    /// still surfaced.
    #[test]
    fn unknown_mysql_number_falls_back_to_other() {
        let diagnostics = mysql_diagnostics(1907, None, "stub".to_string());
        assert_eq!(diagnostics.class, DbClass::Other);
        assert!(!diagnostics.transient);
        assert!(diagnostics.mysql_number.is_some());
    }

    /// Transport errors (Io / Tls / Config / Protocol / PoolTimedOut) —
    /// refused is transient; everything the spec pins as permanent stays
    /// permanent.
    #[test]
    fn transport_errors_classify_by_shape() {
        let refused = diagnose(&io_error(std::io::ErrorKind::ConnectionRefused));
        assert_eq!(refused.class, DbClass::Refused);
        assert!(refused.transient, "a refused port must be retried");
        assert_eq!(refused.code_label().as_deref(), Some("connection refused"));

        let timed_out = diagnose(&io_error(std::io::ErrorKind::TimedOut));
        assert_eq!(timed_out.class, DbClass::Timeout);
        assert!(timed_out.transient);

        let tls = diagnose(&sqlx::Error::Tls("stub tls failure".into()));
        assert_eq!(tls.class, DbClass::Tls);
        assert!(!tls.transient, "TLS negotiation failure is permanent");

        let protocol = diagnose(&sqlx::Error::Protocol("stub handshake failure".to_string()));
        assert_eq!(protocol.class, DbClass::AuthPlugin);
        assert!(!protocol.transient);

        let config = diagnose(&sqlx::Error::Configuration("stub config".into()));
        assert_eq!(config.class, DbClass::Config);
        assert!(!config.transient);

        let invalid = diagnose(&sqlx::Error::InvalidArgument("stub invalid".to_string()));
        assert_eq!(invalid.class, DbClass::Config);
        assert!(!invalid.transient);

        let pool = diagnose(&sqlx::Error::PoolTimedOut);
        assert_eq!(pool.class, DbClass::Timeout);
        assert!(pool.transient);
    }

    /// A `Database` error that is NOT a real `MySqlDatabaseError` (the stub)
    /// must fall back to Other with the SQLSTATE kept — never panic.
    #[test]
    fn non_mysql_database_error_falls_back_to_other() {
        let diagnostics = diagnose(&stub_db_error(Some("HY000"), "stub server text"));
        assert_eq!(diagnostics.class, DbClass::Other);
        assert_eq!(diagnostics.sqlstate.as_deref(), Some("HY000"));
        assert_eq!(diagnostics.message, "stub server text");
        assert!(!diagnostics.transient);
    }

    /// `diagnose_migrate`: journal-integrity variants carry guard hints that
    /// never say "fresh database" and never suggest hand-editing a
    /// `success = 1` row; connection variants unwrap to the full diagnosis.
    #[test]
    fn migrate_errors_are_journal_guards_not_fresh_database() {
        let dirty = super::diagnose_migrate(&sqlx::migrate::MigrateError::Dirty(3));
        assert_eq!(dirty.class, DbClass::Other);
        assert!(dirty.message.contains("3"), "{:?}", dirty.message);
        assert!(dirty.hint.contains("DELETE FROM _sqlx_migrations"));
        assert!(dirty.hint.contains("success = 0"));
        assert!(dirty.hint.contains("NEVER hand-edit"));
        assert!(!dirty.hint.contains("fresh database"));

        let mismatch = super::diagnose_migrate(&sqlx::migrate::MigrateError::VersionMismatch(2));
        assert_eq!(mismatch.class, DbClass::Other);
        assert!(
            mismatch.hint.contains("checksum break"),
            "{}",
            mismatch.hint
        );
        assert!(!mismatch.hint.contains("fresh database"));

        let missing = super::diagnose_migrate(&sqlx::migrate::MigrateError::VersionMissing(1));
        assert_eq!(missing.class, DbClass::Other);
        assert!(missing.hint.contains("dropped and re-migrated"));

        // `Execute(#[from] sqlx::Error)` unwraps to the FULL connection
        // diagnosis (refused → Refused, transient).
        let executed = super::diagnose_migrate(&sqlx::migrate::MigrateError::Execute(io_error(
            std::io::ErrorKind::ConnectionRefused,
        )));
        assert_eq!(executed.class, DbClass::Refused);
        assert!(executed.transient);
    }

    /// `Target::from_options` derives the redacted dial description from the
    /// SAME parsed options the pool uses, and the rendered forms never carry
    /// the password.
    #[test]
    fn target_from_options_is_redacted_and_complete() {
        let options: sqlx::mysql::MySqlConnectOptions =
            "mysql://parts_user:sekrit-do-not-log@db.parts.internal:3337/kanpad_parts\
             ?charset=utf8mb4&collation=utf8mb4_unicode_ci"
                .parse()
                .expect("the fixture URL must parse");
        let target = Target::from_options(&options, super::DbSource::DatabaseUrl);
        assert_eq!(target.host, "db.parts.internal");
        assert_eq!(target.port, 3337);
        assert_eq!(target.database.as_deref(), Some("kanpad_parts"));
        assert_eq!(target.username, "parts_user");
        assert_eq!(target.socket, None);
        assert_eq!(target.source.label(), "DATABASE_URL");

        // The Debug render (used in logs) and the CLI line carry no secret.
        let rendered = format!("{target:?}");
        assert!(!rendered.contains("sekrit-do-not-log"), "{rendered}");
        let line = target.describe_line();
        assert!(line.contains("db.parts.internal:3337"), "{line}");
        assert!(line.contains("user \"parts_user\""), "{line}");
        assert!(line.contains("source DATABASE_URL"), "{line}");
        assert!(!line.contains("sekrit-do-not-log"), "{line}");
    }

    /// Every diagnostics log line carries the SAME field set, the cause, and
    /// never the password.
    #[test]
    fn diagnostics_log_renders_the_full_field_set_without_secrets() {
        let options: sqlx::mysql::MySqlConnectOptions =
            "mysql://log_user:sekrit-password-x@127.0.0.1:1/kanpad?charset=utf8mb4"
                .parse()
                .expect("the fixture URL must parse");
        let target = Target::from_options(&options, super::DbSource::Parts);
        let diagnostics = diagnose(&io_error(std::io::ErrorKind::ConnectionRefused));

        let logs = crate::test_support::capture_logs(|| {
            diagnostics.log(&target, Duration::from_millis(7))
        });
        // Human format: `%`-recorded values render unquoted, `str`-typed
        // values quoted (the JSON formatter renders both as strings).
        for field in [
            "host=127.0.0.1",
            "port=1",
            "username=log_user",
            "source=\"DB_* parts\"",
            "elapsed_ms=7",
            "class=\"refused\"",
            "transient=true",
            "Connection refused",
            "nothing is listening",
        ] {
            assert!(logs.contains(field), "missing {field:?} in {logs}");
        }
        assert!(!logs.contains("sekrit-password-x"), "{logs}");
        assert!(!logs.contains("mysql://"), "{logs}");
    }

    /// The one-line CLI summary: MySQL number in parens when the server
    /// answered, transport short-code otherwise, nothing when redundant.
    #[test]
    fn describe_and_code_label_render_the_one_line_summary() {
        let with_number = mysql_diagnostics(1045, Some("28000"), "stub".to_string());
        assert_eq!(with_number.describe(), "access denied (1045)");
        assert_eq!(with_number.code_label().as_deref(), Some("1045"));

        let refused = diagnose(&io_error(std::io::ErrorKind::ConnectionRefused));
        assert_eq!(refused.describe(), "refused (connection refused)");

        let other = mysql_diagnostics(1907, None, "stub".to_string());
        assert_eq!(other.describe(), "other (1907)");
    }

    /// `wait_until_ready`: a REFUSED port is transient → it retries within
    /// the budget, and the total elapsed stays bounded by the budget even
    /// though `dial_timeout` is larger (the SYN-hang shape).
    #[tokio::test]
    async fn wait_until_ready_retries_refused_within_the_budget() {
        let options: sqlx::mysql::MySqlConnectOptions =
            "mysql://kanpad:stub@127.0.0.1:1/kanpad?charset=utf8mb4"
                .parse()
                .expect("the fixture URL must parse");
        let target = Target::from_options(&options, super::DbSource::Parts);
        let started = std::time::Instant::now();
        let result = super::wait_until_ready(
            &options,
            &target,
            Duration::from_secs(1),
            Duration::from_secs(2), // LARGER than the budget on purpose
        )
        .await;
        let elapsed = started.elapsed();
        let (diagnostics, attempts) = result.expect_err("port 1 must refuse");
        assert_eq!(diagnostics.class, DbClass::Refused, "{diagnostics:?}");
        assert!(
            attempts > 1,
            "a refused port must retry within the budget, got {attempts}"
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "the budget (1 s) must bound the loop even with a 2 s dial_timeout: {elapsed:?}"
        );
    }

    /// The /ready shape: a 500 ms budget stays under the kubelet's 1 s
    /// `timeoutSeconds` default on any failure shape.
    #[tokio::test]
    async fn wait_until_ready_honours_the_small_readiness_budget() {
        let options: sqlx::mysql::MySqlConnectOptions =
            "mysql://kanpad:stub@127.0.0.1:1/kanpad?charset=utf8mb4"
                .parse()
                .expect("the fixture URL must parse");
        let target = Target::from_options(&options, super::DbSource::Parts);
        let started = std::time::Instant::now();
        let result = super::wait_until_ready(
            &options,
            &target,
            Duration::from_millis(500),
            Duration::from_millis(500),
        )
        .await;
        let elapsed = started.elapsed();
        let (diagnostics, _) = result.expect_err("port 1 must refuse");
        assert_eq!(diagnostics.class, DbClass::Refused);
        assert!(
            elapsed < Duration::from_millis(900),
            "a 500 ms budget must stay under the 1 s kubelet default: {elapsed:?}"
        );
    }

    /// One successful direct-dial probe reports the server version and the
    /// selected database — gated on `DATABASE_URL` like
    /// `health.rs::ready_200_when_db_up` (skip+pass when unset/unparsable).
    #[tokio::test]
    async fn probe_reports_the_server_version_when_the_db_is_up() {
        let Some(database_url) = std::env::var("DATABASE_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .and_then(|url| url.parse::<sqlx::mysql::MySqlConnectOptions>().ok())
        else {
            eprintln!(
                "SKIP probe_reports_the_server_version_when_the_db_is_up: DATABASE_URL unset or unparsable"
            );
            return;
        };
        let target = Target::from_options(&database_url, super::DbSource::DatabaseUrl);
        let report = super::probe(&database_url, &target)
            .await
            .expect("the shared test database must answer");
        assert!(!report.server_version.is_empty());
        assert!(
            report.selected_database.is_some(),
            "the shared DATABASE_URL selects a database"
        );
    }
}
