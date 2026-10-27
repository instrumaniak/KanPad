//! `PATCH /api/v1/columns/:id{sort_order}` (T23b).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T23a): all scenarios run
//! sequentially in one test with distinct emails. Cards are inserted via
//! direct SQL with scrambled `created_at` vs `position` (card routes land in
//! T24a).
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: `PATCH` ignores `sort_order`, positions unchanged):
//! `PATCH {sort_order:asc|desc} → 200 {data}` (single set-based
//! `UPDATE ... CASE` rewrite by `created_at`, verified by DB re-read +
//! second GET); bad enum → `400 errors[]`; verb path `/:id/sort` → 404
//! (deleted route); foreign/missing column → `404` (never 403); every 4xx
//! asserts problem+json + exact `type` URI.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board, json_body,
    json_of, register, send,
};
use axum::http::StatusCode;
use sqlx::MySqlPool;

async fn titles_by_position(pool: &MySqlPool, column_id: i64) -> Vec<String> {
    sqlx::query_scalar("SELECT `title` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC")
        .bind(column_id)
        .fetch_all(pool)
        .await
        .expect("titles re-read")
}

#[sqlx::test(migrations = "./migrations")]
async fn columns_sort_rewrite_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous sort is 401 (AuthSession guard).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/columns/1",
            None,
            Some(json_body(&serde_json::json!({"sort_order": "asc"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::UNAUTHORIZED, "anon sort: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "unauthorized",
        "Anonymous sort is 401 (AuthSession guard)",
    );

    // 1. Register owner + intruder; owner creates one board (3 default
    //    columns), intruder creates a foreign board.
    let (owner_jar, _) = register(app(), "sort-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "sort-intruder@example.com").await;
    let board_id = create_board(
        app(),
        &owner_jar,
        &serde_json::json!({"name": "Sort Board"}),
    )
    .await;
    let foreign_board =
        create_board(app(), &intruder_jar, &serde_json::json!({"name": "Theirs"})).await;

    let col_id: i64 = sqlx::query_scalar(
        "SELECT `id` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC LIMIT 1",
    )
    .bind(board_id)
    .fetch_one(&pool)
    .await
    .expect("first column id");

    // 2. Seed three cards with `created_at` scrambled against `position`
    //    (position order C,A,B vs created_at order A,B,C).
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `position`, `created_at`) VALUES \
         ('Card C', ?, 0, '2026-01-03 00:00:00.000000'), \
         ('Card A', ?, 1, '2026-01-01 00:00:00.000000'), \
         ('Card B', ?, 2, '2026-01-02 00:00:00.000000')",
    )
    .bind(col_id)
    .bind(col_id)
    .bind(col_id)
    .execute(&pool)
    .await
    .expect("seed cards");
    assert_eq!(
        titles_by_position(&pool, col_id).await,
        ["Card C", "Card A", "Card B"],
        "seed order is scrambled by design"
    );

    // 3. `asc` → 200 {data} (no `message` key); DB re-read proves the
    //    rewrite, positions dense 0..N.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{col_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"sort_order": "asc"}))),
        ),
    )
    .await;
    let sorted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "sort asc: {sorted}");
    assert!(sorted.get("message").is_none(), "no message key: {sorted}");
    assert_eq!(sorted["data"]["id"], col_id);
    assert_eq!(
        titles_by_position(&pool, col_id).await,
        ["Card A", "Card B", "Card C"],
        "asc rewrites positions by created_at"
    );
    let positions: Vec<i32> = sqlx::query_scalar(
        "SELECT `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(col_id)
    .fetch_all(&pool)
    .await
    .expect("positions re-read");
    assert_eq!(positions, [0, 1, 2], "dense 0..N after asc");

    // 4. Response cards ride `position ASC`; second GET confirms the wire
    //    order matches the DB.
    let wire: Vec<String> = sorted["data"]["cards"]
        .as_array()
        .expect("cards[]")
        .iter()
        .map(|card| card["title"].as_str().expect("title").to_string())
        .collect();
    assert_eq!(wire, ["Card A", "Card B", "Card C"]);
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
    assert_eq!(status, StatusCode::OK, "re-get after asc: {listed}");
    let listed_titles: Vec<String> = listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .find(|column| column["id"] == col_id)
        .expect("sorted column listed")["cards"]
        .as_array()
        .expect("cards[]")
        .iter()
        .map(|card| card["title"].as_str().expect("title").to_string())
        .collect();
    assert_eq!(listed_titles, ["Card A", "Card B", "Card C"]);

    // 5. `desc` → 200; DB re-read proves the reverse rewrite.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{col_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"sort_order": "desc"}))),
        ),
    )
    .await;
    let sorted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "sort desc: {sorted}");
    assert_eq!(
        titles_by_position(&pool, col_id).await,
        ["Card C", "Card B", "Card A"],
        "desc rewrites positions by created_at"
    );

    // 6. Idempotent: sorting `desc` again keeps the exact order.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{col_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"sort_order": "desc"}))),
        ),
    )
    .await;
    let sorted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "sort desc again: {sorted}");
    assert_eq!(
        titles_by_position(&pool, col_id).await,
        ["Card C", "Card B", "Card A"],
        "repeat sort is stable"
    );

    // 7. Bad enum → 400 errors[] (incl. uppercase, which the enum rejects);
    //    wrong JSON type → 400 body-shape error.
    for (payload, what) in [
        (serde_json::json!({"sort_order": "sideways"}), "bad enum"),
        (serde_json::json!({"sort_order": "ASC"}), "uppercase enum"),
        (serde_json::json!({"sort_order": ""}), "empty enum"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/columns/{col_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error", "Bad enum");
        assert_errors_nonempty(&failure, what);
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{col_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"sort_order": 123}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "numeric enum: {failure}");
    assert_problem(status, &headers, &failure, "invalid-json", "Bad enum");

    // 8. Deleted verb path `/:id/sort` → 404 (never a sort handler).
    for method in ["PATCH", "GET", "POST"] {
        let body = if method == "GET" {
            None
        } else {
            Some(json_body(&serde_json::json!({"sort_order": "asc"})))
        };
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                method,
                &format!("/api/v1/columns/{col_id}/sort"),
                Some(&owner_jar),
                body,
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
            "Deleted verb path `/:id/sort`",
        );
    }

    // 9. Foreign/missing column → 404 (never 403); garbage id → 400.
    let intruder_col: i64 = sqlx::query_scalar(
        "SELECT c.`id` FROM `columns` c WHERE c.`board_id` = ? ORDER BY c.`position` LIMIT 1",
    )
    .bind(foreign_board)
    .fetch_one(&pool)
    .await
    .expect("intruder column id");
    for id in [intruder_col.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/columns/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"sort_order": "asc"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "sort {id}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Foreign/missing column",
        );
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/columns/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"sort_order": "asc"}))),
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
        "Foreign/missing column",
    );

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
