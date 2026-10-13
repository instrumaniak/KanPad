//! Black-box e2e (rest slice): labels, checklists + checklist items, notes,
//! tags, and the card-labels M2M — driven over real HTTP against
//! `127.0.0.1:3001` (T50c).
//!
//! Unlike the in-process suites (which drive the `Router` via `oneshot`),
//! this suite binds the real app with `axum::serve` on `:3001` and speaks
//! HTTP/1.1 over TCP, so status lines, headers (`Location`, `Set-Cookie`,
//! `served-by`, `Content-Type`), and cookie jars are asserted on the wire.
//! It mirrors the Nest slices `backend/test/labels.e2e-spec.ts` (absent —
//! labels have no Nest e2e file; covered here from the Rust contract) and
//! `backend/test/checklists.e2e-spec.ts` (CRUD + items + authorization +
//! validation, with Rust-contract statuses: `201+Location` creates, `204`
//! deletes, `404` for foreign ids instead of Nest's `403`, `400+errors[]`
//! validation instead of Nest's bare `400`), plus the notes/tags/card-labels
//! surface from the remaining Nest specs.
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T25/T26a): one flow,
//! distinct users, sequential steps. Request budget stays well under the
//! T30 governor defaults (120 global/min; exactly 2 auth-register hits
//! against the 5/min auth bucket). NO session-store wipe here:
//! `tower_sessions.session` is server-global — this suite holds only its
//! own jars and asserts no absolute store counts.
//!
//! Contract: `specs/api-rewrite-rs-specs/20-api-contract.md` (§ labels,
//! § checklists, § notes, § tags, § cards label-M2M) +
//! `21-api-standard-shapes.md` (same rows). Every mutation is re-read via a
//! second GET or a direct sqlx `SELECT`.
//!
//! Touch ownership: T50c owns this file; the `api.rs` mod-registration line
//! is shared with T50a/T50b (expected union conflict on merge).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use backend_rs::{config::Settings, create_app, state::AppState};
use serde_json::Value;
use sqlx::MySqlPool;
use tower_sessions_sqlx_store::MySqlStore;

const BASE: &str = "127.0.0.1:3001";
const PROBLEM_JSON: &str = "application/problem+json";

/// Build the app on the `#[sqlx::test]` pool (in-process state, real TCP
/// serve — `create_app` never spawns the session sweeper).
fn app_on_pool(pool: &MySqlPool) -> axum::Router {
    let settings = Settings {
        ..Default::default()
    };
    let mut state = AppState::from_settings(settings);
    state.pool = pool.clone();
    state.sessions = MySqlStore::new(pool.clone());
    create_app(state)
}

// ---------------------------------------------------------------------------
// Minimal blocking HTTP/1.1 client (no new deps in T50c scope).
// ---------------------------------------------------------------------------

struct WireResponse {
    status: u16,
    headers: HashMap<String, Vec<String>>,
    body: Vec<u8>,
}

fn blocking_roundtrip(
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<&str>,
) -> WireResponse {
    let mut stream = TcpStream::connect(BASE).expect("connect :3001");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout sets");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("write timeout sets");
    let payload = body.unwrap_or("");
    let mut request =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:3001\r\nConnection: close\r\n");
    if let Some(jar) = cookie {
        request.push_str(&format!("Cookie: {jar}\r\n"));
    }
    if body.is_some() {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    request.push_str("\r\n");
    request.push_str(payload);
    stream
        .write_all(request.as_bytes())
        .expect("request writes");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("response reads");
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> WireResponse {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response has a header terminator");
    let (head, rest) = raw.split_at(sep);
    let body_raw = &rest[4..];
    let head = String::from_utf8_lossy(head);
    let mut lines = head.lines();
    let status_line = lines.next().expect("response has a status line");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("status line has a code")
        .parse()
        .expect("status code parses");
    let mut headers: HashMap<String, Vec<String>> = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers
                .entry(name.trim().to_ascii_lowercase())
                .or_default()
                .push(value.trim().to_string());
        }
    }
    let chunked = headers.get("transfer-encoding").is_some_and(|values| {
        values
            .iter()
            .any(|v| v.to_ascii_lowercase().contains("chunked"))
    });
    let body = if chunked {
        dechunk(body_raw)
    } else {
        body_raw.to_vec()
    };
    WireResponse {
        status,
        headers,
        body,
    }
}

fn dechunk(mut input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let eol = input
            .windows(2)
            .position(|w| w == b"\r\n")
            .expect("chunk has a size line");
        let size_hex = std::str::from_utf8(&input[..eol]).expect("chunk size is ASCII");
        let size = usize::from_str_radix(size_hex.trim(), 16).expect("chunk size parses");
        input = &input[eol + 2..];
        if size == 0 {
            break;
        }
        out.extend_from_slice(&input[..size]);
        input = &input[size + 2..];
    }
    out
}

async fn http(method: &str, path: &str, cookie: Option<&str>, body: Option<Value>) -> WireResponse {
    let method = method.to_string();
    let path = path.to_string();
    let cookie = cookie.map(str::to_string);
    let body_text = body.map(|value| value.to_string());
    tokio::task::spawn_blocking(move || {
        blocking_roundtrip(&method, &path, cookie.as_deref(), body_text.as_deref())
    })
    .await
    .expect("blocking HTTP join")
}

fn json_of(body: &[u8]) -> Value {
    assert!(!body.is_empty(), "expected a JSON body, got empty");
    serde_json::from_slice(body).expect("response is JSON")
}

fn header_first(headers: &HashMap<String, Vec<String>>, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.first()).cloned()
}

/// First `name=value` pair of every `Set-Cookie`, joined as a `Cookie` header.
fn cookie_jar(headers: &HashMap<String, Vec<String>>) -> String {
    headers
        .get("set-cookie")
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|cookie| cookie.split(';').next().unwrap_or("").trim())
        .filter(|pair| !pair.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn assert_problem(response: &WireResponse, body: &Value, slug: &str, what: &str) {
    let content_type =
        header_first(&response.headers, "content-type").expect("error must set Content-Type");
    assert_eq!(
        content_type, PROBLEM_JSON,
        "{what}: status {}",
        response.status
    );
    assert_eq!(
        body["type"],
        format!("https://kanpad.dev/problems/{slug}"),
        "{what}: status {}",
        response.status
    );
    assert_eq!(body["status"], response.status, "{what}");
}

fn assert_errors_nonempty(body: &Value, what: &str) {
    assert!(
        body["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()),
        "{what} must carry errors[]: {body}"
    );
}

async fn register(jar_holder: &mut String, email: &str) -> i64 {
    let response = http(
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({"email": email, "password": "Str0ng!pass"})),
    )
    .await;
    let body = json_of(&response.body);
    assert_eq!(response.status, 201, "register {email}: {body}");
    let jar = cookie_jar(&response.headers);
    assert!(!jar.is_empty(), "register sets a session cookie");
    *jar_holder = jar;
    body["data"]["id"].as_i64().expect("register returns id")
}

async fn label_count(pool: &MySqlPool, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
        .bind(user_id as i32)
        .fetch_one(pool)
        .await
        .expect("label count re-reads")
}

#[sqlx::test(migrations = "./migrations")]
async fn e2e_rest_full_flow(pool: MySqlPool) {
    // 0. Serve the real app on :3001 (black-box: real TCP + HTTP wire).
    let listener = tokio::net::TcpListener::bind(BASE)
        .await
        .expect("bind :3001 for black-box e2e");
    let server_pool = pool.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app_on_pool(&server_pool))
            .await
            .expect("e2e server serves");
    });

    // Health poll doubles as the black-box liveness proof (`served-by: rust`
    // on the wire, never throttled).
    let mut health = http("GET", "/health", None, None).await;
    for _ in 0..50 {
        if health.status == 200 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        health = http("GET", "/health", None, None).await;
    }
    assert_eq!(health.status, 200, "GET /health is 200 on :3001");
    assert_eq!(
        json_of(&health.body),
        serde_json::json!({"status": "ok"}),
        "health body"
    );
    assert_eq!(
        header_first(&health.headers, "served-by").as_deref(),
        Some("rust"),
        "served-by header on the wire"
    );

    // 1. Register owner + intruder (starts each at the 6 seeded defaults).
    let mut owner_jar = String::new();
    let mut intruder_jar = String::new();
    let owner_id = register(&mut owner_jar, "e2e-rest-owner@example.com").await;
    let intruder_id = register(&mut intruder_jar, "e2e-rest-intruder@example.com").await;
    assert_eq!(label_count(&pool, owner_id).await, 6);
    assert_eq!(label_count(&pool, intruder_id).await, 6);

    // Setup: owner board + card (columns come free with the board);
    // intruder board + card for foreign-id 404 probes.
    let response = http(
        "POST",
        "/api/v1/boards",
        Some(&owner_jar),
        Some(serde_json::json!({"name": "Rest Board"})),
    )
    .await;
    let created_board = json_of(&response.body);
    assert_eq!(response.status, 201, "create board: {created_board}");
    let col_id = created_board["data"]["columns"][0]["id"]
        .as_i64()
        .expect("column id");
    let board_id = created_board["data"]["id"].as_i64().expect("board id");
    let response = http(
        "POST",
        "/api/v1/cards",
        Some(&owner_jar),
        Some(serde_json::json!({"title": "Rest card", "column_id": col_id})),
    )
    .await;
    let created_card = json_of(&response.body);
    assert_eq!(response.status, 201, "create card: {created_card}");
    let card_id = created_card["data"]["id"].as_i64().expect("card id");
    let response = http(
        "POST",
        "/api/v1/boards",
        Some(&intruder_jar),
        Some(serde_json::json!({"name": "Theirs"})),
    )
    .await;
    let foreign_board = json_of(&response.body);
    assert_eq!(response.status, 201, "foreign board: {foreign_board}");
    let foreign_col = foreign_board["data"]["columns"][0]["id"]
        .as_i64()
        .expect("foreign column id");
    let response = http(
        "POST",
        "/api/v1/cards",
        Some(&intruder_jar),
        Some(serde_json::json!({"title": "Intruder card", "column_id": foreign_col})),
    )
    .await;
    let intruder_card_body = json_of(&response.body);
    assert_eq!(response.status, 201, "intruder card: {intruder_card_body}");
    let foreign_card = intruder_card_body["data"]["id"]
        .as_i64()
        .expect("foreign card id");
    // Intruder's first default label is the guaranteed-foreign label id.
    let response = http("GET", "/api/v1/labels", Some(&intruder_jar), None).await;
    let intruder_labels = json_of(&response.body);
    assert_eq!(response.status, 200, "intruder labels: {intruder_labels}");
    let foreign_label = intruder_labels["data"][0]["id"]
        .as_i64()
        .expect("foreign label id");

    // ---- Labels ---------------------------------------------------------
    // 2. Create → 201 + Location; name trimmed; DB re-read finds the row.
    let response = http(
        "POST",
        "/api/v1/labels",
        Some(&owner_jar),
        Some(serde_json::json!({"name": "  Side  ", "color": "red"})),
    )
    .await;
    let created = json_of(&response.body);
    assert_eq!(response.status, 201, "create label: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let side_id = created["data"]["id"].as_i64().expect("label id");
    assert_eq!(
        header_first(&response.headers, "location").as_deref(),
        Some(format!("/api/v1/labels/{side_id}").as_str()),
        "201 sets Location"
    );
    assert_eq!(created["data"]["name"], "Side");
    let stored: (String, String) =
        sqlx::query_as("SELECT `name`, `color` FROM `labels` WHERE `id` = ?")
            .bind(side_id)
            .fetch_one(&pool)
            .await
            .expect("created label re-reads");
    assert_eq!(stored, ("Side".to_string(), "red".to_string()));
    // Second GET proves persistence (not an echo).
    let response = http("GET", "/api/v1/labels", Some(&owner_jar), None).await;
    let listed = json_of(&response.body);
    assert_eq!(response.status, 200, "list after create: {listed}");
    assert_eq!(listed["total"], 7, "6 defaults + Side");
    assert_eq!(
        header_first(&response.headers, "served-by").as_deref(),
        Some("rust"),
        "served-by on API response"
    );
    // Deprecated `/api/*` alias serves the same handler.
    let response = http("GET", "/api/labels", Some(&owner_jar), None).await;
    assert_eq!(
        response.status,
        200,
        "alias list: {}",
        json_of(&response.body)
    );

    // 3. Per-user uniqueness → 409 incl. case-variant (utf8mb4_unicode_ci);
    //    bad color → 400 errors[]; failed creates insert nothing.
    for (payload, what) in [
        (
            serde_json::json!({"name": "Side", "color": "blue"}),
            "exact dup",
        ),
        (
            serde_json::json!({"name": "side", "color": "blue"}),
            "case-variant dup",
        ),
    ] {
        let response = http("POST", "/api/v1/labels", Some(&owner_jar), Some(payload)).await;
        let failure = json_of(&response.body);
        assert_eq!(response.status, 409, "{what}: {failure}");
        assert_problem(&response, &failure, "conflict", what);
    }
    let response = http(
        "POST",
        "/api/v1/labels",
        Some(&owner_jar),
        Some(serde_json::json!({"name": "Bad", "color": "teal"})),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 400, "bad color: {failure}");
    assert_problem(&response, &failure, "validation-error", "bad color");
    assert_errors_nonempty(&failure, "bad color");
    assert_eq!(label_count(&pool, owner_id).await, 7);

    // 4. PATCH rename → 200; re-GET + SELECT confirm. Patch onto an owned
    //    name → 409; foreign id → 404 (never 403).
    let response = http(
        "PATCH",
        &format!("/api/v1/labels/{side_id}"),
        Some(&owner_jar),
        Some(serde_json::json!({"name": "  Renamed  "})),
    )
    .await;
    let updated = json_of(&response.body);
    assert_eq!(response.status, 200, "patch label: {updated}");
    assert_eq!(updated["data"]["name"], "Renamed");
    let renamed: String = sqlx::query_scalar("SELECT `name` FROM `labels` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("patched label re-reads");
    assert_eq!(renamed, "Renamed");
    let response = http(
        "PATCH",
        &format!("/api/v1/labels/{side_id}"),
        Some(&owner_jar),
        Some(serde_json::json!({"name": "Urgent"})),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 409, "patch onto owned name: {failure}");
    assert_problem(&response, &failure, "conflict", "patch dup");
    let response = http(
        "PATCH",
        &format!("/api/v1/labels/{foreign_label}"),
        Some(&owner_jar),
        Some(serde_json::json!({"name": "Hijack"})),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "foreign patch: {failure}");
    assert_problem(&response, &failure, "not-found", "foreign patch");

    // 5. DELETE → 204 empty body; row gone on re-read; repeat → 404.
    let response = http(
        "DELETE",
        &format!("/api/v1/labels/{side_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(response.status, 204, "delete label carries no JSON");
    assert!(response.body.is_empty(), "204 carries an empty body");
    let gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `id` = ?")
        .bind(side_id)
        .fetch_one(&pool)
        .await
        .expect("deleted label re-reads gone");
    assert_eq!(gone, 0);
    let response = http("GET", "/api/v1/labels", Some(&owner_jar), None).await;
    assert_eq!(json_of(&response.body)["total"], 6, "back to 6 defaults");
    let response = http(
        "DELETE",
        &format!("/api/v1/labels/{side_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "repeat delete: {failure}");
    assert_problem(&response, &failure, "not-found", "repeat delete");

    // ---- Checklists + items ---------------------------------------------
    // 6. Create checklist on card → 201 + Location; DB + GET re-read.
    let response = http(
        "POST",
        &format!("/api/v1/cards/{card_id}/checklists"),
        Some(&owner_jar),
        Some(serde_json::json!({"title": "Acceptance"})),
    )
    .await;
    let created = json_of(&response.body);
    assert_eq!(response.status, 201, "create checklist: {created}");
    let checklist_id = created["data"]["id"].as_i64().expect("checklist id");
    assert_eq!(
        header_first(&response.headers, "location").as_deref(),
        Some(format!("/api/v1/checklists/{checklist_id}").as_str())
    );
    let stored: String = sqlx::query_scalar("SELECT `title` FROM `checklists` WHERE `id` = ?")
        .bind(checklist_id)
        .fetch_one(&pool)
        .await
        .expect("created checklist re-reads");
    assert_eq!(stored, "Acceptance");
    let response = http(
        "GET",
        &format!("/api/v1/checklists/{checklist_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let detail = json_of(&response.body);
    assert_eq!(response.status, 200, "get checklist: {detail}");
    assert_eq!(detail["data"]["title"], "Acceptance");
    // Card-scoped list (small-N, unpaginated `data[]`).
    let response = http(
        "GET",
        &format!("/api/v1/cards/{card_id}/checklists"),
        Some(&owner_jar),
        None,
    )
    .await;
    let scoped = json_of(&response.body);
    assert_eq!(response.status, 200, "card-scoped list: {scoped}");
    assert_eq!(scoped["data"].as_array().expect("data[]").len(), 1);
    // Foreign card → 404 on create; blank title → 400 errors[].
    let response = http(
        "POST",
        &format!("/api/v1/cards/{foreign_card}/checklists"),
        Some(&owner_jar),
        Some(serde_json::json!({"title": "Hijack"})),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "foreign card create: {failure}");
    assert_problem(&response, &failure, "not-found", "foreign card");
    let response = http(
        "POST",
        &format!("/api/v1/cards/{card_id}/checklists"),
        Some(&owner_jar),
        Some(serde_json::json!({"title": "   "})),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 400, "blank title: {failure}");
    assert_problem(&response, &failure, "validation-error", "blank title");
    assert_errors_nonempty(&failure, "blank title");

    // 7. PATCH title → 200; re-GET + SELECT confirm.
    let response = http(
        "PATCH",
        &format!("/api/v1/checklists/{checklist_id}"),
        Some(&owner_jar),
        Some(serde_json::json!({"title": "Renamed list"})),
    )
    .await;
    let patched = json_of(&response.body);
    assert_eq!(response.status, 200, "patch checklist: {patched}");
    let stored: String = sqlx::query_scalar("SELECT `title` FROM `checklists` WHERE `id` = ?")
        .bind(checklist_id)
        .fetch_one(&pool)
        .await
        .expect("patched checklist re-reads");
    assert_eq!(stored, "Renamed list");

    // 8. Items: create two (positions 0 then MAX+1=1) → 201; partial
    //    `is_completed` patch persists; SELECT re-reads each step.
    let response = http(
        "POST",
        &format!("/api/v1/checklists/{checklist_id}/items"),
        Some(&owner_jar),
        Some(serde_json::json!({"text": "First item"})),
    )
    .await;
    let item_one = json_of(&response.body);
    assert_eq!(response.status, 201, "create item: {item_one}");
    let item_id = item_one["data"]["id"].as_i64().expect("item id");
    assert_eq!(item_one["data"]["position"], 0);
    assert_eq!(item_one["data"]["is_completed"], false);
    let response = http(
        "POST",
        &format!("/api/v1/checklists/{checklist_id}/items"),
        Some(&owner_jar),
        Some(serde_json::json!({"text": "Second item"})),
    )
    .await;
    let item_two = json_of(&response.body);
    assert_eq!(response.status, 201, "second item: {item_two}");
    assert_eq!(item_two["data"]["position"], 1, "MAX+1 default");
    let second_id = item_two["data"]["id"].as_i64().expect("second item id");
    let response = http(
        "PATCH",
        &format!("/api/v1/checklist-items/{item_id}"),
        Some(&owner_jar),
        Some(serde_json::json!({"is_completed": true})),
    )
    .await;
    let toggled = json_of(&response.body);
    assert_eq!(response.status, 200, "toggle item: {toggled}");
    assert_eq!(toggled["data"]["is_completed"], true);
    let completed: i64 =
        sqlx::query_scalar("SELECT `is_completed` FROM `checklist_items` WHERE `id` = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("toggled item re-reads");
    assert_eq!(completed, 1);
    // Negative position → 400 errors[].
    let response = http(
        "POST",
        &format!("/api/v1/checklists/{checklist_id}/items"),
        Some(&owner_jar),
        Some(serde_json::json!({"text": "Bad", "position": -1})),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 400, "negative position: {failure}");
    assert_problem(&response, &failure, "validation-error", "negative position");
    // Item DELETE → 204; checklist detail re-read shows one item left.
    let response = http(
        "DELETE",
        &format!("/api/v1/checklist-items/{second_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(response.status, 204, "delete item is 204");
    assert!(response.body.is_empty(), "204 carries an empty body");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `checklist_items` WHERE `checklist_id` = ?")
            .bind(checklist_id)
            .fetch_one(&pool)
            .await
            .expect("item delete re-reads");
    assert_eq!(remaining, 1);

    // 9. Checklist DELETE → 204; items cascade (`SELECT COUNT` = 0); card
    //    detail re-read shows no checklists.
    let response = http(
        "DELETE",
        &format!("/api/v1/checklists/{checklist_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(response.status, 204, "delete checklist is 204");
    let orphan: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `checklist_items` WHERE `checklist_id` = ?")
            .bind(checklist_id)
            .fetch_one(&pool)
            .await
            .expect("item cascade re-reads");
    assert_eq!(orphan, 0, "delete cascades items");
    let response = http(
        "GET",
        &format!("/api/v1/cards/{card_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let card_detail = json_of(&response.body);
    assert_eq!(response.status, 200, "card detail: {card_detail}");
    assert_eq!(
        card_detail["data"]["checklists"],
        serde_json::json!([]),
        "checklist gone from card detail"
    );

    // ---- Card-labels M2M --------------------------------------------------
    // 10. Fresh label for attach; PUT attach → 200 idempotent (double PUT =
    //     one row); deprecated POST alias (`{label_id}` + legacy `{labelId}`)
    //     likewise; card-detail GET shows the labels.
    let response = http(
        "POST",
        "/api/v1/labels",
        Some(&owner_jar),
        Some(serde_json::json!({"name": "Attachable", "color": "blue"})),
    )
    .await;
    let attach_label = json_of(&response.body);
    assert_eq!(response.status, 201, "attach label: {attach_label}");
    let attach_id = attach_label["data"]["id"]
        .as_i64()
        .expect("attach label id");
    let link_count = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = ? AND `label_id` = ?",
        )
        .bind(card_id)
        .bind(attach_id)
        .fetch_one(&pool)
        .await
        .expect("link re-reads")
    };
    let response = http(
        "PUT",
        &format!("/api/v1/cards/{card_id}/labels/{attach_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let attached = json_of(&response.body);
    assert_eq!(response.status, 200, "PUT attach: {attached}");
    assert_eq!(link_count().await, 1);
    let response = http(
        "PUT",
        &format!("/api/v1/cards/{card_id}/labels/{attach_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(
        response.status,
        200,
        "double PUT: {}",
        json_of(&response.body)
    );
    assert_eq!(link_count().await, 1, "double attach stays one row");
    let response = http(
        "POST",
        "/api/v1/labels",
        Some(&owner_jar),
        Some(serde_json::json!({"name": "Second", "color": "green"})),
    )
    .await;
    let second_label_body = json_of(&response.body);
    assert_eq!(response.status, 201, "second label: {second_label_body}");
    let second_label = second_label_body["data"]["id"]
        .as_i64()
        .expect("second label id");
    let response = http(
        "POST",
        &format!("/api/v1/cards/{card_id}/labels"),
        Some(&owner_jar),
        Some(serde_json::json!({"label_id": second_label})),
    )
    .await;
    let alias_attached = json_of(&response.body);
    assert_eq!(response.status, 200, "POST alias attach: {alias_attached}");
    let response = http(
        "POST",
        &format!("/api/v1/cards/{card_id}/labels"),
        Some(&owner_jar),
        Some(serde_json::json!({"labelId": second_label})),
    )
    .await;
    let legacy_attached = json_of(&response.body);
    assert_eq!(
        response.status, 200,
        "legacy camelCase attach: {legacy_attached}"
    );
    let alias_links: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = ? AND `label_id` = ?",
    )
    .bind(card_id)
    .bind(second_label)
    .fetch_one(&pool)
    .await
    .expect("alias link re-reads");
    assert_eq!(alias_links, 1, "alias re-attach stays one row");
    let response = http(
        "GET",
        &format!("/api/v1/cards/{card_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let with_labels = json_of(&response.body);
    assert_eq!(response.status, 200, "card with labels: {with_labels}");
    assert_eq!(
        with_labels["data"]["labels"]
            .as_array()
            .expect("labels[]")
            .len(),
        2,
        "both labels on card detail"
    );
    // Foreign label → 404 (never 403).
    let response = http(
        "PUT",
        &format!("/api/v1/cards/{card_id}/labels/{foreign_label}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "foreign label attach: {failure}");
    assert_problem(&response, &failure, "not-found", "foreign label");

    // 11. Detach → 204; card-detail GET re-read drops it; repeat → 404.
    let response = http(
        "DELETE",
        &format!("/api/v1/cards/{card_id}/labels/{attach_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(response.status, 204, "detach is 204");
    assert_eq!(link_count().await, 0, "link row gone");
    let response = http(
        "GET",
        &format!("/api/v1/cards/{card_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let detached = json_of(&response.body);
    assert_eq!(response.status, 200, "card after detach: {detached}");
    assert_eq!(
        detached["data"]["labels"]
            .as_array()
            .expect("labels[]")
            .len(),
        1,
        "one label left on re-read"
    );
    let response = http(
        "DELETE",
        &format!("/api/v1/cards/{card_id}/labels/{attach_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "detach unassigned: {failure}");
    assert_problem(&response, &failure, "not-found", "detach unassigned");

    // ---- Tags -------------------------------------------------------------
    // 12. Create → 201 + Location; dup + case-variant → 409; PATCH trims;
    //     list is a real Page; DELETE → 204 with COUNT=0 re-read.
    let response = http(
        "POST",
        "/api/v1/tags",
        Some(&owner_jar),
        Some(serde_json::json!({"name": "  Backend  "})),
    )
    .await;
    let tag_created = json_of(&response.body);
    assert_eq!(response.status, 201, "create tag: {tag_created}");
    let tag_id = tag_created["data"]["id"].as_i64().expect("tag id");
    assert_eq!(
        header_first(&response.headers, "location").as_deref(),
        Some(format!("/api/v1/tags/{tag_id}").as_str())
    );
    assert_eq!(tag_created["data"]["name"], "Backend", "name trimmed");
    let stored_tag: String = sqlx::query_scalar("SELECT `name` FROM `tags` WHERE `id` = ?")
        .bind(tag_id)
        .fetch_one(&pool)
        .await
        .expect("created tag re-reads");
    assert_eq!(stored_tag, "Backend");
    for (payload, what) in [
        (serde_json::json!({"name": "Backend"}), "exact dup"),
        (serde_json::json!({"name": "backend"}), "case-variant dup"),
    ] {
        let response = http("POST", "/api/v1/tags", Some(&owner_jar), Some(payload)).await;
        let failure = json_of(&response.body);
        assert_eq!(response.status, 409, "{what}: {failure}");
        assert_problem(&response, &failure, "conflict", what);
    }
    let response = http(
        "PATCH",
        &format!("/api/v1/tags/{tag_id}"),
        Some(&owner_jar),
        Some(serde_json::json!({"name": "  Frontend  "})),
    )
    .await;
    let tag_patched = json_of(&response.body);
    assert_eq!(response.status, 200, "patch tag: {tag_patched}");
    assert_eq!(tag_patched["data"]["name"], "Frontend");
    let renamed_tag: String = sqlx::query_scalar("SELECT `name` FROM `tags` WHERE `id` = ?")
        .bind(tag_id)
        .fetch_one(&pool)
        .await
        .expect("patched tag re-reads");
    assert_eq!(renamed_tag, "Frontend");
    let response = http("GET", "/api/v1/tags", Some(&owner_jar), None).await;
    let tag_list = json_of(&response.body);
    assert_eq!(response.status, 200, "tag list: {tag_list}");
    assert!(
        tag_list["total"].as_i64().expect("total") >= 1,
        "paginated list carries total: {tag_list}"
    );
    assert!(tag_list["data"].is_array(), "Page data[]: {tag_list}");

    // ---- Notes ------------------------------------------------------------
    // 13. Create board-linked note with duplicated tag_ids → 201; dedup
    //     persists a single link row; detail GET shows `content`.
    let response = http(
        "POST",
        "/api/v1/notes",
        Some(&owner_jar),
        Some(serde_json::json!({
            "title": "Board kickoff",
            "content": "# Hello",
            "board_id": board_id,
            "tag_ids": [tag_id, tag_id],
        })),
    )
    .await;
    let note_created = json_of(&response.body);
    assert_eq!(response.status, 201, "create note: {note_created}");
    let note_id = note_created["data"]["id"].as_i64().expect("note id");
    assert_eq!(
        header_first(&response.headers, "location").as_deref(),
        Some(format!("/api/v1/notes/{note_id}").as_str())
    );
    let note_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `note_tags` WHERE `note_id` = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .expect("note_tags dedup re-reads");
    assert_eq!(note_links, 1, "tag_ids dedup persists a single row");
    let response = http(
        "GET",
        &format!("/api/v1/notes/{note_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let note_detail = json_of(&response.body);
    assert_eq!(response.status, 200, "note detail: {note_detail}");
    assert_eq!(
        note_detail["data"]["content"], "# Hello",
        "content round-trips"
    );
    // List is a Page with NO `content` key; filters all hit the created row.
    let response = http("GET", "/api/v1/notes", Some(&owner_jar), None).await;
    let note_list = json_of(&response.body);
    assert_eq!(response.status, 200, "note list: {note_list}");
    assert!(note_list["data"].is_array(), "Page data[]");
    for item in note_list["data"].as_array().expect("data[]") {
        assert!(
            item.get("content").is_none(),
            "list never selects TEXT: {item}"
        );
    }
    for query in [
        format!("?board_id={board_id}"),
        "?search=kickoff".to_string(),
        "?type=board".to_string(),
        format!("?tag_id={tag_id}"),
    ] {
        let response = http(
            "GET",
            &format!("/api/v1/notes{query}"),
            Some(&owner_jar),
            None,
        )
        .await;
        let filtered = json_of(&response.body);
        assert_eq!(response.status, 200, "filter {query}: {filtered}");
        let ids: Vec<i64> = filtered["data"]
            .as_array()
            .expect("data[]")
            .iter()
            .map(|item| item["id"].as_i64().expect("id"))
            .collect();
        assert!(
            ids.contains(&note_id),
            "filter {query} hits the row: {filtered}"
        );
    }
    // Multiple FKs → 400; unknown tag → 404 (create matches update).
    let response = http(
        "POST",
        "/api/v1/notes",
        Some(&owner_jar),
        Some(serde_json::json!({
            "title": "Two parents",
            "content": "x",
            "board_id": board_id,
            "card_id": card_id,
        })),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 400, "multi-FK: {failure}");
    assert_problem(&response, &failure, "validation-error", "multi-FK");
    let response = http(
        "POST",
        "/api/v1/notes",
        Some(&owner_jar),
        Some(serde_json::json!({
            "title": "Bad tag",
            "content": "x",
            "tag_ids": [999999],
        })),
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "unknown tag: {failure}");
    assert_problem(&response, &failure, "not-found", "unknown tag");

    // 14. PATCH title → 200 with GET re-read; DELETE → 204 with GET-404 +
    //     COUNT=0; tag DELETE → 204 with COUNT=0.
    let response = http(
        "PATCH",
        &format!("/api/v1/notes/{note_id}"),
        Some(&owner_jar),
        Some(serde_json::json!({"title": "Renamed note"})),
    )
    .await;
    let note_patched = json_of(&response.body);
    assert_eq!(response.status, 200, "patch note: {note_patched}");
    let response = http(
        "GET",
        &format!("/api/v1/notes/{note_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let reread = json_of(&response.body);
    assert_eq!(response.status, 200, "re-get after patch: {reread}");
    assert_eq!(reread["data"]["title"], "Renamed note");
    let response = http(
        "DELETE",
        &format!("/api/v1/notes/{note_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(response.status, 204, "delete note is 204");
    assert!(response.body.is_empty(), "204 carries an empty body");
    let response = http(
        "GET",
        &format!("/api/v1/notes/{note_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    let failure = json_of(&response.body);
    assert_eq!(response.status, 404, "re-get deleted note: {failure}");
    assert_problem(&response, &failure, "not-found", "deleted note");
    let note_gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `notes` WHERE `id` = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .expect("deleted note re-reads gone");
    assert_eq!(note_gone, 0);
    let response = http(
        "DELETE",
        &format!("/api/v1/tags/{tag_id}"),
        Some(&owner_jar),
        None,
    )
    .await;
    assert_eq!(response.status, 204, "delete tag is 204");
    let tag_gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `tags` WHERE `id` = ?")
        .bind(tag_id)
        .fetch_one(&pool)
        .await
        .expect("deleted tag re-reads gone");
    assert_eq!(tag_gone, 0);

    server.abort();
    // NO TEARDOWN WIPE/DROP: the shared session store must survive until
    // every suite in this process is done.
}
