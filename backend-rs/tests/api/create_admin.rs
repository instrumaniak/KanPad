//! `backend-rs create-admin` CLI (T12).
//!
//! Single `#[sqlx::test]` setup on purpose: all scenarios run sequentially
//! inside one test against one isolated pool (fresh migrated DB, so no admin
//! row exists at start), with distinct emails per scenario. The
//! `tower_sessions.session` store table is server-global — this suite never
//! touches it (no jars here, no wipe).
//!
//! Covered per `50-tasks.md` T12 AC + `10-architecture.md` create-admin CLI:
//! * creates admin with role `admin`; DB re-read finds the row (+ Argon2id
//!   PHC hash, never bcrypt, + exactly 6 default labels via prod
//!   `common::label_seed`)
//! * second run (no `--force`) → exit code 2, no second row
//! * `--force` → reset-password path, exit 0, still exactly one admin row
//! * `--email`/`--password` non-interactive path works (all scenarios drive
//!   [`run_create_admin`] directly with explicit args — no manual ops)
//! * validation failure (bad email / weak / overlong password / missing
//!   input) → exit code 1

use backend_rs::{
    admin::{run_create_admin, AdminExit, CreateAdminArgs},
    config::Settings,
    features::auth::store::verify_argon2,
};
use sqlx::MySqlPool;

fn args(email: &str, password: &str) -> CreateAdminArgs {
    CreateAdminArgs {
        email: Some(email.to_string()),
        password: Some(password.to_string()),
        force: false,
    }
}

async fn admin_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM `users` WHERE `role` = 'admin'")
        .fetch_one(pool)
        .await
        .expect("admin count reads")
}

async fn label_count(pool: &MySqlPool, user_id: i32) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("label count reads")
}

#[sqlx::test(migrations = "./migrations")]
async fn create_admin_full_flow(pool: MySqlPool) {
    let settings = Settings::default();
    assert_eq!(admin_count(&pool).await, 0, "fresh DB has no admin");

    // 1. Happy path (non-interactive `--email`/`--password`): mixed-case +
    //    padded email normalizes; exit 0.
    let outcome = run_create_admin(
        &pool,
        &settings,
        args("  Admin@Example.COM ", "Str0ng!pass"),
    )
    .await;
    assert_eq!(
        outcome.exit,
        AdminExit::Success,
        "create: {}",
        outcome.message
    );
    assert_eq!(outcome.exit.code(), 0);

    // DB re-read (not just the outcome): exactly one admin row, normalized
    // email, role `admin`, Argon2id PHC hash (never bcrypt).
    let row: (i32, String, String, String) = sqlx::query_as(
        "SELECT `id`, `email`, `role`, `password` FROM `users` WHERE `role` = 'admin'",
    )
    .fetch_one(&pool)
    .await
    .expect("admin row persists");
    assert_eq!(row.1, "admin@example.com");
    assert_eq!(row.2, "admin");
    assert!(
        row.3.starts_with("$argon2id$v=19$m=19456,t=1,p=1$"),
        "PHC must pin m=19456,t=1,p=1, got: {}",
        row.3
    );
    assert!(verify_argon2("Str0ng!pass", &row.3));
    assert_eq!(
        label_count(&pool, row.0).await,
        6,
        "admin gets 6 default labels"
    );

    // 2. Second run without `--force` → exit 2, no second row.
    let outcome =
        run_create_admin(&pool, &settings, args("second@example.com", "An0ther!pass")).await;
    assert_eq!(
        outcome.exit,
        AdminExit::AlreadyExists,
        "second run: {}",
        outcome.message
    );
    assert_eq!(outcome.exit.code(), 2);
    assert_eq!(admin_count(&pool).await, 1, "no second admin row");
    let taken: Option<i32> =
        sqlx::query_scalar("SELECT `id` FROM `users` WHERE `email` = 'second@example.com'")
            .fetch_optional(&pool)
            .await
            .expect("email lookup reads");
    assert_eq!(taken, None, "second email must not be inserted");

    // 3. `--force` resets the password: exit 0, still one admin row, new
    //    password verifies and the old one does not.
    let forced = CreateAdminArgs {
        force: true,
        ..args("admin@example.com", "N3w!password")
    };
    let outcome = run_create_admin(&pool, &settings, forced).await;
    assert_eq!(
        outcome.exit,
        AdminExit::Success,
        "force: {}",
        outcome.message
    );
    assert_eq!(admin_count(&pool).await, 1, "force must not add a row");
    let hash: String = sqlx::query_scalar("SELECT `password` FROM `users` WHERE `role` = 'admin'")
        .fetch_one(&pool)
        .await
        .expect("hash re-reads");
    assert!(
        verify_argon2("N3w!password", &hash),
        "new password verifies"
    );
    assert!(
        !verify_argon2("Str0ng!pass", &hash),
        "old password stops verifying"
    );

    // 4. Validation failures → exit 1 (no rows added).
    for (email, password) in [
        ("not-an-email", "Str0ng!pass"),        // bad email
        ("valid@example.com", "weakpass"),      // no digit/symbol
        ("valid@example.com", "short1!"),       // < 8 chars
        ("valid@example.com", &"a".repeat(73)), // > 72 bytes
    ] {
        let outcome = run_create_admin(&pool, &settings, args(email, password)).await;
        assert_eq!(
            outcome.exit,
            AdminExit::Validation,
            "email={email:?} must 400-classify: {}",
            outcome.message
        );
        assert_eq!(outcome.exit.code(), 1);
    }
    // Multibyte input over 72 bytes (38 chars) must also fail: a bare
    // char-length check would pass it and bcrypt would silently truncate.
    let multibyte = format!("a1{}", "é".repeat(36));
    assert_eq!(multibyte.len(), 74);
    let outcome = run_create_admin(&pool, &settings, args("valid@example.com", &multibyte)).await;
    assert_eq!(outcome.exit, AdminExit::Validation);
    // Missing input (interactive prompt declined upstream) → exit 1.
    for missing in [
        CreateAdminArgs {
            email: None,
            password: Some("Str0ng!pass".to_string()),
            force: false,
        },
        CreateAdminArgs {
            email: Some("valid@example.com".to_string()),
            password: None,
            force: false,
        },
    ] {
        let outcome = run_create_admin(&pool, &settings, missing).await;
        assert_eq!(outcome.exit, AdminExit::Validation);
    }
    assert_eq!(admin_count(&pool).await, 1, "validation runs add no rows");
}
