//! Binary boundary: clap CLI + boot + graceful shutdown.
//! `anyhow` lives only here, never in the library.

use std::net::Ipv4Addr;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use sqlx::{MySqlPool, Row};
use tower_sessions::session_store::ExpiredDeletion;

use backend_rs::{build_state, config::Settings, create_app};

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
    CreateAdmin,
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
        Commands::CreateAdmin => create_admin().await,
        Commands::ExportOpenapi => export_openapi().await,
    }
}

/// Init process-wide tracing once: `RUST_LOG` respected (`RUST_LOG=debug`
/// raises verbosity), sane `info` default without it. Re-init (tests) is
/// ignored rather than panicking.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("backend_rs=info,tower_http=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

async fn serve(auto_migrate: bool) -> Result<()> {
    init_tracing();
    let settings = Settings::load().context("failed to load settings")?;
    let state = build_state(settings);
    if auto_migrate {
        if state.settings.is_production() {
            bail!("--auto-migrate is dev/compose only; refusing in production");
        }
        tracing::info!(
            instance_id = %state.instance_id,
            "applying pending migrations (--auto-migrate)"
        );
        sqlx::migrate!("./migrations")
            .run(&state.pool)
            .await
            .context("failed to apply migrations")?;
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
    let app = create_app(state.clone());
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .with_context(|| format!("failed to bind port {port}"))?;
    tracing::info!(instance_id = %instance_id, port, "backend-rs listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    // HTTP drained (SIGINT/SIGTERM): close idle MySQL connections cleanly,
    // then log shutdown with the instance id for correlation.
    tracing::info!(
        instance_id = %instance_id,
        "shutdown signal received: draining, then closing pool"
    );
    state.pool.close().await;
    sweeper_handle.abort();
    let _ = sweeper.await;
    tracing::info!(instance_id = %instance_id, "backend-rs shutdown complete");
    Ok(())
}

/// Applied migration versions (ascending) from the sqlx journal.
async fn applied_versions(pool: &MySqlPool) -> Result<Vec<i64>> {
    let rows = sqlx::query("SELECT version FROM _sqlx_migrations ORDER BY version ASC")
        .fetch_all(pool)
        .await
        .context("failed to read migration journal")?;
    Ok(rows
        .iter()
        .map(|row| row.get::<i64, _>("version"))
        .collect())
}

async fn migrate(op: MigrateOp) -> Result<()> {
    init_tracing();
    let settings = Settings::load().context("failed to load settings")?;
    let state = build_state(settings);
    let migrator = sqlx::migrate!("./migrations");
    match op {
        MigrateOp::Run => {
            migrator
                .run(&state.pool)
                .await
                .context("failed to apply migrations")?;
            let applied = applied_versions(&state.pool).await?;
            println!("migrations applied: {}", applied.len());
            Ok(())
        }
        MigrateOp::Info => {
            let applied = match applied_versions(&state.pool).await {
                Ok(versions) => versions,
                Err(_) => {
                    println!("journal: no _sqlx_migrations table yet (fresh database)");
                    Vec::new()
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
            println!("applied: {} pending: {pending}", applied.len());
            Ok(())
        }
        MigrateOp::Revert => {
            if state.settings.is_production() {
                bail!("migrate revert is dev-only; production rollback = restore backup");
            }
            let applied = applied_versions(&state.pool).await?;
            let Some(last) = applied.last().copied() else {
                println!("nothing to revert");
                return Ok(());
            };
            let target = applied.iter().rev().nth(1).copied().unwrap_or(0);
            migrator
                .undo(&state.pool, target)
                .await
                .context("failed to revert migration")?;
            println!("reverted migration {last}");
            Ok(())
        }
    }
}

async fn create_admin() -> Result<()> {
    bail!("skeleton: create-admin is wired in T12");
}

async fn export_openapi() -> Result<()> {
    // Deliberately boots no pool and no sessions: prints the committed stub.
    print!("{}", include_str!("../openapi.json"));
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
