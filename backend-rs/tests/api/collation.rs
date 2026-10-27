//! The **collation pin** is load-bearing, not decorative (T66, `54` Part C §C6).
//!
//! sqlx 0.8 always sent `SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci`.
//! **sqlx 0.9 sends only `SET NAMES utf8mb4`** and lets the *server* pick the
//! collation — which on MySQL 8 is `utf8mb4_0900_ai_ci`, a **different
//! collation with different comparison rules** from this project's schema
//! (`utf8mb4_unicode_ci`). Without the `collation=utf8mb4_unicode_ci` pin in
//! the `DATABASE_URL`, the driver version alone would silently change the
//! connection's collation.
//!
//! This suite pins the pin: it asserts that the **connection's** collation is
//! the project's, and that it matches the pin in the URL the app was given.

use sqlx::MySqlPool;

/// The project's schema collation — the one every table is created with.
const PROJECT_COLLATION: &str = "utf8mb4_unicode_ci";

/// The connection's collation must be the project's — regardless of what the
/// server would have picked on its own.
#[sqlx::test(migrations = "./migrations")]
async fn the_connection_uses_the_projects_collation(pool: MySqlPool) {
    let collation: String = sqlx::query_scalar("SELECT @@collation_connection")
        .fetch_one(&pool)
        .await
        .expect("the collation is readable");
    assert_eq!(
        collation, PROJECT_COLLATION,
        "the connection must use the project's collation — sqlx 0.9 lets the \\
         server pick it unless the URL pins it"
    );
}

/// The pin must be **in the URL the app was given**, not just in effect by
/// luck (the server's default would have been `utf8mb4_0900_ai_ci`).
#[sqlx::test(migrations = "./migrations")]
async fn the_database_url_carries_the_collation_pin(pool: MySqlPool) {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is set for the test run");
    assert!(
        url.contains(&format!("collation={PROJECT_COLLATION}")),
        "the DATABASE_URL must carry the collation pin: {url}"
    );
    // And the pin is what produced the connection's collation.
    let collation: String = sqlx::query_scalar("SELECT @@collation_connection")
        .fetch_one(&pool)
        .await
        .expect("the collation is readable");
    assert_eq!(collation, PROJECT_COLLATION);
}
