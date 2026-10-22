//! auth store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Registration inserts the user and seeds the six default labels in ONE
//! transaction owned by the route (`routes.rs`): the hash runs BEFORE
//! `tx.begin()`, and the tx holds only `INSERT user` + the single
//! multi-row `INSERT labels` from `common::label_seed` (no network `.await`
//! while the tx is open).

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier},
};
use sqlx::MySqlExecutor;

use crate::{
    common::ids::UserId,
    error::{AppError, Internal},
};

/// Argon2id memory cost in KiB: 19 MiB (`10-architecture.md` AuthN/Z).
const ARGON2_MEMORY_KIB: u32 = 19 * 1024;

/// Argon2id parallelism (`p=1`, spec-pinned).
const ARGON2_LANES: u32 = 1;

/// Stored user row projected to the auth shape (id/email/role — never the
/// password hash; hashes only leave the DB inside a verify call).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUser {
    /// `users.id` (`INT` ↔ `i32` newtype, bound directly).
    pub id: UserId,
    /// Normalized email (`trim` + lowercase).
    pub email: String,
    /// `user` on register (admin only via the T12 CLI).
    pub role: String,
}

/// Login credential row: the [`StoredUser`] projection plus its password
/// hash (Argon2id for new rows, legacy cost-10 `$2a$`/`$2b$` bcrypt
/// pre-cutover — a successful bcrypt login rehashes to Argon2id in the same
/// request, see [`update_password_hash`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRow {
    /// Projected user (id/email/role).
    pub user: StoredUser,
    /// Raw `users.password` (PHC string, never logged).
    pub password_hash: String,
}

/// Find the login credential row by normalized email (`None` = unknown; the
/// login route runs [`dummy_verify`] on this branch so unknown emails cost
/// the same Argon2 work as known ones — no enumeration oracle).
#[tracing::instrument(skip(ex))]
pub async fn find_auth_row_by_email<'e, E>(ex: E, email: &str) -> Result<Option<AuthRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `id`, `email`, `role`, `password` FROM `users` WHERE `email` = ?",
        email
    )
    .fetch_optional(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|row| AuthRow {
        user: StoredUser {
            id: UserId::from(row.id),
            email: row.email,
            role: row.role,
        },
        password_hash: row.password,
    }))
}

/// Verify a password against an Argon2id PHC hash. Unparseable input (legacy
/// bcrypt, garbage) is `false` — never an error (the login route falls
/// through to [`verify_bcrypt_truncated` next).
#[tracing::instrument(skip(password, hash))]
pub fn verify_argon2(password: &str, hash: &str) -> bool {
    // argon2 0.6 / password-hash 0.6: the verifier takes the PHC STRING
    // directly (`PasswordVerifier<str>`) and parses it internally, so an
    // unparseable hash (legacy bcrypt, garbage) is an `Err` — i.e. `false`,
    // exactly as before. The explicit `PasswordHash::new` parse is gone with
    // the PHC types' move to the `phc` crate.
    Argon2::default()
        .verify_password(password.as_bytes(), hash)
        .is_ok()
}

/// Verify a password against a legacy pre-cutover bcrypt hash (cost-10
/// `$2a$`/`$2b$`, see `auth.service.ts:41-42`). The password is truncated to
/// 72 **bytes** before verify (Nest/bcryptjs semantics — bcrypt truncates;
/// overlong inputs 400 upstream, this is the verify-side mirror). Non-bcrypt
/// hashes short-circuit to `false`.
#[tracing::instrument(skip(password, hash))]
pub fn verify_bcrypt_truncated(password: &str, hash: &str) -> bool {
    if !(hash.starts_with("$2a$") || hash.starts_with("$2b$")) {
        return false;
    }
    let bytes = password.as_bytes();
    let truncated = &bytes[..bytes
        .len()
        .min(crate::common::validation::MAX_PASSWORD_BYTES)];
    bcrypt::verify(truncated, hash).unwrap_or(false)
}

/// Unknown-email timing filler: runs a real Argon2 hash + verify at the
/// current `hash_cost` so the 401 path costs the same Argon2 work as the
/// known-user path (no user-enumeration timing delta). The bcrypt-path delta
/// on known-bcrypt users is accepted for one release (spec-recorded).
/// Failures are swallowed — this is best-effort timing noise, never auth
/// logic.
#[tracing::instrument]
pub fn dummy_verify(hash_cost: u32) {
    // Runtime-derived (not a checked-in static string) because the params
    // vary by env (`t=1` in tests, `t>=2` in prod) — derived at current
    // params is exactly what "static PHC at current params" means here.
    if let Ok(phc) = hash_password("kanpad-unknown-email-dummy", hash_cost) {
        let _ = verify_argon2("kanpad-unknown-email-dummy", &phc);
    }
}

/// Replace the stored hash after a successful legacy-bcrypt login (same
/// request upgrade to Argon2id — never mint bcrypt). Zero affected rows
/// (concurrent delete between verify and write) maps to `401`: the user is
/// gone, so there is nothing to authenticate.
#[tracing::instrument(skip(ex, new_hash))]
pub async fn update_password_hash<'e, E>(ex: E, id: i32, new_hash: &str) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "UPDATE `users` SET `password` = ? WHERE `id` = ?",
        new_hash,
        id
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    if result.rows_affected() == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

/// Hash a new password with Argon2id (`m=19MiB, t=hash_cost, p=1`).
///
/// Runs BEFORE `tx.begin()`: the tx must hold only the user `INSERT` + the
/// label seed. `hash_cost` comes from `Settings` (tests pin `t=1` for speed,
/// default `t=2`; production boot refuses `< 2`). Failures carry no payload
/// ([`Internal`] is a unit struct — the password never reaches logs).
#[tracing::instrument(skip(password))]
pub fn hash_password(password: &str, hash_cost: u32) -> Result<String, AppError> {
    let params = Params::new(ARGON2_MEMORY_KIB, hash_cost, ARGON2_LANES, None)
        .map_err(|_| AppError::Internal(Internal))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    // argon2 0.6 / password-hash 0.6: the salt is generated INTERNALLY by
    // `PasswordHasher::hash_password` (the `getrandom` feature, on by
    // default) — `SaltString` is gone with the PHC types' move to the `phc`
    // crate. The generated salt is 16 bytes, as before.
    argon2
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| AppError::Internal(Internal))
}

/// Find a user by normalized email (`None` = unknown; the login path in T11
/// runs a dummy Argon2 verify on this branch against enumeration).
#[tracing::instrument(skip(ex))]
pub async fn find_user_by_email<'e, E>(ex: E, email: &str) -> Result<Option<StoredUser>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `id`, `email`, `role` FROM `users` WHERE `email` = ?",
        email
    )
    .fetch_optional(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|row| StoredUser {
        id: UserId::from(row.id),
        email: row.email,
        role: row.role,
    }))
}

/// Insert a user row, returning the new `users.id`. Accepts the pool or
/// `&mut Transaction` (the register route passes its tx). A `1062` race
/// maps to `409` via the shared `Db` arm; `last_insert_id` overflow maps to
/// `500` (never a silent `as` truncation — see `30-db-schema.md` mapping).
///
/// Single-statement on purpose: generic executors (`E: MySqlExecutor`) are
/// consumed by one query, so multi-statement flows compose single-statement
/// fns (see [`find_user_by_id`]) instead of reusing `ex`.
#[tracing::instrument(skip(ex, password_hash))]
pub async fn insert_user_row<'e, E>(
    ex: E,
    email: &str,
    password_hash: &str,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `users` (`email`, `password`) VALUES (?, ?)",
        email,
        password_hash
    )
    .execute(ex)
    .await
    .map_err(AppError::Db)?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Find a user by id (`RowNotFound` → `404` via the shared `Db` arm — the
/// kernel's foreign-id rule; the register route only calls this for an id it
/// just inserted, so the branch is unreachable there).
#[tracing::instrument(skip(ex))]
pub async fn find_user_by_id<'e, E>(ex: E, id: i32) -> Result<StoredUser, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT `id`, `email`, `role` FROM `users` WHERE `id` = ?",
        id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(StoredUser {
        id: UserId::from(row.id),
        email: row.email,
        role: row.role,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2id_phc_pins_memory_time_and_lanes() {
        let hash = hash_password("Str0ng!pass", 1).expect("hashing succeeds");
        assert!(
            hash.starts_with("$argon2id$v=19$m=19456,t=1,p=1$"),
            "PHC must pin m=19456,t=1,p=1, got: {hash}"
        );
        assert!(
            !hash.starts_with("$2a$") && !hash.starts_with("$2b$"),
            "must not mint bcrypt hashes"
        );
        // Random salt: same password hashes differently each call.
        let again = hash_password("Str0ng!pass", 1).expect("hashing succeeds");
        assert_ne!(hash, again, "salt must be random per hash");
    }
}
