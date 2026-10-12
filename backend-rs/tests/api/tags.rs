//! `GET/POST /api/v1/tags` + `PATCH/DELETE /api/v1/tags/{id}` (T27b).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T25 labels): all scenarios
//! run sequentially in one test with distinct emails. This suite registers its
//! owner + intruder via `POST /api/v1/auth/register` (real Argon2id hashes,
//! live cookie jars). Tags have NO lazy seed (unlike labels): a fresh user
//! lists `total == 0`.
//!
//! NO session wipe here: `tower_sessions.session` is server-global. This
//! suite holds only its own jars, so concurrent suites are no-ops for it.
//!
//! Covered (red first: the tags router is an empty stub):
//! list `200 Page` (`data,page,limit,total`, `id ASC`);
//! `POST {name(1..50,trim,per-user unique→409),color?(free-form ≤20, default
//! teal)} → 201 + Location` (blank/overlong/missing/overlong-color → 400
//! `errors[]`; unknown fields ignored; quote payload stored literally;
//! case-variant `'Work'` vs `'work'` → 409 under `utf8mb4_unicode_ci`);
//! `PATCH → 200` (trim + dup → 409 incl. case-variant; blank/overlong names
//! and overlong colors → 400; empty-body patch is a no-op 200; `GET`-style bad
//! id `abc` → 400 `invalid-path`);
//! `DELETE → 204` empty body (repeat/foreign → 404, garbage → 400);
//! cross-user id → 404 (no oracle, NEVER 403); strict pagination
//! (`page=0`/`limit=0`/`limit=101`/non-integer → 400 `errors[]`);
//! notes link: `tag_ids` dedup persists a single `note_tags` row; foreign or
//! missing tag on note create → 404 (no oracle, never 403);
//! every 4xx asserts `Content-Type: application/problem+json` + exact `type`
//! URI; every mutation re-reads state via second GET or direct `SELECT`.
//!
//! Contract: `specs/api-rewrite-rs-specs/20-api-contract.md` tags section
//! (`GET → Page<Tag>`; `POST {name(per-user unique→409, trim, not-empty),
//! color?(default teal, free-form ≤20)}→201+Location`; `PATCH →200` with 409 +
//! trim + not-empty; `DELETE→204`; same case-collision note as labels
//! (`UQ_tags_user_name`)) + `21-api-standard-shapes.md` (same rows; both
//! `DELETE` → `204`).
//! Schema: `30-db-schema.md` tags row (`name varchar50`, `color varchar20
//! default 'teal'`, `created_at` ONLY — no `updated_at` BY DESIGN) +
//! `UQ_tags_user_name UNIQUE(user_id,name)` + `note_tags(note_id,tag_id)`
//! composite PK with `ON DELETE CASCADE` both sides.

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Request, StatusCode},
    Router,
};
use backend_rs::{config::Settings, create_app, state::AppState};
use serde_json::Value;
use sqlx::MySqlPool;
use tower::ServiceExt;
use tower_sessions_sqlx_store::MySqlStore;

const PROBLEM_JSON: &str = "application/problem+json";

/// Build the app on the `#[sqlx::test]` pool (in-process, no sweeper —
/// `create_app` never spawns one).
fn app_on_pool(pool: &MySqlPool) -> Router {
    let settings = Settings {
        ..Default::default()
    };
    let mut state = AppState::from_settings(settings);
    state.pool = pool.clone();
    state.sessions = MySqlStore::new(pool.clone());
    create_app(state)
}

fn json_body(value: &Value) -> String {
    value.to_string()
}

fn authed_request(
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<String>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(jar) = cookie {
        builder = builder.header(header::COOKIE, jar);
    }
    if let Some(payload) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        builder.body(Body::from(payload)).expect("request builds")
    } else {
        builder.body(Body::empty()).expect("request builds")
    }
}

async fn send(app: Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app.oneshot(request).await.expect("tags oneshot");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body reads");
    (status, headers, bytes.to_vec())
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("response is JSON")
}

/// First `name=value` pair of every `Set-Cookie`, joined as a `Cookie` header.
fn cookie_jar(headers: &HeaderMap) -> String {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(|cookie| cookie.split(';').next().unwrap_or("").trim())
        .filter(|pair| !pair.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn assert_problem(status: StatusCode, headers: &HeaderMap, body: &Value, slug: &str) {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .expect("error must set Content-Type")
        .to_str()
        .expect("Content-Type is ASCII");
    assert_eq!(content_type, PROBLEM_JSON, "status: {status}");
    assert_eq!(
        body["type"],
        format!("https://kanpad.dev/problems/{slug}"),
        "status: {status}"
    );
    assert_eq!(body["status"], status.as_u16());
}

fn assert_errors_nonempty(body: &Value, what: &str) {
    assert!(
        body["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()),
        "{what} must carry errors[]: {body}"
    );
}

async fn register(app: Router, email: &str) -> (String, i64) {
    let payload = serde_json::json!({"email": email, "password": "Str0ng!pass"});
    let (status, headers, bytes) = send(
        app,
        authed_request(
            "POST",
            "/api/v1/auth/register",
            None,
            Some(json_body(&payload)),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "register {email}: {body}");
    let jar = cookie_jar(&headers);
    assert!(!jar.is_empty(), "register sets a session cookie");
    let id = body["data"]["id"].as_i64().expect("register returns id");
    (jar, id)
}

async fn tag_count(pool: &MySqlPool, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM `tags` WHERE `user_id` = ?")
        .bind(user_id as i32)
        .fetch_one(pool)
        .await
        .expect("tag count re-reads")
}

#[sqlx::test(migrations = "./migrations")]
async fn tags_crud_full_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every verb (AuthSession guard).
    for (method, path, body) in [
        ("GET", "/api/v1/tags", None),
        (
            "POST",
            "/api/v1/tags",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        (
            "PATCH",
            "/api/v1/tags/1",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        ("DELETE", "/api/v1/tags/1", None),
    ] {
        let (status, headers, bytes) = send(app(), authed_request(method, path, None, body)).await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}: {failure}"
        );
        assert_problem(status, &headers, &failure, "unauthorized");
    }

    // 1. Register owner + intruder: a fresh user owns zero tags (no lazy
    //    seed — unlike labels).
    let (owner_jar, owner_id) = register(app(), "tags-owner@example.com").await;
    let (intruder_jar, intruder_id) = register(app(), "tags-intruder@example.com").await;
    assert_ne!(owner_id, intruder_id);
    assert_eq!(tag_count(&pool, owner_id).await, 0);
    assert_eq!(tag_count(&pool, intruder_id).await, 0);

    // 2. List is a real `Page` (never a bare array), empty for a fresh user.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/tags", Some(&owner_jar), None),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list: {listed}");
    assert_eq!(listed["page"], 1);
    assert_eq!(listed["limit"], 50);
    assert_eq!(listed["total"], 0);
    assert_eq!(listed["data"].as_array().expect("data is array").len(), 0);
    assert!(listed.get("message").is_none(), "no message key: {listed}");
    // Deprecated `/api/*` alias serves the same handler.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/tags", Some(&owner_jar), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alias list: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["total"], 0);

    // 3. `POST {name,color} → 201 + Location`; DB re-read finds the row.
    //    Name is trimmed server-side (`"  Side  "` stores `"Side"`).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/tags",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "  Side  ", "color": "blue"}),
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
    assert_eq!(location, format!("/api/v1/tags/{side_id}"));
    assert_eq!(created["data"]["name"], "Side");
    assert_eq!(created["data"]["color"], "blue");
    assert!(
        created["data"]["created_at"].is_string(),
        "created_at: {created}"
    );
    assert!(
        created["data"].get("updated_at").is_none(),
        "tags carry no updated_at by design: {created}"
    );
    let stored: (String, String, i64) =
        sqlx::query_as("SELECT `name`, `color`, `user_id` FROM `tags` WHERE `id` = ?")
            .bind(side_id)
            .fetch_one(&pool)
            .await
            .expect("created tag re-reads");
    assert_eq!(stored, ("Side".to_string(), "blue".to_string(), owner_id));

    // 4. Omitted color defaults to teal (server-side, persisted).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/tags",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Plain"}))),
        ),
    )
    .await;
    let plain = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "default color: {plain}");
    let plain_id = plain["data"]["id"].as_i64().expect("Plain id");
    assert_eq!(plain["data"]["color"], "teal");
    let plain_color: String = sqlx::query_scalar("SELECT `color` FROM `tags` WHERE `id` = ?")
        .bind(plain_id)
        .fetch_one(&pool)
        .await
        .expect("default color re-reads");
    assert_eq!(plain_color, "teal");

    // 5. Validation: blank / overlong name / missing name / overlong color →
    //    400 `errors[]`; non-string fields are body-shape 400s.
    for (payload, what) in [
        (serde_json::json!({"name": ""}), "empty name"),
        (serde_json::json!({"name": "   "}), "whitespace name"),
        (serde_json::json!({"name": "a".repeat(51)}), "overlong name"),
        (
            serde_json::json!({"name": "Long", "color": "x".repeat(21)}),
            "overlong color",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/tags",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error");
        assert_errors_nonempty(&failure, what);
    }
    for (payload, what) in [
        (serde_json::json!({"name": 123}), "non-string name"),
        (
            serde_json::json!({"name": "X", "color": 7}),
            "non-string color",
        ),
        (serde_json::json!({}), "empty body"),
        (serde_json::json!({"color": "blue"}), "missing name"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/tags",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "invalid-json");
    }
    // Color is free-form (NOT the label enum): teal/hex/words all 201.
    for color in ["chartreuse", "#abc123", "RED"] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/tags",
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"name": format!("Free-{color}"), "color": color}),
                )),
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "free-form color {color}: {}",
            json_of(&bytes)
        );
        assert_eq!(json_of(&bytes)["data"]["color"], color);
    }
    // Unknown fields are ignored (Nest parity): still 201.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/tags",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Extra", "bogus": 1}))),
        ),
    )
    .await;
    let extra = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unknown fields ignored: {extra}"
    );
    assert_eq!(extra["data"]["color"], "teal");
    // Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/tags",
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
        sqlx::query_scalar("SELECT `name` FROM `tags` WHERE `name` = 'O''Brien'")
            .fetch_one(&pool)
            .await
            .expect("quote payload re-reads literally");
    assert_eq!(quote_name, "O'Brien");
    let quote_id: i64 = sqlx::query_scalar("SELECT `id` FROM `tags` WHERE `name` = 'O''Brien'")
        .fetch_one(&pool)
        .await
        .expect("quote id re-reads");

    // 6. Per-user uniqueness → 409 (exact dup, whitespace-padded dup, and
    //    case-variant `'Work'` vs `'work'` under `utf8mb4_unicode_ci`).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/tags",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Work"}))),
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
        (serde_json::json!({"name": "Work"}), "exact dup"),
        (serde_json::json!({"name": "  Work  "}), "padded dup"),
        (serde_json::json!({"name": "work"}), "case-variant dup"),
        (serde_json::json!({"name": "WORK"}), "upper dup"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/tags",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::CONFLICT, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "conflict");
    }
    let work_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `tags` WHERE `user_id` = ? AND `name` = 'Work'")
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
            "/api/v1/tags",
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({"name": "Work"}))),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "cross-user same name: {}",
        json_of(&bytes)
    );
    let intruder_work_id = json_of(&bytes)["data"]["id"]
        .as_i64()
        .expect("intruder Work id");

    // 7. Strict pagination: slices + total stable; out-of-range → 400.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/tags?limit=2&page=2", Some(&owner_jar), None),
    )
    .await;
    let page2 = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page 2: {page2}");
    assert_eq!(page2["page"], 2);
    assert_eq!(page2["limit"], 2);
    assert_eq!(page2["data"].as_array().expect("array").len(), 2);
    let owner_total = tag_count(&pool, owner_id).await;
    assert_eq!(page2["total"], owner_total);
    for query in ["page=0", "limit=0", "limit=101", "page=abc", "limit=1.5"] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/tags?{query}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "?{query}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error");
        assert_errors_nonempty(&failure, query);
    }
    // Unknown query keys are ignored (serde default, Nest parity): still 200.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/tags?sort=evil", Some(&owner_jar), None),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unknown query key ignored: {}",
        json_of(&bytes)
    );

    // 8. `PATCH → 200`; DB re-read confirms (rename trims, recolor sticks).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/tags/{side_id}"),
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"name": "  Renamed  ", "color": "mauve"}),
            )),
        ),
    )
    .await;
    let updated = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch: {updated}");
    assert_eq!(updated["data"]["id"], side_id);
    assert_eq!(updated["data"]["name"], "Renamed");
    assert_eq!(updated["data"]["color"], "mauve");
    let renamed: (String, String) =
        sqlx::query_as("SELECT `name`, `color` FROM `tags` WHERE `id` = ?")
            .bind(side_id)
            .fetch_one(&pool)
            .await
            .expect("patched tag re-reads");
    assert_eq!(renamed, ("Renamed".to_string(), "mauve".to_string()));
    // Empty-body patch is a no-op 200 (row untouched).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/tags/{side_id}"),
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
            &format!("/api/v1/tags/{side_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"color": "puce"}))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "color patch: {}", json_of(&bytes));
    let recolored: String = sqlx::query_scalar("SELECT `color` FROM `tags` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("recolor re-reads");
    assert_eq!(recolored, "puce");
    // Blank / overlong name and overlong color → 400 with `errors[]`.
    for (payload, what) in [
        (serde_json::json!({"name": ""}), "empty patch name"),
        (serde_json::json!({"name": "   "}), "blank patch name"),
        (
            serde_json::json!({"name": "a".repeat(51)}),
            "overlong patch name",
        ),
        (
            serde_json::json!({"color": "x".repeat(21)}),
            "overlong patch color",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/tags/{side_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error");
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
                &format!("/api/v1/tags/{side_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::CONFLICT, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "conflict");
    }
    let kept: String = sqlx::query_scalar("SELECT `name` FROM `tags` WHERE `id` = ?")
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
            &format!("/api/v1/tags/{side_id}"),
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
    for (id, what) in [
        (intruder_work_id.to_string(), "foreign patch"),
        ("999999".to_string(), "missing patch"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/tags/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"name": "Hijack"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/tags/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");
    // Failed hijack left the foreign row untouched (DB re-read).
    let foreign_name: String = sqlx::query_scalar("SELECT `name` FROM `tags` WHERE `id` = ?")
        .bind(intruder_work_id)
        .fetch_one(&pool)
        .await
        .expect("foreign row survives hijack");
    assert_eq!(foreign_name, "Work");

    // 9. Notes link: `tag_ids` dedup persists a single `note_tags` row.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Tagged",
                "content": "hello",
                "tag_ids": [side_id, side_id, plain_id],
            }))),
        ),
    )
    .await;
    let note = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "note with tags: {note}");
    let note_id = note["data"]["id"].as_i64().expect("note id");
    assert_eq!(
        note["data"]["tags"].as_array().expect("tags array").len(),
        2,
        "deduped tags: {note}"
    );
    let link_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `note_tags` WHERE `note_id` = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .expect("note_tags re-read keeps deduped rows");
    assert_eq!(link_rows, 2, "duplicate tag id persists a single row");
    // Second GET re-reads the same link set (persistence proof).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/notes/{note_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let reread = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "note re-read: {reread}");
    assert_eq!(
        reread["data"]["tags"].as_array().expect("tags array").len(),
        2
    );

    // 10. Foreign or missing tag on note create → 404 (never 403).
    for (tag_ids, what) in [
        (serde_json::json!([intruder_work_id]), "foreign tag"),
        (serde_json::json!([999999]), "missing tag"),
        (
            serde_json::json!([side_id, intruder_work_id]),
            "mixed own + foreign",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/notes",
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"title": "Bad link", "content": "x", "tag_ids": tag_ids}),
                )),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }

    // 11. `DELETE → 204` empty body; row gone on re-read, others survive.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/tags/{side_id}"),
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
    let gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `tags` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("deleted row re-reads gone");
    assert_eq!(gone, 0, "tag row gone");
    // The deleted tag's note links fan out via `ON DELETE CASCADE`.
    let orphan_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `note_tags` WHERE `tag_id` = ?")
            .bind(side_id)
            .fetch_one(&pool)
            .await
            .expect("cascade re-reads");
    assert_eq!(orphan_links, 0, "note_tags cascade on tag delete");
    // Repeat delete → 404; foreign delete → 404 (row intact); garbage → 400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/tags/{side_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "repeat delete: {failure}");
    assert_problem(status, &headers, &failure, "not-found");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/tags/{intruder_work_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign delete: {failure}");
    assert_problem(status, &headers, &failure, "not-found");
    let foreign_survives: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `tags` WHERE `id` = ?")
        .bind(intruder_work_id)
        .fetch_one(&pool)
        .await
        .expect("foreign row re-reads");
    assert_eq!(foreign_survives, 1, "foreign tag survives delete attempt");
    let (status, headers, bytes) = send(
        app(),
        authed_request("DELETE", "/api/v1/tags/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage delete: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");

    // 12. Quote-named tag deletes cleanly too (bound params everywhere).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/tags/{quote_id}"),
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
    let _ = plain_id;

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
