//! users feature slice (T13): internal repository helper ONLY — no routes.
//!
//! Nest source: `backend/src/users/users.controller.ts:1-6` is an empty
//! `@Controller('api/users')` (zero handlers); `users.service.ts:14-31` is
//! internal only (`findById`/`findByEmail`/`create`). The rewrite mirrors
//! that: this module exposes the user helpers with NO router mounted (user
//! reads go through `GET /api/v1/auth/me`; admin creation via the T12 CLI).
//! There is deliberately no `routes.rs` here — `lib::versioned_routes`
//! never merges a users router, so every `/api/users` path 404s from the
//! router (asserted in `tests/api/users.rs`).
//!
//! Query ownership: the `users` SQL lives in
//! [`crate::features::auth::store`] (T10/T11 own it and its behavior
//! coverage — `find_user_by_id`, `find_user_by_email` + credential lookup,
//! `insert_user_row`). This module re-exports that owner (zero new
//! `query!` macros, zero `.sqlx/` entries) so auth-adjacent callers can name
//! the Nest-shaped dependency (`users::store`) without forking the queries.
//! See `CONTRACT-CHANGES.md` §9 for the out-of-scope record + rationale.

pub mod store;
