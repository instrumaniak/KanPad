//! Single integration-test target. Suite modules (`health`, `auth_register`, …)
//! are added by their tasks; each keeps `--test api <filter>` valid.
//!
//! NOTE: suite files live in `tests/api/`, so every `mod` needs an explicit
//! `#[path = "api/<name>.rs"]` — a bare `mod health;` would resolve to
//! `tests/health.rs`, which does not exist.

#[path = "api/auth_register.rs"]
mod auth_register;
#[path = "api/auth_session.rs"]
mod auth_session;
#[path = "api/boards_create.rs"]
mod boards_create;
#[path = "api/create_admin.rs"]
mod create_admin;
#[path = "api/errors.rs"]
mod errors;
#[path = "api/health.rs"]
mod health;
#[path = "api/helpers.rs"]
mod helpers;
#[path = "api/projects.rs"]
mod projects;
#[path = "api/users.rs"]
mod users;
#[path = "api/validation.rs"]
mod validation;
