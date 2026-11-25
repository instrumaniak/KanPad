//! `GET /api/v1/boards/{boardId}/columns?embed_limit=n` (T048).
//!
//! Covers default embed limit (10), bounds (1..=100), invalid values returning
//! 400 `validation-error` with `errors[]` naming `embed_limit`, and
//! `total_cards` remaining authoritative when embeds are limited.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board, json_of,
    register, send,
};
use axum::http::StatusCode;
use serde_json::json;
use sqlx::MySqlPool;

async fn seed_cards(pool: &MySqlPool, column_id: i64, count: usize) {
    for i in 0..count {
        let title = format!("Card {i:02}");
        let sort_key = format!("a{i:02}");
        sqlx::query("INSERT INTO `cards` (`title`, `column_id`, `sort_key`) VALUES (?, ?, ?)")
            .bind(title)
            .bind(column_id)
            .bind(sort_key)
            .execute(pool)
            .await
            .expect("seed card");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn columns_embed_limit_validation_and_total_cards(pool: MySqlPool) {
    let app = || app_on_pool(&pool);
    let (owner_jar, _) = register(app(), "embed-owner@example.com").await;
    let board_id = create_board(app(), &owner_jar, &json!({"name": "Embed Board"})).await;

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
    let col_id = columns[0]["id"].as_i64().expect("column id");

    // Seed 12 cards in the first column; other default columns remain empty.
    seed_cards(&pool, col_id, 12).await;

    // Default omitted embed_limit => 10 embedded, total_cards authoritative.
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
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "default embed: {body}");
    let first = &body["data"][0];
    assert_eq!(
        first["total_cards"], 12,
        "total_cards authoritative: {body}"
    );
    assert_eq!(first["cards"].as_array().expect("cards[]").len(), 10);
    assert_eq!(first["cards"][0]["title"], "Card 00");
    assert_eq!(first["cards"][9]["title"], "Card 09");

    // embed_limit=1
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns?embed_limit=1"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "embed_limit=1: {body}");
    assert_eq!(body["data"][0]["total_cards"], 12);
    assert_eq!(
        body["data"][0]["cards"].as_array().expect("cards[]").len(),
        1
    );

    // embed_limit=100 (above total, returns all)
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns?embed_limit=100"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "embed_limit=100: {body}");
    assert_eq!(body["data"][0]["total_cards"], 12);
    assert_eq!(
        body["data"][0]["cards"].as_array().expect("cards[]").len(),
        12
    );

    // Invalid embed_limit values => 400 validation-error with errors[].embed_limit
    for (query, what) in [
        ("?embed_limit=0", "zero embed_limit"),
        ("?embed_limit=101", "over-max embed_limit"),
        ("?embed_limit=-1", "negative embed_limit"),
        ("?embed_limit=abc", "non-integer embed_limit"),
        ("?embed_limit=1.5", "float embed_limit"),
        ("?embed_limit=", "empty embed_limit"),
        ("?embed_limit=4294967296", "overflow embed_limit"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/boards/{board_id}/columns{query}"),
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
                .any(|e| e["field"] == "embed_limit"),
            "{what} must name embed_limit field: {failure}"
        );
    }

    // Ownership 404 no-oracle and garbage id 400 invalid-path on embed_limit endpoint.
    let (intruder_jar, _) = register(app(), "embed-intruder@example.com").await;
    let foreign_board_id =
        create_board(app(), &intruder_jar, &json!({"name": "Foreign Embed"})).await;
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{foreign_board_id}/columns?embed_limit=5"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign board: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "foreign board");

    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/boards/999999/columns?embed_limit=5",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "missing board: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "missing board");

    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/boards/abc/columns?embed_limit=5",
            Some(&owner_jar),
            None,
        ),
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
