//! Single integration-test target. Suite modules (`health`, `auth_register`, …)
//! are added by their tasks; each keeps `--test api <filter>` valid.
//!
//! NOTE: suite files live in `tests/api/`, so every `mod` needs an explicit
//! `#[path = "api/<name>.rs"]` — a bare `mod health;` would resolve to
//! `tests/health.rs`, which does not exist.

#[path = "api/errors.rs"]
mod errors;
#[path = "api/health.rs"]
mod health;
