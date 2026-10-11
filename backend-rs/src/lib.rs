//! backend-rs: KanPad API rewrite (Axum 0.8 + sqlx 0.8 + MySQL, single crate).
//!
//! Composition root for in-process tests. [`create_app`] builds the router;
//! [`build_state`] builds state. This module NEVER spawns the session-expiry
//! sweeper — only `main.rs` does (see 10-architecture.md Sessions).

pub mod common;
pub mod config;
pub mod db;
pub mod docs;
pub mod features;
pub mod state;

use axum::{http::header, response::IntoResponse, routing::get, Json, Router};
use docs::HealthResponse;
use state::AppState;

/// Build application state from already-loaded settings (no I/O, no pool).
pub fn build_state(settings: config::Settings) -> AppState {
    AppState::from_settings(settings)
}

/// Assemble the router. Session sweeper is deliberately NOT spawned here.
pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api-docs/openapi.json", get(openapi_json))
        .with_state(state)
}

/// Liveness probe: stable shape `{status:"ok"}`, no DB touch, no auth.
#[utoipa::path(get, path = "/health", responses((status = 200, body = HealthResponse)))]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// Serve the committed `openapi.json` stub byte-identically (T40a owns the real export).
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("../openapi.json"),
    )
}
