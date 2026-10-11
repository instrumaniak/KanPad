//! `GET/POST /api/v1/projects` + `PATCH/DELETE /api/v1/projects/{id}` (T20).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T10/T11): all scenarios
//! run sequentially in one test with distinct emails. Fixture `users.sql`
//! passwords are opaque placeholders, so this suite registers its owner +
//! intruder via `POST /api/v1/auth/register` (real Argon2id hashes, live
//! cookie jars) and seeds boards/columns/cards via direct SQL (no boards API
//! exists yet — T21a owns it).
//!
//! NO session wipe here: `tower_sessions.session` is server-global and T11's
//! auth-session suite owns this process's one coordinated wipe at its
//! startup. This suite holds only its own jars, so concurrent wipes are
//! no-ops for it.
//!
//! Covered (red first: the projects router is an empty stub):
//! list `200 Page` (`data,page,limit,total`) with REAL `board_count` via a
//! single `LEFT JOIN ... GROUP BY` (store-level `Questions`-counter proof:
//! count + page = exactly 2 statements for any P, calibrated in-test);
//! `?q&sort&order` filters (LIKE `%`/`_`/`'` escaped, built in Rust +
//! `push_bind` only; sort allowlist violation → 400); strict pagination
//! (`page=0`/`limit=0`/`limit=101`/non-integer → 400 `errors[]`);
//! `POST {name} → 201 + Location` (empty/blank/missing/overlong/non-string →
//! 400; unknown fields ignored; quote payload stored literally);
//! `PATCH {name} → 200` (same-name stays 200; empty/missing → 400;
//! `GET`-style bad id `abc` → 400 `invalid-path`);
//! `DELETE → 204` empty body with app-level board cascade (DB FK is
//! `SET NULL`, so the store deletes boards explicitly in-tx; columns/cards
//! follow via DB cascades — re-read via `SELECT COUNT`);
//! cross-user id → 404 (no oracle, NEVER 403); every 4xx asserts
//! `Content-Type: application/problem+json` + exact `type` URI; every
//! mutation re-reads state via second GET or direct `SELECT`.
//!
//! Board-detail re-GET (`GET /api/v1/boards/:id → 404`) cannot be asserted
//! here — the board detail route lands in T21a. Cascade is proven via
//! `SELECT COUNT(*) FROM boards WHERE project_id = ?` = 0 instead.

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
    let response = app.oneshot(request).await.expect("projects oneshot");
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

async fn direct_count(pool: &MySqlPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(pool)
        .await
        .expect("count re-reads")
}

#[sqlx::test(migrations = "./migrations")]
async fn projects_crud_full_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every verb (AuthSession guard).
    for (method, path, body) in [
        ("GET", "/api/v1/projects", None),
        (
            "POST",
            "/api/v1/projects",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        (
            "PATCH",
            "/api/v1/projects/1",
            Some(json_body(&serde_json::json!({"name": "Nope"}))),
        ),
        ("DELETE", "/api/v1/projects/1", None),
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

    // 1. Register owner + intruder (distinct jars, real hashes).
    let (owner_jar, owner_id) = register(app(), "proj-owner@example.com").await;
    let (intruder_jar, intruder_id) = register(app(), "proj-intruder@example.com").await;
    assert_ne!(owner_id, intruder_id);

    // 2. Empty list is a real `Page` (never bare `data.length` shape).
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/projects", Some(&owner_jar), None),
    )
    .await;
    let empty = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty list: {empty}");
    assert_eq!(
        empty,
        serde_json::json!({"data": [], "page": 1, "limit": 50, "total": 0}),
        "empty Page shape"
    );

    // 3. `POST {name} → 201 + Location`; DB re-read finds the row.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Alpha"}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create Alpha: {created}");
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
    let alpha_id = created["data"]["id"].as_i64().expect("create returns id");
    assert_eq!(location, format!("/api/v1/projects/{alpha_id}"));
    assert_eq!(created["data"]["name"], "Alpha");
    assert_eq!(created["data"]["board_count"], 0);
    assert!(
        created["data"]["created_at"].is_string(),
        "timestamps: {created}"
    );
    assert!(
        created["data"]["updated_at"].is_string(),
        "timestamps: {created}"
    );
    let stored: (String, i64) =
        sqlx::query_as("SELECT `name`, `user_id` FROM `projects` WHERE `id` = ?")
            .bind(alpha_id)
            .fetch_one(&pool)
            .await
            .expect("created project re-reads");
    assert_eq!(stored, ("Alpha".to_string(), owner_id));

    // 4. Validation: blank / overlong → 400 `errors[]`; non-string /
    // missing `name` are body-shape 400s (serde rejects first).
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
                "/api/v1/projects",
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
    // Non-string `name` is a body-shape 400 (serde rejects first).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": 123}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "non-string name: {failure}"
    );
    assert_problem(status, &headers, &failure, "invalid-json");
    // Missing `name` on create is a body-shape 400 (serde rejects first).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing name: {failure}");
    assert_problem(status, &headers, &failure, "invalid-json");
    // Unknown fields are ignored (Nest parity): still 201.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Beta", "bogus": 1}))),
        ),
    )
    .await;
    let beta = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unknown fields ignored: {beta}"
    );
    let _beta_id = beta["data"]["id"].as_i64().expect("Beta id");
    // Quote payload is stored literally (injection regression).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
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
        sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `name` = 'O''Brien'")
            .fetch_one(&pool)
            .await
            .expect("quote payload re-reads literally");
    assert_eq!(quote_name, "O'Brien");
    let quote_id: i64 = sqlx::query_scalar("SELECT `id` FROM `projects` WHERE `name` = 'O''Brien'")
        .fetch_one(&pool)
        .await
        .expect("quote id re-reads");

    // 5. More projects (Gamma + `100% Real` for LIKE-escaping) + boards via
    //    SQL (no boards API until T21a): Alpha ×2, Beta ×0, Gamma ×1,
    //    standalone board (NULL project) ×1, intruder project ×1 + board ×1.
    for name in ["Gamma", "100% Real"] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/projects",
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"name": name}))),
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "create {name}: {}",
            json_of(&bytes)
        );
    }
    let gamma_id: i64 = sqlx::query_scalar("SELECT `id` FROM `projects` WHERE `name` = 'Gamma'")
        .fetch_one(&pool)
        .await
        .expect("Gamma id re-reads");
    let foreign_id: i64 = {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/projects",
                Some(&intruder_jar),
                Some(json_body(&serde_json::json!({"name": "Foreign"}))),
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "intruder create: {}",
            json_of(&bytes)
        );
        json_of(&bytes)["data"]["id"].as_i64().expect("Foreign id")
    };
    for (board, project) in [
        ("A-One", Some(alpha_id)),
        ("A-Two", Some(alpha_id)),
        ("G-One", Some(gamma_id)),
        ("Solo", None),
        ("F-One", Some(foreign_id)),
    ] {
        sqlx::query("INSERT INTO `boards` (`name`, `user_id`, `project_id`) VALUES (?, ?, ?)")
            .bind(board)
            .bind(if project == Some(foreign_id) {
                intruder_id
            } else {
                owner_id
            })
            .bind(project)
            .execute(&pool)
            .await
            .expect("board seeds");
    }
    // Column + card under an Alpha board: deep-cascade oracle for DELETE.
    let alpha_board_id: i64 =
        sqlx::query_scalar("SELECT `id` FROM `boards` WHERE `name` = 'A-One'")
            .fetch_one(&pool)
            .await
            .expect("Alpha board re-reads");
    sqlx::query("INSERT INTO `columns` (`name`, `position`, `board_id`) VALUES ('Col', 0, ?)")
        .bind(alpha_board_id)
        .execute(&pool)
        .await
        .expect("column seeds");
    let alpha_column_id: i64 =
        sqlx::query_scalar("SELECT `id` FROM `columns` WHERE `board_id` = ?")
            .bind(alpha_board_id)
            .fetch_one(&pool)
            .await
            .expect("column re-reads");
    sqlx::query("INSERT INTO `cards` (`title`, `column_id`, `position`) VALUES ('Card', ?, 0)")
        .bind(alpha_column_id)
        .execute(&pool)
        .await
        .expect("card seeds");

    // 6. List carries REAL `board_count` (standalone board counts nowhere;
    //    intruder project invisible to the owner).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/projects?sort=name&order=asc",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let listed = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list: {listed}");
    let counts: Vec<(String, i64)> = listed["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|item| {
            (
                item["name"].as_str().expect("name").to_owned(),
                item["board_count"].as_i64().expect("board_count"),
            )
        })
        .collect();
    assert_eq!(
        counts,
        vec![
            ("100% Real".to_string(), 0),
            ("Alpha".to_string(), 2),
            ("Beta".to_string(), 0),
            ("Gamma".to_string(), 1),
            ("O'Brien".to_string(), 0),
        ],
        "board_count per project (owner scope only)"
    );
    assert_eq!(listed["page"], 1);
    assert_eq!(listed["limit"], 50);
    assert_eq!(listed["total"], 5);
    // Deprecated `/api/*` alias serves the same handler.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/projects", Some(&owner_jar), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alias list: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["total"], 5);

    // 7. `?q` filters by name (LIKE, case-insensitive collation).
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/projects?q=alp", Some(&owner_jar), None),
    )
    .await;
    let filtered = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "q filter: {filtered}");
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["data"][0]["name"], "Alpha");
    // `%` in `q` is escaped (literal), not a wildcard: only `100% Real`.
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/projects?q=100%25", Some(&owner_jar), None),
    )
    .await;
    let escaped = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "escaped q: {escaped}");
    assert_eq!(escaped["total"], 1, "`%` must not act as a wildcard");
    assert_eq!(escaped["data"][0]["name"], "100% Real");
    // `'` in `q` is bound, never interpolated.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/projects?q=O%27Brien",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let quoted = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "quote q: {quoted}");
    assert_eq!(quoted["total"], 1);
    assert_eq!(quoted["data"][0]["name"], "O'Brien");

    // 8. `?sort&order`: name asc/desc + created_at default; allowlist
    //    violations (incl. SQL fragments) → 400, never interpolated.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/projects?sort=name&order=desc",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let desc = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "sort desc: {desc}");
    let names: Vec<&str> = desc["data"]
        .as_array()
        .expect("data array")
        .iter()
        .map(|item| item["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec!["O'Brien", "Gamma", "Beta", "Alpha", "100% Real"]
    );
    for query in [
        "sort=evil",
        "sort=name%3BDROP+TABLE+projects",
        "order=sideways",
        "order=name",
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/projects?{query}"),
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

    // 9. Strict pagination: slices + total stable; out-of-range → 400.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/projects?sort=name&order=asc&limit=2&page=2",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let page2 = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page 2: {page2}");
    assert_eq!(page2["page"], 2);
    assert_eq!(page2["limit"], 2);
    assert_eq!(page2["total"], 5);
    assert_eq!(page2["data"].as_array().expect("array").len(), 2);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/projects?limit=2&page=9",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let beyond = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page beyond: {beyond}");
    assert_eq!(beyond["data"], serde_json::json!([]));
    assert_eq!(beyond["total"], 5);
    for query in ["page=0", "limit=0", "limit=101", "page=abc", "limit=1.5"] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/projects?{query}"),
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

    // 10. Query-count proof (store level, dedicated connection): count + page
    //     = exactly 2 statements for ANY P. `SHOW SESSION STATUS` is
    //     connection-scoped, so the delta is attributable; the calibration
    //     pair proves the SHOW pair itself contributes exactly 1.
    {
        use backend_rs::features::projects::store::{count_projects, fetch_project_page};
        let mut conn = pool.acquire().await.expect("dedicated connection");
        async fn questions(conn: &mut sqlx::MySqlConnection) -> u64 {
            let row: (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Questions'")
                .fetch_one(&mut *conn)
                .await
                .expect("Questions reads");
            row.1.parse().expect("Questions is numeric")
        }
        let baseline = questions(&mut conn).await;
        let calibrated = questions(&mut conn).await;
        assert_eq!(
            calibrated - baseline,
            1,
            "SHOW pair contributes exactly 1 (calibration)"
        );
        let before = questions(&mut conn).await;
        let total = count_projects(&mut *conn, owner_id as i32, None)
            .await
            .expect("count runs");
        assert_eq!(total, 5);
        let rows = fetch_project_page(
            &mut *conn,
            owner_id as i32,
            None,
            "created_at",
            "desc",
            1,
            50,
        )
        .await
        .expect("page runs");
        assert_eq!(rows.len(), 5, "page returns all P rows in one fetch");
        let after = questions(&mut conn).await;
        assert_eq!(
            after - before - 1,
            2,
            "list must issue exactly 2 statements regardless of P (no N+1)"
        );
    }

    // 11. `PATCH {name} → 200`; DB re-read confirms; same-name stays 200.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/projects/{alpha_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Alpha Renamed"}))),
        ),
    )
    .await;
    let updated = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch: {updated}");
    assert_eq!(updated["data"]["id"], alpha_id);
    assert_eq!(updated["data"]["name"], "Alpha Renamed");
    assert_eq!(updated["data"]["board_count"], 2, "count survives update");
    let renamed: String = sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `id` = ?")
        .bind(alpha_id)
        .fetch_one(&pool)
        .await
        .expect("patched name re-reads");
    assert_eq!(renamed, "Alpha Renamed");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/projects/{alpha_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Alpha Renamed"}))),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "same-name patch: {}",
        json_of(&bytes)
    );
    // Empty / missing name → 400 with `errors[]`.
    for (payload, what) in [
        (serde_json::json!({"name": ""}), "empty patch name"),
        (serde_json::json!({"name": "   "}), "blank patch name"),
        (serde_json::json!({}), "missing patch name"),
        (
            serde_json::json!({"name": None::<String>}),
            "null patch name",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/projects/{alpha_id}"),
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
    // Foreign / missing id → 404 (no oracle, NEVER 403); garbage → 400.
    for (id, what) in [
        (foreign_id.to_string(), "foreign patch"),
        ("999999".to_string(), "missing patch"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/projects/{id}"),
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
            "/api/v1/projects/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");
    // Failed hijack left the foreign row untouched (DB re-read).
    let foreign_name: String = sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `id` = ?")
        .bind(foreign_id)
        .fetch_one(&pool)
        .await
        .expect("foreign row survives hijack");
    assert_eq!(foreign_name, "Foreign");

    // 12. `DELETE → 204` empty body; boards (+ columns/cards) cascade;
    //     unrelated rows survive.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/projects/{alpha_id}"),
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
    assert_eq!(
        direct_count(
            &pool,
            &format!("SELECT COUNT(*) FROM `projects` WHERE `id` = {alpha_id}")
        )
        .await,
        0,
        "project row gone"
    );
    assert_eq!(
        direct_count(
            &pool,
            &format!("SELECT COUNT(*) FROM `boards` WHERE `project_id` = {alpha_id}")
        )
        .await,
        0,
        "project boards cascade-deleted"
    );
    assert_eq!(
        direct_count(
            &pool,
            &format!("SELECT COUNT(*) FROM `columns` WHERE `board_id` = {alpha_board_id}")
        )
        .await,
        0,
        "board columns cascade-deleted"
    );
    assert_eq!(
        direct_count(
            &pool,
            &format!("SELECT COUNT(*) FROM `cards` WHERE `column_id` = {alpha_column_id}")
        )
        .await,
        0,
        "column cards cascade-deleted"
    );
    assert_eq!(
        direct_count(
            &pool,
            "SELECT COUNT(*) FROM `boards` WHERE `project_id` IS NULL"
        )
        .await,
        1,
        "standalone board survives"
    );
    assert_eq!(
        direct_count(
            &pool,
            &format!("SELECT COUNT(*) FROM `boards` WHERE `project_id` = {foreign_id}")
        )
        .await,
        1,
        "intruder board survives"
    );
    // Repeat delete → 404; foreign delete → 404 (row intact); garbage → 400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/projects/{alpha_id}"),
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
            &format!("/api/v1/projects/{foreign_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign delete: {failure}");
    assert_problem(status, &headers, &failure, "not-found");
    assert_ne!(
        headers
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap_or("?")),
        None,
        "404 delete keeps problem+json Content-Type"
    );
    assert_eq!(
        direct_count(
            &pool,
            &format!("SELECT COUNT(*) FROM `projects` WHERE `id` = {foreign_id}")
        )
        .await,
        1,
        "foreign project survives delete attempt"
    );
    let (status, headers, bytes) = send(
        app(),
        authed_request("DELETE", "/api/v1/projects/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage delete: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");

    // 13. Cross-user isolation: intruder sees only `Foreign`; owner list
    //     shrank by the deleted Alpha (Beta, Gamma, O'Brien, 100% Real left).
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/projects", Some(&intruder_jar), None),
    )
    .await;
    let intruder_list = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "intruder list: {intruder_list}");
    assert_eq!(intruder_list["total"], 1);
    assert_eq!(intruder_list["data"][0]["name"], "Foreign");
    assert_eq!(intruder_list["data"][0]["board_count"], 1);
    let (status, _, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/projects", Some(&owner_jar), None),
    )
    .await;
    let owner_list = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "owner list: {owner_list}");
    assert_eq!(
        owner_list["total"], 4,
        "Alpha gone, quote row still present"
    );

    // 14. Quote-named project deletes cleanly too (bound params everywhere).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/projects/{quote_id}"),
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

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
