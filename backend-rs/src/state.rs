//! Shared application state (Clone; pool + session store join in T05/T11).

use std::sync::Arc;

use crate::config::Settings;

/// Composition-root state. `MySqlPool` (already `Arc`-interned) and the
/// session store are added here by T05/T11; handlers take `State<AppState>`.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Validated boot settings.
    pub settings: Arc<Settings>,
}

impl AppState {
    /// Build state from settings without I/O (test-friendly).
    pub fn from_settings(settings: Settings) -> Self {
        Self {
            settings: Arc::new(settings),
        }
    }
}
