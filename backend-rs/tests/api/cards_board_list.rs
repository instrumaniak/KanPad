//! `GET /api/v1/boards/{boardId}/cards` (T048 board-scoped paginated list).
//!
//! Covers default pagination/sort/order, allowed sorts/orders, deterministic id
//! tiebreaker, column_name, due_date NULL-last ordering in both directions,
//! ownership 404 no-oracle, invalid params/window ceiling 400 validation-error,
//! and empty page shape.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board, json_of,
    register, send,
};
use axum::http::StatusCode;
use serde_json::json;
use sqlx::MySqlPool;

fn names(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|item| item["title"].as_str().expect("title").to_string())
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn board_cards_list_pagination_sort_and_validation(pool: MySqlPool) {
    let app = || app_on_pool(&pool);
    let (owner_jar, _) = register(app(), "board-cards-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "board-cards-intruder@example.com").await;

    let board_id = create_board(app(), &owner_jar, &json!({"name": "List Board"})).await;
    let foreign_board_id = create_board(app(), &intruder_jar, &json!({"name": "Foreign"})).await;
    let _ = foreign_board_id;

    // Get default columns.
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
    let cols = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "columns: {cols}");
    let col_a = cols["data"][0]["id"].as_i64().expect("col a");
    let col_b = cols["data"][1]["id"].as_i64().expect("col b");
    let col_a_name = cols["data"][0]["name"]
        .as_str()
        .expect("col a name")
        .to_string();
    let col_b_name = cols["data"][1]["name"]
        .as_str()
        .expect("col b name")
        .to_string();

    // Seed cards with explicit timestamps/due_date for deterministic ordering.
    // Due-date NULLs must be last in both asc and desc.
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `sort_key`, `due_date`, `created_at`, `updated_at`) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("Alpha")
    .bind(col_a)
    .bind("a0")
    .bind(None::<chrono::NaiveDateTime>)
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .execute(&pool)
    .await
    .expect("seed alpha");

    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `sort_key`, `due_date`, `created_at`, `updated_at`) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("Bravo")
    .bind(col_a)
    .bind("a1")
    .bind(Some(chrono::NaiveDateTime::parse_from_str("2026-05-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()))
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-02 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-03 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .execute(&pool)
    .await
    .expect("seed bravo");

    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `sort_key`, `due_date`, `created_at`, `updated_at`) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("Charlie")
    .bind(col_b)
    .bind("a0")
    .bind(Some(chrono::NaiveDateTime::parse_from_str("2026-04-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()))
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-03 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-02 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .execute(&pool)
    .await
    .expect("seed charlie");

    // Same updated_at as Alpha to test deterministic id tiebreaker (same direction).
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `sort_key`, `due_date`, `created_at`, `updated_at`) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("Delta")
    .bind(col_b)
    .bind("a1")
    .bind(None::<chrono::NaiveDateTime>)
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-04 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .bind(chrono::NaiveDateTime::parse_from_str("2026-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .execute(&pool)
    .await
    .expect("seed delta");

    // Default: page=1,limit=20,sort=updated_at,order=desc
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "default list: {body}");
    assert_eq!(body["page"], 1);
    assert_eq!(body["limit"], 20);
    assert_eq!(body["total"], 4);
    let data = body["data"].as_array().expect("data[]");
    assert_eq!(data.len(), 4);
    let alpha_id = data.iter().find(|c| c["title"] == "Alpha").unwrap()["id"]
        .as_i64()
        .unwrap();
    let delta_id = data.iter().find(|c| c["title"] == "Delta").unwrap()["id"]
        .as_i64()
        .unwrap();
    let tail_ids_desc = if delta_id > alpha_id {
        [delta_id, alpha_id]
    } else {
        [alpha_id, delta_id]
    };
    // updated_at desc: Bravo(01-03), Charlie(01-02), then same updated_at (01-01) by id desc.
    let expected_default: Vec<i64> = std::iter::once(
        data.iter().find(|c| c["title"] == "Bravo").unwrap()["id"]
            .as_i64()
            .unwrap(),
    )
    .chain(std::iter::once(
        data.iter().find(|c| c["title"] == "Charlie").unwrap()["id"]
            .as_i64()
            .unwrap(),
    ))
    .chain(tail_ids_desc)
    .collect();
    let actual_default: Vec<i64> = data.iter().map(|c| c["id"].as_i64().unwrap()).collect();
    assert_eq!(
        actual_default, expected_default,
        "default updated_at desc, id desc: {body}"
    );

    // Field/shape + locked position semantics (per-column 0-based sort-key order).
    for item in data {
        let title = item["title"].as_str().expect("title");
        let cn = item["column_name"].as_str().expect("column_name");
        assert!(
            cn == col_a_name || cn == col_b_name,
            "unexpected column_name {cn}: {item}"
        );
        assert!(
            item.get("description").is_none(),
            "no description TEXT: {item}"
        );
        assert!(item["has_description"].is_boolean());
        assert!(item["column_id"].is_i64());
        assert!(item["labels"].is_array());
        assert!(item["due_date"].is_string() || item["due_date"].is_null());
        assert!(item["created_at"].is_string());
        assert!(item["updated_at"].is_string());
        assert!(
            item.get("checklist_progress").is_none(),
            "no checklist_progress without checklists: {item}"
        );
        let (expected_col, expected_pos) = match title {
            "Alpha" => (col_a, 0),
            "Bravo" => (col_a, 1),
            "Charlie" => (col_b, 0),
            "Delta" => (col_b, 1),
            _ => panic!("unexpected card {title}"),
        };
        assert_eq!(
            item["column_id"].as_i64().unwrap(),
            expected_col,
            "column_id for {title}: {item}"
        );
        assert_eq!(
            item["position"].as_i64().unwrap(),
            expected_pos,
            "per-column position for {title}: {item}"
        );
    }

    // due_date asc: NULLs last, id tiebreaker asc among nulls
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?sort=due_date&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "due_date asc: {body}");
    let due_names = names(&body);
    assert_eq!(due_names[0], "Charlie");
    assert_eq!(due_names[1], "Bravo");
    // last two are NULL due_date, ordered by id asc.
    let tail_ids: Vec<i64> = body["data"].as_array().unwrap()[2..4]
        .iter()
        .map(|c| c["id"].as_i64().unwrap())
        .collect();
    let mut expected_tail = [alpha_id, delta_id];
    expected_tail.sort();
    assert_eq!(
        tail_ids, expected_tail,
        "due_date asc null tail id asc: {body}"
    );

    // due_date desc: NULLs last, id tiebreaker desc among nulls
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?sort=due_date&order=desc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "due_date desc: {body}");
    let due_names = names(&body);
    assert_eq!(due_names[0], "Bravo");
    assert_eq!(due_names[1], "Charlie");
    let tail_ids: Vec<i64> = body["data"].as_array().unwrap()[2..4]
        .iter()
        .map(|c| c["id"].as_i64().unwrap())
        .collect();
    let mut expected_tail_desc = [alpha_id, delta_id];
    expected_tail_desc.sort();
    expected_tail_desc.reverse();
    assert_eq!(
        tail_ids, expected_tail_desc,
        "due_date desc null tail id desc: {body}"
    );

    // Allowed sorts/orders smoke: all combinations return 200
    for sort in ["created_at", "updated_at", "due_date", "title"] {
        for order in ["asc", "desc"] {
            let (status, _, bytes) = send(
                app(),
                authed_request(
                    "GET",
                    &format!("/api/v1/boards/{board_id}/cards?sort={sort}&order={order}"),
                    Some(&owner_jar),
                    None,
                ),
            )
            .await;
            let body = json_of(&bytes);
            assert_eq!(status, StatusCode::OK, "sort={sort} order={order}: {body}");
            assert_eq!(body["total"], 4);
        }
    }

    // Pagination
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?page=1&limit=2&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page 1: {body}");
    assert_eq!(body["page"], 1);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["total"], 4);
    assert_eq!(names(&body), vec!["Alpha", "Bravo"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?page=2&limit=2&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page 2: {body}");
    assert_eq!(names(&body), vec!["Charlie", "Delta"]);

    // Invalid params => 400 validation-error with errors[] naming the field.
    for (query, what, field) in [
        ("?page=0", "zero page", "page"),
        ("?page=-1", "negative page", "page"),
        ("?page=abc", "non-integer page", "page"),
        ("?page=1.5", "float page", "page"),
        ("?page=", "empty page", "page"),
        ("?page=4294967296&limit=100", "overflow page", "page"),
        ("?limit=0", "zero limit", "limit"),
        ("?limit=-1", "negative limit", "limit"),
        ("?limit=101", "over limit", "limit"),
        ("?limit=abc", "non-integer limit", "limit"),
        ("?limit=1.5", "float limit", "limit"),
        ("?limit=", "empty limit", "limit"),
        ("?sort=bogus", "invalid sort", "sort"),
        ("?sort=", "empty sort", "sort"),
        ("?order=bogus", "invalid order", "order"),
        ("?order=", "empty order", "order"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/boards/{board_id}/cards{query}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error", what);
        assert_errors_nonempty(&failure, what);
        assert!(
            failure["errors"]
                .as_array()
                .expect("errors[]")
                .iter()
                .any(|e| e["field"] == field),
            "{what} must name {field}: {failure}"
        );
    }

    // Window ceiling: page*limit > 10_000 => 400 validation-error on page
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?page=101&limit=100"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "window ceiling: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "validation-error",
        "window ceiling",
    );
    assert!(
        failure["errors"]
            .as_array()
            .expect("errors[]")
            .iter()
            .any(|e| e["field"] == "page"),
        "window ceiling must name page: {failure}"
    );

    // Boundary at window is allowed
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?page=100&limit=100"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "window boundary: {body}");

    // Ownership 404 no-oracle: missing and foreign board
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/boards/999999/cards", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "missing board: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "missing board");

    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{foreign_board_id}/cards"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign board: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "foreign board");

    // Garbage board id -> 400 invalid-path
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/boards/abc/cards", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "garbage board id: {failure}"
    );
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "garbage board id",
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn board_cards_list_empty_page_shape(pool: MySqlPool) {
    let app = || app_on_pool(&pool);
    let (owner_jar, _) = register(app(), "board-cards-empty@example.com").await;
    let board_id = create_board(app(), &owner_jar, &json!({"name": "Empty Board"})).await;

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?page=1&limit=20"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty page: {body}");
    assert_eq!(body["page"], 1);
    assert_eq!(body["limit"], 20);
    assert_eq!(body["total"], 0);
    assert_eq!(body["data"], json!([]));
}
