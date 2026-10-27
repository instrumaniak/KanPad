//! `POST /api/v1/columns/:id/move-cards` (T23c).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T23a): all scenarios run
//! sequentially in one test with distinct emails. This suite registers its
//! owner + intruder via `POST /api/v1/auth/register` (real Argon2id hashes,
//! live cookie jars), creates boards via the API, and seeds cards via direct
//! SQL (card routes land in T24a).
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: the route does not exist yet):
//! `POST /columns/:id/move-cards{target_column_id} → 200 {data:{moved_count}}`
//! (append at `MAX+1` in tx, source `ORDER BY position ASC` so the target
//! mirrors source order); same-id → `400`; cross-board (same user) → `400`;
//! foreign/missing source/target → `404` (never 403); empty source →
//! `200 {moved_count:0}` (NOT an error); garbage id → `400 invalid-path`;
//! bad bodies → `400`; every 4xx asserts problem+json (+ exact `type` URI,
//! `errors[]` on validation-class failures); every mutation re-reads state
//! via second GET + direct `SELECT`; verb path `/:id/move-all` → `404`.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board, json_body,
    json_of, register, send,
};
use axum::Router;
use axum::http::StatusCode;
use sqlx::MySqlPool;

async fn column_ids(app: Router, jar: &str, board_id: i64) -> Vec<i64> {
    let (status, _, bytes) = send(
        app,
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list columns: {listed}");
    listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|column| column["id"].as_i64().expect("column id"))
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn columns_move_cards_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 (AuthSession guard).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/columns/1/move-cards",
            None,
            Some(json_body(&serde_json::json!({"target_column_id": 2}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::UNAUTHORIZED, "anon: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "unauthorized",
        "Anonymous access is 401 (AuthSession guard)",
    );

    // 1. Register owner + intruder; owner creates board A (3 default columns).
    let (owner_jar, _) = register(app(), "move-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "move-intruder@example.com").await;
    let board_a = create_board(app(), &owner_jar, &serde_json::json!({"name": "Move A"})).await;
    let cols_a = column_ids(app(), &owner_jar, board_a).await;
    assert_eq!(cols_a.len(), 3, "board A starts with 3 columns");
    let (source_id, target_id) = (cols_a[0], cols_a[1]);

    // 2. Seed cards via SQL: 3 in source (positions 0..2), 1 in target.
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `position`, `description`) VALUES \
         ('Alpha', ?, 0, 'alpha body'), \
         ('Beta', ?, 1, NULL), \
         ('Gamma', ?, 2, 'gamma body')",
    )
    .bind(source_id)
    .bind(source_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .expect("seed source cards");
    sqlx::query("INSERT INTO `cards` (`title`, `column_id`, `position`) VALUES ('Existing', ?, 0)")
        .bind(target_id)
        .execute(&pool)
        .await
        .expect("seed target card");

    // 3. Success → 200 {data:{moved_count:3}}, no `message` key.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{source_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_id}),
            )),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "move-cards: {moved}");
    assert!(moved.get("message").is_none(), "no message key: {moved}");
    assert_eq!(moved["data"]["moved_count"], 3, "moved count: {moved}");

    // 4. DB re-read BOTH columns: source drained, target mirrors source
    //    order appended at MAX+1 (existing card keeps position 0).
    let source_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .expect("source re-read");
    assert_eq!(source_count, 0, "source drained");
    let target_rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT `title`, `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(target_id)
    .fetch_all(&pool)
    .await
    .expect("target re-read");
    assert_eq!(
        target_rows,
        [
            ("Existing".to_string(), 0),
            ("Alpha".to_string(), 1),
            ("Beta".to_string(), 2),
            ("Gamma".to_string(), 3),
        ],
        "target mirrors source order at MAX+1"
    );

    // 5. Second GET confirms the nested embed matches the DB state.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_a}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-get after move: {listed}");
    let columns = listed["data"].as_array().expect("data[]");
    let source = columns
        .iter()
        .find(|column| column["id"] == source_id)
        .expect("source listed");
    assert_eq!(source["cards"], serde_json::json!([]));
    let target = columns
        .iter()
        .find(|column| column["id"] == target_id)
        .expect("target listed");
    let titles: Vec<&str> = target["cards"]
        .as_array()
        .expect("cards[]")
        .iter()
        .map(|card| card["title"].as_str().expect("title"))
        .collect();
    assert_eq!(titles, ["Existing", "Alpha", "Beta", "Gamma"]);

    // 6. Empty source → 200 {moved_count:0}, NOT an error; target untouched.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_a}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Empty Source"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create empty column: {created}"
    );
    let empty_id = created["data"]["id"].as_i64().expect("column id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{empty_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_id}),
            )),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty move: {moved}");
    assert_eq!(moved["data"]["moved_count"], 0, "empty → 0: {moved}");
    let target_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .expect("target count re-read");
    assert_eq!(target_count, 4, "target untouched by empty move");

    // 7. Same-id → 400 validation-error + errors[].
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{target_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_id}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "same-id: {failure}");
    assert_problem(status, &headers, &failure, "validation-error", "Same-id");
    assert_errors_nonempty(&failure, "same-id");

    // 8. Cross-board (same user) → 400; DB unchanged for both columns.
    let board_b = create_board(app(), &owner_jar, &serde_json::json!({"name": "Move B"})).await;
    let cols_b = column_ids(app(), &owner_jar, board_b).await;
    let foreign_board_col = cols_b[0];
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{target_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": foreign_board_col}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "cross-board: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "validation-error",
        "Cross-board (same user)",
    );
    assert_errors_nonempty(&failure, "cross-board");
    let target_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .expect("target re-read after rejected move");
    assert_eq!(target_count, 4, "rejected move writes nothing");

    // 9. Foreign source/target (intruder's) + missing ids → 404, never 403.
    let intruder_board =
        create_board(app(), &intruder_jar, &serde_json::json!({"name": "Theirs"})).await;
    let intruder_col = column_ids(app(), &intruder_jar, intruder_board).await[0];
    for (source, target, what) in [
        (intruder_col, target_id, "foreign source"),
        (target_id, intruder_col, "foreign target"),
        (999_999, target_id, "missing source"),
        (target_id, 999_999, "missing target"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &format!("/api/v1/columns/{source}/move-cards"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"target_column_id": target}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Foreign source/target (intruder's) + missing ids",
        );
    }

    // 10. Garbage source id → 400 invalid-path.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/columns/abc/move-cards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_id}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "Garbage source id",
    );

    // 11. Bad bodies → 400: missing key (invalid-json), zero id and wrong
    //     type (validation-error / invalid-json respectively).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{target_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing key: {failure}");
    assert_problem(status, &headers, &failure, "invalid-json", "Bad bodies");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{target_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"target_column_id": 0}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "zero id: {failure}");
    assert_problem(status, &headers, &failure, "validation-error", "Bad bodies");
    assert_errors_nonempty(&failure, "zero id");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/columns/{target_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"target_column_id": "x"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "wrong type: {failure}");
    assert_problem(status, &headers, &failure, "invalid-json", "Bad bodies");

    // 12. Deleted verb path `/:id/move-all` → 404 on both mounts.
    for path in [
        format!("/api/v1/columns/{target_id}/move-all"),
        format!("/api/columns/{target_id}/move-all"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &path,
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"target_column_id": target_id}),
                )),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "verb path {path}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Deleted verb path `/:id/move-all`",
        );
    }

    // 13. Deprecated `/api/*` alias serves the same handler: seed one card
    //     into the empty column, move it via the alias, re-read the DB.
    sqlx::query("INSERT INTO `cards` (`title`, `column_id`, `position`) VALUES ('Alias', ?, 0)")
        .bind(empty_id)
        .execute(&pool)
        .await
        .expect("seed alias card");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/columns/{empty_id}/move-cards"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"target_column_id": target_id}),
            )),
        ),
    )
    .await;
    let moved = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "alias move: {moved}");
    assert_eq!(moved["data"]["moved_count"], 1, "alias moved: {moved}");
    let alias_pos: i32 =
        sqlx::query_scalar("SELECT `position` FROM `cards` WHERE `title` = 'Alias'")
            .fetch_one(&pool)
            .await
            .expect("alias card re-read");
    assert_eq!(alias_pos, 4, "alias card appended at MAX+1");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
