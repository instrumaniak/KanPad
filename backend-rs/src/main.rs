//! Binary boundary: clap CLI + boot + graceful shutdown.
//! `anyhow` lives only here, never in the library.

use std::net::{Ipv4Addr, SocketAddr};

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
    // `ConnectInfo` feeds the T30 governor's peer-IP key extractor (without
    // it every connection would share the fallback bucket).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
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

/// `create-admin`: interactive prompts for missing values (`dialoguer` for
/// email + final confirm, `rpassword` for the hidden password + its repeat),
/// `--email`/`--password` flags for non-interactive use, then the shared
/// [`backend_rs::admin::run_create_admin`] core. Exits with the spec-pinned
/// code (`0` ok, `1` validation/input, `2` admin-already-exists, `3`
/// DB/config) — `process::exit` is deliberate: there is no server to drain.
async fn create_admin(email: Option<String>, password: Option<String>, force: bool) -> Result<()> {
    use backend_rs::admin::{AdminExit, CreateAdminArgs};
    init_tracing();
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
