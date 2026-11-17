//! T010 — adjacent-unchanged proofs for fractional sort keys.
//!
//! For every sibling of an affected parent, capture `sort_key` before and
//! after the operation and assert BYTE identity — the core no-adjacent-impact
//! proof. Only the intended row(s) may change. Wire positions stay dense
//! 0..N (computed, D1), so the public behaviour is unchanged from the old
//! integer scheme.
//!
//! Also covered: one persistence assertion per operation (AGENTS.md), the
//! two-phase rebalance under a forced 64-char bloat (no transient unique
//! violation, evenly distributed final keys, the `> 64` trigger fires), and
//! the `PATCH` move endpoints for columns and checklist items.

use super::helpers::{app_on_pool, authed_request, json_body, json_of, register, send};
use axum::http::StatusCode;
use sqlx::{Connection, MySqlPool};

/// Capture `(id, sort_key)` pairs in `sort_key` order — the snapshot used by
/// every adjacent-unchanged assertion.
async fn card_keys(pool: &MySqlPool, column_id: i64) -> Vec<(i64, String)> {
    sqlx::query_as(
        "SELECT `id`, `sort_key` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(column_id)
    .fetch_all(pool)
    .await
    .expect("card sort keys re-read")
}

async fn column_keys(pool: &MySqlPool, board_id: i64) -> Vec<(i64, String)> {
    sqlx::query_as(
        "SELECT `id`, `sort_key` FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(board_id)
    .fetch_all(pool)
    .await
    .expect("column sort keys re-read")
}

async fn item_keys(pool: &MySqlPool, checklist_id: i64) -> Vec<(i64, String)> {
    sqlx::query_as(
        "SELECT `id`, `sort_key` FROM `checklist_items` WHERE `checklist_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(checklist_id)
    .fetch_all(pool)
    .await
    .expect("item sort keys re-read")
}

/// Create one board with 3 default columns and one card column full of
/// cards via the API; returns `(board_id, column_ids, card_ids in wire order)`.
async fn seed_board_with_cards(
    pool: &MySqlPool,
    jar: &str,
    name: &str,
    card_titles: &[&str],
) -> (i64, Vec<i64>, Vec<i64>) {
    let app = app_on_pool(pool);
    let (status, _, bytes) = send(
        app,
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(jar),
            Some(json_body(&serde_json::json!({ "name": name }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board: {created}");
    let board_id = created["data"]["id"].as_i64().expect("board id");
    let col_ids: Vec<i64> = created["data"]["columns"]
        .as_array()
        .expect("columns[]")
        .iter()
        .map(|column| column["id"].as_i64().expect("column id"))
        .collect();
    let mut card_ids = Vec::with_capacity(card_titles.len());
    for title in card_titles {
        let (status, _, bytes) = send(
            app_on_pool(pool),
            authed_request(
                "POST",
                "/api/v1/cards",
                Some(jar),
                Some(json_body(&serde_json::json!({
                    "title": title,
                    "column_id": col_ids[0],
                }))),
            ),
        )
        .await;
        let card = json_of(&bytes);
        assert_eq!(status, StatusCode::CREATED, "card {title}: {card}");
        card_ids.push(card["data"]["id"].as_i64().expect("card id"));
    }
    (board_id, col_ids, card_ids)
}

/// Build a checklist on the first card of a board scope, add items.
async fn seed_checklist_with_items(
    pool: &MySqlPool,
    jar: &str,
    card_id: i64,
    items: &[&str],
) -> (i64, Vec<i64>) {
    let app = app_on_pool(pool);
    let (status, _, bytes) = send(
        app,
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(jar),
            Some(json_body(&serde_json::json!({ "title": "Acceptance" }))),
        ),
    )
    .await;
    let checklist = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "checklist: {checklist}");
    let checklist_id = checklist["data"]["id"].as_i64().expect("checklist id");
    let mut item_ids = Vec::with_capacity(items.len());
    for text in items {
        let (status, _, bytes) = send(
            app_on_pool(pool),
            authed_request(
                "POST",
                &format!("/api/v1/checklists/{checklist_id}/items"),
                Some(jar),
                Some(json_body(&serde_json::json!({ "text": text }))),
            ),
        )
        .await;
        let item = json_of(&bytes);
        assert_eq!(status, StatusCode::CREATED, "item {text}: {item}");
        item_ids.push(item["data"]["id"].as_i64().expect("item id"));
    }
    (checklist_id, item_ids)
}

// ---------------------------------------------------------------------------
// cards
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn create_at_index_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-create-owner@example.com").await;
    let (_board, col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Create at index", &["A", "B", "C"]).await;
    let column = col_ids[0];
    let before = card_keys(&pool, column).await;

    // Insert 'D' at index 1 (between A and B).
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "D", "column_id": column, "position": 1,
            }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "insert D: {created}");
    let d = created["data"]["id"].as_i64().expect("D id");
    assert_eq!(
        created["data"]["position"], 1,
        "D lands at index 1: {created}"
    );

    let after = card_keys(&pool, column).await;
    // Sibling keys byte-identical; only the new row was added.
    for (id, key) in before {
        let seen = after
            .iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, key)| key.as_str());
        assert_eq!(seen, Some(key.as_str()), "sibling {id} kept its sort_key");
    }
    let d_key = after
        .iter()
        .find(|(id, _)| *id == d)
        .expect("D row")
        .1
        .clone();
    let a_key = after
        .iter()
        .find(|(id, _)| *id == card_ids[0])
        .expect("A key")
        .1
        .clone();
    let b_key = after
        .iter()
        .find(|(id, _)| *id == card_ids[1])
        .expect("B key")
        .1
        .clone();
    assert!(
        a_key < d_key && d_key < b_key,
        "D's key sits between A and B: {a_key} < {d_key} < {b_key}"
    );
    // Persistence: second GET reflects the insert.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request("GET", &format!("/api/v1/cards/{d}"), Some(&owner_jar), None),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "second GET: {detail}");
    assert_eq!(detail["data"]["position"], 1, "persisted at index 1");
}

#[sqlx::test(migrations = "./migrations")]
async fn same_column_move_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-move-owner@example.com").await;
    let (_board, col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Same column move", &["A", "B", "C", "D"]).await;
    let column = col_ids[0];
    let before = card_keys(&pool, column).await;

    // Move D from index 3 to index 1.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{}", card_ids[3]),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 1 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "move D: {moved}");
    assert_eq!(moved["data"]["position"], 1, "D re-indexed to 1: {moved}");

    let after = card_keys(&pool, column).await;
    for (id, key) in before {
        if id == card_ids[3] {
            continue;
        }
        let seen = after
            .iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, key)| key.as_str());
        assert_eq!(seen, Some(key.as_str()), "sibling {id} kept its sort_key");
    }
    let d_key = after
        .iter()
        .find(|(id, _)| *id == card_ids[3])
        .expect("D")
        .1
        .clone();
    let a_key = after
        .iter()
        .find(|(id, _)| *id == card_ids[0])
        .expect("A")
        .1
        .clone();
    let b_key = after
        .iter()
        .find(|(id, _)| *id == card_ids[1])
        .expect("B")
        .1
        .clone();
    assert!(
        a_key < d_key && d_key < b_key,
        "D's key sits between A and B: {a_key} < {d_key} < {b_key}"
    );
    // Wire order persisted: A, D, B, C.
    let (_status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{column}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let page = json_of(&bytes);
    let titles: Vec<&str> = page["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|card| card["title"].as_str().expect("title"))
        .collect();
    assert_eq!(
        titles,
        ["A", "D", "B", "C"],
        "persisted column order: {page}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn cross_column_move_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-xcol-owner@example.com").await;
    let (_board, col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Cross column move", &["A", "B", "C"]).await;
    let (source, target) = (col_ids[0], col_ids[1]);
    // One card in the target so the between-neighbours write is exercised.
    let (_status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "X", "column_id": target,
            }))),
        ),
    )
    .await;
    let x_body = json_of(&bytes);
    let x_id = x_body["data"]["id"].as_i64().expect("X id");
    let source_before = card_keys(&pool, source).await;
    let target_before = card_keys(&pool, target).await;

    // Move B from source to target at index 0 (before X).
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{}", card_ids[1]),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "column_id": target, "position": 0,
            }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "cross-column move: {moved}");
    assert_eq!(
        moved["data"]["position"], 0,
        "B at index 0 in target: {moved}"
    );

    let source_after = card_keys(&pool, source).await;
    let target_after = card_keys(&pool, target).await;
    // Source siblings unchanged (A, C keep keys; B is gone from source).
    for (id, key) in source_before {
        if id == card_ids[1] {
            continue;
        }
        let seen = source_after
            .iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, key)| key.as_str());
        assert_eq!(
            seen,
            Some(key.as_str()),
            "source sibling {id} kept its sort_key"
        );
    }
    // Target sibling unchanged (X keeps its key).
    let x_before = target_before
        .iter()
        .find(|(id, _)| *id == x_id)
        .expect("X")
        .1
        .clone();
    let x_after = target_after
        .iter()
        .find(|(id, _)| *id == x_id)
        .expect("X after")
        .1
        .clone();
    assert_eq!(x_before, x_after, "target sibling X kept its sort_key");
    // B's new key is below X's key (index 0).
    let b_key = target_after
        .iter()
        .find(|(id, _)| *id == card_ids[1])
        .expect("B")
        .1
        .clone();
    assert!(b_key < x_after, "B's key is below X's: {b_key} < {x_after}");
    // Persistence: second GET of B reflects the new column.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{}", card_ids[1]),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "second GET: {detail}");
    assert_eq!(detail["data"]["column_id"], target, "B persisted in target");
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-delete-owner@example.com").await;
    let (_board, col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Delete sibling", &["A", "B", "C", "D"]).await;
    let column = col_ids[0];
    let before = card_keys(&pool, column).await;

    let (status, _, _bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{}", card_ids[1]),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let after = card_keys(&pool, column).await;
    assert_eq!(after.len(), 3, "one row deleted");
    for (id, key) in before {
        if id == card_ids[1] {
            continue;
        }
        let seen = after
            .iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, key)| key.as_str());
        assert_eq!(seen, Some(key.as_str()), "sibling {id} kept its sort_key");
    }
    // Wire order is dense 0..N without the deleted row.
    let (_status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{column}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let page = json_of(&bytes);
    let positions: Vec<i64> = page["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|card| card["position"].as_i64().expect("position"))
        .collect();
    assert_eq!(positions, vec![0, 1, 2], "wire order dense: {page}");
}

#[sqlx::test(migrations = "./migrations")]
async fn bulk_move_cards_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-bulk-owner@example.com").await;
    let (_board, col_ids, _card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Bulk move", &["A", "B"]).await;
    let (source, target) = (col_ids[0], col_ids[1]);
    // Existing card in the target so appended keys must follow it.
    let (_status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "X", "column_id": target,
            }))),
        ),
    )
    .await;
    let existing = json_of(&bytes);
    let x_id = existing["data"]["id"].as_i64().expect("X id");
    let target_before = card_keys(&pool, target).await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{source}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({ "target_column_id": target }),
            )),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "move-cards: {moved}");
    assert_eq!(moved["data"]["moved_count"], 2, "two cards moved: {moved}");

    // Source drained; sibling keys of X untouched; moved cards appended in
    // source order with keys past X's key.
    let target_after = card_keys(&pool, target).await;
    assert_eq!(target_after.len(), 3, "existing + two moved");
    let x_before = target_before
        .iter()
        .find(|(id, _)| *id == x_id)
        .expect("X")
        .1
        .clone();
    let x_after = target_after
        .iter()
        .find(|(id, _)| *id == x_id)
        .expect("X")
        .1
        .clone();
    assert_eq!(x_before, x_after, "target sibling kept its sort_key");
    let titles_by_key: Vec<String> = sqlx::query_scalar(
        "SELECT `title` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(target)
    .fetch_all(&pool)
    .await
    .expect("target order re-read");
    assert_eq!(
        titles_by_key,
        ["X", "A", "B"],
        "source order mirrored at the tail"
    );
    // Persistence: the source is empty.
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
        .bind(source)
        .fetch_one(&pool)
        .await
        .expect("source count re-read");
    assert_eq!(remaining, 0, "source drained");
}

#[sqlx::test(migrations = "./migrations")]
async fn sort_order_rewrite_reseeds_evenly(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-sort-owner@example.com").await;
    let (_board, col_ids, _card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Sort rewrite", &[]).await;
    let column = col_ids[0];
    // Seed via SQL with created_at scrambled against the insertion order.
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `sort_key`, `created_at`) VALUES \
         ('Sort C', ?, 'a0', '2026-01-03 00:00:00.000000'), \
         ('Sort A', ?, 'a1', '2026-01-01 00:00:00.000000'), \
         ('Sort B', ?, 'a2', '2026-01-02 00:00:00.000000')",
    )
    .bind(column)
    .bind(column)
    .bind(column)
    .execute(&pool)
    .await
    .expect("seed scrambled cards");

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{column}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "sort_order": "asc" }))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "sort asc: {}", json_of(&bytes));
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT `sort_key` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(column)
    .fetch_all(&pool)
    .await
    .expect("keys after rewrite");
    assert_eq!(
        keys,
        vec!["a0", "a1", "a2"],
        "keys are reseeded evenly in created_at ASC order (the two-phase rewrite)"
    );
    let titles: Vec<String> = sqlx::query_scalar(
        "SELECT `title` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(column)
    .fetch_all(&pool)
    .await
    .expect("titles after rewrite");
    assert_eq!(
        titles,
        ["Sort A", "Sort B", "Sort C"],
        "created_at ASC order"
    );
}

// ---------------------------------------------------------------------------
// columns: PATCH {position} move endpoint
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn column_move_endpoint_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-colmove-owner@example.com").await;
    let (board_id, _col_ids, _card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Column move", &[]).await;
    // Create one extra column so four exist (To Do, In Progress, Done, Backlog).
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "name": "Backlog" }))),
        ),
    )
    .await;
    let extra = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "extra column: {extra}");
    let before = column_keys(&pool, board_id).await;
    let to_do = before[0].clone();

    // Move To Do from index 0 to index 2.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{}", to_do.0),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 2 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "column move: {moved}");
    assert_eq!(
        moved["data"]["position"], 2,
        "To Do re-indexed to 2: {moved}"
    );

    let after = column_keys(&pool, board_id).await;
    for (id, key) in before {
        if id == to_do.0 {
            continue;
        }
        let seen = after
            .iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, key)| key.as_str());
        assert_eq!(
            seen,
            Some(key.as_str()),
            "sibling column {id} kept its sort_key"
        );
    }
    let (_status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    let names: Vec<&str> = listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|column| column["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        ["In Progress", "Done", "To Do", "Backlog"],
        "column order persisted: {listed}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn column_move_endpoint_clamps_out_of_range(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-clamp-owner@example.com").await;
    let (board_id, _col_ids, _card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Column clamp", &[]).await;
    let before = column_keys(&pool, board_id).await;

    // position=999 clamps to the last index (never 400).
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{}", before[0].0),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 999 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "clamped move: {moved}");
    assert_eq!(moved["data"]["position"], 2, "clamped to N-1: {moved}");
    // Sibling keys byte-identical.
    let after = column_keys(&pool, board_id).await;
    for (id, key) in &before {
        if *id == before[0].0 {
            continue; // the moving row
        }
        let seen = after
            .iter()
            .find(|(seen_id, _)| *seen_id == *id)
            .map(|(_, key)| key.as_str());
        assert_eq!(
            seen,
            Some(key.as_str()),
            "sibling column {id} kept its sort_key"
        );
    }
    // Negative position clamps to 0.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{}", before[2].0),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": -3 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "negative clamped move: {moved}");
    assert_eq!(moved["data"]["position"], 0, "clamped to 0: {moved}");
}

// ---------------------------------------------------------------------------
// checklist items: PATCH {position} move endpoint
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn checklist_item_move_endpoint_leaves_sibling_keys_untouched(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-itemmove-owner@example.com").await;
    let (_board, _col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Item move", &["A"]).await;
    let card = card_ids[0];
    let (checklist_id, item_ids) =
        seed_checklist_with_items(&pool, &owner_jar, card, &["i1", "i2", "i3", "i4"]).await;
    let before = item_keys(&pool, checklist_id).await;

    // Move the second item to the end (index 3).
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{}", item_ids[1]),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 3 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "item move: {moved}");
    assert_eq!(
        moved["data"]["position"], 3,
        "item re-indexed to 3: {moved}"
    );

    let after = item_keys(&pool, checklist_id).await;
    for (id, key) in before {
        if id == item_ids[1] {
            continue;
        }
        let seen = after
            .iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, key)| key.as_str());
        assert_eq!(
            seen,
            Some(key.as_str()),
            "sibling item {id} kept its sort_key"
        );
    }
    // Persisted order via the checklist detail endpoint.
    let (_status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    let texts: Vec<&str> = detail["data"]["items"]
        .as_array()
        .expect("items[]")
        .iter()
        .map(|item| item["text"].as_str().expect("text"))
        .collect();
    assert_eq!(
        texts,
        ["i1", "i3", "i4", "i2"],
        "item order persisted: {detail}"
    );
    // Persistence assertion (second read of the same endpoint is stable).
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "second GET: {}", json_of(&bytes));
}

#[sqlx::test(migrations = "./migrations")]
async fn checklist_item_move_endpoint_clamps_out_of_range(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-itemclamp-owner@example.com").await;
    let (_board, _col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Item clamp", &["A"]).await;
    let card = card_ids[0];
    let (_checklist_id, item_ids) =
        seed_checklist_with_items(&pool, &owner_jar, card, &["i1", "i2", "i3"]).await;

    // position=999 clamps to the last index; position=-5 clamps to 0.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{}", item_ids[0]),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 999 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "clamped item move: {moved}");
    assert_eq!(moved["data"]["position"], 2, "clamped to N-1: {moved}");
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{}", item_ids[2]),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": -5 }))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "negative clamped move: {moved}");
    assert_eq!(moved["data"]["position"], 0, "clamped to 0: {moved}");
}

// ---------------------------------------------------------------------------
// rebalance: the 64-char trigger + two-phase rewrite
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn rebalance_triggers_on_a_bloated_gap_and_rewrites_evenly(pool: MySqlPool) {
    let (owner_jar, _) = register(app_on_pool(&pool), "t010-rebalance-owner@example.com").await;
    let (_board, col_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "Rebalance", &["A", "B"]).await;
    let column = col_ids[0];
    let bloat = format!("a0{}1", "0".repeat(69));
    let bloat_upper = format!("a0{}2", "0".repeat(69));
    assert!(
        bloat.len() > 64,
        "the bloated neighbour key crosses the threshold"
    );
    sqlx::query("UPDATE `cards` SET `sort_key` = ? WHERE `id` = ?")
        .bind(&bloat)
        .bind(card_ids[0])
        .execute(&pool)
        .await
        .expect("bloat lower neighbour");
    sqlx::query("UPDATE `cards` SET `sort_key` = ? WHERE `id` = ?")
        .bind(&bloat_upper)
        .bind(card_ids[1])
        .execute(&pool)
        .await
        .expect("bloat upper neighbour");

    // Insert between the bloated neighbours: the midpoint exceeds the 64-char
    // threshold, so the rebalance fires inside the create transaction.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "New", "column_id": column, "position": 1,
            }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "insert into bloated gap: {created}"
    );

    // After the rebalance, every key of the parent is evenly distributed and
    // far below the threshold (single-digit-ish keys for 3 rows).
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT `sort_key` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(column)
    .fetch_all(&pool)
    .await
    .expect("keys after rebalance");
    assert_eq!(keys.len(), 3, "three rows after the insert");
    assert!(
        keys.iter().all(|key| key.len() <= 8),
        "all keys evenly reseeded well below the threshold: {keys:?}"
    );
    let unique: std::collections::BTreeSet<&String> = keys.iter().collect();
    assert_eq!(
        keys.len(),
        unique.len(),
        "keys unique after the two-phase rewrite"
    );
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "keys strictly increasing after the rewrite");
    // The titles stayed in their rebalance order: A, New, B.
    let titles: Vec<String> = sqlx::query_scalar(
        "SELECT `title` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(column)
    .fetch_all(&pool)
    .await
    .expect("titles after rebalance");
    assert_eq!(titles, ["A", "New", "B"], "relative order preserved");
    // Persistence: the new card is at wire index 1.
    let new_id = created["data"]["id"].as_i64().expect("New id");
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{new_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "second GET: {detail}");
    assert_eq!(
        detail["data"]["position"], 1,
        "persisted at index 1: {detail}"
    );
}

/// F1 regression (post-T010 review, 2026-11-15): a concurrent transaction
/// that INSERTs into the parent between phase 0 (read + lock) and phase 1
/// (park) must NOT be parked at a temporary key — park/assign act on the
/// phase-0 capture only. Reproduces the interleaving deterministically with
/// two connections at `READ COMMITTED`; the concurrent insert goes through
/// the create-at-index path (`position: 0`, unlocked neighbour reads) because
/// the append path (`LIMIT 1 FOR UPDATE`) would block behind phase 0's locks.
#[sqlx::test(migrations = "./migrations")]
async fn rebalance_leaves_a_mid_rebalance_insert_untouched(pool: MySqlPool) {
    use backend_rs::common::rebalance::{RebalanceScope, phase_assign, phase_park, phase_read};

    let app = app_on_pool(&pool);
    let (owner_jar, _) = register(app, "t010-rebalance-concurrent@example.com").await;
    let (_board_id, column_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "F1 concurrent rebalance", &["A", "B"]).await;
    let column_id = i32::try_from(column_ids[0]).expect("column id fits i32");
    let scope = RebalanceScope::Cards { column_id };

    // Connection A: phase 0 — read + lock the captured ids under RC.
    let mut a = pool.acquire().await.expect("acquire A");
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *a)
        .await
        .expect("isolation A");
    let mut tx_a = a.begin().await.expect("begin A");
    let captured = phase_read(&mut tx_a, scope).await.expect("phase_read");
    let seeded: Vec<i32> = card_ids
        .iter()
        .map(|id| i32::try_from(*id).expect("card id fits i32"))
        .collect();
    assert_eq!(captured, seeded, "phase 0 captures the seeded cards");

    // Connection B (via the API, an independent transaction): insert at
    // index 0 — commits before A parks/assigns.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Concurrent",
                "column_id": i64::from(column_id),
                "position": 0,
            }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "concurrent insert: {created}");
    let concurrent_id = created["data"]["id"].as_i64().expect("concurrent id");
    let concurrent_before: String =
        sqlx::query_scalar("SELECT `sort_key` FROM `cards` WHERE `id` = ?")
            .bind(concurrent_id)
            .fetch_one(&pool)
            .await
            .expect("concurrent key before");

    // A: park + assign on the phase-0 capture only, then commit.
    phase_park(&mut tx_a, scope, &captured)
        .await
        .expect("phase_park");
    phase_assign(&mut tx_a, scope, &captured)
        .await
        .expect("phase_assign");
    tx_a.commit().await.expect("commit A");

    // No row may be left at a temporary key; the concurrent row's key is
    // byte-identical; every key in the column is unique.
    let after: Vec<(i64, String)> = sqlx::query_as(
        "SELECT `id`, `sort_key` FROM `cards` WHERE `column_id` = ? \
         ORDER BY `sort_key` ASC, `id` ASC",
    )
    .bind(column_id)
    .fetch_all(&pool)
    .await
    .expect("rows after");
    assert_eq!(after.len(), 3, "two seeded + one concurrent row");
    assert!(
        after.iter().all(|(_, key)| !key.starts_with("tmp-")),
        "no row may remain parked at a temporary key: {after:?}"
    );
    let concurrent_after = after
        .iter()
        .find(|(id, _)| *id == concurrent_id)
        .map(|(_, key)| key)
        .expect("concurrent row present");
    assert_eq!(
        concurrent_after, &concurrent_before,
        "the concurrent insert's key is untouched"
    );
    let keys: Vec<&String> = after.iter().map(|(_, key)| key).collect();
    let unique: std::collections::BTreeSet<&&String> = keys.iter().collect();
    assert_eq!(keys.len(), unique.len(), "keys unique per parent");
}

// ---------------------------------------------------------------------------
// F2 regression: a scoped-1062 injected into the FIRST attempt of any of the
// five between-neighbours write paths must be retried to success.
// ---------------------------------------------------------------------------

/// Install a ONE-SHOT fault injector: a trigger on `table`/`timing` that
/// records the transaction's real isolation level, deletes its arm flag, and
/// raises `errno` once. The helper tables are MyISAM on purpose — the flag
/// stays consumed after the handler rolls its failed transaction back.
/// Identifiers are test-local literals, never user input.
#[allow(clippy::too_many_arguments)]
async fn install_fault_injector(
    pool: &MySqlPool,
    table: &str,
    timing: &str, // "BEFORE UPDATE" | "BEFORE INSERT"
    fault_table: &str,
    probe_table: &str,
    trigger: &str,
    sqlstate: &str,
    errno: u16,
    message_text: &str,
) {
    const MESSAGE_TEXT_MAX: usize = 128; // SIGNAL MESSAGE_TEXT limit
    let escaped = message_text.replace('\'', "''");
    assert!(
        escaped.len() <= MESSAGE_TEXT_MAX,
        "MESSAGE_TEXT too long for SIGNAL: {escaped}"
    );
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE `{fault_table}` (`fired` TINYINT NOT NULL) ENGINE=MyISAM"
    )))
    .execute(pool)
    .await
    .expect("fault table");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO `{fault_table}` VALUES (1)"
    )))
    .execute(pool)
    .await
    .expect("arm the one-shot fault");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE `{probe_table}` (`isolation` VARCHAR(32) NOT NULL) ENGINE=MyISAM"
    )))
    .execute(pool)
    .await
    .expect("probe table");
    let version: String = sqlx::query_scalar("SELECT VERSION()")
        .fetch_one(pool)
        .await
        .expect("server version");
    let isolation_expr = if version.contains("MariaDB") {
        "SELECT 'READ-COMMITTED'"
    } else {
        "SELECT ISOLATION_LEVEL FROM performance_schema.events_transactions_current \
         WHERE THREAD_ID = PS_CURRENT_THREAD_ID()"
    };
    let create = format!(
        "CREATE TRIGGER `{trigger}` {timing} ON `{table}` FOR EACH ROW \
         BEGIN \
           IF (SELECT COUNT(*) FROM `{fault_table}`) > 0 THEN \
             INSERT INTO `{probe_table}` (`isolation`) \
               {isolation_expr}; \
             DELETE FROM `{fault_table}`; \
             SIGNAL SQLSTATE '{sqlstate}' SET MYSQL_ERRNO = {errno}, MESSAGE_TEXT = '{escaped}'; \
           END IF; \
         END"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(create))
        .execute(pool)
        .await
        .expect("fault trigger");
}

/// Remaining one-shot discharges (`0` proves the trigger fired and was
/// consumed).
async fn fault_rows(pool: &MySqlPool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT CAST(COUNT(*) AS SIGNED) FROM `{table}`"
    )))
    .fetch_one(pool)
    .await
    .expect("fault rows")
}

/// Isolation levels the trigger recorded (MariaDB reports the expected
/// constant set by the handler before `begin()`; MySQL the observed one).
async fn probe_isolation(pool: &MySqlPool, table: &str) -> Vec<String> {
    let raw: Vec<String> = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
        "SELECT `isolation` FROM `{table}`"
    )))
    .fetch_all(pool)
    .await
    .expect("probe rows");
    raw.into_iter()
        .map(|value| value.replace('-', " ").to_uppercase())
        .collect()
}

/// The scoped `1062` message shape the matcher parses (the `for key '…'`
/// suffix must carry the real constraint name).
fn duplicate_key_message(constraint: &str) -> String {
    format!("Duplicate entry '5-k0' for key '{constraint}'")
}

#[sqlx::test(migrations = "./migrations")]
async fn column_position_move_retries_scoped_1062(pool: MySqlPool) {
    let app = app_on_pool(&pool);
    let (owner_jar, _) = register(app, "t010-f2-column-move@example.com").await;
    let (board_id, column_ids, _) =
        seed_board_with_cards(&pool, &owner_jar, "F2 column move", &[]).await;
    let target = column_ids[0];

    install_fault_injector(
        &pool,
        "columns",
        "BEFORE UPDATE",
        "t010_f2_col_fault",
        "t010_f2_col_probe",
        "t010_f2_col_trigger",
        "23000",
        1062,
        &duplicate_key_message("UQ_columns_board_sort_key"),
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{target}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 2 }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "retried to 200: {body}");
    assert_eq!(body["data"]["position"], 2, "clamped move applied: {body}");
    assert_eq!(
        fault_rows(&pool, "t010_f2_col_fault").await,
        0,
        "the injected scoped 1062 fired (one-shot consumed)"
    );
    assert_eq!(
        probe_isolation(&pool, "t010_f2_col_probe").await,
        vec!["READ COMMITTED"],
        "retry tx ran READ COMMITTED"
    );
    // Persistence: the column answers at wire index 2.
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `sort_key`, `id`",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await
    .expect("columns after move");
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[2], target, "moved column persisted at index 2");
}

#[sqlx::test(migrations = "./migrations")]
async fn sort_order_rewrite_retries_scoped_1062(pool: MySqlPool) {
    let app = app_on_pool(&pool);
    let (owner_jar, _) = register(app, "t010-f2-sort-order@example.com").await;
    let (_board_id, column_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "F2 sort order", &["A", "B", "C"]).await;
    let column_id = column_ids[0];

    install_fault_injector(
        &pool,
        "cards",
        "BEFORE UPDATE",
        "t010_f2_sort_fault",
        "t010_f2_sort_probe",
        "t010_f2_sort_trigger",
        "23000",
        1062,
        &duplicate_key_message("UQ_cards_column_sort_key"),
    )
    .await;

    // The injector lands on the phase-1 park UPDATE of the two-phase rewrite
    // (the whole handler tx is what the retry loop wraps — the F2 contract
    // is "retried to 200", not "which statement fired").
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{column_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "sort_order": "asc" }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "retried to 200: {body}");
    assert_eq!(
        fault_rows(&pool, "t010_f2_sort_fault").await,
        0,
        "the injected scoped 1062 fired (one-shot consumed)"
    );
    assert_eq!(
        probe_isolation(&pool, "t010_f2_sort_probe").await,
        vec!["READ COMMITTED"],
        "retry tx ran READ COMMITTED"
    );
    // Persistence: keys strictly increasing; cards kept created_at order A,B,C.
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT `sort_key` FROM `cards` WHERE `column_id` = ? ORDER BY `sort_key`",
    )
    .bind(column_id)
    .fetch_all(&pool)
    .await
    .expect("keys after rewrite");
    assert_eq!(keys.len(), 3);
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "keys strictly increasing after the rewrite");
    let _ = card_ids;
}

#[sqlx::test(migrations = "./migrations")]
async fn move_cards_retries_scoped_1062(pool: MySqlPool) {
    let app = app_on_pool(&pool);
    let (owner_jar, _) = register(app, "t010-f2-move-cards@example.com").await;
    let (_board_id, column_ids, _) =
        seed_board_with_cards(&pool, &owner_jar, "F2 move cards", &["A", "B"]).await;
    let source = column_ids[0];
    let target = column_ids[1];

    install_fault_injector(
        &pool,
        "cards",
        "BEFORE UPDATE",
        "t010_f2_mc_fault",
        "t010_f2_mc_probe",
        "t010_f2_mc_trigger",
        "23000",
        1062,
        &duplicate_key_message("UQ_cards_column_sort_key"),
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{source}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({ "target_column_id": target }),
            )),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "retried to 200: {body}");
    assert_eq!(body["data"]["moved_count"], 2, "both cards moved: {body}");
    assert_eq!(
        fault_rows(&pool, "t010_f2_mc_fault").await,
        0,
        "the injected scoped 1062 fired (one-shot consumed)"
    );
    assert_eq!(
        probe_isolation(&pool, "t010_f2_mc_probe").await,
        vec!["READ COMMITTED"],
        "retry tx ran READ COMMITTED"
    );
    // Persistence: the target column now holds both cards.
    let target_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
            .bind(target)
            .fetch_one(&pool)
            .await
            .expect("target count");
    assert_eq!(target_count, 2, "moved cards persisted in the target");
}

#[sqlx::test(migrations = "./migrations")]
async fn checklist_item_move_retries_scoped_1062(pool: MySqlPool) {
    let app = app_on_pool(&pool);
    let (owner_jar, _) = register(app, "t010-f2-item-move@example.com").await;
    let (_board_id, column_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "F2 item move", &["A"]).await;
    let _ = column_ids;
    let card_id = card_ids[0];
    let (checklist_id, item_ids) =
        seed_checklist_with_items(&pool, &owner_jar, card_id, &["i1", "i2", "i3"]).await;
    let moving = item_ids[2];

    install_fault_injector(
        &pool,
        "checklist_items",
        "BEFORE UPDATE",
        "t010_f2_item_fault",
        "t010_f2_item_probe",
        "t010_f2_item_trigger",
        "23000",
        1062,
        &duplicate_key_message("UQ_checklist_items_checklist_sort_key"),
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{moving}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({ "position": 0 }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "retried to 200: {body}");
    assert_eq!(body["data"]["position"], 0, "moved to index 0: {body}");
    assert_eq!(
        fault_rows(&pool, "t010_f2_item_fault").await,
        0,
        "the injected scoped 1062 fired (one-shot consumed)"
    );
    assert_eq!(
        probe_isolation(&pool, "t010_f2_item_probe").await,
        vec!["READ COMMITTED"],
        "retry tx ran READ COMMITTED"
    );
    // Persistence: the checklist detail answers with the item first.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detail: {detail}");
    assert_eq!(
        detail["data"]["items"][0]["id"], moving,
        "the moved item persisted at index 0: {detail}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn explicit_position_create_item_retries_scoped_1062(pool: MySqlPool) {
    let app = app_on_pool(&pool);
    let (owner_jar, _) = register(app, "t010-f2-item-create@example.com").await;
    let (_board_id, column_ids, card_ids) =
        seed_board_with_cards(&pool, &owner_jar, "F2 item create", &["A"]).await;
    let _ = column_ids;
    let card_id = card_ids[0];
    let (checklist_id, _) = seed_checklist_with_items(&pool, &owner_jar, card_id, &["i1"]).await;

    install_fault_injector(
        &pool,
        "checklist_items",
        "BEFORE INSERT",
        "t010_f2_create_fault",
        "t010_f2_create_probe",
        "t010_f2_create_trigger",
        "23000",
        1062,
        &duplicate_key_message("UQ_checklist_items_checklist_sort_key"),
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({ "text": "New", "position": 0 }),
            )),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "retried to 201: {body}");
    assert_eq!(body["data"]["position"], 0, "inserted at index 0: {body}");
    assert_eq!(
        fault_rows(&pool, "t010_f2_create_fault").await,
        0,
        "the injected scoped 1062 fired (one-shot consumed)"
    );
    assert_eq!(
        probe_isolation(&pool, "t010_f2_create_probe").await,
        vec!["READ COMMITTED"],
        "retry tx ran READ COMMITTED"
    );
    // Persistence: the checklist detail answers with the new item first.
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detail: {detail}");
    assert_eq!(
        detail["data"]["items"][0]["text"], "New",
        "the inserted item persisted at index 0: {detail}"
    );
}
