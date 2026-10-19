//! `PATCH /api/v1/boards/{id}` + `DELETE /api/v1/boards/{id}` (T22).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T21a/T21b): all scenarios
//! run sequentially in one test with distinct emails. This suite registers its
//! owner + intruder via `POST /api/v1/auth/register` (real Argon2id hashes,
//! live cookie jars) and creates projects/boards via the API.
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: PATCH/DELETE routes do not exist yet):
//! `PATCH {name?,background_color?,project_id?|null,view_mode?,is_archived?}
//! → 200 {data}` (foreign `project_id` → 404, never 403; bad `view_mode`
//! enum → 400 `errors[]`; `is_archived` flips idempotent — double PATCH same
//! 200, state stable on re-read);
//! `DELETE /:id → 204` hard delete (active board → 409 `conflict` problem,
//! NOT 403; archived → 204, re-GET 404 + columns cascade gone via `SELECT`);
//! no verb routes (`/:id/archive|/restore|/permanent` → 404 from the router);
//! every 4xx asserts `Content-Type: application/problem+json` + exact `type`
//! URI; every mutation re-reads state via second GET or direct `SELECT`.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board_json,
    create_project, json_body, json_of, register, send, PROBLEM_JSON,
};
use axum::http::{header, StatusCode};
use sqlx::MySqlPool;

#[sqlx::test(migrations = "./migrations")]
async fn boards_state_update_archive_delete_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on both verbs (AuthSession guard).
    for (method, path, body) in [
        (
            "PATCH",
            "/api/v1/boards/1",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        ("DELETE", "/api/v1/boards/1", None),
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

    // 1. Register owner + intruder; owner creates two projects, one board
    //    linked to the first project.
    let (owner_jar, _) = register(app(), "state-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "state-intruder@example.com").await;
    let project_a = create_project(app(), &owner_jar, "Project A").await;
    let project_b = create_project(app(), &owner_jar, "Project B").await;
    let foreign_project = create_project(app(), &intruder_jar, "Foreign Project").await;
    let created = create_board_json(
        app(),
        &owner_jar,
        &serde_json::json!({"name": "Stateful", "project_id": project_a}),
    )
    .await;
    let board_id = created["data"]["id"].as_i64().expect("board id");

    // 2. Multi-field PATCH → 200 {data} with every field applied; re-GET
    //    confirms the persisted state (no echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "name": "Renamed",
                "background_color": "#FF5733",
                "view_mode": "list",
            }))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "multi-field patch: {patched}");
    assert_eq!(patched["data"]["id"], board_id);
    assert_eq!(patched["data"]["name"], "Renamed");
    assert_eq!(patched["data"]["background_color"], "#FF5733");
    assert_eq!(patched["data"]["view_mode"], "list");
    assert_eq!(patched["data"]["project_id"], project_a);
    assert_eq!(
        patched["data"]["columns"]
            .as_array()
            .expect("patch nests columns")
            .len(),
        3,
        "patch response keeps the nested columns"
    );
    let stored: (String, String, String) = sqlx::query_as(
        "SELECT `name`, `background_color`, `view_mode` FROM `boards` WHERE `id` = ?",
    )
    .bind(board_id)
    .fetch_one(&pool)
    .await
    .expect("patched board re-reads");
    assert_eq!(
        stored,
        (
            "Renamed".to_string(),
            "#FF5733".to_string(),
            "list".to_string()
        )
    );
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-get after patch: {detail}");
    assert_eq!(detail["data"]["name"], "Renamed");
    assert_eq!(detail["data"]["view_mode"], "list");

    // 3. Quote payload on update is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "O'Brien"}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "quote patch: {patched}");
    assert_eq!(patched["data"]["name"], "O'Brien");
    let quote_name: String = sqlx::query_scalar("SELECT `name` FROM `boards` WHERE `id` = ?")
        .bind(board_id)
        .fetch_one(&pool)
        .await
        .expect("quote payload re-reads literally");
    assert_eq!(quote_name, "O'Brien");

    // 4. Validation: blank/overlong name, bad hex, bad view_mode enum →
    //    400 `errors[]`; wrong JSON types → 400 body-shape errors.
    for (payload, what) in [
        (serde_json::json!({"name": ""}), "empty name"),
        (serde_json::json!({"name": "   "}), "whitespace name"),
        (
            serde_json::json!({"name": "a".repeat(256)}),
            "overlong name",
        ),
        (serde_json::json!({"background_color": "red"}), "bad hex"),
        (serde_json::json!({"view_mode": "grid"}), "bad view_mode"),
        (
            serde_json::json!({"view_mode": "BOARD"}),
            "uppercase view_mode",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/boards/{board_id}"),
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
            "Validation: blank/overlong name, bad hex, bad view_mode…",
        );
        assert_errors_nonempty(&failure, what);
    }
    for (payload, what) in [
        (
            serde_json::json!({"project_id": "abc"}),
            "string project_id",
        ),
        (serde_json::json!({"project_id": 1.5}), "float project_id"),
        (serde_json::json!({"project_id": 0}), "zero project_id"),
        (serde_json::json!({"project_id": -3}), "negative project_id"),
        (serde_json::json!({"view_mode": 123}), "numeric view_mode"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/boards/{board_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .expect("Content-Type")
            .to_str()
            .expect("ASCII");
        assert_eq!(content_type, PROBLEM_JSON);
        if matches!(what, "zero project_id" | "negative project_id") {
            assert_problem(
                status,
                &headers,
                &failure,
                "validation-error",
                "Validation: blank/overlong name, bad hex, bad view_mode…",
            );
            assert_errors_nonempty(&failure, what);
        } else {
            assert_problem(
                status,
                &headers,
                &failure,
                "invalid-json",
                "Validation: blank/overlong name, bad hex, bad view_mode…",
            );
        }
    }

    // 5. `project_id` reassignment to an owned project links (200 + ref);
    //    explicit `null` detaches; foreign/missing → 404 (never 403) with
    //    the previous link untouched.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"project_id": project_b}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "reassign project: {patched}");
    assert_eq!(patched["data"]["project_id"], project_b);
    assert_eq!(patched["data"]["project"]["id"], project_b);
    assert_eq!(patched["data"]["project"]["name"], "Project B");
    let stored_project: Option<i64> =
        sqlx::query_scalar("SELECT `project_id` FROM `boards` WHERE `id` = ?")
            .bind(board_id)
            .fetch_one(&pool)
            .await
            .expect("reassigned board re-reads");
    assert_eq!(stored_project, Some(project_b));
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"project_id": null}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detach project: {patched}");
    assert!(
        patched["data"]["project_id"].is_null(),
        "detach clears the link: {patched}"
    );
    assert!(
        patched["data"]["project"].is_null(),
        "detach clears the ref: {patched}"
    );
    let stored_project: Option<i64> =
        sqlx::query_scalar("SELECT `project_id` FROM `boards` WHERE `id` = ?")
            .bind(board_id)
            .fetch_one(&pool)
            .await
            .expect("detached board re-reads");
    assert_eq!(stored_project, None);
    for (value, what) in [
        (serde_json::json!(foreign_project), "foreign project_id"),
        (serde_json::json!(999_999), "missing project_id"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/boards/{board_id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"project_id": value}))),
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
            "`project_id` reassignment to an owned project links (200…",
        );
    }
    let stored_project: Option<i64> =
        sqlx::query_scalar("SELECT `project_id` FROM `boards` WHERE `id` = ?")
            .bind(board_id)
            .fetch_one(&pool)
            .await
            .expect("failed reassign re-reads");
    assert_eq!(
        stored_project, None,
        "failed reassignment leaves the link untouched"
    );

    // 6. Empty `{}` PATCH is a no-op 200 (state unchanged on re-read).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty patch: {patched}");
    assert_eq!(patched["data"]["id"], board_id);
    assert_eq!(patched["data"]["name"], "O'Brien");

    // 7. Archive/restore are `is_archived` flips, idempotent: double PATCH
    //    same 200, state stable on re-read.
    for (value, what) in [(true, "archive"), (true, "archive again")] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/boards/{board_id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"is_archived": value}))),
            ),
        )
        .await;
        let patched = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "{what}: {patched}");
        assert_eq!(patched["data"]["is_archived"], true, "{what}: {patched}");
    }
    let archived_flag: i8 = sqlx::query_scalar("SELECT `is_archived` FROM `boards` WHERE `id` = ?")
        .bind(board_id)
        .fetch_one(&pool)
        .await
        .expect("archived board re-reads");
    assert_eq!(archived_flag, 1);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "re-get after archive: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(
        json_of(&bytes)["data"]["is_archived"],
        true,
        "archive stable on re-read"
    );
    for what in ["restore", "restore again"] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/boards/{board_id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"is_archived": false}))),
            ),
        )
        .await;
        let patched = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "{what}: {patched}");
        assert_eq!(patched["data"]["is_archived"], false, "{what}: {patched}");
    }
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "re-get after restore: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(
        json_of(&bytes)["data"]["is_archived"],
        false,
        "restore stable on re-read"
    );

    // 8. Deleting an ACTIVE board → 409 state conflict (NOT 403); the board
    //    survives (re-GET 200).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::CONFLICT, "delete active: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "conflict",
        "Deleting an ACTIVE board",
    );
    let (status, _, _) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "active board survives 409");

    // 9. Archive, then DELETE → 204 (empty body); re-GET 404; the board row
    //    and its columns cascade gone via `SELECT`.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"is_archived": true}))),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "pre-delete archive: {}",
        json_of(&bytes)
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete archived: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries an empty body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "re-get deleted: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "not-found",
        "Archive, then DELETE",
    );
    let board_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `boards` WHERE `id` = ?")
        .bind(board_id)
        .fetch_one(&pool)
        .await
        .expect("deleted board count re-reads");
    assert_eq!(board_count, 0, "board row is gone");
    let column_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `columns` WHERE `board_id` = ?")
            .bind(board_id)
            .fetch_one(&pool)
            .await
            .expect("deleted board columns re-read");
    assert_eq!(column_count, 0, "columns cascade gone");
    // Deleting twice → 404 (already gone, still no oracle).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/boards/{board_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "delete twice: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "not-found",
        "Archive, then DELETE",
    );

    // 10. Deprecated `/api/*` alias serves the same PATCH handler.
    let alias_board = create_board_json(app(), &owner_jar, &serde_json::json!({"name": "Alias"}))
        .await["data"]["id"]
        .as_i64()
        .expect("alias board id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/boards/{alias_board}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Aliased"}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "alias patch: {patched}");
    assert_eq!(patched["data"]["name"], "Aliased");

    // 11. Ownership: foreign/missing id → 404 (never 403) on PATCH+DELETE;
    //     garbage id → 400 `invalid-path`.
    let intruder_board =
        create_board_json(app(), &intruder_jar, &serde_json::json!({"name": "Theirs"})).await
            ["data"]["id"]
            .as_i64()
            .expect("intruder board id");
    for (method, what) in [
        (
            "PATCH",
            Some(json_body(&serde_json::json!({"name": "Hijack"}))),
        ),
        ("DELETE", None),
    ] {
        for (id, case) in [
            (intruder_board.to_string(), "foreign board"),
            ("999999".to_string(), "missing board"),
        ] {
            let (status, headers, bytes) = send(
                app(),
                authed_request(
                    method,
                    &format!("/api/v1/boards/{id}"),
                    Some(&owner_jar),
                    what.clone(),
                ),
            )
            .await;
            let failure = json_of(&bytes);
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {case}: {failure}");
            assert_problem(
                status,
                &headers,
                &failure,
                "not-found",
                "Ownership: foreign/missing id",
            );
        }
        let (status, headers, bytes) = send(
            app(),
            authed_request(method, "/api/v1/boards/abc", Some(&owner_jar), what.clone()),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{method} garbage id: {failure}"
        );
        assert_problem(
            status,
            &headers,
            &failure,
            "invalid-path",
            "Ownership: foreign/missing id",
        );
    }

    // 12. Deleted verb routes (`/:id/archive|/restore|/permanent`) → 404
    //     from the router on every method (never a 200 state change).
    for route in ["archive", "restore", "permanent"] {
        for method in ["GET", "POST", "PATCH", "DELETE"] {
            let body = if method == "POST" || method == "PATCH" {
                Some(json_body(&serde_json::json!({})))
            } else {
                None
            };
            let (status, headers, bytes) = send(
                app(),
                authed_request(
                    method,
                    &format!("/api/v1/boards/{alias_board}/{route}"),
                    Some(&owner_jar),
                    body,
                ),
            )
            .await;
            let failure = json_of(&bytes);
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "{method} /boards/:id/{route}: {failure}"
            );
            assert_problem(
                status,
                &headers,
                &failure,
                "not-found",
                "Deleted verb routes (`/:id/archive|/restore|/permanent`)",
            );
        }
    }

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
