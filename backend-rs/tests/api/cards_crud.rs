//! `POST /api/v1/cards` + `PATCH|GET|DELETE /api/v1/cards/:id` +
//! `GET /api/v1/columns/:col/cards` (T24a).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T23a): all scenarios run
//! sequentially in one test with distinct emails. Registers owner + intruder
//! via `POST /api/v1/auth/register` (real Argon2id hashes, live cookie jars)
//! and creates boards/columns via the API.
//!
//! NO session wipe here: `tower_sessions.session` is server-global. This
//! suite holds only its own jars, so concurrent suites are unaffected.
//!
//! Covered (red first: cards routes do not exist yet):
//! `POST {title,column_id,position?,description?,due_date?} → 201 {data} +
//! Location` (explicit `position` = insert-with-open-gap, default `MAX+1` via
//! `FOR UPDATE`); `PATCH` scalars-only `→ 200`; `DELETE → 204` (gap-close in
//! same tx, dense `0..N` on re-read); `GET /:id → 200 CardDetail`
//! (checklists{items}, labels); `GET /columns/:col/cards?page&limit → 200
//! Page<CardSummary>` (list NEVER selects TEXT: no `description` key +
//! `has_description` flag; `checklist_progress` only when checklists exist);
//! foreign column/card → `404` (never 403); overlong description / bad
//! due_date → `400 errors[]`; every 4xx asserts problem+json + exact `type`
//! URI; every mutation re-reads state via second GET or direct `SELECT`.

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
    let response = app.oneshot(request).await.expect("cards oneshot");
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

#[sqlx::test(migrations = "./migrations")]
async fn cards_crud_list_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every cards route (AuthSession guard).
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/cards",
            Some(json_body(
                &serde_json::json!({"title": "Nope", "column_id": 1}),
            )),
        ),
        (
            "PATCH",
            "/api/v1/cards/1",
            Some(json_body(&serde_json::json!({"title": "Nope"}))),
        ),
        ("GET", "/api/v1/cards/1", None),
        ("DELETE", "/api/v1/cards/1", None),
        ("GET", "/api/v1/columns/1/cards", None),
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

    // 1. Register owner + intruder; owner creates one board (3 default
    //    columns), intruder creates a foreign board.
    let (owner_jar, _owner_id) = register(app(), "cards-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "cards-intruder@example.com").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Card Board"}))),
        ),
    )
    .await;
    let created_board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create board: {created_board}");
    let board_columns = created_board["data"]["columns"]
        .as_array()
        .expect("board nests columns");
    assert_eq!(board_columns.len(), 3);
    let col_id = board_columns[0]["id"].as_i64().expect("column id");
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
    // Intruder creates one card in their column (for foreign-card 404s —
    // column ids and card ids share no sequence, so a column id is not a
    // valid foreign card probe).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({
                "title": "Intruder card",
                "column_id": foreign_col,
            }))),
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

    // 2. Create → 201 + Location; DB re-read + GET detail confirm.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "First card",
                "column_id": col_id,
                "description": "First card body",
                "due_date": "2026-06-01T00:00:00.000Z",
            }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create card: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let card_id = created["data"]["id"].as_i64().expect("card id");
    assert_eq!(created["data"]["title"], "First card");
    assert_eq!(created["data"]["column_id"], col_id);
    assert_eq!(created["data"]["position"], 0);
    assert_eq!(created["data"]["description"], "First card body");
    assert!(
        created["data"]["due_date"]
            .as_str()
            .is_some_and(|due| due.starts_with("2026-06-01")),
        "due_date round-trips: {created}"
    );
    assert_eq!(created["data"]["labels"], serde_json::json!([]));
    assert_eq!(created["data"]["checklists"], serde_json::json!([]));
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII");
    assert_eq!(location, format!("/api/v1/cards/{card_id}"));
    let stored: (String, i32) =
        sqlx::query_as("SELECT `title`, `position` FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("created card re-reads");
    assert_eq!(stored, ("First card".to_string(), 0));
    // Second GET proves persistence (not an echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "get detail: {detail}");
    assert_eq!(detail["data"]["id"], card_id);
    assert_eq!(detail["data"]["description"], "First card body");
    assert_eq!(detail["data"]["checklists"], serde_json::json!([]));
    assert!(
        detail["data"].get("checklist_progress").is_none(),
        "no progress key without checklists: {detail}"
    );

    // 3. Default position appends MAX+1; explicit position opens a gap.
    for title in ["Second card", "Third card"] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/cards",
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"title": title, "column_id": col_id}),
                )),
            ),
        )
        .await;
        let body = json_of(&bytes);
        assert_eq!(status, StatusCode::CREATED, "append {title}: {body}");
    }
    let positions: Vec<(String, i32)> = sqlx::query_as(
        "SELECT `title`, `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(col_id)
    .fetch_all(&pool)
    .await
    .expect("positions re-read");
    assert_eq!(
        positions,
        vec![
            ("First card".to_string(), 0),
            ("Second card".to_string(), 1),
            ("Third card".to_string(), 2),
        ]
    );
    // Explicit position=1 shifts 1..N up by one (never a verbatim duplicate).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Inserted card", "column_id": col_id, "position": 1,
            }))),
        ),
    )
    .await;
    let inserted = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "insert at 1: {inserted}");
    let inserted_id = inserted["data"]["id"].as_i64().expect("inserted id");
    assert_eq!(inserted["data"]["position"], 1);
    let positions: Vec<(String, i32)> = sqlx::query_as(
        "SELECT `title`, `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(col_id)
    .fetch_all(&pool)
    .await
    .expect("gap-open re-read");
    assert_eq!(
        positions,
        vec![
            ("First card".to_string(), 0),
            ("Inserted card".to_string(), 1),
            ("Second card".to_string(), 2),
            ("Third card".to_string(), 3),
        ],
        "explicit position opens a gap"
    );

    // 4. Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "O'Brien", "column_id": col_id,
            }))),
        ),
    )
    .await;
    let quoted = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "quote payload: {quoted}");
    let quote_id = quoted["data"]["id"].as_i64().expect("quote id");
    let stored_title: String = sqlx::query_scalar("SELECT `title` FROM `cards` WHERE `id` = ?")
        .bind(quote_id)
        .fetch_one(&pool)
        .await
        .expect("quote re-reads literally");
    assert_eq!(stored_title, "O'Brien");

    // 5. Validation: blank/overlong title, overlong description, bad due_date
    //    → 400 errors[]; wrong JSON type → 400 invalid-json.
    for (payload, what) in [
        (
            serde_json::json!({"title": "", "column_id": col_id}),
            "empty title",
        ),
        (
            serde_json::json!({"title": "   ", "column_id": col_id}),
            "whitespace title",
        ),
        (
            serde_json::json!({"title": "a".repeat(256), "column_id": col_id}),
            "overlong title",
        ),
        (
            serde_json::json!({"title": "Ok", "column_id": col_id, "description": "a".repeat(10_001)}),
            "overlong description",
        ),
        (
            serde_json::json!({"title": "Ok", "column_id": col_id, "due_date": "not-a-date"}),
            "bad due_date",
        ),
        (
            serde_json::json!({"title": "Ok", "column_id": col_id, "due_date": "2026-02-30"}),
            "impossible due_date",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/cards",
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
        (
            serde_json::json!({"title": 123, "column_id": col_id}),
            "numeric title",
        ),
        (
            serde_json::json!({"title": "Ok", "column_id": "abc"}),
            "string column_id",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/cards",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "invalid-json");
    }
    // column_id range violation (0) → 400 validation-error.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"title": "Ok", "column_id": 0}),
            )),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "column_id 0: {failure}");
    assert_problem(status, &headers, &failure, "validation-error");

    // 6. Foreign/missing column → 404 (never 403).
    for (id, what) in [
        (foreign_col.to_string(), "foreign column"),
        ("999999".to_string(), "missing column"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/cards",
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"title": "Hijack", "column_id": id.parse::<i64>().unwrap_or(-1)}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "create {what}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    for (method, path, body, what) in [
        (
            "GET",
            format!("/api/v1/cards/{foreign_card}"),
            None,
            "foreign card",
        ),
        (
            "GET",
            "/api/v1/cards/999999".to_string(),
            None,
            "missing card",
        ),
        (
            "GET",
            format!("/api/v1/columns/{foreign_col}/cards"),
            None,
            "foreign column list",
        ),
        (
            "GET",
            "/api/v1/columns/999999/cards".to_string(),
            None,
            "missing column list",
        ),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request(method, &path, Some(&owner_jar), body)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    // Garbage ids → 400 invalid-path.
    for path in ["/api/v1/cards/abc", "/api/v1/columns/abc/cards"] {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", path, Some(&owner_jar), None)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "garbage {path}: {failure}");
        assert_problem(status, &headers, &failure, "invalid-path");
    }

    // 7. Detail embeds labels + checklists{items} + progress only when the
    //    card owns checklists (seeded via SQL).
    sqlx::query("INSERT INTO `checklists` (`title`, `card_id`) VALUES ('Acceptance', ?)")
        .bind(card_id)
        .execute(&pool)
        .await
        .expect("seed checklist");
    let checklist: i64 = sqlx::query_scalar("SELECT `id` FROM `checklists` WHERE `card_id` = ?")
        .bind(card_id)
        .fetch_one(&pool)
        .await
        .expect("checklist id");
    sqlx::query(
        "INSERT INTO `checklist_items` (`text`, `is_completed`, `checklist_id`, `position`) VALUES \
         ('Write test', 0, ?, 0), ('Pass test', 1, ?, 1)",
    )
    .bind(checklist)
    .bind(checklist)
    .execute(&pool)
    .await
    .expect("seed items");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detail with nests: {detail}");
    let checklists = detail["data"]["checklists"]
        .as_array()
        .expect("checklists[]");
    assert_eq!(checklists.len(), 1);
    assert_eq!(checklists[0]["title"], "Acceptance");
    assert_eq!(checklists[0]["items"].as_array().expect("items[]").len(), 2);
    assert_eq!(
        detail["data"]["checklist_progress"],
        serde_json::json!({"completed": 1, "total": 2, "percent": 50}),
        "progress only when checklists exist: {detail}"
    );

    // 8. List → 200 Page<CardSummary>: no `description` key anywhere,
    //    `has_description` flags, progress only when checklists exist.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{col_id}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let page = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list cards: {page}");
    assert!(page.get("message").is_none(), "no message key: {page}");
    assert_eq!(page["page"], 1);
    assert_eq!(page["limit"], 50);
    let items = page["data"].as_array().expect("data[]");
    assert!(items.len() >= 5, "all seeded cards listed: {page}");
    assert_eq!(
        page["total"].as_u64().expect("total"),
        items.len() as u64,
        "total matches data length on a single page: {page}"
    );
    let positions: Vec<i64> = items
        .iter()
        .map(|item| item["position"].as_i64().expect("position"))
        .collect();
    let mut sorted = positions.clone();
    sorted.sort_unstable();
    assert_eq!(positions, sorted, "list ordered position ASC: {page}");
    for item in items {
        assert!(
            item.get("description").is_none(),
            "list never selects TEXT: {item}"
        );
        assert!(item["has_description"].is_boolean(), "flag present: {item}");
        assert!(item["labels"].is_array(), "labels[] present: {item}");
        assert!(item["due_date"].is_null() || item["due_date"].is_string());
    }
    let first = items
        .iter()
        .find(|item| item["id"] == card_id)
        .expect("first card listed");
    assert_eq!(first["has_description"], true);
    assert_eq!(
        first["checklist_progress"],
        serde_json::json!({"completed": 1, "total": 2, "percent": 50})
    );
    let plain = items
        .iter()
        .find(|item| item["id"] == inserted_id)
        .expect("inserted card listed");
    assert_eq!(plain["has_description"], false);
    assert!(
        plain.get("checklist_progress").is_none(),
        "no progress key without checklists: {plain}"
    );
    // Pagination slices + strict range errors.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{col_id}/cards?page=1&limit=2"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let sliced = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "paged list: {sliced}");
    assert_eq!(sliced["data"].as_array().expect("data[]").len(), 2);
    assert_eq!(sliced["page"], 1);
    assert_eq!(sliced["limit"], 2);
    for uri in [
        format!("/api/v1/columns/{col_id}/cards?page=0&limit=50"),
        format!("/api/v1/columns/{col_id}/cards?page=1&limit=101"),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", &uri, Some(&owner_jar), None)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "bad {uri}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error");
        assert_errors_nonempty(&failure, &uri);
    }

    // 9. PATCH scalars → 200; re-GET + SELECT confirm (no echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Renamed card",
                "description": "Updated body",
                "due_date": "2026-12-31",
            }))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch scalars: {patched}");
    assert_eq!(patched["data"]["id"], card_id);
    assert_eq!(patched["data"]["title"], "Renamed card");
    assert_eq!(patched["data"]["description"], "Updated body");
    assert!(
        patched["data"]["due_date"]
            .as_str()
            .is_some_and(|due| due.starts_with("2026-12-31")),
        "due_date persists: {patched}"
    );
    let stored: (String, Option<String>) =
        sqlx::query_as("SELECT `title`, `description` FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("patched card re-reads");
    assert_eq!(
        stored,
        ("Renamed card".to_string(), Some("Updated body".to_string()))
    );
    // Explicit null clears the description (null vs absent stays observable).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"description": null}))),
        ),
    )
    .await;
    let cleared = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "clear description: {cleared}");
    assert!(
        cleared["data"]["description"].is_null(),
        "null emitted: {cleared}"
    );
    let stored_desc: Option<String> =
        sqlx::query_scalar("SELECT `description` FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("cleared description re-reads");
    assert_eq!(stored_desc, None);
    // Empty {} PATCH is a no-op 200.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{card_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let noop = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty patch: {noop}");
    assert_eq!(noop["data"]["title"], "Renamed card");
    // Scalar validation → 400 errors[].
    for (payload, what) in [
        (serde_json::json!({"title": "  "}), "blank title"),
        (
            serde_json::json!({"description": "a".repeat(10_001)}),
            "overlong description",
        ),
        (
            serde_json::json!({"due_date": "yesterday-ish"}),
            "bad due_date",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/cards/{card_id}"),
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
    // PATCH foreign/missing → 404 (never 403); garbage id → 400.
    for id in [foreign_card.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/cards/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"title": "Hijack"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "patch {id}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/cards/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");

    // 10. Deprecated `/api/*` alias serves the same create handler.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/cards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"title": "Alias card", "column_id": col_id}),
            )),
        ),
    )
    .await;
    let aliased = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "alias create: {aliased}");
    assert_eq!(aliased["data"]["title"], "Alias card");

    // 11. DELETE a middle card → 204 empty body; positions close to dense
    //     0..N on re-read; re-GET 404s.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{inserted_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete card: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries an empty body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    let positions: Vec<i32> = sqlx::query_scalar(
        "SELECT `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(col_id)
    .fetch_all(&pool)
    .await
    .expect("positions re-read");
    let dense: Vec<i32> = (0..positions.len() as i32).collect();
    assert_eq!(positions, dense, "gap closed, dense 0..N");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{col_id}/cards?page=1&limit=50"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let relisted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "re-list after delete: {relisted}");
    let listed_positions: Vec<i64> = relisted["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|item| item["position"].as_i64().expect("position"))
        .collect();
    let expected: Vec<i64> = (0..listed_positions.len() as i64).collect();
    assert_eq!(listed_positions, expected, "wire order dense: {relisted}");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/cards/{inserted_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "re-get deleted: {failure}");
    assert_problem(status, &headers, &failure, "not-found");

    // 12. Deleting twice → 404; foreign/missing → 404 (never 403); garbage →
    //     400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{inserted_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "delete twice: {failure}");
    assert_problem(status, &headers, &failure, "not-found");
    for id in [foreign_card.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "DELETE",
                &format!("/api/v1/cards/{id}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "delete {id}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request("DELETE", "/api/v1/cards/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
