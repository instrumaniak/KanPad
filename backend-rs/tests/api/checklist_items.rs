//! `POST /api/v1/checklists/:id/items` +
//! `PATCH|DELETE /api/v1/checklist-items/:itemId` (T26b).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T26a): all scenarios run
//! sequentially in one test with distinct emails. Registers owner + intruder
//! via `POST /api/v1/auth/register` (real Argon2id hashes, live cookie jars)
//! and creates boards/columns/cards/checklists via the API.
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: item routes do not exist yet):
//! `POST {text,position?} → 201 {data} + Location` (`MAX+1` default);
//! `PATCH {text?,is_completed?} → 200` (partial persists on re-read);
//! `DELETE → 204` (empty body); deep ownership
//! (`item → checklist → card → column → board.user` — foreign item/checklist →
//! `404`, never 403); negative/float `position` → `400 errors[]`; every 4xx
//! asserts problem+json + exact `type` URI; every mutation re-reads state via
//! second GET or direct `SELECT`.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, json_body, json_of,
    register, send,
};
use axum::http::{StatusCode, header};
use sqlx::MySqlPool;

#[sqlx::test(migrations = "./migrations")]
async fn checklist_items_crud_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every item route (AuthSession guard).
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/checklists/1/items",
            Some(json_body(&serde_json::json!({"text": "Nope"}))),
        ),
        ("GET", "/api/v1/checklists/1", None),
        (
            "PATCH",
            "/api/v1/checklist-items/1",
            Some(json_body(&serde_json::json!({"text": "Nope"}))),
        ),
        ("DELETE", "/api/v1/checklist-items/1", None),
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
            "Anonymous access is 401 on every item route (AuthSession…",
        );
    }

    // 1. Register owner + intruder; owner creates board → column → card →
    //    checklist; intruder creates a foreign checklist + item.
    let (owner_jar, _owner_id) = register(app(), "items-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "items-intruder@example.com").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Items Board"}))),
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
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Groceries"}))),
        ),
    )
    .await;
    let created_list = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create checklist: {created_list}"
    );
    let checklist_id = created_list["data"]["id"].as_i64().expect("checklist id");
    // Intruder's foreign checklist + item (for foreign-item 404 probes).
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
    let intruder_card = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "intruder card: {intruder_card}"
    );
    let foreign_card = intruder_card["data"]["id"]
        .as_i64()
        .expect("foreign card id");
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
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{foreign_checklist}/items"),
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({"text": "Theirs item"}))),
        ),
    )
    .await;
    let intruder_item = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "intruder item: {intruder_item}"
    );
    let foreign_item = intruder_item["data"]["id"]
        .as_i64()
        .expect("foreign item id");

    // 2. Create → 201 + Location; DB re-read + checklist-detail GET confirm.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"text": "Buy milk"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create item: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let item_id = created["data"]["id"].as_i64().expect("item id");
    assert_eq!(created["data"]["text"], "Buy milk");
    assert_eq!(created["data"]["is_completed"], false);
    assert_eq!(created["data"]["checklist_id"], checklist_id);
    assert_eq!(created["data"]["position"], 0, "first item is 0: {created}");
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
    assert_eq!(location, format!("/api/v1/checklist-items/{item_id}"));
    let stored: String = sqlx::query_scalar("SELECT `text` FROM `checklist_items` WHERE `id` = ?")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("created item re-reads");
    assert_eq!(stored, "Buy milk");
    // Second GET (checklist detail) proves persistence (not an echo).
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
    assert_eq!(status, StatusCode::OK, "detail after create: {detail}");
    let embedded = detail["data"]["items"].as_array().expect("items[]");
    assert_eq!(embedded.len(), 1, "one item embedded: {detail}");
    assert_eq!(embedded[0]["id"], item_id);
    assert_eq!(embedded[0]["text"], "Buy milk");

    // 3. Second create defaults to MAX+1; explicit position writes verbatim.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"text": "Buy eggs"}))),
        ),
    )
    .await;
    let second = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "second item: {second}");
    let second_id = second["data"]["id"].as_i64().expect("second id");
    assert_eq!(second["data"]["position"], 1, "default is MAX+1: {second}");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"text": "Pinned", "position": 5}),
            )),
        ),
    )
    .await;
    let pinned = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "explicit position: {pinned}");
    let pinned_id = pinned["data"]["id"].as_i64().expect("pinned id");
    assert_eq!(pinned["data"]["position"], 5);
    let stored_pos: i32 =
        sqlx::query_scalar("SELECT `position` FROM `checklist_items` WHERE `id` = ?")
            .bind(pinned_id)
            .fetch_one(&pool)
            .await
            .expect("explicit position re-reads");
    assert_eq!(stored_pos, 5);
    // Cleanup: the pinned row is not part of later counts.
    sqlx::query("DELETE FROM `checklist_items` WHERE `id` = ?")
        .bind(pinned_id)
        .execute(&pool)
        .await
        .expect("pinned cleanup");

    // 4. Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"text": "O'Brien"}))),
        ),
    )
    .await;
    let quoted = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "quote payload: {quoted}");
    let quote_id = quoted["data"]["id"].as_i64().expect("quote id");
    let stored_text: String =
        sqlx::query_scalar("SELECT `text` FROM `checklist_items` WHERE `id` = ?")
            .bind(quote_id)
            .fetch_one(&pool)
            .await
            .expect("quote re-reads literally");
    assert_eq!(stored_text, "O'Brien");
    // Cleanup: the quote row is not part of later counts.
    sqlx::query("DELETE FROM `checklist_items` WHERE `id` = ?")
        .bind(quote_id)
        .execute(&pool)
        .await
        .expect("quote cleanup");

    // 5. Validation: blank/overlong text → 400 errors[]; negative position →
    //    400 errors[]; float position → 400 invalid-json (serde rejects first);
    //    missing text → 400 invalid-json; garbage checklist id → 400
    //    invalid-path.
    for (payload, what) in [
        (serde_json::json!({"text": ""}), "empty text"),
        (serde_json::json!({"text": "   "}), "whitespace text"),
        (
            serde_json::json!({"text": "a".repeat(256)}),
            "overlong text",
        ),
        (
            serde_json::json!({"text": "Ok", "position": -1}),
            "negative position",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &format!("/api/v1/checklists/{checklist_id}/items"),
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
            "Validation: blank/overlong text",
        );
        assert_errors_nonempty(&failure, what);
    }
    // Float position never reaches garde — serde rejects it first.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"text": "Ok", "position": 1.5}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "float position: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "Validation: blank/overlong text",
    );
    // Missing `text` on create is a body-shape 400 (serde rejects first).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing text: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-json",
        "Validation: blank/overlong text",
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/checklists/abc/items",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"text": "Ok"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "garbage checklist id: {failure}"
    );
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "Validation: blank/overlong text",
    );

    // 6. Foreign/missing checklist → 404 (never 403) on create.
    for (id, what) in [
        (foreign_checklist.to_string(), "foreign checklist"),
        ("999999".to_string(), "missing checklist"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &format!("/api/v1/checklists/{id}/items"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"text": "Hijack"}))),
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
            "Foreign/missing checklist",
        );
    }
    // Foreign/missing item → 404 (never 403) on patch/delete; garbage id →
    // 400 invalid-path.
    for (method, path, body, what) in [
        (
            "PATCH",
            format!("/api/v1/checklist-items/{foreign_item}"),
            Some(json_body(&serde_json::json!({"text": "Hijack"}))),
            "foreign item patch",
        ),
        (
            "PATCH",
            "/api/v1/checklist-items/999999".to_string(),
            Some(json_body(&serde_json::json!({"text": "Hijack"}))),
            "missing item patch",
        ),
        (
            "DELETE",
            format!("/api/v1/checklist-items/{foreign_item}"),
            None,
            "foreign item delete",
        ),
        (
            "DELETE",
            "/api/v1/checklist-items/999999".to_string(),
            None,
            "missing item delete",
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
            "Foreign/missing checklist",
        );
    }
    for path in ["/api/v1/checklist-items/abc"] {
        for (method, body) in [
            (
                "PATCH",
                Some(json_body(&serde_json::json!({"text": "Hijack"}))),
            ),
            ("DELETE", None),
        ] {
            let (status, headers, bytes) =
                send(app(), authed_request(method, path, Some(&owner_jar), body)).await;
            let failure = json_of(&bytes);
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{method} garbage {path}: {failure}"
            );
            assert_problem(
                status,
                &headers,
                &failure,
                "invalid-path",
                "Foreign/missing checklist",
            );
        }
    }

    // 7. PATCH is_completed partial → 200; re-GET + SELECT confirm (no echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{item_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"is_completed": true}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch is_completed: {patched}");
    assert_eq!(patched["data"]["id"], item_id);
    assert_eq!(patched["data"]["is_completed"], true);
    assert_eq!(
        patched["data"]["text"], "Buy milk",
        "untouched field stable: {patched}"
    );
    let stored_flag: i8 =
        sqlx::query_scalar("SELECT `is_completed` FROM `checklist_items` WHERE `id` = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("patched flag re-reads");
    assert_eq!(stored_flag, 1);
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
    let reread_item = reread["data"]["items"]
        .as_array()
        .expect("items[]")
        .iter()
        .find(|item| item["id"] == item_id)
        .expect("patched item embedded")
        .clone();
    assert_eq!(reread_item["is_completed"], true);
    // PATCH text partial → 200; SELECT confirms.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{item_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"text": "Buy oat milk"}))),
        ),
    )
    .await;
    let renamed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch text: {renamed}");
    assert_eq!(renamed["data"]["text"], "Buy oat milk");
    assert_eq!(
        renamed["data"]["is_completed"], true,
        "untouched flag stable: {renamed}"
    );
    let stored_text: String =
        sqlx::query_scalar("SELECT `text` FROM `checklist_items` WHERE `id` = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("renamed item re-reads");
    assert_eq!(stored_text, "Buy oat milk");
    // PATCH validation → 400 errors[].
    for (payload, what) in [
        (serde_json::json!({"text": "  "}), "blank text"),
        (
            serde_json::json!({"text": "a".repeat(256)}),
            "overlong text",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/checklist-items/{item_id}"),
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
            "PATCH is_completed partial",
        );
        assert_errors_nonempty(&failure, what);
    }
    // Empty {} PATCH is a no-op 200.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/checklist-items/{item_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let noop = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty patch: {noop}");
    assert_eq!(noop["data"]["text"], "Buy oat milk");
    assert_eq!(noop["data"]["is_completed"], true);

    // 8. Deprecated `/api/*` alias serves the same create handler.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/checklists/{checklist_id}/items"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"text": "Alias item"}))),
        ),
    )
    .await;
    let aliased = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "alias create: {aliased}");
    assert_eq!(aliased["data"]["text"], "Alias item");
    let alias_id = aliased["data"]["id"].as_i64().expect("alias id");
    sqlx::query("DELETE FROM `checklist_items` WHERE `id` = ?")
        .bind(alias_id)
        .execute(&pool)
        .await
        .expect("alias cleanup");

    // 9. DELETE → 204 empty body; re-GET detail no longer embeds the row;
    //    `SELECT COUNT` = 0.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/checklist-items/{item_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete item: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries an empty body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `checklist_items` WHERE `id` = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("item delete re-reads");
    assert_eq!(remaining, 0);
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
    let relisted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-get after delete: {relisted}");
    let relisted_ids: Vec<i64> = relisted["data"]["items"]
        .as_array()
        .expect("items[]")
        .iter()
        .map(|item| item["id"].as_i64().expect("id"))
        .collect();
    assert!(
        !relisted_ids.contains(&item_id),
        "deleted row gone from detail: {relisted}"
    );
    assert!(
        relisted_ids.contains(&second_id),
        "sibling survives: {relisted}"
    );

    // 10. Deleting twice → 404.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/checklist-items/{item_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "delete twice: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "Deleting twice");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
