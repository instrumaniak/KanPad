//! users internal repository helper (T13): re-export of the auth-store owner.
//!
//! Nest mapping (`backend/src/users/users.service.ts`):
//! * `findById(id)` (`users.service.ts:14-19`) → [`find_user_by_id`]
//!   (owner: `auth::store`; `RowNotFound` → 404 via the shared `Db` arm).
//! * `findByEmail(email)` (`users.service.ts:21-26`) → [`find_user_by_email`]
//!   (owner: `auth::store`; the login credential projection WITH hash is
//!   [`find_auth_row_by_email`], used only by the T11 login path).
//! * `create(dto)` (`users.service.ts:28-31`) → [`insert_user_row`] (owner:
//!   `auth::store`; single-statement, pool- or tx-generic; the register
//!   route pairs it with the label seed in one tx).
//!
//! No SQL here on purpose: new `query!` macros would fork coverage owned by
//! T10/T11 and add `.sqlx/` entries for identical statements. Behavior
//! coverage stays in the T10/T11 auth suites (`auth_register`,
//! `auth_session`); T13 pins only the routing absence + this re-export
//! identity (`tests/api/users.rs`).

pub use crate::features::auth::store::{
    AuthRow, StoredUser, find_auth_row_by_email, find_user_by_email, find_user_by_id,
    insert_user_row,
};
