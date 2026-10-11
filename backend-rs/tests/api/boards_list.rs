//! `GET /api/v1/boards?project_id&archived&sort&order&page&limit` (T21b).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T20/T21a): all scenarios
//! run sequentially in one test with distinct emails. This suite registers
//! its owner + intruder via `POST /api/v1/auth/register` (real Argon2id
//! hashes, live cookie jars) and creates projects/boards via the API.
//!
//! NO session wipe here: `tower_sessions.session` is server-global and T11's
//! auth-session suite owns this process's one coordinated wipe at its
//! startup. This suite holds only its own jars, so concurrent wipes are
//! no-ops for it.
//!
//! Covered (red first: the list route does not exist yet):
//! default list hides archived; `?archived=true` includes them;
//! `?project_id=` filters (foreign project → empty page, never 403/404
//! oracle); `?project_id=abc` → 400 `invalid-query`; `?project_id=0` → 400
//! `validation-error` with `errors[]`; `?sort=`/`?order=` allowlist
//! violations → 400 (injection regression); `Page` shape
//! (`data,page,limit,total`); pagination slices; `GET /boards/archived` is
//! NOT a list (`:id` int-parse → 400, never 200); every list asserts
//! ownership scoping (intruder rows absent); every 4xx asserts
//! `Content-Type: application/problem+json` + exact `type` URI.

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
    let response = app.oneshot(request).await.expect("boards oneshot");
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

async fn create_project(app: Router, jar: &str, name: &str) -> i64 {
    let payload = serde_json::json!({"name": name});
    let (status, _, bytes) = send(
        app,
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(jar),
            Some(json_body(&payload)),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create project {name}: {body}");
    body["data"]["id"].as_i64().expect("project id")
}

async fn create_board(app: Router, jar: &str, payload: &Value) -> Value {
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
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create board {payload}: {body}"
    );
    body
}

async fn get_page(app: Router, jar: &str, query: &str) -> (StatusCode, HeaderMap, Value) {
    let (status, headers, bytes) = send(
        app,
        authed_request("GET", &format!("/api/v1/boards{query}"), Some(jar), None),
    )
    .await;
    (status, headers, json_of(&bytes))
}

fn names_of(page: &Value) -> Vec<String> {
    page["data"]
        .as_array()
        .expect("Page carries data[]")
        .iter()
        .map(|item| {
            item["name"]
                .as_str()
                .expect("board item carries name")
                .to_string()
        })
        .collect()
}

fn assert_page_shape(page: &Value, what: &str) {
    assert!(page["data"].is_array(), "{what} must carry data[]: {page}");
    for key in ["page", "limit", "total"] {
        assert!(
            page[key].is_number(),
            "{what} must carry numeric `{key}`: {page}"
        );
    }
    let raw = page.to_string();
    assert!(
        !raw.contains("\"columns\""),
        "{what} list items must not nest columns[]: {page}"
    );
    assert!(
        !raw.contains("\"message\""),
        "{what} must not carry `message`: {page}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn boards_list_filters_full_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous list is 401 (AuthSession guard).
    let (status, headers, bytes) =
        send(app(), authed_request("GET", "/api/v1/boards", None, None)).await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::UNAUTHORIZED, "anon list: {failure}");
    assert_problem(status, &headers, &failure, "unauthorized");

    // 1. Register owner + intruder; owner creates a project, intruder another.
    let (owner_jar, _) = register(app(), "list-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "list-intruder@example.com").await;
    let project_id = create_project(app(), &owner_jar, "List Project").await;
    let foreign_project_id = create_project(app(), &intruder_jar, "Foreign Project").await;

    // Owner boards: two active (one linked), one archived via direct SQL
    // (T22 owns the archive endpoint; the flag is setup state here, not a
    // mutation under test). Intruder boards must never leak into the
    // owner's lists.
    let solo = create_board(app(), &owner_jar, &serde_json::json!({"name": "List Solo"})).await;
    let solo_id = solo["data"]["id"].as_i64().expect("solo id");
    let linked = create_board(
        app(),
        &owner_jar,
        &serde_json::json!({"name": "List Linked", "project_id": project_id}),
    )
    .await;
    assert_eq!(linked["data"]["project_id"], project_id);
    let archived = create_board(
        app(),
        &owner_jar,
        &serde_json::json!({"name": "List Archived"}),
    )
    .await;
    let archived_id = archived["data"]["id"].as_i64().expect("archived id");
    sqlx::query("UPDATE `boards` SET `is_archived` = 1 WHERE `id` = ?")
        .bind(archived_id)
        .execute(&pool)
        .await
        .expect("archive flag setups");
    create_board(
        app(),
        &intruder_jar,
        &serde_json::json!({"name": "List Theirs", "project_id": foreign_project_id}),
    )
    .await;
    create_board(
        app(),
        &intruder_jar,
        &serde_json::json!({"name": "List Theirs Solo"}),
    )
    .await;

    // 2. Default list hides archived + intruder rows; Page shape holds.
    let (status, _, page) = get_page(app(), &owner_jar, "").await;
    assert_eq!(status, StatusCode::OK, "default list: {page}");
    assert_page_shape(&page, "default list");
    assert_eq!(page["page"], 1);
    assert_eq!(page["limit"], 50);
    assert_eq!(page["total"], 2, "two active owner boards: {page}");
    assert_eq!(page["data"].as_array().expect("data[]").len(), 2);
    let names = names_of(&page);
    assert!(names.contains(&"List Solo".to_string()), "{names:?}");
    assert!(names.contains(&"List Linked".to_string()), "{names:?}");
    assert!(
        !names.iter().any(|name| name.contains("Theirs")),
        "intruder rows absent: {names:?}"
    );
    let ids: Vec<i64> = page["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|item| item["id"].as_i64().expect("board id"))
        .collect();
    assert!(
        !ids.contains(&archived_id),
        "archived hidden by default: {ids:?}"
    );
    assert!(ids.contains(&solo_id), "owner rows present: {ids:?}");
    // Linked item carries its project ref; solo carries nulls.
    for item in page["data"].as_array().expect("data[]") {
        if item["name"] == "List Linked" {
            assert_eq!(item["project_id"], project_id);
            assert_eq!(item["project"]["id"], project_id);
            assert_eq!(item["project"]["name"], "List Project");
        } else {
            assert!(item["project_id"].is_null(), "solo: {item}");
            assert!(item["project"].is_null(), "solo: {item}");
        }
        assert!(item["created_at"].is_string(), "timestamps: {item}");
        assert!(item["updated_at"].is_string(), "timestamps: {item}");
        assert_eq!(item["is_archived"], false);
    }

    // 3. `?archived=true` includes archived; intruder rows still absent.
    let (status, _, page) = get_page(app(), &owner_jar, "?archived=true").await;
    assert_eq!(status, StatusCode::OK, "archived=true: {page}");
    assert_page_shape(&page, "archived=true");
    assert_eq!(page["total"], 3, "archived included: {page}");
    let names = names_of(&page);
    assert!(names.contains(&"List Archived".to_string()), "{names:?}");
    assert!(
        !names.iter().any(|name| name.contains("Theirs")),
        "intruder rows absent with archived=true: {names:?}"
    );
    // Explicit `?archived=false` behaves like the default (hides archived).
    let (status, _, page) = get_page(app(), &owner_jar, "?archived=false").await;
    assert_eq!(status, StatusCode::OK, "archived=false: {page}");
    assert_eq!(page["total"], 2, "archived hidden: {page}");

    // 4. `?project_id=` filters; foreign project → empty page (never a
    //    403/404 oracle); intruder sees only their own rows.
    let (status, _, page) = get_page(app(), &owner_jar, &format!("?project_id={project_id}")).await;
    assert_eq!(status, StatusCode::OK, "project filter: {page}");
    assert_eq!(page["total"], 1, "only the linked board: {page}");
    assert_eq!(names_of(&page), vec!["List Linked".to_string()]);
    let (status, _, page) = get_page(
        app(),
        &owner_jar,
        &format!("?project_id={foreign_project_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "foreign project: {page}");
    assert_page_shape(&page, "foreign project");
    assert_eq!(page["total"], 0, "foreign → empty page: {page}");
    assert!(page["data"].as_array().expect("data[]").is_empty());
    let (status, _, page) = get_page(app(), &owner_jar, "?project_id=999999").await;
    assert_eq!(status, StatusCode::OK, "missing project: {page}");
    assert_eq!(page["total"], 0, "missing → empty page: {page}");
    let (status, _, intruder_page) = get_page(app(), &intruder_jar, "").await;
    assert_eq!(status, StatusCode::OK, "intruder list: {intruder_page}");
    assert_eq!(
        intruder_page["total"], 2,
        "intruder owns two: {intruder_page}"
    );
    let intruder_names = names_of(&intruder_page);
    assert!(
        !intruder_names.iter().any(|name| name.contains("List Solo")),
        "owner rows absent for intruder: {intruder_names:?}"
    );
    // Deprecated `/api/*` alias serves the same handler.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/boards", Some(&owner_jar), None),
    )
    .await;
    let alias = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "alias list: {alias}");
    assert_eq!(alias["total"], 2);

    // 5. Pagination slices (`?limit=2` over 2 active + archived via
    //    `archived=true` = 3 rows); out-of-range page → empty data, same total.
    let (status, _, first) = get_page(app(), &owner_jar, "?archived=true&limit=2&page=1").await;
    assert_eq!(status, StatusCode::OK, "page 1: {first}");
    assert_eq!(first["page"], 1);
    assert_eq!(first["limit"], 2);
    assert_eq!(first["total"], 3);
    assert_eq!(first["data"].as_array().expect("data[]").len(), 2);
    let (status, _, second) = get_page(app(), &owner_jar, "?archived=true&limit=2&page=2").await;
    assert_eq!(status, StatusCode::OK, "page 2: {second}");
    assert_eq!(second["total"], 3);
    assert_eq!(second["data"].as_array().expect("data[]").len(), 1);
    let mut both = names_of(&first);
    both.extend(names_of(&second));
    both.sort();
    assert_eq!(both, vec!["List Archived", "List Linked", "List Solo"]);
    let (status, _, empty) = get_page(app(), &owner_jar, "?archived=true&limit=2&page=9").await;
    assert_eq!(status, StatusCode::OK, "far page: {empty}");
    assert_eq!(empty["total"], 3);
    assert!(empty["data"].as_array().expect("data[]").is_empty());

    // 6. Sort/order: `?sort=name&order=asc` is deterministic on distinct
    //    names (default `updated_at DESC` needs no exact-order assert).
    let (status, _, page) = get_page(app(), &owner_jar, "?sort=name&order=asc").await;
    assert_eq!(status, StatusCode::OK, "name asc: {page}");
    assert_eq!(names_of(&page), vec!["List Linked", "List Solo"]);
    let (status, _, page) = get_page(app(), &owner_jar, "?sort=name&order=desc").await;
    assert_eq!(status, StatusCode::OK, "name desc: {page}");
    assert_eq!(names_of(&page), vec!["List Solo", "List Linked"]);

    // 7. Garbage query values → 400 problem+json (never a 200 list, never a
    //    silent ignore): `?project_id=abc` fails int-parse (`invalid-query`);
    //    `0`/negative fail the garde range (`validation-error` + errors[]);
    //    `?sort=`/`?order=` violations fail the allowlist; bad
    //    `?page=`/`?limit=` fail strict pagination; quote-named boards prove
    //    bound params (stored literally, not executed).
    let (status, headers, failure) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/boards?project_id=abc",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure_body = json_of(&failure);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "string project_id: {failure_body}"
    );
    assert_problem(status, &headers, &failure_body, "invalid-query");
    for (query, what) in [
        ("?project_id=0", "zero project_id"),
        ("?project_id=-3", "negative project_id"),
        ("?project_id=1.5", "float project_id"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/boards{query}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        // Floats never reach garde (serde rejects → `invalid-query`);
        // ints reach the `range(min = 1)` rule (`validation-error`).
        if what == "float project_id" {
            assert_problem(status, &headers, &failure, "invalid-query");
        } else {
            assert_problem(status, &headers, &failure, "validation-error");
            assert_errors_nonempty(&failure, what);
        }
    }
    for (query, what) in [
        ("?sort=evil", "unknown sort key"),
        ("?sort=name%3BDROP%20TABLE%20boards", "sort injection attempt"),
        ("?order=sideways", "unknown order"),
        ("?order=name", "order/sort swap"),
        ("?page=0", "zero page"),
        ("?limit=0", "zero limit"),
        ("?limit=101", "over-ceiling limit"),
        ("?limit=abc", "non-integer limit"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/boards{query}"),
                Some(&owner_jar),
                None,
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
            PROBLEM_JSON,
            "{what}"
        );
        assert!(
            ["validation-error", "invalid-query"]
                .iter()
                .any(|slug| failure["type"] == format!("https://kanpad.dev/problems/{slug}")),
            "{what} must be a 400 problem: {failure}"
        );
        if failure["type"] == "https://kanpad.dev/problems/validation-error" {
            assert_errors_nonempty(&failure, what);
        }
    }
    // Unknown query keys are ignored (serde default, matching Nest).
    let (status, _, page) = get_page(app(), &owner_jar, "?bogus=1").await;
    assert_eq!(status, StatusCode::OK, "unknown keys ignored: {page}");
    assert_eq!(page["total"], 2);
    // Quote payload stored literally (bound params, never interpolated).
    create_board(
        app(),
        &owner_jar,
        &serde_json::json!({"name": "List O'Brien"}),
    )
    .await;
    let stored: String =
        sqlx::query_scalar("SELECT `name` FROM `boards` WHERE `name` = 'List O''Brien'")
            .fetch_one(&pool)
            .await
            .expect("quote payload re-reads literally");
    assert_eq!(stored, "List O'Brien");

    // 8. Deleted-route regression: `GET /api/v1/boards/archived` is NOT a
    //    boards list — `archived` fails `:id` int-parse → 400
    //    `invalid-path`, never a 200 list.
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/boards/archived", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "deleted /archived route: {failure}"
    );
    assert_problem(status, &headers, &failure, "invalid-path");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
