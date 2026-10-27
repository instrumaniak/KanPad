//! `POST /api/v1/boards` + `GET /api/v1/boards/{id}` (T21a).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T20): all scenarios run
//! sequentially in one test with distinct emails. This suite registers its
//! owner + intruder via `POST /api/v1/auth/register` (real Argon2id hashes,
//! live cookie jars) and creates projects via `POST /api/v1/projects`.
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: the boards router is an empty stub):
//! `POST {name,background_color?,project_id?} → 201 + Location` (board + 3
//! default columns `To Do/In Progress/Done` 0/1/2 in one tx; blank/overlong
//! name → 400 `errors[]`; bad hex → 400; unknown fields ignored; quote
//! payload stored literally; `project_id` type garbage → 400;
//! foreign/missing `project_id` → 404, never 403);
//! `GET /:id → 200 {data:{…board,columns[]}}` (nested unpaginated, `position
//! ASC`, no cards; no `description`/`content` keys; foreign/missing → 404;
//! garbage id → 400 `invalid-path`); every 4xx asserts
//! `Content-Type: application/problem+json` + exact `type` URI; every
//! mutation re-reads state via second GET or direct `SELECT`.

use super::helpers::{
    PROBLEM_JSON, app_on_pool, assert_errors_nonempty, assert_no_legacy_keys, assert_problem,
    authed_request, create_project, json_body, json_of, register, send,
};
use axum::http::{StatusCode, header};
use sqlx::MySqlPool;

#[sqlx::test(migrations = "./migrations")]
async fn boards_create_and_detail_full_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on both verbs (AuthSession guard).
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/boards",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        ("GET", "/api/v1/boards/1", None),
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
            "Anonymous access is 401 on both verbs (AuthSession guard)",
        );
    }

    // 1. Register owner + intruder; owner creates a project via the API.
    let (owner_jar, owner_id) = register(app(), "board-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "board-intruder@example.com").await;
    let project_id = create_project(app(), &owner_jar, "Owner Project").await;
    let foreign_project_id = create_project(app(), &intruder_jar, "Foreign Project").await;

    // 2. `POST {name} → 201 + Location` (defaults: `#0079BF`, no project).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Solo"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create Solo: {created}");
    assert_no_legacy_keys(&created, "create");
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII")
        .to_owned();
    let solo_id = created["data"]["id"].as_i64().expect("create returns id");
    assert_eq!(location, format!("/api/v1/boards/{solo_id}"));
    assert_eq!(created["data"]["name"], "Solo");
    assert_eq!(created["data"]["background_color"], "#0079BF");
    assert!(
        created["data"]["project_id"].is_null(),
        "no project: {created}"
    );
    assert!(
        created["data"]["project"].is_null(),
        "no project: {created}"
    );
    assert_eq!(created["data"]["is_archived"], false);
    assert_eq!(created["data"]["view_mode"], "board");
    assert!(
        created["data"]["created_at"].is_string(),
        "timestamps: {created}"
    );
    assert!(
        created["data"]["updated_at"].is_string(),
        "timestamps: {created}"
    );
    let columns = created["data"]["columns"]
        .as_array()
        .expect("create nests columns");
    assert_eq!(
        columns
            .iter()
            .map(|column| (
                column["name"].as_str().expect("column name"),
                column["position"].as_i64().expect("column position")
            ))
            .collect::<Vec<_>>(),
        vec![("To Do", 0), ("In Progress", 1), ("Done", 2)],
        "exactly the 3 default columns in order"
    );
    for column in columns {
        assert!(
            column["id"].as_i64().is_some_and(|id| id > 0),
            "column ids: {column}"
        );
    }
    // DB re-read: the board row + exactly 3 dense columns.
    let stored: (String, String, i64) =
        sqlx::query_as("SELECT `name`, `background_color`, `user_id` FROM `boards` WHERE `id` = ?")
            .bind(solo_id)
            .fetch_one(&pool)
            .await
            .expect("created board re-reads");
    assert_eq!(
        stored,
        ("Solo".to_string(), "#0079BF".to_string(), owner_id)
    );
    let column_rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT `name`, `position` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC",
    )
    .bind(solo_id)
    .fetch_all(&pool)
    .await
    .expect("created columns re-read");
    assert_eq!(
        column_rows,
        vec![
            ("To Do".to_string(), 0),
            ("In Progress".to_string(), 1),
            ("Done".to_string(), 2),
        ],
        "tx created exactly 3 dense columns"
    );

    // 3. `GET /:id → 200` with the same nested shape (second-GET re-read).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{solo_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detail: {detail}");
    assert_no_legacy_keys(&detail, "detail");
    assert_eq!(detail["data"]["id"], solo_id);
    assert_eq!(detail["data"]["name"], "Solo");
    assert_eq!(
        detail["data"]["columns"]
            .as_array()
            .expect("detail nests columns")
            .len(),
        3,
        "detail carries the 3 columns"
    );
    let positions: Vec<i64> = detail["data"]["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .map(|column| column["position"].as_i64().expect("position"))
        .collect();
    assert_eq!(positions, vec![0, 1, 2], "columns ordered position ASC");
    // Deprecated `/api/*` alias serves the same handler.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/boards/{solo_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alias detail: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["data"]["id"], solo_id);

    // 4. `POST` with an owned `project_id` links the board (+ project ref).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "name": "Linked",
                "background_color": "#FF5733",
                "project_id": project_id,
            }))),
        ),
    )
    .await;
    let linked = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create Linked: {linked}");
    assert_no_legacy_keys(&linked, "linked create");
    let linked_id = linked["data"]["id"].as_i64().expect("linked id");
    assert_eq!(
        headers
            .get(header::LOCATION)
            .expect("201 sets Location")
            .to_str()
            .expect("Location is ASCII"),
        format!("/api/v1/boards/{linked_id}"),
    );
    assert_eq!(linked["data"]["background_color"], "#FF5733");
    assert_eq!(linked["data"]["project_id"], project_id);
    assert_eq!(linked["data"]["project"]["id"], project_id);
    assert_eq!(linked["data"]["project"]["name"], "Owner Project");
    let stored_project: Option<i64> =
        sqlx::query_scalar("SELECT `project_id` FROM `boards` WHERE `id` = ?")
            .bind(linked_id)
            .fetch_one(&pool)
            .await
            .expect("linked board re-reads");
    assert_eq!(stored_project, Some(project_id), "project link persisted");

    // 5. Foreign / missing `project_id` → 404 (no oracle, NEVER 403), and no
    //    board row is created (tx rolled back before the insert).
    let boards_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `boards`")
        .fetch_one(&pool)
        .await
        .expect("board count re-reads");
    for (value, what) in [
        (serde_json::json!(foreign_project_id), "foreign project_id"),
        (serde_json::json!(999_999), "missing project_id"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/boards",
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"name": "Hijack", "project_id": value}),
                )),
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
            "Foreign / missing `project_id`",
        );
    }
    let boards_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `boards`")
        .fetch_one(&pool)
        .await
        .expect("board count re-reads");
    assert_eq!(
        boards_before, boards_after,
        "failed creates leave no board rows"
    );

    // 6. `project_id` int-parse failure → 400 (serde rejects before garde);
    //    `0` / negative → 400 `errors[]` (garde range).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "Bad", "project_id": "abc"}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "string project_id: {failure}"
    );
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "`project_id` int-parse failure",
    );
    for (value, what) in [
        (serde_json::json!(0), "zero project_id"),
        (serde_json::json!(-3), "negative project_id"),
        (serde_json::json!(1.5), "float project_id"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/boards",
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"name": "Bad", "project_id": value}),
                )),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_eq!(
            headers
                .get(header::CONTENT_TYPE)
                .expect("Content-Type")
                .to_str()
                .expect("ASCII"),
            PROBLEM_JSON
        );
        if value == serde_json::json!(1.5) {
            assert_problem(
                status,
                &headers,
                &failure,
                "invalid-json",
                "`project_id` int-parse failure",
            );
        } else {
            assert_problem(
                status,
                &headers,
                &failure,
                "validation-error",
                "`project_id` int-parse failure",
            );
            assert_errors_nonempty(&failure, what);
        }
    }

    // 7. Name validation: blank / overlong → 400 `errors[]`; missing /
    //    non-string → body-shape 400; unknown fields ignored (still 201);
    //    bad hex → 400 `errors[]`; quote payload stored literally.
    for (payload, what) in [
        (serde_json::json!({"name": ""}), "empty name"),
        (serde_json::json!({"name": "   "}), "whitespace name"),
        (
            serde_json::json!({"name": "a".repeat(256)}),
            "overlong name",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/boards",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "validation-error",
            "Name validation: blank / overlong",
        );
        assert_errors_nonempty(&failure, what);
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": 123}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "non-string name: {failure}"
    );
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "Name validation: blank / overlong",
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing name: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "Name validation: blank / overlong",
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "Tinted", "background_color": "red"}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "bad hex: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "validation-error",
        "Name validation: blank / overlong",
    );
    assert_errors_nonempty(&failure, "bad hex");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Extra", "bogus": 1}))),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unknown fields ignored: {}",
        json_of(&bytes)
    );
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "O'Brien"}))),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "quote name: {}",
        json_of(&bytes)
    );
    let quote_name: String =
        sqlx::query_scalar("SELECT `name` FROM `boards` WHERE `name` = 'O''Brien'")
            .fetch_one(&pool)
            .await
            .expect("quote payload re-reads literally");
    assert_eq!(quote_name, "O'Brien");

    // 8. Detail ownership: foreign / missing id → 404 (never 403); garbage →
    //    400 `invalid-path`.
    let intruder_board_id = {
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
        assert_eq!(
            status,
            StatusCode::CREATED,
            "intruder board: {}",
            json_of(&bytes)
        );
        json_of(&bytes)["data"]["id"]
            .as_i64()
            .expect("intruder board id")
    };
    for (id, what) in [
        (intruder_board_id.to_string(), "foreign board"),
        ("999999".to_string(), "missing board"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/boards/{id}"),
                Some(&owner_jar),
                None,
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
            "Detail ownership: foreign / missing id",
        );
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/boards/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "Detail ownership: foreign / missing id",
    );

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
