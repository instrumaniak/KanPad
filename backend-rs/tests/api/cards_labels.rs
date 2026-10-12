//! `PUT /api/v1/cards/:id/labels/:label_id` + deprecated
//! `POST /api/v1/cards/:id/labels` alias + `DELETE /:id/labels/:label_id`
//! (T24c).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T24a): all scenarios run
//! sequentially in one test with distinct emails. Registers owner + intruder
//! via `POST /api/v1/auth/register` (real Argon2id hashes, live cookie jars)
//! and creates boards/columns/cards/labels via the API.
//!
//! NO session wipe here: `tower_sessions.session` is server-global. This
//! suite holds only its own jars, so concurrent suites are unaffected.
//!
//! Covered (red first: label M2M routes do not exist yet):
//! `PUT /:id/labels/:label_id → 200 {data:Card}` idempotent (double attach =
//! one row on re-read via `GET` detail + direct `SELECT COUNT(*)`); deprecated
//! `POST /:id/labels {label_id}` alias → 200 (legacy `{labelId}` also
//! accepted); `DELETE /:id/labels/:label_id → 204` (re-read: label gone);
//! detach unassigned → `404` (not 409); foreign card/label → `404` (never
//! 403); garbage ids → `400 invalid-path`; bad alias body → `400`
//! validation-error with `errors[]`; every 4xx asserts problem+json + exact
//! `type` URI; every mutation re-reads state via second GET or direct
//! `SELECT`.
//!
//! Contract: `20-api-contract.md` § cards (`PUT /:id/labels/:label_id → 200
//! {data:Card}` idempotent, `POST /:id/labels` alias one release deprecated
//! with `{label_id}` body + legacy `{labelId}`, `DELETE … → 204` with 404 if
//! unassigned) + `21-api-standard-shapes.md` row "`POST /api/cards/:id/labels
//! {labelId}` → `200` | `PUT /api/v1/cards/:id/labels/:label_id` → `200
//! {data:Card}` … `DELETE …` → `204`; unassigned → `404`". Schema:
//! `30-db-schema.md` `card_labels(card_id,label_id composite PK, both
//! CASCADE)` + `IDX_card_labels_label_id(label_id)`.

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
    let response = app.oneshot(request).await.expect("labels M2M oneshot");
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

async fn create_label(app: Router, jar: &str, name: &str, color: &str) -> i64 {
    let payload = serde_json::json!({"name": name, "color": color});
    let (status, headers, bytes) = send(
        app,
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(jar),
            Some(json_body(&payload)),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create label: {body}");
    let location = headers
        .get(header::LOCATION)
        .expect("create label sets Location")
        .to_str()
        .expect("Location is ASCII")
        .to_owned();
    let id = body["data"]["id"].as_i64().expect("label id");
    assert!(
        location.ends_with(&format!("/api/v1/labels/{id}")),
        "Location: {location}"
    );
    id
}

/// Direct `SELECT COUNT(*)` re-read of one M2M link (never trusts the
/// response envelope alone).
async fn link_count(pool: &MySqlPool, card_id: i64, label_id: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = ? AND `label_id` = ?",
    )
    .bind(card_id)
    .bind(label_id)
    .fetch_one(pool)
    .await
    .expect("link count reads")
}

#[sqlx::test(migrations = "./migrations")]
async fn cards_labels_m2m_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every label-M2M route (AuthSession guard).
    for (method, path, body) in [
        ("PUT", "/api/v1/cards/1/labels/1", None),
        (
            "POST",
            "/api/v1/cards/1/labels",
            Some(json_body(&serde_json::json!({"label_id": 1}))),
        ),
        ("DELETE", "/api/v1/cards/1/labels/1", None),
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
    //    columns); owner creates one card; both mint one label each.
    let (owner_jar, _) = register(app(), "labels-m2m-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "labels-m2m-intruder@example.com").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Label Board"}))),
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
            Some(json_body(&serde_json::json!({
                "title": "Labelled card",
                "column_id": col_id,
            }))),
        ),
    )
    .await;
    let created_card = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create card: {created_card}");
    let card_id = created_card["data"]["id"].as_i64().expect("card id");
    let label_id = create_label(app(), &owner_jar, "M2M Red", "red").await;
    let second_label = create_label(app(), &owner_jar, "M2M Blue", "blue").await;
    // Intruder mints a foreign label + foreign card (label ids and card ids
    // share no sequence, so the foreign ids must come from live rows).
    let foreign_label = create_label(app(), &intruder_jar, "Theirs", "green").await;
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

    // 2. PUT attach → 200 {data} with the label nested; DB re-read confirms
    //    exactly one link row (response + SELECT, never the envelope alone).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PUT",
            &format!("/api/v1/cards/{card_id}/labels/{label_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let attached = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "PUT attach: {attached}");
    assert!(
        attached.get("message").is_none(),
        "no message key: {attached}"
    );
    let nested: Vec<i64> = attached["data"]["labels"]
        .as_array()
        .expect("detail nests labels")
        .iter()
        .map(|label| label["id"].as_i64().expect("label id"))
        .collect();
    assert_eq!(nested, vec![label_id], "one label nested: {attached}");
    assert_eq!(
        link_count(&pool, card_id, label_id).await,
        1,
        "exactly one link row"
    );

    // 3. PUT attach is idempotent: double attach → 200, still one row.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PUT",
            &format!("/api/v1/cards/{card_id}/labels/{label_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let reattached = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "PUT re-attach: {reattached}");
    assert_eq!(
        link_count(&pool, card_id, label_id).await,
        1,
        "double attach stays one row"
    );

    // 4. Deprecated POST alias with `{label_id}` → 200 (idempotent too);
    //    legacy `{labelId}` camelCase → 200 as well.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/labels"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"label_id": second_label}))),
        ),
    )
    .await;
    let alias_attached = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::OK,
        "POST alias attach: {alias_attached}"
    );
    assert_eq!(
        link_count(&pool, card_id, second_label).await,
        1,
        "alias wrote one link row"
    );
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/labels"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"labelId": second_label}))),
        ),
    )
    .await;
    let legacy_attached = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::OK,
        "POST legacy camelCase attach: {legacy_attached}"
    );
    assert_eq!(
        link_count(&pool, card_id, second_label).await,
        1,
        "legacy re-attach stays one row"
    );

    // 5. GET detail re-read shows both labels (attach proof via second GET).
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
    assert_eq!(status, StatusCode::OK, "detail re-read: {detail}");
    let mut nested: Vec<i64> = detail["data"]["labels"]
        .as_array()
        .expect("detail nests labels")
        .iter()
        .map(|label| label["id"].as_i64().expect("label id"))
        .collect();
    nested.sort_unstable();
    let mut expected = vec![label_id, second_label];
    expected.sort_unstable();
    assert_eq!(nested, expected, "both labels nested: {detail}");

    // 6. DELETE → 204 (empty body); re-read via GET + SELECT: label gone.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{card_id}/labels/{label_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "detach: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries no body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    assert_eq!(
        link_count(&pool, card_id, label_id).await,
        0,
        "link row gone"
    );
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
    assert_eq!(status, StatusCode::OK, "detail after detach: {detail}");
    let nested: Vec<i64> = detail["data"]["labels"]
        .as_array()
        .expect("detail nests labels")
        .iter()
        .map(|label| label["id"].as_i64().expect("label id"))
        .collect();
    assert_eq!(nested, vec![second_label], "one label left: {detail}");

    // 7. Detach unassigned → 404 (not 409), problem+json + exact type URI.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{card_id}/labels/{label_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "detach unassigned: {failure}"
    );
    assert_problem(status, &headers, &failure, "not-found");

    // 8. Foreign card / foreign label → 404 (never 403, no oracle).
    for (method, path, body) in [
        (
            "PUT",
            format!("/api/v1/cards/{foreign_card}/labels/{label_id}"),
            None,
        ),
        (
            "PUT",
            format!("/api/v1/cards/{card_id}/labels/{foreign_label}"),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/cards/{foreign_card}/labels/{label_id}"),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/cards/{card_id}/labels/{foreign_label}"),
            None,
        ),
        (
            "POST",
            format!("/api/v1/cards/{foreign_card}/labels"),
            Some(json_body(&serde_json::json!({"label_id": label_id}))),
        ),
        (
            "POST",
            format!("/api/v1/cards/{card_id}/labels"),
            Some(json_body(&serde_json::json!({"label_id": foreign_label}))),
        ),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request(method, &path, Some(&owner_jar), body)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    // Missing ids (card 999999 / label 999999 share no sequence with live
    // rows — both must 404, never 403).
    for (method, path, body) in [
        (
            "PUT",
            format!("/api/v1/cards/999999/labels/{label_id}"),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/cards/{card_id}/labels/999999"),
            None,
        ),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request(method, &path, Some(&owner_jar), body)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }

    // 9. Garbage ids → 400 `invalid-path` (never plain-text).
    for (method, path, body) in [
        ("PUT", "/api/v1/cards/abc/labels/1".to_string(), None),
        ("PUT", format!("/api/v1/cards/{card_id}/labels/xyz"), None),
        (
            "DELETE",
            format!("/api/v1/cards/{card_id}/labels/xyz"),
            None,
        ),
    ] {
        let (status, headers, bytes) =
            send(app(), authed_request(method, &path, Some(&owner_jar), body)).await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{method} {path}: {failure}"
        );
        assert_problem(status, &headers, &failure, "invalid-path");
    }

    // 10. Bad alias bodies → 400 validation-error with `errors[]` (missing
    //     key, zero id); wrong content shape is 400, never 500.
    for payload in [
        serde_json::json!({}),
        serde_json::json!({"label_id": 0}),
        serde_json::json!({"labelId": 0}),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                &format!("/api/v1/cards/{card_id}/labels"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "alias {payload}: {failure}"
        );
        assert_problem(status, &headers, &failure, "validation-error");
        assert_errors_nonempty(&failure, "alias body");
    }

    // 11. Deprecated `/api` (pre-v1) prefix serves the same PUT handler.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PUT",
            &format!("/api/cards/{card_id}/labels/{label_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let aliased = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "deprecated prefix PUT: {aliased}");
    assert_eq!(
        link_count(&pool, card_id, label_id).await,
        1,
        "deprecated prefix wrote the link"
    );
}
