//! Binary boundary: clap CLI + boot + graceful shutdown.
//! `anyhow` lives only here, never in the library.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use sqlx::{MySqlPool, migrate::Migrate};
use tower_sessions::session_store::ExpiredDeletion;

use backend_rs::{
    build_state, config::Settings, create_app_with_csp, db, init_tracing_from_env, metrics_addr,
    metrics_router,
};

/// KanPad Rust API server.
#[derive(Debug, Parser)]
#[command(name = "backend-rs", version)]
struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    command: Commands,
}

/// CLI subcommands per 10-architecture.md.
#[derive(Debug, Subcommand)]
enum Commands {
    /// Serve the API (never auto-migrates unless `--auto-migrate`).
    Serve {
        /// Run pending migrations at boot (dev/compose only).
        #[arg(long, default_value_t = false)]
        auto_migrate: bool,
    },
    /// Migration operations.
    Migrate {
        /// Operation to perform.
        #[command(subcommand)]
        op: MigrateOp,
    },
    /// Create (or reset) the admin user (wired in T12).
    CreateAdmin {
        /// Admin email address (prompted interactively when omitted).
        #[arg(long)]
        email: Option<String>,
        /// Admin password (prompted hidden when omitted; prefer the prompt —
        /// the flag value stays visible in shell history).
        #[arg(long)]
        password: Option<String>,
        /// Reset the existing admin's password instead of refusing.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Print `openapi.json` to stdout (boots no pool, no sessions).
    ExportOpenapi,
}

/// `migrate` operations.
#[derive(Debug, Subcommand)]
enum MigrateOp {
    /// Apply pending migrations.
    Run,
    /// Show applied/pending migrations.
    Info,
    /// Revert the last migration (dev only, never prod).
    Revert,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve { auto_migrate } => serve(auto_migrate).await,
        Commands::Migrate { op } => migrate(op).await,
        Commands::CreateAdmin {
            email,
            password,
            force,
        } => create_admin(email, password, force).await,
        Commands::ExportOpenapi => export_openapi().await,
    }
}

/// Upper bound on the post-drain pool close: `Pool::close()` waits for
/// checked-out connections, and one stuck query must not hang shutdown
/// forever.
const POOL_CLOSE_TIMEOUT: Duration = Duration::from_secs(10);

async fn serve(auto_migrate: bool) -> Result<()> {
    init_tracing_from_env();
    let settings = Settings::load().context("failed to load settings")?;
    let state = build_state(settings);
    // T005 (step 4): fail fast on an unreachable database — BEFORE the
    // session sweeper spawns, before bind, before `--auto-migrate`. The
    // probe dials directly (cause + class visible on attempt 1); a pooled
    // acquire would retry internally and surface a causeless
    // `PoolTimedOut`. `DB_STARTUP_CHECK=warn` continues here so a DB-less
    // server can serve degraded (/health 200, /ready 503) — but it never
    // exempts a migration (see below).
    let Some((options, target)) = db::Target::for_settings(&state.settings) else {
        bail!(
            "cannot connect to the database: the configured DATABASE_URL does not parse as a \
             MySQL connection URL — check the URL syntax (the password must be percent-encoded; \
             it is never logged)"
        );
    };
    let probe_report = startup_db_probe(&state.settings, &options, &target, Some(&state)).await?;
    if auto_migrate {
        if state.settings.is_production() {
            bail!("--auto-migrate is dev/compose only; refusing in production");
        }
        // `DB_STARTUP_CHECK=warn` buys a DB-less server, never a
        // silently-skipped migration: without a reachable database the
        // auto-migrate aborts HERE (exit 1, pointing at the probe warning
        // above) instead of failing later on a causeless pool timeout.
        if probe_report.is_none() {
            bail!(
                "--auto-migrate: cannot apply migrations — the database is unreachable \
                 (DB_STARTUP_CHECK=warn does not exempt migrations; see the probe warning above)"
            );
        }
        tracing::info!(
            instance_id = %state.instance_id,
            "applying pending migrations (--auto-migrate)"
        );
        let started = std::time::Instant::now();
        if let Err(err) = sqlx::migrate!("./migrations").run(&state.pool).await {
            let diagnostics = backend_rs::db::diagnose_migrate(&err);
            diagnostics.log(&target, started.elapsed());
            bail!(
                "failed to apply migrations: {}: {} — {}",
                diagnostics.describe(),
                diagnostics.message,
                diagnostics.hint
            );
        }
    }
    let port = state.settings.port;
    let instance_id = state.instance_id.clone();
    // Session-expiry sweeper: the ONLY spawn site in the codebase
    // (`lib::create_app` never spawns — tests would leak a task + pool
    // connection each). `continuously_delete_expired(60s)` purges rows past
    // `expiry_date` (the store's `load` already ignores them per-request);
    // the `AbortHandle` below stops it after HTTP drain.
    let sweeper = tokio::spawn(
        state
            .sessions
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
    );
    let sweeper_handle = sweeper.abort_handle();
    // CSP inline-script hashes: computed AT BOOT from the `public/index.html`
    // the server actually serves (never a hand-maintained constant — a stale
    // hash breaks the SPA's inline theme bootstrap SILENTLY).
    //
    // DEGRADE PATH: `backend-rs/public/` is not in git (the Dockerfile copies
    // `frontend/dist/` into it before the image build), so in a dev tree the
    // file does not exist and the list is empty — the CSP is still emitted,
    // without a hash. The hashes are logged so an operator can see whether
    // the served SPA is covered.
    let script_hashes = backend_rs::boot_inline_script_hashes();
    tracing::info!(
        script_hashes = ?script_hashes,
        "CSP inline-script hashes computed from the served SPA index"
    );
    let app = create_app_with_csp(state.clone(), script_hashes);
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .with_context(|| format!("failed to bind port {port}"))?;
    tracing::info!(instance_id = %instance_id, port, "backend-rs listening");
    // Prometheus listener: a SEPARATE socket. `METRICS_PORT` unset means no
    // metrics endpoint at all (fail closed); the router is never mounted on
    // the app, so a scrape bypasses auth/CORS/throttling and cannot record
    // itself.
    let metrics_server = match metrics_addr(&state.settings) {
        Some(address) => {
            let listener = tokio::net::TcpListener::bind(address)
                .await
                .with_context(|| format!("failed to bind metrics listener on {address}"))?;
            tracing::info!(instance_id = %instance_id, %address, "metrics listener bound");
            let metrics_app = metrics_router(state.clone());
            Some(tokio::spawn(async move {
                axum::serve(listener, metrics_app)
                    .with_graceful_shutdown(shutdown_signal())
                    .await
            }))
        }
        None => {
            tracing::info!(
                instance_id = %instance_id,
                "METRICS_PORT unset: no metrics endpoint is exposed"
            );
            None
        }
    };
    // `ConnectInfo` feeds the T30 governor's peer-IP key extractor (without
    // it every connection would share the fallback bucket).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server error")?;
    if let Some(handle) = metrics_server {
        handle.abort();
        let _ = handle.await;
    }
    // HTTP drained (SIGINT/SIGTERM): close idle MySQL connections cleanly,
    // then log shutdown with the instance id for correlation.
    tracing::info!(
        instance_id = %instance_id,
        "shutdown signal received: draining, then closing pool"
    );
    match tokio::time::timeout(POOL_CLOSE_TIMEOUT, state.pool.close()).await {
        Ok(()) => {
            tracing::info!(instance_id = %instance_id, "database pool closed");
        }
        Err(_) => {
            tracing::warn!(
                instance_id = %instance_id,
                timeout_secs = POOL_CLOSE_TIMEOUT.as_secs(),
                "database pool close timed out; exiting anyway"
            );
        }
    }
    sweeper_handle.abort();
    let _ = sweeper.await;
    tracing::info!(instance_id = %instance_id, "backend-rs shutdown complete");
    Ok(())
}

/// Probe the database BEFORE any work touches it: a direct dial
/// (`db::wait_until_ready`, transient-only retry, bounded budget) — never
/// `pool.acquire()`, whose internal retry loop returns a causeless
/// `PoolTimedOut` and out-sources retry policy to `acquire_timeout`.
///
/// `state`: when the caller has an [`backend_rs::state::AppState`] (serve,
/// create-admin) the outcome is recorded through
/// [`backend_rs::state::AppState::record_db_probe`] — the transition log
/// (`Unknown→Down` warn / `Up→Down` error / `Down→Up` info), the
/// `kanpad_db_up` gauge and the `kanpad_db_connect_failures_total{class}`
/// counter. Ephemeral CLI processes (`migrate *`) pass `None` and get the
/// plain full-field ERROR line instead — their metrics would die with the
/// process.
///
/// `DB_STARTUP_CHECK=require` (default) fails the process with the
/// one-line cause + hint; `warn` logs a warning and continues so a DB-less
/// server can still boot degraded — never a silently-skipped
/// `--auto-migrate` (the caller aborts that explicitly).
///
/// Returns the successful [`db::ProbeReport`] so the caller can catch a
/// "connected to the wrong database" (`selected_database` vs the
/// configured name, e.g. a leftover `DB_NAME`) — `None` in warn mode.
async fn startup_db_probe(
    settings: &Settings,
    options: &sqlx::mysql::MySqlConnectOptions,
    target: &db::Target,
    state: Option<&backend_rs::state::AppState>,
) -> Result<Option<db::ProbeReport>> {
    let started = std::time::Instant::now();
    let outcome = db::wait_until_ready(
        options,
        target,
        settings.db_startup_timeout(),
        settings.db_ready_probe_timeout(),
    )
    .await;
    match outcome {
        Ok((report, attempts)) => {
            if let Some(state) = state {
                state.record_db_probe(Ok(&report), target, started.elapsed());
            }
            tracing::info!(
                database = target.database.as_deref().unwrap_or("<none>"),
                server_version = %report.server_version,
                attempts,
                elapsed_ms = %started.elapsed().as_millis(),
                "database reachable"
            );
            // The server may pick a DIFFERENT default database than the
            // configured name (a leftover `DB_NAME`, an URL without one):
            // say so before any work touches the wrong database.
            if report.selected_database.as_deref() != target.database.as_deref() {
                tracing::warn!(
                    configured = target.database.as_deref().unwrap_or("<none>"),
                    selected = report.selected_database.as_deref().unwrap_or("<none>"),
                    "the server selected a different database than configured — \
                     check DB_NAME / DATABASE_URL"
                );
            }
            Ok(Some(report))
        }
        Err((diagnostics, attempts)) => {
            let elapsed = started.elapsed();
            match state {
                Some(state) => {
                    // The transition logger carries the full field set
                    // (warn on the first observed failure, error after).
                    state.record_db_probe(Err(&diagnostics), target, elapsed);
                }
                None => {
                    diagnostics.log(target, elapsed);
                }
            }
            let summary = format!(
                "cannot connect to the database (attempts: {attempts}, elapsed: {}s): {} — {}",
                elapsed.as_secs(),
                diagnostics.describe(),
                diagnostics.hint
            );
            if settings.db_startup_check == backend_rs::config::StartupCheck::Warn {
                eprintln!("warning: {summary}");
                eprintln!(
                    "continuing: DB_STARTUP_CHECK=warn (a DB-less server keeps serving: \
                     /health 200, /ready 503; --auto-migrate will still abort)"
                );
                return Ok(None);
            }
            bail!("{summary}");
        }
    }
}

/// Applied migration versions (ascending) — from **sqlx's own API**, not
/// from a hand-rolled query against `_sqlx_migrations`.
///
/// The journal table name is a sqlx implementation detail (sqlx 0.9 even
/// makes it configurable), so querying it directly was reaching into another
/// crate's internals. `Migrate::list_applied_migrations` is the supported
/// way — and it means `migrate info` and `migrate run` can never disagree
/// about what is applied.
///
/// Split from `applied_versions` so the RAW [`sqlx::migrate::MigrateError`]
/// is classifiable (`diagnose_migrate` keys the "fresh database" branch on
/// MySQL 1146 — an anyhow-wrapped error cannot be classified). The anyhow
/// contexting stays at the call sites that still want it.
async fn journal_versions(
    pool: &MySqlPool,
    migrator: &sqlx::migrate::Migrator,
) -> std::result::Result<Vec<i64>, sqlx::migrate::MigrateError> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(sqlx::migrate::MigrateError::Execute)?;
    let applied = conn.list_applied_migrations(&migrator.table_name).await?;
    Ok(applied
        .into_iter()
        .map(|migration| migration.version)
        .collect())
}

/// [`journal_versions`] with the anyhow context `migrate run`/`revert` want
/// — every error is real there, so none of them is classifiable as "fresh".
async fn applied_versions(
    pool: &MySqlPool,
    migrator: &sqlx::migrate::Migrator,
) -> Result<Vec<i64>> {
    journal_versions(pool, migrator)
        .await
        .context("failed to read the migration journal")
}

async fn migrate(op: MigrateOp) -> Result<()> {
    init_tracing_from_env();
    let settings = Settings::load().context("failed to load settings")?;
    let state = build_state(settings);
    let migrator = sqlx::migrate!("./migrations");
    // Fail fast on an unreachable database BEFORE the op touches the pool:
    // the probe dials directly (cause + class visible on attempt 1), while
    // a pool acquire would retry internally and surface a causeless
    // `PoolTimedOut`.
    //
    // T005 (Step 0 evidence): a wrong password or a refused port used to
    // print `applied: 0 pending: 2` ("fresh database") and exit 0. The
    // probe + the classification below make every outcome visible and
    // accurate.
    let Some((options, target)) = db::Target::for_settings(&state.settings) else {
        bail!(
            "cannot connect to the database: the configured DATABASE_URL does not parse as a \
             MySQL connection URL — check the URL syntax (the password must be percent-encoded; \
             it is never logged)"
        );
    };
    // Ephemeral CLI process: no AppState yet, so no transition log and no
    // metrics (they would die with the process) — the plain full-field
    // ERROR line instead.
    startup_db_probe(&state.settings, &options, &target, None).await?;
    match op {
        MigrateOp::Run => {
            let started = std::time::Instant::now();
            if let Err(err) = migrator.run(&state.pool).await {
                let diagnostics = db::diagnose_migrate(&err);
                diagnostics.log(&target, started.elapsed());
                bail!(
                    "failed to apply migrations: {}: {} — {}",
                    diagnostics.describe(),
                    diagnostics.message,
                    diagnostics.hint
                );
            }
            let applied = applied_versions(&state.pool, &migrator).await?;
            println!("migrations applied: {}", applied.len());
            Ok(())
        }
        MigrateOp::Info => {
            // Print the target BEFORE the status lines so a `RUST_LOG=error`
            // operator still sees WHICH database was inspected (stdout is
            // not filtered by tracing).
            println!("{}", target.describe_line());
            let applied = match journal_versions(&state.pool, &migrator).await {
                Ok(versions) => versions,
                // ONLY 1146 means "the journal was never created" (sqlx runs
                // a bare SELECT against `_sqlx_migrations` with no
                // existence check — `sqlx-mysql-0.9.0/src/migrate.rs`). EVERY
                // other error is a real failure: the old `Err(_)` catch-all
                // printed "fresh database" for outages and lock-ups alike.
                Err(err) if db::diagnose_migrate(&err).class == db::DbClass::MissingTable => {
                    println!("journal: no _sqlx_migrations table yet (fresh database)");
                    Vec::new()
                }
                Err(err) => {
                    let started = std::time::Instant::now();
                    let diagnostics = db::diagnose_migrate(&err);
                    diagnostics.log(&target, started.elapsed());
                    bail!(
                        "cannot read the migration journal: {}: {} — {}",
                        diagnostics.describe(),
                        diagnostics.message,
                        diagnostics.hint
                    );
                }
            };
            let mut pending = 0;
            for migration in migrator.migrations.iter() {
                let status = if applied.contains(&migration.version) {
                    "applied"
                } else {
                    pending += 1;
                    "pending"
                };
                println!("{status}: {} {}", migration.version, migration.description);
            }
            // The LAST line is load-bearing: humans and
            // `docs/data-migration-live.md` gates grep it.
            println!("applied: {} pending: {pending}", applied.len());
            Ok(())
        }
        MigrateOp::Revert => {
            if state.settings.is_production() {
                bail!("migrate revert is dev-only; production rollback = restore backup");
            }
            let applied = applied_versions(&state.pool, &migrator).await?;
            let Some(last) = applied.last().copied() else {
                println!("nothing to revert");
                return Ok(());
            };
            let target_version = applied.iter().rev().nth(1).copied().unwrap_or(0);
            let started = std::time::Instant::now();
            if let Err(err) = migrator.undo(&state.pool, target_version).await {
                let diagnostics = db::diagnose_migrate(&err);
                diagnostics.log(&target, started.elapsed());
                bail!(
                    "failed to revert migration: {}: {} — {}",
                    diagnostics.describe(),
                    diagnostics.message,
                    diagnostics.hint
                );
            }
            println!("reverted migration {last}");
            Ok(())
        }
    }
}

/// `create-admin`: interactive prompts for missing values (`dialoguer` for
/// email + final confirm, `rpassword` for the hidden password + its repeat),
/// `--email`/`--password` flags for non-interactive use, then the shared
/// [`backend_rs::admin::run_create_admin`] core. Exits with the spec-pinned
/// code (`0` ok, `1` validation/input, `2` admin-already-exists, `3`
/// DB/config) — `process::exit` is deliberate: there is no server to drain.
async fn create_admin(email: Option<String>, password: Option<String>, force: bool) -> Result<()> {
    use backend_rs::admin::{AdminExit, CreateAdminArgs};
    init_tracing_from_env();
    let settings = match Settings::load() {
        Ok(settings) => settings,
        Err(err) => {
            eprintln!("create-admin: configuration error: {err}");
            std::process::exit(AdminExit::Db.code());
        }
    };
    let prompted = email.is_none() || password.is_none();
    let email = match email {
        Some(email) => email,
        None => match dialoguer::Input::<String>::new()
            .with_prompt("Enter admin email")
            .interact_text()
        {
            Ok(email) => email,
            Err(_) => {
                eprintln!("create-admin: email input cancelled");
                std::process::exit(AdminExit::Validation.code());
            }
        },
    };
    let password = match password {
        Some(password) => {
            eprintln!(
                "create-admin: warning: --password stays visible in shell history; prefer interactive entry"
            );
            password
        }
        None => {
            let first = match rpassword::prompt_password("Enter admin password: ") {
                Ok(password) => password,
                Err(_) => {
                    eprintln!("create-admin: password input cancelled");
                    std::process::exit(AdminExit::Validation.code());
                }
            };
            let second = match rpassword::prompt_password("Confirm admin password: ") {
                Ok(password) => password,
                Err(_) => {
                    eprintln!("create-admin: password input cancelled");
                    std::process::exit(AdminExit::Validation.code());
                }
            };
            if first != second {
                eprintln!("create-admin: passwords do not match");
                std::process::exit(AdminExit::Validation.code());
            }
            first
        }
    };
    // Confirm destructive/intended writes, but never block fully-scripted
    // runs (`--email` + `--password` together must stay non-interactive).
    if prompted {
        let prompt = if force {
            "Reset the existing admin password?".to_string()
        } else {
            format!("Create admin user {email}?")
        };
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(prompt)
            .default(false)
            .interact()
            .unwrap_or(false);
        if !confirmed {
            eprintln!("create-admin: cancelled");
            std::process::exit(AdminExit::Validation.code());
        }
    }
    let state = build_state(settings);
    // T005 (step 4): probe BEFORE any admin work — the same direct-dial
    // probe + Require/Warn policy as serve and `migrate *`. A Require
    // failure maps to the existing `AdminExit::Db` (code 3) after the
    // hint is printed; a Warn failure continues (the admin insert then
    // fails with its own DB exit, which keeps the outcome honest).
    let Some((options, target)) = db::Target::for_settings(&state.settings) else {
        eprintln!(
            "create-admin: the configured DATABASE_URL does not parse as a MySQL connection \
             URL — check the URL syntax (the password must be percent-encoded; it is never logged)"
        );
        std::process::exit(AdminExit::Db.code());
    };
    if let Err(err) = startup_db_probe(&state.settings, &options, &target, Some(&state)).await {
        eprintln!("create-admin: {err:#}");
        std::process::exit(AdminExit::Db.code());
    }
    let outcome = backend_rs::admin::run_create_admin(
        &state.pool,
        &state.settings,
        CreateAdminArgs {
            email: Some(email),
            password: Some(password),
            force,
        },
    )
    .await;
    if outcome.exit == AdminExit::Success {
        println!("{}", outcome.message);
    } else {
        eprintln!("create-admin: {}", outcome.message);
    }
    std::process::exit(outcome.exit.code());
}

async fn export_openapi() -> Result<()> {
    // Deliberately boots no pool and no sessions: renders the spec from the
    // compiled `ApiDoc` (the same value the snapshot test diffs against the
    // committed `openapi.json` and `GET /api-docs/openapi.json` serves).
    // No `Settings::load` here, so this works with `DATABASE_URL` unset.
    use utoipa::OpenApi;
    print!(
        "{}",
        backend_rs::docs::ApiDoc::openapi()
            .to_pretty_json()
            .context("failed to serialize OpenAPI document")?
    );
    Ok(())
}

/// Wait for SIGINT (Ctrl-C) or SIGTERM (docker/compose stop).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
