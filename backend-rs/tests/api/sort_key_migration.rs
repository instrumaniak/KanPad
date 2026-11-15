//! T010 — migration `0003_sort_key_fractional_ordering` behaviour, proven on
//! a dirty database (duplicate positions, gaps) seeded through the SQL the
//! app used to write, and on a fresh per-test database boot.
//!
//! The backfill test does NOT use `#[sqlx::test]` (whose per-test databases
//! run every migration on an empty schema): it builds a scratch
//! database, applies 0001+0002, seeds rows with gaps and duplicate positions,
//! then applies 0003 and asserts the backfill contract:
//!
//!   * order preserved per parent ((`position`, `id`) order → `sort_key`),
//!   * keys unique per parent and strictly monotonic in that order,
//!   * first key per parent is `c000` (base62, big-endian, `c` + 3 digits),
//!   * `position` no longer exists anywhere,
//!   * the vendored `key_between` round-trips sampled adjacent backfilled
//!     keys (SQL-generated keys parse and compare correctly in Rust).
//!
//! The scratch database is dropped on success (best-effort on failure); its
//! name carries nanosecond uniqueness so parallel CI legs never collide.

use super::helpers::{app_on_pool, authed_request, json_body, json_of, register, send};
use sqlx::{MySqlPool, Row};
// Dynamic identifiers (`CREATE DATABASE kanpad_t010_backfill_<nanos>_<pid>`)
// are built from nanoseconds + the pid only — never from input — and are
// wrapped in `AssertSqlSafe` per sqlx 0.9's `SqlSafeStr` rule.
use sqlx::AssertSqlSafe;

/// `;`-terminated statements in a migration file (the files carry no
/// procedure/trigger/DELIMITER blocks and no semicolons inside string
/// literals — splitting on end-of-line `;` is sufficient).
fn split_migration(source: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut buffer = String::new();
    for line in source.lines() {
        buffer.push_str(line);
        buffer.push('\n');
        if line.trim_end().ends_with(';') {
            let statement = buffer.trim().to_string();
            if !statement.is_empty() {
                statements.push(statement);
            }
            buffer.clear();
        }
    }
    let trailing = buffer.trim().to_string();
    if !trailing.is_empty() {
        statements.push(trailing);
    }
    statements
}

/// `DATABASE_URL` with the query string and the database path stripped
/// (server-level connection — used to create/drop scratch databases).
fn server_url() -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is set for the test run");
    let without_query = url.split('?').next().unwrap_or(&url);
    let slash = without_query
        .rfind('/')
        .expect("the DATABASE_URL carries a database path");
    without_query[..slash].to_string()
}

/// `DATABASE_URL` with the database path swapped for `scratch`.
fn scratch_url(scratch: &str) -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is set for the test run");
    let without_query = url.split('?').next().unwrap_or(&url);
    let slash = without_query
        .rfind('/')
        .expect("the DATABASE_URL carries a database path");
    format!("{}/{scratch}", &without_query[..slash])
}

fn scratch_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    format!("kanpad_t010_backfill_{nanos}_{}", std::process::id())
}

/// Seed the pre-`0003` world: gaps and duplicate positions on all three
/// tables, FK chains included.
async fn seed_pre_0003(pool: &MySqlPool) {
    let statements = [
        "INSERT INTO `users` (`id`, `email`, `password`) VALUES (1, 'sortkey@example.com', 'x')",
        "INSERT INTO `boards` (`id`, `name`, `user_id`) VALUES (1, 'board-a', 1), (2, 'board-b', 1)",
        // Board 1: duplicates at 3 and 9, never dense. Board 2: clean.
        "INSERT INTO `columns` (`id`, `name`, `position`, `board_id`) VALUES \
         (1, 'col-A-1', 7, 1), (2, 'col-A-2', 3, 1), (3, 'col-A-3', 3, 1), \
         (4, 'col-A-4', 9, 1), (5, 'col-A-5', 9, 1), \
         (6, 'col-B-1', 0, 2), (7, 'col-B-2', 4, 2)",
        // Column 1: duplicates at 1 and 8; column 6: duplicate at 2.
        "INSERT INTO `cards` (`id`, `title`, `column_id`, `position`) VALUES \
         (1, 'c1', 1, 5), (2, 'c2', 1, 1), (3, 'c3', 1, 1), \
         (4, 'c4', 1, 4), (5, 'c5', 1, 8), (6, 'c6', 1, 8), \
         (7, 'c7', 6, 2), (8, 'c8', 6, 0), (9, 'c9', 6, 2)",
        "INSERT INTO `checklists` (`id`, `title`, `card_id`) VALUES (1, 'cl', 1)",
        "INSERT INTO `checklist_items` (`id`, `text`, `checklist_id`, `position`) VALUES \
         (1, 'i1', 1, 0), (2, 'i2', 1, 0), (3, 'i3', 1, 3), (4, 'i4', 1, 3), (5, 'i5', 1, 6)",
    ];
    for statement in statements {
        sqlx::query(statement)
            .execute(pool)
            .await
            .expect("seed pre-0003 row");
    }
}

/// The (id) order under the pre-migration `(position, id)` semantics and the
/// post-migration `(sort_key, id)` semantics, plus the `sort_key` dump — all
/// as literal statements (`assert_problem`-style explicitness; no dynamic
/// SQL, table names travel as constants, values are bound).
const POST_ORDER: &[(&str, &str, &str)] = &[
    (
        "columns",
        "board_id",
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    ),
    (
        "cards",
        "column_id",
        "SELECT `id` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    ),
    (
        "checklist_items",
        "checklist_id",
        "SELECT `id` FROM `checklist_items` WHERE `checklist_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    ),
];
const PRE_ORDER: &[(&str, &str, &str)] = &[
    (
        "columns",
        "board_id",
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `position`, `id`",
    ),
    (
        "cards",
        "column_id",
        "SELECT `id` FROM `cards` WHERE `column_id` = ? ORDER BY `position`, `id`",
    ),
    (
        "checklist_items",
        "checklist_id",
        "SELECT `id` FROM `checklist_items` WHERE `checklist_id` = ? ORDER BY `position`, `id`",
    ),
];
const KEYS: &[(&str, &str, &str)] = &[
    (
        "columns",
        "board_id",
        "SELECT `sort_key` FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key` ASC",
    ),
    (
        "cards",
        "column_id",
        "SELECT `sort_key` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC",
    ),
    (
        "checklist_items",
        "checklist_id",
        "SELECT `sort_key` FROM `checklist_items` WHERE `checklist_id` = ? ORDER BY `sort_key` ASC",
    ),
];

fn statement<'a>(
    list: &'a [(&'a str, &'a str, &'a str)],
    table: &str,
    parent_column: &str,
) -> &'a str {
    list.iter()
        .find(|&&(t, p, _)| t == table && p == parent_column)
        .map(|(_, _, sql)| *sql)
        .expect("statement for table")
}

#[tokio::test]
async fn backfill_preserves_order_and_generates_unique_monotonic_keys() {
    const TABLES: &[(&str, &str, &[i64])] = &[
        ("columns", "board_id", &[1, 2]),
        ("cards", "column_id", &[1, 6]),
        ("checklist_items", "checklist_id", &[1]),
    ];

    let scratch = scratch_name();
    let server = server_url();
    let scratch_db = scratch_url(&scratch);

    let admin = MySqlPool::connect(&server).await.expect("admin connection");
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE `{scratch}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )))
    .execute(&admin)
    .await
    .expect("create scratch database");
    admin.close().await;

    // The body runs as a spawned task so its scratch database is dropped on
    // the failure path too (F5, 2026-11-15): a panicking `.await` unwinds
    // past any cleanup placed after it, but the `JoinHandle` always resolves
    // — so cleanup runs, then the test fails with the body's panic message.
    let body = tokio::spawn(async move {
        let pool = MySqlPool::connect(&scratch_db)
            .await
            .expect("connect scratch");
        for file in ["0001_baseline.sql", "0002_convert_to_utf8mb4.sql"] {
            let source = std::fs::read_to_string(format!("./migrations/{file}"))
                .unwrap_or_else(|error| panic!("read {file}: {error}"));
            for statement in split_migration(&source) {
                sqlx::raw_sql(AssertSqlSafe(statement.clone()))
                    .execute(&pool)
                    .await
                    .unwrap_or_else(|error| {
                        panic!("apply statement of {file}: {statement:?}: {error}")
                    });
            }
        }
        seed_pre_0003(&pool).await;

        // Capture the pre-migration order (position, id) per parent.
        let mut pre_orders = Vec::new();
        for (table, parent_column, parents) in TABLES {
            for &parent in *parents {
                let sql = statement(PRE_ORDER, table, parent_column);
                let order: Vec<i64> = sqlx::query_scalar(sql)
                    .bind(parent)
                    .fetch_all(&pool)
                    .await
                    .expect("read pre-migration order");
                pre_orders.push((*table, *parent_column, parent, order));
            }
        }

        // Apply 0003 statement-by-statement (the splitter is exercised here on
        // purpose; the in-app migrator applies the same bytes as one file).
        let source = std::fs::read_to_string("./migrations/0003_sort_key_fractional_ordering.sql")
            .expect("read 0003");
        for statement in split_migration(&source) {
            sqlx::raw_sql(AssertSqlSafe(statement.clone()))
                .execute(&pool)
                .await
                .unwrap_or_else(|error| panic!("apply statement of 0003: {statement:?}: {error}"));
        }

        // 1. Order preserved: the id order under `sort_key` must equal the
        //    pre-migration (position, id) order.
        for (table, parent_column, parent, pre_order) in &pre_orders {
            let sql = statement(POST_ORDER, table, parent_column);
            let post_order: Vec<i64> = sqlx::query_scalar(sql)
                .bind(*parent)
                .fetch_all(&pool)
                .await
                .expect("read post-migration order");
            assert_eq!(
                &post_order, pre_order,
                "{table} parent {parent}: pre-migration order preserved"
            );
        }

        // 2. Keys unique per parent, 3. first key is `c000`, 4. strictly
        //    monotonic, 5. adjacent pairs round-trip through `key_between`.
        for (table, parent_column, parents) in TABLES {
            for &parent in *parents {
                let sql = statement(KEYS, table, parent_column);
                let keys: Vec<String> = sqlx::query_scalar(sql)
                    .bind(parent)
                    .fetch_all(&pool)
                    .await
                    .expect("read sort_key column");
                let unique: std::collections::BTreeSet<&String> = keys.iter().collect();
                assert_eq!(
                    keys.len(),
                    unique.len(),
                    "{table} parent {parent}: keys unique"
                );
                assert_eq!(
                    keys.first().map(String::as_str),
                    Some("c000"),
                    "{table} parent {parent}: first key"
                );
                for pair in keys.windows(2) {
                    assert!(
                        pair[0] < pair[1],
                        "{table} parent {parent}: monotonic: {pair:?}"
                    );
                }
                for pair in keys.windows(2) {
                    let between =
                        backend_rs::common::order_key::key_between(Some(&pair[0]), Some(&pair[1]))
                            .unwrap_or_else(|error| {
                                panic!("key_between({}, {}): {error}", pair[0], pair[1])
                            });
                    assert!(
                        between > pair[0] && between < pair[1],
                        "between key {between} for {pair:?}"
                    );
                }
            }
        }

        // 6. `position` no longer exists on any of the three tables.
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.columns \
             WHERE table_schema = DATABASE() AND column_name = 'position' \
               AND table_name IN ('columns', 'cards', 'checklist_items')",
        )
        .fetch_one(&pool)
        .await
        .expect("information_schema check");
        assert_eq!(remaining, 0, "no `position` column may remain");

        // 7. The `sort_key` column is `ascii_bin` (the load-bearing collation).
        let rows = sqlx::query(
            "SELECT table_name AS tbl, collation_name AS coll FROM information_schema.columns \
             WHERE table_schema = DATABASE() AND column_name = 'sort_key' \
               AND table_name IN ('columns', 'cards', 'checklist_items')",
        )
        .fetch_all(&pool)
        .await
        .expect("collation check");
        for row in &rows {
            let table: String = row.get("tbl");
            let collation: Option<String> = row.get("coll");
            assert_eq!(
                collation.as_deref(),
                Some("ascii_bin"),
                "{table}.sort_key must use ascii_bin"
            );
        }

        pool.close().await;
    });

    // F5 (2026-11-15): cleanup runs on BOTH paths — a panicking body resolves
    // to a `JoinError` here instead of unwinding past the drop.
    let joined = body.await;
    let admin = MySqlPool::connect(&server)
        .await
        .expect("admin reconnect for cleanup");
    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS `{scratch}`"
    )))
    .execute(&admin)
    .await
    .expect("drop scratch database");
    admin.close().await;

    if let Err(error) = joined {
        // Re-throw the body's panic with its own message preserved.
        let payload = error.into_panic();
        let message = match payload.downcast::<String>() {
            Ok(text) => *text,
            Err(other) => match other.downcast::<&'static str>() {
                Ok(text) => text.to_string(),
                Err(_) => "unknown panic in backfill body".to_string(),
            },
        };
        panic!("{message}");
    }
}

/// Fresh per-test database (all migrations applied): board create seeds the
/// 3 default columns and the columns list answers end to end.
#[sqlx::test(migrations = "./migrations")]
async fn fresh_database_migrates_then_board_crud_works(pool: MySqlPool) {
    let app = app_on_pool(&pool);
    let (jar, _user) = register(app, "sortkey-fresh@example.com").await;
    let response = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(jar.as_str()),
            Some(json_body(&serde_json::json!({ "name": "SortKey Board" }))),
        ),
    )
    .await;
    assert_eq!(response.0, axum::http::StatusCode::CREATED);
    let body = json_of(&response.2);
    let columns = body["data"]["columns"]
        .as_array()
        .expect("columns on the board body");
    let positions: Vec<i64> = columns
        .iter()
        .map(|column| column["position"].as_i64().expect("integer position"))
        .collect();
    assert_eq!(positions, vec![0, 1, 2], "default columns are dense");
}
