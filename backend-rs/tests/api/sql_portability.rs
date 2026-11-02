//! SQL portability guard (T006).
//!
//! Every statement exercised here runs against whatever `DATABASE_URL` points
//! at, so the same code is exercised on the MySQL 8.4 leg AND the MariaDB
//! 10.6 leg in CI. A statement that parses on one server but not the other
//! fails the leg it does not parse on — which is exactly the failure mode T006
//! exists to prevent (the session-store `AS new` upsert that shipped green on
//! MySQL 8.4 and 500'd every correct-password login on MariaDB 10.6).
//!
//! The suite uses `#[sqlx::test(migrations = "./migrations")]` per-test
//! databases, so it never touches the shared `kanpad` schema. Like every other
//! DB-backed suite, it requires `DATABASE_URL` at runtime (it does not skip).

use sqlx::MySqlPool;

/// The portable session upsert round-trips: insert then update, one row, payload
/// replaced. This is the statement T006 fixed.
#[sqlx::test(migrations = "./migrations")]
async fn session_upsert_round_trips(pool: MySqlPool) {
    // insert
    sqlx::query(
        "INSERT INTO `sessions` (`id`, `data`, `expiry_date`) VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE `data` = ?, `expiry_date` = ?",
    )
    .bind("port-1")
    .bind(b"first".to_vec())
    .bind(chrono::Utc::now().naive_utc())
    .bind(b"first".to_vec())
    .bind(chrono::Utc::now().naive_utc())
    .execute(&pool)
    .await
    .expect("portable upsert parses on this server (insert)");

    // update (same id)
    sqlx::query(
        "INSERT INTO `sessions` (`id`, `data`, `expiry_date`) VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE `data` = ?, `expiry_date` = ?",
    )
    .bind("port-1")
    .bind(b"second".to_vec())
    .bind(chrono::Utc::now().naive_utc())
    .bind(b"second".to_vec())
    .bind(chrono::Utc::now().naive_utc())
    .execute(&pool)
    .await
    .expect("portable upsert parses on this server (update)");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `sessions` WHERE `id` = ?")
        .bind("port-1")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 1, "upsert must not create a second row");

    let data: Vec<u8> = sqlx::query_scalar("SELECT `data` FROM `sessions` WHERE `id` = ?")
        .bind("port-1")
        .fetch_one(&pool)
        .await
        .expect("data");
    assert_eq!(data, b"second", "upsert must replace the payload");
}

/// The sweeper statement parses on both servers.
#[sqlx::test(migrations = "./migrations")]
async fn sweeper_delete_parses(pool: MySqlPool) {
    sqlx::query("DELETE FROM `sessions` WHERE `expiry_date` < UTC_TIMESTAMP(6)")
        .execute(&pool)
        .await
        .expect("sweeper delete parses on this server");
}

/// The collation pin holds on both servers.
#[sqlx::test(migrations = "./migrations")]
async fn collation_connection_is_unicode(pool: MySqlPool) {
    let collation: String = sqlx::query_scalar("SELECT @@collation_connection")
        .fetch_one(&pool)
        .await
        .expect("collation readable");
    assert_eq!(collation, "utf8mb4_unicode_ci");
}

/// A multi-row INSERT parses on both servers (board default columns shape).
#[sqlx::test(migrations = "./migrations")]
async fn multi_row_insert_parses(pool: MySqlPool) {
    // seed a user + project + board first (FK chain)
    sqlx::query(
        "INSERT INTO `users` (`id`, `email`, `password`) VALUES (9001, 'port@example.com', 'x')",
    )
    .execute(&pool)
    .await
    .expect("user insert");
    sqlx::query("INSERT INTO `projects` (`id`, `name`, `user_id`) VALUES (9001, 'p', 9001)")
        .execute(&pool)
        .await
        .expect("project insert");
    sqlx::query(
        "INSERT INTO `boards` (`id`, `name`, `user_id`, `project_id`) VALUES (9001, 'b', 9001, 9001)",
    )
    .execute(&pool)
    .await
    .expect("board insert");

    sqlx::query(
        "INSERT INTO `columns` (`name`, `board_id`, `position`) VALUES \
         ('To Do', 9001, 0), ('In Progress', 9001, 1), ('Done', 9001, 2)",
    )
    .execute(&pool)
    .await
    .expect("multi-row insert parses on this server");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `columns` WHERE `board_id` = 9001")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 3);
}

/// INSERT IGNORE parses on both servers (card-labels shape).
#[sqlx::test(migrations = "./migrations")]
async fn insert_ignore_parses(pool: MySqlPool) {
    sqlx::query(
        "INSERT INTO `users` (`id`, `email`, `password`) VALUES (9002, 'port2@example.com', 'x')",
    )
    .execute(&pool)
    .await
    .expect("user insert");
    sqlx::query(
        "INSERT INTO `labels` (`id`, `name`, `color`, `user_id`) VALUES (9002, 'Bug', 'red', 9002)",
    )
    .execute(&pool)
    .await
    .expect("label insert");
    sqlx::query("INSERT INTO `boards` (`id`, `name`, `user_id`) VALUES (9002, 'b2', 9002)")
        .execute(&pool)
        .await
        .expect("board insert");
    sqlx::query(
        "INSERT INTO `columns` (`id`, `name`, `board_id`, `position`) VALUES (9002, 'c', 9002, 0)",
    )
    .execute(&pool)
    .await
    .expect("column insert");
    sqlx::query(
        "INSERT INTO `cards` (`id`, `title`, `column_id`, `position`) VALUES (9002, 'card', 9002, 0)",
    )
    .execute(&pool)
    .await
    .expect("card insert");

    // INSERT IGNORE must parse (and not fail on a duplicate).
    sqlx::query("INSERT IGNORE INTO `card_labels` (`card_id`, `label_id`) VALUES (9002, 9002)")
        .execute(&pool)
        .await
        .expect("INSERT IGNORE parses on this server");
    sqlx::query("INSERT IGNORE INTO `card_labels` (`card_id`, `label_id`) VALUES (9002, 9002)")
        .execute(&pool)
        .await
        .expect("INSERT IGNORE is idempotent");
}

/// A CHECK-constrained insert that violates the constraint must fail on both
/// servers (portability of CHECK constraints).
#[sqlx::test(migrations = "./migrations")]
async fn check_constraint_rejects_bad_value(pool: MySqlPool) {
    sqlx::query(
        "INSERT INTO `users` (`id`, `email`, `password`) VALUES (9003, 'port3@example.com', 'x')",
    )
    .execute(&pool)
    .await
    .expect("user insert");

    let result = sqlx::query(
        "INSERT INTO `labels` (`name`, `color`, `user_id`) VALUES ('Bad', 'neon', 9003)",
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "a bad label color must be rejected on this server"
    );
}
