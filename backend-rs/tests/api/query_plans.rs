//! B2 — query plan regression guards (shape only: index used, no full scan).
//!
//! T029 note: its conditional index migration can flip these plan shapes in
//! either direction — re-check after it lands.
//!
//! Runs inside the per-test ephemeral DB (`#[sqlx::test]`). Never on a
//! shared/persistent schema.

#![allow(missing_docs, unreachable_pub)]

use sqlx::{MySqlPool, Row};

/// Concrete ids returned from `seed` so EXPLAINs bind to rows that actually exist.
struct SeedIds {
    board_id: i64,
    column_id: i64,
}

async fn seed(pool: &MySqlPool) -> SeedIds {
    sqlx::query("INSERT INTO `users` (`email`, `password`) VALUES (?, ?)")
        .bind("qp@test.local")
        .bind("x")
        .execute(pool)
        .await
        .expect("user");
    let user_id: i64 = sqlx::query_scalar("SELECT `id` FROM `users` WHERE `email` = ?")
        .bind("qp@test.local")
        .fetch_one(pool)
        .await
        .expect("user id");

    // Seed multiple boards so the optimizer actually uses the per-parent
    // indexes instead of scanning tiny tables.
    for b in 0..5 {
        sqlx::query("INSERT INTO `boards` (`name`, `user_id`) VALUES (?, ?)")
            .bind(format!("qp-board-{b}"))
            .bind(user_id)
            .execute(pool)
            .await
            .expect("board");
    }
    let board_id: i64 = sqlx::query_scalar("SELECT `id` FROM `boards` WHERE `name` = ?")
        .bind("qp-board-0")
        .fetch_one(pool)
        .await
        .expect("board id");

    for b in 0..5 {
        let bid: i64 = sqlx::query_scalar("SELECT `id` FROM `boards` WHERE `name` = ?")
            .bind(format!("qp-board-{b}"))
            .fetch_one(pool)
            .await
            .expect("bid");
        for i in 0..20 {
            sqlx::query("INSERT INTO `columns` (`name`, `sort_key`, `board_id`) VALUES (?, ?, ?)")
                .bind(format!("col-{i}"))
                .bind(format!("c{i:02}"))
                .bind(bid)
                .execute(pool)
                .await
                .expect("column");
        }
    }
    let col_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(board_id)
    .fetch_all(pool)
    .await
    .expect("col ids");

    for &cid in &col_ids {
        for j in 0..50 {
            sqlx::query("INSERT INTO `cards` (`title`, `sort_key`, `column_id`) VALUES (?, ?, ?)")
                .bind(format!("card-{j}"))
                .bind(format!("a{j:03}"))
                .bind(cid)
                .execute(pool)
                .await
                .expect("card");
        }
    }

    // 50 notes per board
    sqlx::query("INSERT INTO `tags` (`name`, `user_id`) VALUES (?, ?)")
        .bind("t")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("tag");
    let tag_id: i64 = sqlx::query_scalar("SELECT `id` FROM `tags` WHERE `user_id` = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("tag id");
    for b in 0..5 {
        let bid: i64 = sqlx::query_scalar("SELECT `id` FROM `boards` WHERE `name` = ?")
            .bind(format!("qp-board-{b}"))
            .fetch_one(pool)
            .await
            .expect("bid");
        for i in 0..50 {
            sqlx::query(
                "INSERT INTO `notes` (`title`, `content`, `board_id`, `user_id`) VALUES (?, ?, ?, ?)",
            )
            .bind(format!("note-{b}-{i}"))
            .bind("content")
            .bind(bid)
            .bind(user_id)
            .execute(pool)
            .await
            .expect("note");
            let note_id: i64 = sqlx::query_scalar("SELECT `id` FROM `notes` WHERE `title` = ?")
                .bind(format!("note-{b}-{i}"))
                .fetch_one(pool)
                .await
                .expect("note id");
            sqlx::query("INSERT INTO `note_tags` (`note_id`, `tag_id`) VALUES (?, ?)")
                .bind(note_id)
                .bind(tag_id)
                .execute(pool)
                .await
                .expect("note_tag");
        }
    }

    let _ = sqlx::query("ANALYZE TABLE `boards`, `columns`, `cards`, `notes`, `tags`, `note_tags`")
        .execute(pool)
        .await;

    SeedIds {
        board_id,
        column_id: *col_ids.first().expect("at least one column"),
    }
}

fn assert_indexed_plan(rows: &[sqlx::mysql::MySqlRow]) {
    for r in rows {
        let ty: Option<String> = r.try_get("type").ok().flatten();
        let key: Option<String> = r.try_get("key").ok().flatten();
        match ty.as_deref() {
            Some("ALL") => panic!("full table scan detected: type={ty:?}, key={key:?}"),
            // Reject full index scans too: these hot paths should seek/filter by index,
            // not scan every index entry. Portable across MySQL 8.4/MariaDB 11.4 tabular EXPLAIN.
            Some("index") => panic!("full index scan detected: type={ty:?}, key={key:?}"),
            Some(_) => {}
            None => panic!("missing EXPLAIN type column: key={key:?}"),
        }
        assert!(
            key.as_deref().is_some_and(|k| !k.trim().is_empty()),
            "expected index access but key missing/empty: type={ty:?}, key={key:?}"
        );
    }
}

/// Row for `table_name` in a tabular EXPLAIN (both engines name the column
/// `table`; its VALUE is the alias used in the query when there is one —
/// `n` for `FROM notes n` — so callers pass the alias).
fn plan_row<'a>(rows: &'a [sqlx::mysql::MySqlRow], table_name: &str) -> &'a sqlx::mysql::MySqlRow {
    rows.iter()
        .find(|r| {
            r.try_get::<Option<String>, _>("table")
                .ok()
                .flatten()
                .as_deref()
                == Some(table_name)
        })
        .unwrap_or_else(|| panic!("no EXPLAIN row for table {table_name}"))
}

/// Applicability guard for an **unforced** plan: `table_name` must list
/// `index_name` among its `possible_keys`. The optimizer may still choose a
/// scan on small seeded datasets (MariaDB does for the notes list), so this
/// asserts "the index exists and is usable for this shape" without hinting.
fn assert_possible_index(rows: &[sqlx::mysql::MySqlRow], table_name: &str, index_name: &str) {
    let possible: Option<String> = plan_row(rows, table_name)
        .try_get("possible_keys")
        .ok()
        .flatten();
    let possible = possible.unwrap_or_default();
    assert!(
        possible.split(',').any(|k| k.trim() == index_name),
        "expected {index_name} in possible_keys for {table_name}: {possible:?}"
    );
}

/// Access-path guard for a **hinted** plan: the `table_name` row must have
/// selected exactly `index_name` (a non-empty `key` alone would let any other
/// index satisfy the assertion).
fn assert_selected_index(rows: &[sqlx::mysql::MySqlRow], table_name: &str, index_name: &str) {
    let key: Option<String> = plan_row(rows, table_name).try_get("key").ok().flatten();
    assert_eq!(
        key.as_deref(),
        Some(index_name),
        "expected {table_name} to select {index_name}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_board_columns_load(pool: MySqlPool) {
    let ids = seed(&pool).await;
    // board detail + columns (two queries)
    let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(
        "EXPLAIN SELECT `id`, `name` FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(ids.board_id)
    .fetch_all(&pool)
    .await
    .expect("explain");
    assert_indexed_plan(&rows);
    // EXPLAIN on a board lookup by id
    let rows: Vec<sqlx::mysql::MySqlRow> =
        sqlx::query("EXPLAIN SELECT `id`, `name` FROM `boards` WHERE `id` = ?")
            .bind(ids.board_id)
            .fetch_all(&pool)
            .await
            .expect("explain");
    assert_indexed_plan(&rows);
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_cards_move_transaction(pool: MySqlPool) {
    let ids = seed(&pool).await;
    // last-key read + neighbour read shape: SELECT cards by column ordered by sort_key
    let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(
        "EXPLAIN SELECT `id`, `sort_key` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(ids.column_id)
    .fetch_all(&pool)
    .await
    .expect("explain");
    assert_indexed_plan(&rows);
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_pagination_open_call(pool: MySqlPool) {
    let ids = seed(&pool).await;
    let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(
        "EXPLAIN SELECT `id`, `title` FROM `cards` WHERE `column_id` = ? AND `sort_key` > ? ORDER BY `sort_key` ASC LIMIT 20",
    )
    .bind(ids.column_id)
    .bind("a000")
    .fetch_all(&pool)
    .await
    .expect("explain");
    assert_indexed_plan(&rows);
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_notes_list_left_join(pool: MySqlPool) {
    let ids = seed(&pool).await;
    // Shape note: the production list query is additionally user-scoped and
    // DISTINCT (notes::store::note_list_from + push_note_filter) — this guard
    // pins the board-filtered `updated_at DESC, id DESC` ordering shape that
    // IDX_notes_board_updated serves.
    //
    // The two EXPLAINs differ ONLY by the FORCE INDEX hint (`sqlx::query`
    // takes `impl SqlSafeStr` — a `&'static str` — so no format!).
    /// No hint: what the optimizer actually does with this shape.
    const EXPLAIN_UNFORCED: &str = "EXPLAIN SELECT n.`id`, n.`title`, t.`name` FROM `notes` n LEFT JOIN `note_tags` nt ON nt.`note_id` = n.`id` LEFT JOIN `tags` t ON t.`id` = nt.`tag_id` WHERE n.`board_id` = ? ORDER BY n.`updated_at` DESC, n.`id` DESC";
    /// Forced: the access path the index can deliver for this shape.
    const EXPLAIN_FORCED: &str = "EXPLAIN SELECT n.`id`, n.`title`, t.`name` FROM `notes` n FORCE INDEX (`IDX_notes_board_updated`) LEFT JOIN `note_tags` nt ON nt.`note_id` = n.`id` LEFT JOIN `tags` t ON t.`id` = nt.`tag_id` WHERE n.`board_id` = ? ORDER BY n.`updated_at` DESC, n.`id` DESC";

    // 1) Applicability, unforced: on small seeded datasets MariaDB's optimizer
    //    may still pick ALL+filesort (CI evidence: MariaDB 11.4 leg, 2026-11-20)
    //    despite the index being usable — so assert candidacy via possible_keys
    //    instead of forcing a plan choice here.
    let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(EXPLAIN_UNFORCED)
        .bind(ids.board_id)
        .fetch_all(&pool)
        .await
        .expect("explain (unforced)");
    assert_possible_index(&rows, "n", "IDX_notes_board_updated");

    // 2) Access path, forced: FORCE INDEX stabilizes the plan across MySQL 8.4 /
    //    MariaDB 11.4 so the "every row indexed" invariant can still be checked
    //    (this guard asserts the index is usable for the shape — not that the
    //    optimizer would choose it).
    let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(EXPLAIN_FORCED)
        .bind(ids.board_id)
        .fetch_all(&pool)
        .await
        .expect("explain (forced)");
    assert_indexed_plan(&rows);
    assert_selected_index(&rows, "n", "IDX_notes_board_updated");
}
