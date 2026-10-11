//! Binary boundary: clap CLI + boot + graceful shutdown.
//! `anyhow` lives only here, never in the library.

use std::net::Ipv4Addr;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

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

async fn serve(auto_migrate: bool) -> Result<()> {
    let settings = Settings::load().context("failed to load settings")?;
    if auto_migrate {
        println!("warning: --auto-migrate is a no-op until T02 lands backend-rs/migrations/");
    }
    let port = settings.port;
    let state = build_state(settings);
    let app = create_app(state);
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .with_context(|| format!("failed to bind port {port}"))?;
    println!("backend-rs listening on :{port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    println!("backend-rs shutdown complete");
    Ok(())
}

async fn migrate(op: MigrateOp) -> Result<()> {
    match op {
        MigrateOp::Info => {
            // Skeleton: T02 owns backend-rs/migrations/. Info must work with no DB.
            println!("applied: 0 pending: 0 (skeleton: no migrations dir yet, T02 owns it)");
            Ok(())
        }
        MigrateOp::Run => {
            bail!("skeleton: cannot run migrations until T02 lands backend-rs/migrations/");
        }
        MigrateOp::Revert => {
            bail!("skeleton: cannot revert migrations until T02 lands backend-rs/migrations/");
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
