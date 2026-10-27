//! `POST|GET /api/v1/cards/:cardId/checklists` +
//! `GET|PATCH|DELETE /api/v1/checklists/:id` (T26a).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T24a): all scenarios run
//! sequentially in one test with distinct emails. Registers owner + intruder
//! via `POST /api/v1/auth/register` (real Argon2id hashes, live cookie jars)
//! and creates boards/columns/cards via the API.
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: checklist routes do not exist yet):
//! `POST {title} → 201 {data} + Location`; `GET /cards/:id/checklists → 200
//! {data[]}` (small-N, unpaginated — added route); `GET /checklists/:id →
//! 200`; `PATCH {title} → 200`; `DELETE → 204` (items cascade via the DB FK,
//! `SELECT COUNT` = 0 on re-read); deep ownership
//! (`checklist → card → column → board.user` — foreign card/checklist →
//! `404`, never 403); empty title → `400 errors[]`; every 4xx asserts
//! problem+json + exact `type` URI; every mutation re-reads state via second
//! GET or direct `SELECT`.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, json_body, json_of,
    register, send,
};
use axum::http::{StatusCode, header};
use sqlx::MySqlPool;

#[sqlx::test(migrations = "./migrations")]
async fn checklists_crud_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every checklist route (AuthSession guard).
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/cards/1/checklists",
            Some(json_body(&serde_json::json!({"title": "Nope"}))),
        ),
        ("GET", "/api/v1/cards/1/checklists", None),
        ("GET", "/api/v1/checklists/1", None),
        (
            "PATCH",
            "/api/v1/checklists/1",
            Some(json_body(&serde_json::json!({"title": "Nope"}))),
        ),
        ("DELETE", "/api/v1/checklists/1", None),
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
            "Anonymous access is 401 on every checklist route…",
        );
    }

    // 1. Register owner + intruder; owner creates one board (3 default
    //    columns) + one card; intruder creates a foreign board + card.
    let (owner_jar, _owner_id) = register(app(), "checklists-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "checklists-intruder@example.com").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Checklist Board"}))),
        ),
    )
    .await;
    let created_board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create board: {created_board}");
    let col_id = created_board["data"]["columns"][0]["id"]
        .as_i64()
        .expect("column id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"title": "Owned card", "column_id": col_id}),
            )),
        ),
    )
    .await;
    let created_card = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create card: {created_card}");
    let card_id = created_card["data"]["id"].as_i64().expect("card id");
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
    let foreign_col = foreign_board["data"]["columns"][0]["id"]
        .as_i64()
        .expect("foreign column id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&intruder_jar),
            Some(json_body(
                &serde_json::json!({"title": "Intruder card", "column_id": foreign_col}),
            )),
        ),
    )
    .await;
    let intruder_card_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "intruder card: {intruder_card_body}"
    );
    let foreign_card = intruder_card_body["data"]["id"]
        .as_i64()
        .expect("foreign card id");
    // Intruder creates one checklist (for foreign-checklist 404 probes).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{foreign_card}/checklists"),
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({"title": "Theirs"}))),
        ),
    )
    .await;
    let intruder_list = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "intruder checklist: {intruder_list}"
    );
    let foreign_checklist = intruder_list["data"]["id"]
        .as_i64()
        .expect("foreign checklist id");

    // 2. Create → 201 + Location; DB re-read + GET detail confirm.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Acceptance"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create checklist: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let checklist_id = created["data"]["id"].as_i64().expect("checklist id");
    assert_eq!(created["data"]["title"], "Acceptance");
    assert_eq!(created["data"]["card_id"], card_id);
    assert_eq!(
        created["data"]["items"],
        serde_json::json!([]),
        "new checklist has no items: {created}"
    );
    assert!(
        created["data"]["created_at"]
            .as_str()
            .is_some_and(|ts| ts.ends_with('Z')),
        "timestamps are UTC ISO-8601: {created}"
    );
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII");
    assert_eq!(location, format!("/api/v1/checklists/{checklist_id}"));
    let stored: String = sqlx::query_scalar("SELECT `title` FROM `checklists` WHERE `id` = ?")
        .bind(checklist_id)
        .fetch_one(&pool)
        .await
        .expect("created checklist re-reads");
    assert_eq!(stored, "Acceptance");
    // Second GET proves persistence (not an echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "get detail: {detail}");
    assert_eq!(detail["data"]["id"], checklist_id);
    assert_eq!(detail["data"]["title"], "Acceptance");
    assert_eq!(detail["data"]["card_id"], card_id);

    // 3. Second checklist on the same card; card-scoped list returns both.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Release QA"}))),
        ),
    )
    .await;
    let second = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "second checklist: {second}");
    let second_id = second["data"]["id"].as_i64().expect("second id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "card-scoped list: {listed}");
    assert!(listed.get("message").is_none(), "no message key: {listed}");
    let items = listed["data"].as_array().expect("data[]");
    assert_eq!(items.len(), 2, "both checklists listed: {listed}");
    let listed_ids: Vec<i64> = items
        .iter()
        .map(|item| item["id"].as_i64().expect("id"))
        .collect();
    assert!(
        listed_ids.contains(&checklist_id) && listed_ids.contains(&second_id),
        "list returns its checklists: {listed}"
    );
    for item in items {
        assert_eq!(item["card_id"], card_id);
        assert!(item["items"].is_array(), "items[] embedded: {item}");
    }

    // 4. Seed items via SQL; single GET embeds items position-ordered.
    sqlx::query(
        "INSERT INTO `checklist_items` (`text`, `is_completed`, `checklist_id`, `position`) VALUES \
         ('Write test', 0, ?, 0), ('Pass test', 1, ?, 1)",
    )
    .bind(checklist_id)
    .bind(checklist_id)
    .execute(&pool)
    .await
    .expect("seed items");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detail with items: {detail}");
    let embedded = detail["data"]["items"].as_array().expect("items[]");
    assert_eq!(embedded.len(), 2);
    assert_eq!(embedded[0]["text"], "Write test");
    assert_eq!(embedded[0]["is_completed"], false);
    assert_eq!(embedded[1]["is_completed"], true);

    // 5. Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "O'Brien"}))),
        ),
    )
    .await;
    let quoted = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "quote payload: {quoted}");
    let quote_id = quoted["data"]["id"].as_i64().expect("quote id");
    let stored_title: String =
        sqlx::query_scalar("SELECT `title` FROM `checklists` WHERE `id` = ?")
            .bind(quote_id)
            .fetch_one(&pool)
            .await
            .expect("quote re-reads literally");
    assert_eq!(stored_title, "O'Brien");
    // Cleanup: the quote row is not part of later counts.
    sqlx::query("DELETE FROM `checklists` WHERE `id` = ?")
        .bind(quote_id)
        .execute(&pool)
        .await
        .expect("quote cleanup");

    // 6. Validation: blank/overlong title → 400 errors[]; wrong JSON type →
    //    400 invalid-json; garbage card id → 400 invalid-path.
    for (payload, what) in [
        (serde_json::json!({"title": ""}), "empty title"),
        (serde_json::json!({"title": "   "}), "whitespace title"),
        (
            serde_json::json!({"title": "a".repeat(256)}),
            "overlong title",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &format!("/api/v1/cards/{card_id}/checklists"),
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
            "Validation: blank/overlong title",
        );
        assert_errors_nonempty(&failure, what);
    }
    // Missing `title` on create is a body-shape 400 (serde rejects first).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing title: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "Validation: blank/overlong title",
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": 123}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "numeric title: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "Validation: blank/overlong title",
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards/abc/checklists",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Ok"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "garbage card id: {failure}"
    );
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "Validation: blank/overlong title",
    );

    // 7. Foreign/missing card → 404 (never 403) on create + list.
    for (id, what) in [
        (foreign_card.to_string(), "foreign card"),
        ("999999".to_string(), "missing card"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &format!("/api/v1/cards/{id}/checklists"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"title": "Hijack"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "create {what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Foreign/missing card",
        );
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/cards/{id}/checklists"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "list {what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Foreign/missing card",
        );
    }
    // Foreign/missing checklist → 404 (never 403) on get/patch/delete;
    // garbage id → 400 invalid-path.
    for (method, path, body, what) in [
        (
            "GET",
            format!("/api/v1/checklists/{foreign_checklist}"),
            None,
            "foreign checklist",
        ),
        (
            "GET",
            "/api/v1/checklists/999999".to_string(),
            None,
            "missing checklist",
        ),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request(method, &path, Some(&owner_jar), body)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Foreign/missing card",
        );
    }
    for path in ["/api/v1/checklists/abc"] {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", path, Some(&owner_jar), None)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "garbage {path}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "invalid-path",
            "Foreign/missing card",
        );
    }

    // 8. PATCH title → 200; re-GET + SELECT confirm (no echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Renamed"}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch title: {patched}");
    assert_eq!(patched["data"]["id"], checklist_id);
    assert_eq!(patched["data"]["title"], "Renamed");
    let stored: String = sqlx::query_scalar("SELECT `title` FROM `checklists` WHERE `id` = ?")
        .bind(checklist_id)
        .fetch_one(&pool)
        .await
        .expect("patched checklist re-reads");
    assert_eq!(stored, "Renamed");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let reread = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-get after patch: {reread}");
    assert_eq!(reread["data"]["title"], "Renamed");
    // PATCH validation → 400 errors[].
    for (payload, what) in [
        (serde_json::json!({"title": "  "}), "blank title"),
        (
            serde_json::json!({"title": "a".repeat(256)}),
            "overlong title",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/checklists/{checklist_id}"),
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
            "PATCH title",
        );
        assert_errors_nonempty(&failure, what);
    }
    // Empty {} PATCH is a no-op 200.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let noop = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty patch: {noop}");
    assert_eq!(noop["data"]["title"], "Renamed");
    // PATCH foreign/missing → 404 (never 403); garbage id → 400.
    for id in [foreign_checklist.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/checklists/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"title": "Hijack"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "patch {id}: {failure}");
        assert_problem(status, &headers, &failure, "not-found", "PATCH title");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/checklists/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path", "PATCH title");

    // 9. Deprecated `/api/*` alias serves the same create handler.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Alias list"}))),
        ),
    )
    .await;
    let aliased = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "alias create: {aliased}");
    assert_eq!(aliased["data"]["title"], "Alias list");
    let alias_id = aliased["data"]["id"].as_i64().expect("alias id");
    sqlx::query("DELETE FROM `checklists` WHERE `id` = ?")
        .bind(alias_id)
        .execute(&pool)
        .await
        .expect("alias cleanup");

    // 10. DELETE → 204 empty body; re-GET 404s; items cascade
    //     (`SELECT COUNT` = 0 for both tables).
    let items_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `checklist_items` WHERE `checklist_id` = ?")
            .bind(checklist_id)
            .fetch_one(&pool)
            .await
            .expect("items exist before delete");
    assert_eq!(items_before, 2);
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete checklist: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries an empty body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `checklists` WHERE `id` = ?")
        .bind(checklist_id)
        .fetch_one(&pool)
        .await
        .expect("checklist delete re-reads");
    assert_eq!(remaining, 0);
    let orphan_items: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `checklist_items` WHERE `checklist_id` = ?")
            .bind(checklist_id)
            .fetch_one(&pool)
            .await
            .expect("item cascade re-reads");
    assert_eq!(orphan_items, 0, "delete cascades items");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "re-get deleted: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "DELETE");
    // Card-scoped list no longer contains the deleted row.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let relisted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-list after delete: {relisted}");
    let relisted_ids: Vec<i64> = relisted["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|item| item["id"].as_i64().expect("id"))
        .collect();
    assert!(
        !relisted_ids.contains(&checklist_id),
        "deleted row gone from list: {relisted}"
    );
    assert!(
        relisted_ids.contains(&second_id),
        "sibling survives: {relisted}"
    );

    // 11. Deleting twice → 404; foreign/missing → 404 (never 403); garbage →
    //     400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/checklists/{checklist_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "delete twice: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "Deleting twice");
    for id in [foreign_checklist.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "DELETE",
                &format!("/api/v1/checklists/{id}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "delete {id}: {failure}");
        assert_problem(status, &headers, &failure, "not-found", "Deleting twice");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request("DELETE", "/api/v1/checklists/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path", "Deleting twice");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
