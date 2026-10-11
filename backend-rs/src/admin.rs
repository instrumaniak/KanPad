//! `create-admin` CLI core (T12; `10-architecture.md` create-admin CLI).
//!
//! Thin boundary over the T10 auth store: [`run_create_admin`] validates with
//! **the same [`common::validation`] email + password fns as registration**
//! (single shared helpers — no drift; the Nest source drifted here), hashes
//! with the T10 Argon2id [`hash_password`] **before** opening the tx, and
//! seeds the six default labels via prod
//! [`common::label_seed::ensure_default_labels`] in the same tx as the
//! insert. `main.rs` owns prompting/exit; tests drive this module directly
//! (no manual ops, no subprocess).
//!
//! Exit codes (spec-pinned): `0` ok, `1` validation/input error, `2`
//! admin-already-exists, `3` DB/config error.

use secrecy::{ExposeSecret, SecretString};
use sqlx::MySqlPool;

use crate::{
    common::{
        label_seed::ensure_default_labels,
        validation::{
            normalize_email, validate_email, validate_password_bytes, validate_register_password,
        },
    },
    config::Settings,
    features::auth::store::hash_password,
};

/// `create-admin` inputs (mirrors the clap flags in `main.rs`; `None` means
/// the interactive prompt was declined upstream — an input error, never a
/// silent default).
#[derive(Debug, Clone, Default)]
pub struct CreateAdminArgs {
    /// Admin email (`--email`; prompted via `dialoguer` when `None`).
    pub email: Option<String>,
    /// Admin password (`--password`; prompted hidden via `rpassword` when
    /// `None`). Moved into a [`SecretString`] on entry and zeroized after
    /// hashing.
    pub password: Option<String>,
    /// `--force`: reset the existing admin's password instead of refusing
    /// with [`AdminExit::AlreadyExists`].
    pub force: bool,
}

/// Spec-pinned process exit code for `create-admin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminExit {
    /// Admin created (or password reset with `--force`).
    Success,
    /// Validation/input error (bad email, weak/overlong password, missing
    /// input, email already taken by a non-admin user).
    Validation,
    /// An `admin` user already exists and `--force` was not passed.
    AlreadyExists,
    /// DB/config error (pool failure, migration drift, hash failure).
    Db,
}

impl AdminExit {
    /// Numeric exit code (`0/1/2/3` per `10-architecture.md`).
    pub const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Validation => 1,
            Self::AlreadyExists => 2,
            Self::Db => 3,
        }
    }
}

/// [`run_create_admin`] result: the exit plus the human-readable line
/// `main.rs` prints (stdout on success, stderr otherwise).
#[derive(Debug, Clone)]
pub struct AdminOutcome {
    /// Spec-pinned exit.
    pub exit: AdminExit,
    /// Human-readable result line (never contains the password or its hash).
    pub message: String,
}

/// Validate raw CLI input with the registration rules: normalize the email
/// (`trim` + lowercase, exactly like the register route) BEFORE validation,
/// then run the shared email + strength + byte-cap fns. Returns the
/// normalized email — the value stored and looked up.
pub fn validate_admin_credentials(email: &str, password: &str) -> Result<String, String> {
    let normalized = normalize_email(email);
    validate_email(&normalized, &()).map_err(|err| format!("invalid email: {err}"))?;
    validate_register_password(password, &()).map_err(|err| format!("invalid password: {err}"))?;
    validate_password_bytes(password, &()).map_err(|err| format!("invalid password: {err}"))?;
    Ok(normalized)
}

/// Create (or, with `force`, reset the password of) the single admin user.
///
/// * Hash runs BEFORE any tx (the tx holds only `INSERT user` + the single
///   multi-row label `INSERT`, mirroring the register route).
/// * Without `force`, a pre-existing `admin` row refuses with
///   [`AdminExit::AlreadyExists`] and inserts nothing.
/// * A `1062` email race maps to [`AdminExit::Validation`] (the address is
///   taken — an input error, not a DB outage); every other store failure
///   maps to [`AdminExit::Db`].
#[tracing::instrument(skip(pool, settings, args))]
pub async fn run_create_admin(
    pool: &MySqlPool,
    settings: &Settings,
    args: CreateAdminArgs,
) -> AdminOutcome {
    let fail = |exit: AdminExit, message: String| AdminOutcome { exit, message };
    let (raw_email, secret) = match (args.email, args.password) {
        (Some(email), Some(password)) => (email, SecretString::new(password.into_boxed_str())),
        _ => {
            return fail(
                AdminExit::Validation,
                "email and password are required (pass --email/--password or answer the prompts)"
                    .to_string(),
            );
        }
    };
    let email = match validate_admin_credentials(&raw_email, secret.expose_secret()) {
        Ok(normalized) => normalized,
        Err(reason) => return fail(AdminExit::Validation, reason),
    };
    let hash = match hash_password(secret.expose_secret(), settings.hash_cost) {
        Ok(hash) => hash,
        Err(_) => {
            return fail(
                AdminExit::Db,
                "failed to hash the admin password".to_string(),
            );
        }
    };
    // `SecretString` zeroizes on drop; the explicit drop pins the lifetime to
    // "right after hashing" instead of end-of-scope.
    drop(secret);

    let existing =
        sqlx::query!("SELECT `id` FROM `users` WHERE `role` = 'admin' ORDER BY `id` LIMIT 1")
            .fetch_optional(pool)
            .await;
    let existing = match existing {
        Ok(row) => row,
        Err(err) => {
            tracing::error!("create-admin: admin lookup failed");
            return fail(AdminExit::Db, db_message(err));
        }
    };
    if let Some(row) = existing {
        if !args.force {
            return fail(
                AdminExit::AlreadyExists,
                "an admin user already exists (only one superadmin is allowed; pass --force to reset its password)".to_string(),
            );
        }
        if let Err(err) = sqlx::query!(
            "UPDATE `users` SET `password` = ? WHERE `id` = ?",
            hash,
            row.id
        )
        .execute(pool)
        .await
        {
            tracing::error!(error = %err, "create-admin: password reset failed");
            return fail(AdminExit::Db, db_message(err));
        }
        return fail(
            AdminExit::Success,
            "admin password reset successfully".to_string(),
        );
    }

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(error = %err, "create-admin: failed to begin transaction");
            return fail(AdminExit::Db, db_message(err));
        }
    };
    // The `1062` email race is matched on the inner sqlx error (the shared
    // `AppError::Db` arm only maps it at render time — it never becomes
    // `AppError::Conflict`), so an address taken concurrently stays an
    // input error (exit 1), not a DB outage.
    let result = match sqlx::query!(
        "INSERT INTO `users` (`email`, `password`, `role`) VALUES (?, ?, 'admin')",
        email,
        hash
    )
    .execute(&mut *tx)
    .await
    {
        Ok(result) => result,
        Err(sqlx::Error::Database(database_error))
            if database_error.code().as_deref() == Some("1062") =>
        {
            return fail(
                AdminExit::Validation,
                format!("email {email:?} is already registered"),
            );
        }
        Err(err) => {
            tracing::error!(error = %err, "create-admin: admin insert failed");
            return fail(AdminExit::Db, db_message(err));
        }
    };
    let user_id = match i32::try_from(result.last_insert_id()) {
        Ok(id) => id,
        Err(_) => {
            return fail(AdminExit::Db, "failed to create the admin user".to_string());
        }
    };
    if let Err(err) = ensure_default_labels(&mut *tx, user_id).await {
        tracing::error!(error = %err, "create-admin: default-label seed failed");
        return fail(AdminExit::Db, db_message(err));
    }
    if let Err(err) = tx.commit().await {
        tracing::error!(error = %err, "create-admin: transaction commit failed");
        return fail(AdminExit::Db, db_message(err));
    }
    fail(
        AdminExit::Success,
        format!("admin user created successfully: {email}"),
    )
}

/// Generic DB-failure line: fixed string only (driver errors can name
/// users/hosts, so neither the error nor the URL is interpolated).
fn db_message(_: sqlx::Error) -> String {
    "failed to reach the database (check DATABASE_URL and that migrations are applied)".to_string()
}
