//! Black-box e2e: columns → cards slice + sequential position fuzz (T50b).
//!
//! Rewrites the Nest `backend/test/columns.e2e-spec.ts` + `cards.e2e-spec.ts`
//! slice against the Rust `:3001` contract (`20-api-contract.md` §columns,
//! §cards + `21-api-standard-shapes.md` columns/cards rows; schema
//! `30-db-schema.md`). Black-box = HTTP-level through the real router via
//! `oneshot` (the same `create_app` that serves `:3001`); no handler or store
//! internals are touched.
//!
//! Nest-vs-Rust expectation deltas (one assertion set cannot pass on both,
//! so this suite asserts the Rust side only):
//! - Nest `{data,message?}` bare arrays → Rust `Page<T>` on
//!   `GET /columns/:col/cards` (`{data,page,limit,total}`); nested column
//!   lists stay unpaginated `position ASC` on both.
//! - Nest deletes → `200` → Rust `204` with an empty body (columns, cards,
//!   label detach, logout all `204`).
//! - Nest verb URLs (`/:id/sort`, `/:id/move-all`, camelCase
//!   `{targetColumnId}`/`{labelId}`) → Rust noun URLs
//!   (`PATCH /:id{sort_order}`, `POST /:id/move-cards{target_column_id}`,
//!   `PUT /:id/labels/:label_id`); the verb paths must `404` here.
//! - `message` strings are dropped everywhere (asserted absent).
//!
//! Fuzz scope: 500 RANDOM SEQUENTIAL moves with an exact-order oracle against
//! a local clamp model (deterministic seed = reproducible; identical final
//! order required). Sequential only — concurrent-move invariants (dense +
//! unique + `COUNT` unchanged, no exact-order assert) are covered by T24b
//! (`cards_move.rs` §14) and are NOT re-asserted here.
//!
//! Single `#[sqlx::test]` setup on purpose: all scenarios run sequentially in
//! one test with distinct emails (mirrors T23/T24 suites). Every mutation is
//! followed by a DB re-read (second GET or direct `SELECT` — never the echo
//! alone). NO session wipe: `sessions` is per-test-database, so there is
//! nothing shared to wipe (see `helpers.rs` module docs).
//!
//! Request plumbing comes from `super::helpers` — do not re-declare it here.

use super::helpers::{
    app_on_pool, assert_problem, authed_request, json_body, json_of, register, send,
};
use axum::http::{StatusCode, header};
use serde_json::Value;
use sqlx::MySqlPool;

async fn column_card_ids(pool: &MySqlPool, column_id: i64) -> Vec<i64> {
    sqlx::query_scalar("SELECT `id` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC")
        .bind(column_id)
        .fetch_all(pool)
        .await
        .expect("column order re-reads")
}

/// Assert a card column is dense `0..N` and return its card ids in order.
async fn assert_dense(pool: &MySqlPool, column_id: i64, what: &str) -> Vec<i64> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT `id`, `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(column_id)
    .fetch_all(pool)
    .await
    .expect("dense re-read");
    let positions: Vec<i64> = rows.iter().map(|(_, position)| *position).collect();
    let dense: Vec<i64> = (0..positions.len() as i64).collect();
    assert_eq!(positions, dense, "dense 0..N ({what})");
    rows.into_iter().map(|(id, _)| id).collect()
}

/// Deterministic xorshift64* RNG (no extra deps; fixed seed = reproducible).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0, "rng bound must be positive");
        (self.next() % bound as u64) as usize
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn e2e_columns_cards_blackbox_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every columns/cards route (guard proof).
    for (method, path, body) in [
        ("GET", "/api/v1/boards/1/columns", None),
        (
            "POST",
            "/api/v1/boards/1/columns",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        (
            "POST",
            "/api/v1/cards",
            Some(json_body(
                &serde_json::json!({"title": "Nope", "column_id": 1}),
            )),
        ),
        ("GET", "/api/v1/columns/1/cards", None),
        ("GET", "/api/v1/cards/1", None),
        (
            "PATCH",
            "/api/v1/cards/1",
            Some(json_body(&serde_json::json!({"title": "Nope"}))),
        ),
        ("DELETE", "/api/v1/cards/1", None),
    ] {
        let (status, headers, bytes) = send(app(), authed_request(method, path, None, body)).await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}: {failure}"
        );
        assert_problem(
            status,
            &headers,
            &failure,
            "unauthorized",
            "Anonymous access is 401 on every columns/cards route…",
        );
    }

    // 1. Register owner + intruder; owner creates project + board (3 default
    //    columns). DB re-read: user row + exactly 6 seeded labels + board.
    let (owner_jar, owner_id) = register(app(), "e2e-cols-cards-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "e2e-cols-cards-intruder@example.com").await;
    let label_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
        .bind(owner_id)
        .fetch_one(&pool)
        .await
        .expect("seeded labels re-read");
    assert_eq!(label_count, 6, "register seeds exactly 6 labels");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "E2E Project"}))),
        ),
    )
    .await;
    let project = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create project: {project}");
    assert!(
        project.get("message").is_none(),
        "no message key: {project}"
    );
    let project_id = project["data"]["id"].as_i64().expect("project id");
    let stored_project: String = sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `id` = ?")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .expect("project re-reads");
    assert_eq!(stored_project, "E2E Project");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "name": "E2E Board", "project_id": project_id,
            }))),
        ),
    )
    .await;
    let board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create board: {board}");
    let board_id = board["data"]["id"].as_i64().expect("board id");
    assert_eq!(
        board["data"]["columns"]
            .as_array()
            .expect("columns[]")
            .len(),
        3,
        "board creates 3 default columns: {board}"
    );
    let col_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `columns` WHERE `board_id` = ?")
        .bind(board_id)
        .fetch_one(&pool)
        .await
        .expect("board columns re-read");
    assert_eq!(col_count, 3);

    // 2. Columns CRUD (Nest CRUD slice, Rust expectations): create → 201 +
    //    Location + SELECT re-read; list unpaginated `position ASC`.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "To Do"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create column: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let extra_col = created["data"]["id"].as_i64().expect("column id");
    assert_eq!(created["data"]["position"], 3);
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII");
    assert_eq!(location, format!("/api/v1/columns/{extra_col}"));
    let stored: (String, i32) =
        sqlx::query_as("SELECT `name`, `position` FROM `columns` WHERE `id` = ?")
            .bind(extra_col)
            .fetch_one(&pool)
            .await
            .expect("created column re-reads");
    assert_eq!(stored, ("To Do".to_string(), 3));
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list columns: {listed}");
    let columns = listed["data"].as_array().expect("data[]");
    assert_eq!(columns.len(), 4, "3 defaults + 1 created: {listed}");
    let positions: Vec<i64> = columns
        .iter()
        .map(|column| column["position"].as_i64().expect("position"))
        .collect();
    assert_eq!(positions, [0, 1, 2, 3], "wire order position ASC");

    // 3. Rename → 200; re-GET + SELECT confirm (never the echo alone).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{extra_col}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Backlog"}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "rename column: {patched}");
    assert_eq!(patched["data"]["name"], "Backlog");
    let stored_name: String = sqlx::query_scalar("SELECT `name` FROM `columns` WHERE `id` = ?")
        .bind(extra_col)
        .fetch_one(&pool)
        .await
        .expect("renamed column re-reads");
    assert_eq!(stored_name, "Backlog");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-get after rename: {listed}");
    let renamed = listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .find(|column| column["id"] == extra_col)
        .expect("renamed column listed");
    assert_eq!(renamed["name"], "Backlog");

    // 4. Sort (Rust noun shape): seed cards with scrambled created_at, then
    //    PATCH {sort_order:asc} rewrites by created_at; DB re-read proves it.
    //    Deleted Nest verb path `/:id/sort` must 404.
    let sort_col: i64 = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC LIMIT 1",
    )
    .bind(board_id)
    .fetch_one(&pool)
    .await
    .expect("first column id");
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `position`, `created_at`) VALUES \
         ('Sort C', ?, 0, '2026-01-03 00:00:00.000000'), \
         ('Sort A', ?, 1, '2026-01-01 00:00:00.000000'), \
         ('Sort B', ?, 2, '2026-01-02 00:00:00.000000')",
    )
    .bind(sort_col)
    .bind(sort_col)
    .bind(sort_col)
    .execute(&pool)
    .await
    .expect("seed sort cards");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{sort_col}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"sort_order": "asc"}))),
        ),
    )
    .await;
    let sorted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "sort asc: {sorted}");
    let titles: Vec<String> = sqlx::query_scalar(
        "SELECT `title` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(sort_col)
    .fetch_all(&pool)
    .await
    .expect("sort order re-reads");
    assert_eq!(titles, ["Sort A", "Sort B", "Sort C"]);
    for method in ["PATCH", "POST"] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                method,
                &format!("/api/v1/columns/{sort_col}/sort"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"sort_order": "asc"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{method} verb path: {failure}"
        );
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Sort (Rust noun shape): seed cards with scrambled…",
        );
    }
    // Clean the sort seeds so the fuzz below starts from a known set.
    sqlx::query("DELETE FROM `cards` WHERE `column_id` = ?")
        .bind(sort_col)
        .execute(&pool)
        .await
        .expect("clear sort seeds");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
        .bind(sort_col)
        .fetch_one(&pool)
        .await
        .expect("sort cleanup re-read");
    assert_eq!(remaining, 0);

    // 5. Cards CRUD (Nest CRUD slice, Rust expectations): create → 201 +
    //    Location + SELECT; list is Page (never `description` TEXT).
    let source_col: i64 = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC LIMIT 1",
    )
    .bind(board_id)
    .fetch_one(&pool)
    .await
    .expect("source column id");
    let target_col: i64 = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC LIMIT 1 OFFSET 1",
    )
    .bind(board_id)
    .fetch_one(&pool)
    .await
    .expect("target column id");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Task 1", "column_id": source_col,
            }))),
        ),
    )
    .await;
    let card = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create card: {card}");
    assert!(card.get("message").is_none(), "no message key: {card}");
    let card_id = card["data"]["id"].as_i64().expect("card id");
    assert_eq!(card["data"]["column_id"], source_col);
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII");
    assert_eq!(location, format!("/api/v1/cards/{card_id}"));
    let stored: (String, i64, i32) =
        sqlx::query_as("SELECT `title`, `column_id`, `position` FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("created card re-reads");
    assert_eq!(stored.0, "Task 1");
    assert_eq!(stored.1, source_col);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{source_col}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list cards Page: {listed}");
    assert_eq!(listed["page"], 1);
    assert_eq!(listed["limit"], 50);
    assert_eq!(listed["total"], 1);
    let items = listed["data"].as_array().expect("Page data[]");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Task 1");
    assert!(
        items[0].get("description").is_none(),
        "list never selects TEXT: {listed}"
    );
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "card detail: {detail}");
    assert_eq!(detail["data"]["title"], "Task 1");

    // 6. Update title → 200; SELECT + second GET confirm.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Task 1 Updated"}))),
        ),
    )
    .await;
    let updated = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "update card: {updated}");
    assert_eq!(updated["data"]["title"], "Task 1 Updated");
    let stored_title: String = sqlx::query_scalar("SELECT `title` FROM `cards` WHERE `id` = ?")
        .bind(card_id)
        .fetch_one(&pool)
        .await
        .expect("updated card re-reads");
    assert_eq!(stored_title, "Task 1 Updated");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-get after update: {detail}");
    assert_eq!(detail["data"]["title"], "Task 1 Updated");

    // 7. Move to another column → 200; target list GET + SELECT confirm.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"column_id": target_col}))),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "move card: {moved}");
    assert_eq!(moved["data"]["column_id"], target_col);
    let stored_col: i64 = sqlx::query_scalar("SELECT `column_id` FROM `cards` WHERE `id` = ?")
        .bind(card_id)
        .fetch_one(&pool)
        .await
        .expect("moved card re-reads");
    assert_eq!(stored_col, target_col);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{target_col}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "target list after move: {listed}");
    let found = listed["data"]
        .as_array()
        .expect("Page data[]")
        .iter()
        .any(|item| item["id"] == card_id);
    assert!(found, "moved card listed in target: {listed}");

    // 8. Label attach/detach (cards slice M2M): PUT → 200 idempotent,
    //    DELETE → 204; both re-read `card_labels`.
    let label_id: i64 =
        sqlx::query_scalar("SELECT `id` FROM `labels` WHERE `user_id` = ? ORDER BY `id` LIMIT 1")
            .bind(owner_id)
            .fetch_one(&pool)
            .await
            .expect("default label id");
    for _ in 0..2 {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "PUT",
                &format!("/api/v1/cards/{card_id}/labels/{label_id}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let attached = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "PUT attach: {attached}");
    }
    let links: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = ? AND `label_id` = ?",
    )
    .bind(card_id)
    .bind(label_id)
    .fetch_one(&pool)
    .await
    .expect("attach re-reads");
    assert_eq!(links, 1, "double attach still one row");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{card_id}/labels/{label_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "detach: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(bytes.is_empty(), "204 carries an empty body");
    let links: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = ? AND `label_id` = ?",
    )
    .bind(card_id)
    .bind(label_id)
    .fetch_one(&pool)
    .await
    .expect("detach re-reads");
    assert_eq!(links, 0, "detach removes the link row");

    // 9. move-cards (Nest move-all slice, Rust noun shape): seed one card in
    //    the source, move to target, assert {moved_count} + re-read both
    //    columns. Empty source → 200 {moved_count:0}. Deleted verb path 404.
    sqlx::query("DELETE FROM `cards` WHERE `column_id` IN (?, ?)")
        .bind(source_col)
        .bind(target_col)
        .execute(&pool)
        .await
        .expect("clear move-cards stage");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Bulk card", "column_id": source_col,
            }))),
        ),
    )
    .await;
    let bulk = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "seed bulk card: {bulk}");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{source_col}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_col}),
            )),
        ),
    )
    .await;
    let bulk_moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "move-cards: {bulk_moved}");
    assert!(
        bulk_moved.get("message").is_none(),
        "no message key: {bulk_moved}"
    );
    assert_eq!(bulk_moved["data"]["moved_count"], 1);
    let source_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
        .bind(source_col)
        .fetch_one(&pool)
        .await
        .expect("source re-reads empty");
    assert_eq!(source_left, 0);
    let target_now: Vec<i64> = column_card_ids(&pool, target_col).await;
    assert_eq!(target_now.len(), 1, "target holds the moved card");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{source_col}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_col}),
            )),
        ),
    )
    .await;
    let empty = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty move-cards: {empty}");
    assert_eq!(
        empty["data"]["moved_count"], 0,
        "empty source is 200 with 0"
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{source_col}/move-all"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_col}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "verb path: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "not-found",
        "move-cards (Nest move-all slice, Rust noun shape): seed…",
    );

    // 10. Delete card → 204; SELECT COUNT=0 + second list GET confirm.
    //     Delete column → 204; positions re-read dense via second GET.
    let doomed_card = target_now[0];
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{doomed_card}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete card: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(bytes.is_empty(), "204 carries an empty body");
    let gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `id` = ?")
        .bind(doomed_card)
        .fetch_one(&pool)
        .await
        .expect("deleted card re-reads gone");
    assert_eq!(gone, 0);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{target_col}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list after delete: {listed}");
    assert_eq!(listed["total"], 0);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/columns/{extra_col}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete column: {}",
        String::from_utf8_lossy(&bytes)
    );
    let positions: Vec<i32> = sqlx::query_scalar(
        "SELECT `position` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await
    .expect("positions re-read dense");
    let dense: Vec<i32> = (0..positions.len() as i32).collect();
    assert_eq!(positions, dense, "gap closed, dense 0..N");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::OK,
        "re-get after column delete: {listed}"
    );
    let wire: Vec<i64> = listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|column| column["position"].as_i64().expect("position"))
        .collect();
    let expected: Vec<i64> = (0..wire.len() as i64).collect();
    assert_eq!(wire, expected, "wire order dense: {listed}");

    // 11. Ownership: intruder board/column/card → 404 (never 403); garbage
    //     ids → 400. Rejected writes change nothing (SELECT re-read).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({"name": "Theirs"}))),
        ),
    )
    .await;
    let foreign_board = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "foreign board: {foreign_board}"
    );
    let foreign_board_id = foreign_board["data"]["id"].as_i64().expect("board id");
    let foreign_col: i64 = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `position` LIMIT 1",
    )
    .bind(foreign_board_id)
    .fetch_one(&pool)
    .await
    .expect("foreign column id");
    for (method, path, body) in [
        (
            "GET",
            format!("/api/v1/boards/{foreign_board_id}/columns"),
            None,
        ),
        (
            "PATCH",
            format!("/api/v1/columns/{foreign_col}"),
            Some(json_body(&serde_json::json!({"name": "Hijack"}))),
        ),
        (
            "GET",
            format!("/api/v1/columns/{foreign_col}/cards?page=1&limit=50"),
            None,
        ),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request(method, &path, Some(&owner_jar), body)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Ownership: intruder board/column/card",
        );
    }
    for path in [
        "/api/v1/columns/abc/cards?page=1&limit=50",
        "/api/v1/cards/abc",
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", path, Some(&owner_jar), None)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "garbage {path}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "invalid-path",
            "Ownership: intruder board/column/card",
        );
    }

    // 12. Sequential fuzz: 500 random moves over the two stage columns with
    //     an exact-order oracle. A local model mirrors the contract clamp
    //     semantics (same-column → N-1, cross-column → M, omitted position
    //     appends M); the final DB order must match the model EXACTLY and the
    //     total COUNT must be unchanged. Sequential deterministic moves only;
    //     concurrent-move invariants live in T24b.
    sqlx::query("DELETE FROM `cards` WHERE `column_id` IN (?, ?)")
        .bind(source_col)
        .bind(target_col)
        .execute(&pool)
        .await
        .expect("clear fuzz stage");
    for (title, column, position) in [
        ("F0", source_col, 0),
        ("F1", source_col, 1),
        ("F2", source_col, 2),
        ("F3", source_col, 3),
        ("G0", target_col, 0),
        ("G1", target_col, 1),
    ] {
        sqlx::query("INSERT INTO `cards` (`title`, `column_id`, `position`) VALUES (?, ?, ?)")
            .bind(title)
            .bind(column)
            .bind(position)
            .execute(&pool)
            .await
            .expect("seed fuzz card");
    }
    let total_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards`")
        .fetch_one(&pool)
        .await
        .expect("fuzz COUNT baseline");
    let mut model_source = assert_dense(&pool, source_col, "fuzz baseline source").await;
    let mut model_target = assert_dense(&pool, target_col, "fuzz baseline target").await;
    let mut rng = Rng(0xE2EC_01C5);
    for step in 0..500 {
        let all: Vec<i64> = model_source
            .iter()
            .chain(model_target.iter())
            .copied()
            .collect();
        let card = all[rng.below(all.len())];
        let in_source = model_source.contains(&card);
        let cross = rng.below(2) == 0;
        let (target, source) = if cross {
            if in_source {
                (target_col, source_col)
            } else {
                (source_col, target_col)
            }
        } else if in_source {
            (source_col, source_col)
        } else {
            (target_col, target_col)
        };
        let target_len = if target == source_col {
            model_source.len()
        } else {
            model_target.len()
        } as i64;
        let same = source == target;
        let omit_position = !same && step % 4 == 3;
        let requested: Option<i64> = if omit_position {
            None
        } else {
            Some(rng.below((target_len + 7) as usize) as i64 - 3)
        };
        let upper = if same {
            (target_len - 1).max(0)
        } else {
            target_len
        };
        let final_pos = requested.map(|p| p.clamp(0, upper)).unwrap_or(target_len);
        {
            let from = if in_source {
                &mut model_source
            } else {
                &mut model_target
            };
            let idx = from
                .iter()
                .position(|id| *id == card)
                .expect("card in model");
            from.remove(idx);
        }
        {
            let to = if target == source_col {
                &mut model_source
            } else {
                &mut model_target
            };
            to.insert(final_pos as usize, card);
        }
        let mut payload = serde_json::Map::new();
        if !same {
            payload.insert("column_id".to_string(), serde_json::json!(target));
        }
        if !omit_position {
            payload.insert(
                "position".to_string(),
                serde_json::json!(requested.expect("requested set")),
            );
        }
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/cards/{card}"),
                Some(&owner_jar),
                Some(json_body(&Value::Object(payload))),
            ),
        )
        .await;
        let moved = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "fuzz step {step}: {moved}");
        assert_eq!(moved["data"]["column_id"], target, "fuzz step {step}");
        assert_eq!(moved["data"]["position"], final_pos, "fuzz step {step}");
        if step % 50 == 49 {
            assert_eq!(
                assert_dense(&pool, source_col, &format!("fuzz step {step} source")).await,
                model_source,
                "fuzz step {step} model source"
            );
            assert_eq!(
                assert_dense(&pool, target_col, &format!("fuzz step {step} target")).await,
                model_target,
                "fuzz step {step} model target"
            );
        }
    }
    assert_eq!(
        assert_dense(&pool, source_col, "fuzz final source").await,
        model_source,
        "500-move fuzz identical final order (source)"
    );
    assert_eq!(
        assert_dense(&pool, target_col, "fuzz final target").await,
        model_target,
        "500-move fuzz identical final order (target)"
    );
    let total_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards`")
        .fetch_one(&pool)
        .await
        .expect("fuzz COUNT re-read");
    assert_eq!(total_after, total_before, "fuzz moves never lose cards");

    // NO TEARDOWN WIPE/DROP: the shared store must survive until every suite
    // in this process is done.
}
