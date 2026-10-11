//! `GET|POST /api/v1/boards/:boardId/columns` + `PATCH|DELETE /api/v1/columns/:id` (T23a).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T21/T22): all scenarios
//! run sequentially in one test with distinct emails. This suite registers its
//! owner + intruder via `POST /api/v1/auth/register` (real Argon2id hashes,
//! live cookie jars) and creates boards via the API. Cards/labels/checklists
//! used for the nested-embed assertions are inserted via direct SQL (card
//! routes land in T24a).
//!
//! NO session wipe here: `tower_sessions.session` is server-global. This
//! suite holds only its own jars, so concurrent suites are unaffected.
//!
//! Covered (red first: columns routes do not exist yet):
//! `GET /boards/:boardId/columns → 200 {data[]}` (unpaginated, `position ASC`,
//! embeds card summaries — never `description` TEXT — via
//! `common::card_summary`); `POST {name?} → 201 {data} + Location`
//! (`MAX(position)+1` via `FOR UPDATE`, absent name defaults to
//! `New Column`); `PATCH {name} → 200`; `DELETE → 204` (cards cascade +
//! set-based gap-close in tx, dense `0..N` on re-read); foreign board/column
//! → `404` (never 403); every 4xx asserts problem+json + exact `type` URI;
//! every mutation re-reads state via second GET or direct `SELECT`.

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
    let response = app.oneshot(request).await.expect("columns oneshot");
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

async fn create_board(app: Router, jar: &str, payload: &Value) -> i64 {
    let (status, _, bytes) = send(
        app,
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(jar),
            Some(json_body(payload)),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create board: {body}");
    body["data"]["id"].as_i64().expect("board id")
}

#[sqlx::test(migrations = "./migrations")]
async fn columns_crud_gap_close_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every columns route (AuthSession guard).
    for (method, path, body) in [
        ("GET", "/api/v1/boards/1/columns", None),
        (
            "POST",
            "/api/v1/boards/1/columns",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        (
            "PATCH",
            "/api/v1/columns/1",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        ("DELETE", "/api/v1/columns/1", None),
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
    let (owner_jar, owner_id) = register(app(), "cols-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "cols-intruder@example.com").await;
    let board_id = create_board(app(), &owner_jar, &serde_json::json!({"name": "Col Board"})).await;
    let foreign_board =
        create_board(app(), &intruder_jar, &serde_json::json!({"name": "Theirs"})).await;

    // 2. Nested list → 200 {data[3]} `position ASC`, no cards yet, no
    //    `message` key.
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
    assert!(listed.get("message").is_none(), "no message key: {listed}");
    let columns = listed["data"].as_array().expect("data[]");
    assert_eq!(columns.len(), 3, "board starts with 3 columns: {listed}");
    let names: Vec<&str> = columns
        .iter()
        .map(|column| column["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["To Do", "In Progress", "Done"]);
    let positions: Vec<i64> = columns
        .iter()
        .map(|column| column["position"].as_i64().expect("position"))
        .collect();
    assert_eq!(positions, [0, 1, 2]);
    for column in columns {
        assert_eq!(column["board_id"], board_id);
        assert_eq!(column["cards"], serde_json::json!([]));
        assert!(
            column["created_at"].is_string(),
            "timestamps render: {column}"
        );
        assert!(
            column["updated_at"].is_string(),
            "timestamps render: {column}"
        );
    }
    let col_ids: Vec<i64> = columns
        .iter()
        .map(|column| column["id"].as_i64().expect("column id"))
        .collect();

    // 3. Seed cards/labels/checklists via SQL (card routes land in T24a) and
    //    assert the nested card-summary embed: no `description` key,
    //    `has_description`, labels, and `checklist_progress` only when the
    //    card has checklists.
    sqlx::query(
        "INSERT INTO `cards` (`title`, `column_id`, `position`, `description`) VALUES \
         ('First card', ?, 0, 'First card body'), \
         ('Second card', ?, 1, NULL), \
         ('Padded card', ?, 0, '   ')",
    )
    .bind(col_ids[0])
    .bind(col_ids[0])
    .bind(col_ids[2])
    .execute(&pool)
    .await
    .expect("seed cards");
    let first_card: i64 =
        sqlx::query_scalar("SELECT `id` FROM `cards` WHERE `title` = 'First card'")
            .fetch_one(&pool)
            .await
            .expect("first card id");
    let second_card: i64 =
        sqlx::query_scalar("SELECT `id` FROM `cards` WHERE `title` = 'Second card'")
            .fetch_one(&pool)
            .await
            .expect("second card id");
    // Register already seeds the 6 default labels (incl. `Urgent`/red and
    // `Bug`/blue) for the owner — link those, never re-insert (per-user
    // unique `UQ_labels_user_name` would 409/1062).
    sqlx::query("INSERT INTO `card_labels` (`card_id`, `label_id`) SELECT ?, `id` FROM `labels` WHERE `user_id` = ? AND `name` IN ('Urgent', 'Bug')")
        .bind(first_card)
        .bind(owner_id)
        .execute(&pool)
        .await
        .expect("link labels");
    sqlx::query("INSERT INTO `checklists` (`title`, `card_id`) VALUES ('Acceptance', ?)")
        .bind(first_card)
        .execute(&pool)
        .await
        .expect("seed checklist");
    let checklist: i64 = sqlx::query_scalar("SELECT `id` FROM `checklists` WHERE `card_id` = ?")
        .bind(first_card)
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
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list with embeds: {listed}");
    let columns = listed["data"].as_array().expect("data[]");
    let first_col_cards = columns[0]["cards"].as_array().expect("cards[]");
    assert_eq!(
        first_col_cards.len(),
        2,
        "source order by position: {listed}"
    );
    let first = &first_col_cards[0];
    assert_eq!(first["id"], first_card);
    assert_eq!(first["title"], "First card");
    assert_eq!(first["has_description"], true);
    assert_eq!(first["column_id"], col_ids[0]);
    assert_eq!(first["position"], 0);
    assert!(first["due_date"].is_null(), "null due_date: {first}");
    assert!(
        first.get("description").is_none(),
        "never select TEXT: {first}"
    );
    let label_names: Vec<&str> = first["labels"]
        .as_array()
        .expect("labels[]")
        .iter()
        .map(|label| label["name"].as_str().expect("label name"))
        .collect();
    assert_eq!(label_names, ["Urgent", "Bug"]);
    assert_eq!(
        first["checklist_progress"],
        serde_json::json!({"completed": 1, "total": 2, "percent": 50}),
        "progress only when checklists exist: {first}"
    );
    let second = &first_col_cards[1];
    assert_eq!(second["id"], second_card);
    assert_eq!(second["has_description"], false);
    assert_eq!(second["labels"], serde_json::json!([]));
    assert!(
        second.get("checklist_progress").is_none(),
        "no progress key without checklists: {second}"
    );
    let padded = &columns[2]["cards"].as_array().expect("cards[]")[0];
    assert_eq!(
        padded["has_description"], false,
        "whitespace-only description counts as none: {padded}"
    );

    // 4. Create → 201 + Location with MAX+1; DB re-read confirms the row.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Backlog"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create column: {created}");
    let new_id = created["data"]["id"].as_i64().expect("column id");
    assert_eq!(created["data"]["name"], "Backlog");
    assert_eq!(created["data"]["position"], 3);
    assert_eq!(created["data"]["board_id"], board_id);
    assert_eq!(created["data"]["cards"], serde_json::json!([]));
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII");
    assert_eq!(location, format!("/api/v1/columns/{new_id}"));
    let stored: (String, i32) =
        sqlx::query_as("SELECT `name`, `position` FROM `columns` WHERE `id` = ?")
            .bind(new_id)
            .fetch_one(&pool)
            .await
            .expect("created column re-reads");
    assert_eq!(stored, ("Backlog".to_string(), 3));

    // 5. Absent name defaults to `New Column` (MAX+1 continues).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "default name: {created}");
    assert_eq!(created["data"]["name"], "New Column");
    assert_eq!(created["data"]["position"], 4);
    let default_id = created["data"]["id"].as_i64().expect("column id");
    let stored_name: String = sqlx::query_scalar("SELECT `name` FROM `columns` WHERE `id` = ?")
        .bind(default_id)
        .fetch_one(&pool)
        .await
        .expect("default name re-reads");
    assert_eq!(stored_name, "New Column");

    // 6. Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "O'Brien"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "quote payload: {created}");
    assert_eq!(created["data"]["name"], "O'Brien");
    let quote_id = created["data"]["id"].as_i64().expect("column id");
    let stored_name: String = sqlx::query_scalar("SELECT `name` FROM `columns` WHERE `id` = ?")
        .bind(quote_id)
        .fetch_one(&pool)
        .await
        .expect("quote payload re-reads literally");
    assert_eq!(stored_name, "O'Brien");

    // 7. Validation: blank/overlong names → 400 errors[]; wrong JSON type →
    //    400 body-shape error.
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
                &format!("/api/v1/boards/{board_id}/columns"),
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
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": 123}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "numeric name: {failure}");
    assert_problem(status, &headers, &failure, "invalid-json");

    // 8. Foreign/missing board → 404 (never 403); garbage board id → 400.
    for (id, what) in [
        (foreign_board.to_string(), "foreign board"),
        ("999999".to_string(), "missing board"),
    ] {
        for method in ["GET", "POST"] {
            let body = if method == "POST" {
                Some(json_body(&serde_json::json!({"name": "Hijack"})))
            } else {
                None
            };
            let (status, headers, bytes) = send(
                app(),
                authed_request(
                    method,
                    &format!("/api/v1/boards/{id}/columns"),
                    Some(&owner_jar),
                    body,
                ),
            )
            .await;
            let failure = json_of(&bytes);
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {what}: {failure}");
            assert_problem(status, &headers, &failure, "not-found");
        }
    }
    for (method, what) in [("GET", "list"), ("POST", "create")] {
        let body = if method == "POST" {
            Some(json_body(&serde_json::json!({"name": "Hijack"})))
        } else {
            None
        };
        let (status, headers, bytes) = send(
            app(),
            authed_request(method, "/api/v1/boards/abc/columns", Some(&owner_jar), body),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{what} garbage board id: {failure}"
        );
        assert_problem(status, &headers, &failure, "invalid-path");
    }

    // 9. PATCH rename → 200; re-GET + SELECT confirm (no echo).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{new_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Renamed"}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "rename: {patched}");
    assert_eq!(patched["data"]["id"], new_id);
    assert_eq!(patched["data"]["name"], "Renamed");
    assert_eq!(patched["data"]["position"], 3);
    let stored_name: String = sqlx::query_scalar("SELECT `name` FROM `columns` WHERE `id` = ?")
        .bind(new_id)
        .fetch_one(&pool)
        .await
        .expect("renamed column re-reads");
    assert_eq!(stored_name, "Renamed");
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
    assert_eq!(status, StatusCode::OK, "re-get after rename: {listed}");
    let renamed = listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .find(|column| column["id"] == new_id)
        .expect("renamed column listed");
    assert_eq!(renamed["name"], "Renamed");

    // 10. Empty `{}` PATCH is a no-op 200; blank name → 400.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{new_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty patch: {patched}");
    assert_eq!(patched["data"]["name"], "Renamed");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/columns/{new_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "  "}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "blank rename: {failure}");
    assert_problem(status, &headers, &failure, "validation-error");
    assert_errors_nonempty(&failure, "blank rename");

    // 11. PATCH foreign/missing → 404 (never 403); garbage id → 400.
    let intruder_col: i64 = sqlx::query_scalar(
        "SELECT c.`id` FROM `columns` c WHERE c.`board_id` = ? ORDER BY c.`position` LIMIT 1",
    )
    .bind(foreign_board)
    .fetch_one(&pool)
    .await
    .expect("intruder column id");
    for id in [intruder_col.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/columns/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"name": "Hijack"}))),
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
            "/api/v1/columns/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");

    // 12. Deprecated `/api/*` alias serves the same nested handlers.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/boards/{board_id}/columns"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Alias Col"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "alias create: {created}");
    assert_eq!(created["data"]["name"], "Alias Col");

    // 13. DELETE the middle default column (position 1, holding no cards
    //     after the SQL seeds went to 0/2) → 204 empty body; positions close
    //     to dense 0..N on re-read; cards in a deleted column cascade.
    let doomed = col_ids[1];
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/columns/{doomed}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete column: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries an empty body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    let positions: Vec<i32> = sqlx::query_scalar(
        "SELECT `position` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await
    .expect("positions re-read");
    let dense: Vec<i32> = (0..positions.len() as i32).collect();
    assert_eq!(positions, dense, "gap closed, dense 0..N");
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
    assert_eq!(status, StatusCode::OK, "re-get after delete: {listed}");
    let listed_positions: Vec<i64> = listed["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|column| column["position"].as_i64().expect("position"))
        .collect();
    let expected: Vec<i64> = (0..listed_positions.len() as i64).collect();
    assert_eq!(listed_positions, expected, "wire order dense: {listed}");

    // 14. Cards cascade: delete the first default column (holds 2 cards) and
    //     prove the cards + their label links + checklists are gone.
    let card_holder = col_ids[0];
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/columns/{card_holder}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete card holder: {}",
        String::from_utf8_lossy(&bytes)
    );
    let card_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` = ?")
        .bind(card_holder)
        .fetch_one(&pool)
        .await
        .expect("cascaded cards re-read");
    assert_eq!(card_count, 0, "cards cascade gone");
    let link_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = ?")
            .bind(first_card)
            .fetch_one(&pool)
            .await
            .expect("cascaded label links re-read");
    assert_eq!(link_count, 0, "label links cascade gone");
    let checklist_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `checklists` WHERE `card_id` = ?")
            .bind(first_card)
            .fetch_one(&pool)
            .await
            .expect("cascaded checklists re-read");
    assert_eq!(checklist_count, 0, "checklists cascade gone");
    let positions: Vec<i32> = sqlx::query_scalar(
        "SELECT `position` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await
    .expect("positions re-read after cascade delete");
    let dense: Vec<i32> = (0..positions.len() as i32).collect();
    assert_eq!(positions, dense, "gap closed again");

    // 15. Deleting twice → 404; foreign/missing → 404 (never 403); garbage →
    //     400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/columns/{card_holder}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "delete twice: {failure}");
    assert_problem(status, &headers, &failure, "not-found");
    for id in [intruder_col.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "DELETE",
                &format!("/api/v1/columns/{id}"),
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
        authed_request("DELETE", "/api/v1/columns/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
