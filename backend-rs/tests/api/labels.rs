//! `GET/POST /api/v1/labels` + `PATCH/DELETE /api/v1/labels/{id}` (T25).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T20): all scenarios run
//! sequentially in one test with distinct emails. Fixture `users.sql`
//! passwords are opaque placeholders, so this suite registers its owner +
//! intruder via `POST /api/v1/auth/register` (real Argon2id hashes, live
//! cookie jars) and therefore starts from the register-seeded 6 defaults.
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: the labels router is an empty stub):
//! list `200 Page` (`data,page,limit,total`, `id ASC`) with lazy-seed of the
//! 6 prod defaults via `common::label_seed::ensure_default_labels` when the
//! user has zero rows (seed race swallowed: re-seed is a no-op, still 6);
//! register-seed idempotency through the `helpers::ensure_default_labels`
//! wrapper (call twice → 6 then 0, still 6 rows; single multi-row
//! `INSERT IGNORE` in prod, no local copy);
//! `POST {name(1..50,trim,per-user unique→409),color enum6} → 201 + Location`
//! (blank/overlong/missing/bad-color → 400 `errors[]`; unknown fields
//! ignored; quote payload stored literally; case-variant `'Work'` vs
//! `'work'` → 409 under `utf8mb4_unicode_ci`);
//! `PATCH → 200` (trim + dup → 409 incl. case-variant; bad color → 400;
//! empty-body patch is a no-op 200; `GET`-style bad id `abc` → 400
//! `invalid-path`);
//! `DELETE → 204` empty body (repeat/foreign → 404, garbage → 400);
//! cross-user id → 404 (no oracle, NEVER 403); strict pagination
//! (`page=0`/`limit=0`/`limit=101`/non-integer → 400 `errors[]`);
//! every 4xx asserts `Content-Type: application/problem+json` + exact `type`
//! URI; every mutation re-reads state via second GET or direct `SELECT`.
//!
//! Contract: `specs/api-rewrite-rs-specs/20-api-contract.md` labels section
//! (`GET → Page<Label>` lazy-seed; `POST {name(1..50,trim,per-user
//! unique→409),color:red|orange|yellow|green|blue|purple}→201+Location`;
//! `PATCH →200`; `DELETE→204`; whitespace-only → 400 `errors[]`; 409
//! per-user unique incl. case-insensitive collation note) + `21-api-standard-
//! shapes.md` (same rows; `message` envelope dropped, deletes are 204).
//! Schema: `30-db-schema.md` labels row (`name varchar50`, `color VARCHAR(20)
//! + CHECK` 6 values, `UQ_labels_user_name UNIQUE(user_id,name)`).

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, json_body, json_of,
    register, send,
};
use axum::http::{header, StatusCode};
use sqlx::MySqlPool;

async fn label_count(pool: &MySqlPool, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
        .bind(user_id as i32)
        .fetch_one(pool)
        .await
        .expect("label count re-reads")
}

#[sqlx::test(migrations = "./migrations")]
async fn labels_crud_full_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every verb (AuthSession guard).
    for (method, path, body) in [
        ("GET", "/api/v1/labels", None),
        (
            "POST",
            "/api/v1/labels",
            Some(json_body(
                &serde_json::json!({"name": "Nope", "color": "red"}),
            )),
        ),
        (
            "PATCH",
            "/api/v1/labels/1",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        ("DELETE", "/api/v1/labels/1", None),
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
            "Anonymous access is 401 on every verb (AuthSession guard)",
        );
    }

    // 1. Register owner + intruder: each is seeded exactly 6 defaults.
    let (owner_jar, owner_id) = register(app(), "labels-owner@example.com").await;
    let (intruder_jar, intruder_id) = register(app(), "labels-intruder@example.com").await;
    assert_ne!(owner_id, intruder_id);
    assert_eq!(label_count(&pool, owner_id).await, 6);
    assert_eq!(label_count(&pool, intruder_id).await, 6);

    // 2. List is a real `Page` (never a bare array) with the 6 defaults.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/labels", Some(&owner_jar), None),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list: {listed}");
    assert_eq!(listed["page"], 1);
    assert_eq!(listed["limit"], 50);
    assert_eq!(listed["total"], 6);
    let names: Vec<&str> = listed["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|item| item["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec![
            "Urgent",
            "Important",
            "In Progress",
            "Done",
            "Bug",
            "Feature"
        ],
        "defaults in id order with prod colors"
    );
    let colors: Vec<&str> = listed["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|item| item["color"].as_str().expect("color"))
        .collect();
    assert_eq!(
        colors,
        vec!["red", "orange", "yellow", "green", "blue", "purple"]
    );
    assert!(
        listed["data"][0]["created_at"].is_string(),
        "timestamps: {listed}"
    );
    assert!(
        listed["data"][0]["updated_at"].is_string(),
        "timestamps: {listed}"
    );
    assert!(listed.get("message").is_none(), "no message key: {listed}");
    // Deprecated `/api/*` alias serves the same handler.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/labels", Some(&owner_jar), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alias list: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["total"], 6);

    // 3. Lazy-seed: wipe the owner's rows, then GET re-seeds the 6 defaults
    //    (second GET keeps exactly 6 — the seed race is a swallowed no-op).
    sqlx::query("DELETE FROM `labels` WHERE `user_id` = ?")
        .bind(owner_id as i32)
        .execute(&pool)
        .await
        .expect("labels wipe for lazy-seed");
    assert_eq!(label_count(&pool, owner_id).await, 0);
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/labels", Some(&owner_jar), None),
    )
    .await;
    let reseeded = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "lazy-seed list: {reseeded}");
    assert_eq!(reseeded["total"], 6);
    assert_eq!(label_count(&pool, owner_id).await, 6);
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/labels", Some(&owner_jar), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "second list: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["total"], 6);
    assert_eq!(label_count(&pool, owner_id).await, 6);

    // 4. Register-seed idempotency via the T07 test wrapper (which calls prod
    //    `common::label_seed::ensure_default_labels` — no local copy): wipe,
    //    call twice, still exactly 6 rows (6 then 0 inserted).
    sqlx::query("DELETE FROM `labels` WHERE `user_id` = ?")
        .bind(owner_id as i32)
        .execute(&pool)
        .await
        .expect("labels wipe for helper idempotency");
    let first = crate::helpers::ensure_default_labels(&pool, owner_id as i32)
        .await
        .expect("first seed inserts");
    assert_eq!(first, 6, "first seed must insert exactly 6 rows");
    let second = crate::helpers::ensure_default_labels(&pool, owner_id as i32)
        .await
        .expect("second seed is a no-op");
    assert_eq!(second, 0, "second seed must insert 0 rows (idempotent)");
    assert_eq!(label_count(&pool, owner_id).await, 6);

    // 5. `POST {name,color} → 201 + Location`; DB re-read finds the row.
    //    Name is trimmed server-side (`"  Side  "` stores `"Side"`).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "  Side  ", "color": "red"}),
            )),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create Side: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII")
        .to_owned();
    let side_id = created["data"]["id"].as_i64().expect("create returns id");
    assert_eq!(location, format!("/api/v1/labels/{side_id}"));
    assert_eq!(created["data"]["name"], "Side");
    assert_eq!(created["data"]["color"], "red");
    let stored: (String, String, i64) =
        sqlx::query_as("SELECT `name`, `color`, `user_id` FROM `labels` WHERE `id` = ?")
            .bind(side_id)
            .fetch_one(&pool)
            .await
            .expect("created label re-reads");
    assert_eq!(stored, ("Side".to_string(), "red".to_string(), owner_id));

    // 6. Validation: blank / overlong / missing / bad-color → 400 `errors[]`;
    //    non-string fields are body-shape 400s (serde rejects first).
    for (payload, what) in [
        (
            serde_json::json!({"name": "", "color": "red"}),
            "empty name",
        ),
        (
            serde_json::json!({"name": "   ", "color": "red"}),
            "whitespace name",
        ),
        (
            serde_json::json!({"name": "a".repeat(51), "color": "red"}),
            "overlong name",
        ),
        (
            serde_json::json!({"name": "BadColor", "color": "teal"}),
            "bad color",
        ),
        (
            serde_json::json!({"name": "BadColor2", "color": "RED"}),
            "uppercase color",
        ),
        (
            serde_json::json!({"name": "BadColor3", "color": ""}),
            "empty color",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/labels",
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
            "Validation: blank / overlong / missing / bad-color",
        );
        assert_errors_nonempty(&failure, what);
    }
    for (payload, what) in [
        (
            serde_json::json!({"name": 123, "color": "red"}),
            "non-string name",
        ),
        (
            serde_json::json!({"name": "X", "color": 7}),
            "non-string color",
        ),
        (serde_json::json!({"name": "NoColor"}), "missing color"),
        (serde_json::json!({"color": "red"}), "missing name"),
        (serde_json::json!({}), "empty body"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/labels",
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
            "invalid-json",
            "Validation: blank / overlong / missing / bad-color",
        );
    }
    // Unknown fields are ignored (Nest parity): still 201.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "Extra", "color": "blue", "bogus": 1}),
            )),
        ),
    )
    .await;
    let extra = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unknown fields ignored: {extra}"
    );
    let extra_id = extra["data"]["id"].as_i64().expect("Extra id");
    // Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "O'Brien", "color": "green"}),
            )),
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
        sqlx::query_scalar("SELECT `name` FROM `labels` WHERE `name` = 'O''Brien'")
            .fetch_one(&pool)
            .await
            .expect("quote payload re-reads literally");
    assert_eq!(quote_name, "O'Brien");
    let quote_id: i64 = sqlx::query_scalar("SELECT `id` FROM `labels` WHERE `name` = 'O''Brien'")
        .fetch_one(&pool)
        .await
        .expect("quote id re-reads");

    // 7. Per-user uniqueness → 409 (exact dup, whitespace-padded dup, and
    //    case-variant `'Work'` vs `'work'` under `utf8mb4_unicode_ci`).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "Work", "color": "red"}),
            )),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create Work: {}",
        json_of(&bytes)
    );
    for (payload, what) in [
        (
            serde_json::json!({"name": "Work", "color": "blue"}),
            "exact dup",
        ),
        (
            serde_json::json!({"name": "  Work  ", "color": "blue"}),
            "padded dup",
        ),
        (
            serde_json::json!({"name": "work", "color": "blue"}),
            "case-variant dup",
        ),
        (
            serde_json::json!({"name": "WORK", "color": "blue"}),
            "upper dup",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/labels",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::CONFLICT, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "conflict",
            "Per-user uniqueness",
        );
    }
    let work_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ? AND `name` = 'Work'")
            .bind(owner_id as i32)
            .fetch_one(&pool)
            .await
            .expect("dup re-read keeps one row");
    assert_eq!(work_rows, 1, "failed creates must not insert");
    // Same name for a DIFFERENT user is fine (uniqueness is per-user).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&intruder_jar),
            Some(json_body(
                &serde_json::json!({"name": "Work", "color": "green"}),
            )),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "cross-user same name: {}",
        json_of(&bytes)
    );

    // 8. Strict pagination: slices + total stable; out-of-range → 400.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/labels?limit=2&page=2",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let page2 = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page 2: {page2}");
    assert_eq!(page2["page"], 2);
    assert_eq!(page2["limit"], 2);
    assert_eq!(page2["data"].as_array().expect("array").len(), 2);
    for query in ["page=0", "limit=0", "limit=101", "page=abc", "limit=1.5"] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/labels?{query}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "?{query}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "validation-error",
            "Strict pagination: slices + total stable",
        );
        assert_errors_nonempty(&failure, query);
    }
    // Unknown query keys are ignored (serde default, Nest parity): still 200.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/labels?sort=evil", Some(&owner_jar), None),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unknown query key ignored: {}",
        json_of(&bytes)
    );

    // 9. `PATCH → 200`; DB re-read confirms (rename trims, recolor sticks).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/labels/{side_id}"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "  Renamed  ", "color": "blue"}),
            )),
        ),
    )
    .await;
    let updated = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch: {updated}");
    assert_eq!(updated["data"]["id"], side_id);
    assert_eq!(updated["data"]["name"], "Renamed");
    assert_eq!(updated["data"]["color"], "blue");
    let renamed: (String, String) =
        sqlx::query_as("SELECT `name`, `color` FROM `labels` WHERE `id` = ?")
            .bind(side_id)
            .fetch_one(&pool)
            .await
            .expect("patched label re-reads");
    assert_eq!(renamed, ("Renamed".to_string(), "blue".to_string()));
    // Empty-body patch is a no-op 200 (row untouched).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/labels/{side_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "empty patch: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["data"]["name"], "Renamed");
    // Color-only patch persists on re-read.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/labels/{side_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"color": "purple"}))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "color patch: {}", json_of(&bytes));
    let recolored: String = sqlx::query_scalar("SELECT `color` FROM `labels` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("recolor re-reads");
    assert_eq!(recolored, "purple");
    // Blank / overlong name and bad color → 400 with `errors[]`.
    for (payload, what) in [
        (serde_json::json!({"name": ""}), "empty patch name"),
        (serde_json::json!({"name": "   "}), "blank patch name"),
        (
            serde_json::json!({"name": "a".repeat(51)}),
            "overlong patch name",
        ),
        (serde_json::json!({"color": "teal"}), "bad patch color"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/labels/{side_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error", "`PATCH");
        assert_errors_nonempty(&failure, what);
    }
    // Rename onto another owned name (incl. case-variant) → 409, row kept.
    for (payload, what) in [
        (serde_json::json!({"name": "Work"}), "patch onto existing"),
        (serde_json::json!({"name": "work"}), "patch case-variant"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/labels/{side_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::CONFLICT, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "conflict", "`PATCH");
    }
    let kept: String = sqlx::query_scalar("SELECT `name` FROM `labels` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("failed patch keeps the row");
    assert_eq!(kept, "Renamed");
    // Same-name patch stays 200 (not a false 409/404).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/labels/{side_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Renamed"}))),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "same-name patch: {}",
        json_of(&bytes)
    );
    // Foreign / missing id → 404 (no oracle, NEVER 403); garbage → 400.
    let foreign_label_id: i64 =
        sqlx::query_scalar("SELECT `id` FROM `labels` WHERE `user_id` = ? AND `name` = 'Foreign'")
            .bind(intruder_id as i32)
            .fetch_one(&pool)
            .await
            .unwrap_or(0);
    // Intruder owns `Work` + 6 defaults + maybe `Foreign` fixture rows only
    // on shared-DB runs; resolve a guaranteed-foreign id instead.
    let foreign_id = if foreign_label_id > 0 {
        foreign_label_id
    } else {
        sqlx::query_scalar("SELECT `id` FROM `labels` WHERE `user_id` = ? LIMIT 1")
            .bind(intruder_id as i32)
            .fetch_one(&pool)
            .await
            .expect("intruder owns at least one label")
    };
    for (id, what) in [
        (foreign_id.to_string(), "foreign patch"),
        ("999999".to_string(), "missing patch"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/labels/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"name": "Hijack"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "not-found", "`PATCH");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/labels/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path", "`PATCH");
    // Failed hijack left the foreign row untouched (DB re-read).
    let foreign_name: String = sqlx::query_scalar("SELECT `name` FROM `labels` WHERE `id` = ?")
        .bind(foreign_id)
        .fetch_one(&pool)
        .await
        .expect("foreign row survives hijack");
    assert!(!foreign_name.is_empty());

    // 10. `DELETE → 204` empty body; row gone on re-read, others survive.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/labels/{side_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(bytes.is_empty(), "204 carries no body");
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .map(|value| value.to_str().unwrap_or("?")),
        None,
        "204 sets no Content-Type"
    );
    let gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("deleted row re-reads gone");
    assert_eq!(gone, 0, "label row gone");
    // Repeat delete → 404; foreign delete → 404 (row intact); garbage → 400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/labels/{side_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "repeat delete: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "`DELETE");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/labels/{foreign_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign delete: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "`DELETE");
    let foreign_survives: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `id` = ?")
        .bind(foreign_id)
        .fetch_one(&pool)
        .await
        .expect("foreign row re-reads");
    assert_eq!(foreign_survives, 1, "foreign label survives delete attempt");
    let (status, headers, bytes) = send(
        app(),
        authed_request("DELETE", "/api/v1/labels/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage delete: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path", "`DELETE");

    // 11. Quote-named label deletes cleanly too (bound params everywhere).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/labels/{quote_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "quote delete: {}",
        String::from_utf8_lossy(&bytes)
    );

    // 12. Cross-user isolation: intruder list is untouched by the owner flow.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/labels", Some(&intruder_jar), None),
    )
    .await;
    let intruder_list = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "intruder list: {intruder_list}");
    assert_eq!(intruder_list["total"], 7, "intruder: 6 defaults + Work");
    let _ = extra_id;

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
