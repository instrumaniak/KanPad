//! columns handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

/// Feature router (no `with_state` inside; the caller applies state once).
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
}
