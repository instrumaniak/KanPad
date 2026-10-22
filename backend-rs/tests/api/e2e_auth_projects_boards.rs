//! T50a black-box e2e: auth → projects → boards slice vs the Rust server.
//!
//! Unlike every other suite in `tests/api/` (in-process `oneshot`), this
//! suite is BLACK-BOX: it spawns the real app with `axum::serve` on a
//! loopback port and drives it over TCP with a hand-rolled HTTP/1.1 client
//! (no new dev-dep: `Cargo.toml` is outside T50a touch scope). Full middleware
//! stack is exercised (`served-by: rust`, `x-request-id`, governor
//! `route_layer` with `ConnectInfo` peer keys, session cookies, compression
//! bypass via no `Accept-Encoding`).
//!
//! The port is OS-ASSIGNED (ephemeral) by default — see
//! `super::wire::bind_blackbox` — so this suite and `e2e_rest` can run in
//! PARALLEL in the same test binary without an `AddrInUse` clash. Set
//! `E2E_BASE_URL` to pin a fixed port instead.
//!
//! Per-target expectation params (`20-api-contract.md` + `21-api-standard-
//! shapes.md` diverge, so one assertion set cannot pass on both):
//!
//! | Aspect | Rust (exact) | Nest (behavior-class only) |
//! |---|---|---|
//! | errors | RFC9457 `application/problem+json` + exact `type` URI + `instance` | status class + `statusCode`/`message` present |
//! | lists | exact `Page{data,page,limit,total}` | `data` array contains the row |
//! | deletes/logout | `204` empty body | `200` + `message` |
//! | keys | `snake_case` (`board_count`, `project_id`) | `camelCase` tolerated |
//! | active-board delete | `409` state conflict | `200` (verb-route flow) |
//! | verb routes (`/:id/archive|/restore|/permanent`, `/boards/archived`) | `404`/`400` deleted-route table | `200` behavior present |
//! | register/login strictness | `400 errors[]` on weak input | behavior class only (Nest had no pipes on auth) |
//!
//! `E2E_TARGET=nest` selects the Nest column (needs a live Nest server; not
//! reachable in CI — the Rust column is the gate). Default is `rust`.
//!
//! Coverage (Nest `backend/test/auth|projects|boards.e2e-spec.ts` slice,
//! rewritten for the new contract): register/login/logout/me, projects CRUD +
//! real `board_count`, boards create + detail/list + filters/state + delete
//! incl. `409` active-delete + deleted-verb-route `404`s. Every mutation has
//! a DB re-read via a second GET or a direct bound-parameter `SELECT`
//! (never `format!`-interpolated SQL; no session wipe — the `sessions` store
//! is per-test-database, assertions use id-set difference only).
//!
//! Throttle context (`10-architecture.md`): the spawned server raises the
//! governor ceilings (`global/auth 10000/min`, mirroring the per-test-router
//! approach of `throttle.rs`) so the auth-heavy flow stays deterministic;
//! 429 coverage itself stays in T30. The suite still proves the live throttle
//! path where it is safe: repeated `POST logout` never `429`s (logout is
//! unthrottled by design).

use std::net::SocketAddr;

use backend_rs::config::Settings;
use serde_json::Value;
use sqlx::MySqlPool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::helpers::{
    assert_errors_nonempty as assert_errors_nonempty_fields, assert_no_legacy_keys,
};
use super::wire::{WireResponse, assert_problem, parse_response};

// ---------------------------------------------------------------------------
// Per-target expectation params.
// ---------------------------------------------------------------------------

/// Black-box target under test. `Rust` asserts the exact rewrite contract;
/// `Nest` asserts behavior classes only (envelopes, statuses, and routes
/// diverge per `21-api-standard-shapes.md`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Rust,
    Nest,
}

impl Target {
    fn current() -> Self {
        if crate::helpers::e2e_target_is_nest() {
            Self::Nest
        } else {
            Self::Rust
        }
    }

    fn is_rust(self) -> bool {
        self == Self::Rust
    }

    /// Expected status for hard deletes (projects, boards) and logout:
    /// rewrite `204` (no body) vs Nest `200` + `message`.
    fn delete_status(self) -> u16 {
        if self.is_rust() { 204 } else { 200 }
    }
}

// ---------------------------------------------------------------------------
// Minimal HTTP/1.1 black-box client over TCP (hand-rolled: no new dev-dep).
// ---------------------------------------------------------------------------

/// Cookie jar with manual `Set-Cookie` propagation (mirrors the
/// `helpers::JarClient` contract, over the wire).
#[derive(Default)]
struct BbClient {
    host: String,
    port: u16,
    jar: Vec<(String, String)>,
}

impl BbClient {
    fn cookie_header(&self) -> Option<String> {
        if self.jar.is_empty() {
            None
        } else {
            Some(
                self.jar
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        }
    }

    fn store_cookies(&mut self, response: &WireResponse) {
        if let Some(values) = response.headers.get("set-cookie") {
            for raw in values {
                let pair = raw.split(';').next().unwrap_or("").trim();
                let Some((name, value)) = pair.split_once('=') else {
                    continue;
                };
                let name = name.trim().to_string();
                let value = value.trim().to_string();
                self.jar.retain(|(existing, _)| *existing != name);
                if !value.is_empty() {
                    self.jar.push((name, value));
                }
            }
        }
    }

    async fn try_request(
        &mut self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Option<WireResponse> {
        let payload = body.unwrap_or("");
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n",
            self.host, self.port
        );
        if let Some(cookies) = self.cookie_header() {
            head.push_str(&format!("Cookie: {cookies}\r\n"));
        }
        if body.is_some() {
            head.push_str("Content-Type: application/json\r\n");
            head.push_str(&format!("Content-Length: {}\r\n", payload.len()));
        }
        head.push_str("\r\n");

        let mut stream = tokio::net::TcpStream::connect((self.host.as_str(), self.port))
            .await
            .ok()?;
        stream.write_all(head.as_bytes()).await.ok()?;
        stream.write_all(payload.as_bytes()).await.ok()?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.ok()?;
        if raw.is_empty() {
            return None;
        }
        let response = parse_response(&raw, method, path);
        self.store_cookies(&response);
        Some(response)
    }

    async fn request(&mut self, method: &str, path: &str, body: Option<&str>) -> WireResponse {
        self.try_request(method, path, body)
            .await
            .unwrap_or_else(|| panic!("black-box {method} {path}: no HTTP response"))
    }
}

// ---------------------------------------------------------------------------
// Assertion helpers (target-aware).
// ---------------------------------------------------------------------------

/// Every response carries the transport proof: `served-by: rust` +
/// `x-request-id` (middleware context — outermost layers cover 4xx too).
fn assert_transport(response: &WireResponse, what: &str) {
    assert_eq!(
        response.header("served-by"),
        Some("rust"),
        "{what}: served-by must be rust"
    );
    assert!(
        response
            .header("x-request-id")
            .is_some_and(|value| !value.is_empty()),
        "{what}: x-request-id must be echoed"
    );
}

/// Nest behavior-class error assertion: status class + legacy shape only.
fn assert_nest_error(response: &WireResponse, body: &Value, what: &str) {
    assert!(
        (400..600).contains(&response.status),
        "{what}: Nest error must be 4xx/5xx, got {}: {body}",
        response.status
    );
    assert!(
        body.get("statusCode").is_some() || body.get("message").is_some(),
        "{what}: Nest error shape: {body}"
    );
}

fn assert_error(target: Target, response: &WireResponse, body: &Value, slug: &str, what: &str) {
    if target.is_rust() {
        assert_problem(response, body, slug, what);
    } else {
        assert_nest_error(response, body, what);
    }
}

/// Validation-class errors carry a non-empty `errors[]` on the Rust target
/// only (the Nest target has no such field).
///
/// A SANCTIONED EXCEPTION to the `super::helpers` ownership header: a
/// target-aware adapter over the shared [`assert_errors_nonempty_fields`]
/// core (imported under that alias). The Rust column asserts the shared
/// core; the Nest column has no such field, so there is nothing to assert.
fn assert_errors_nonempty(target: Target, body: &Value, what: &str) {
    if target.is_rust() {
        assert_errors_nonempty_fields(body, what);
    }
}

/// Live ids of the session store in THIS test's own database — compare
/// SETS, never absolute counts (parallel suites mint rows concurrently;
/// only this suite's login/logout changes its own delta).
async fn session_ids(pool: &MySqlPool) -> Vec<String> {
    sqlx::query_scalar("SELECT `id` FROM `sessions`")
        .fetch_all(pool)
        .await
        .expect("session ids read")
}

// ---------------------------------------------------------------------------
// Server spawn (real TCP on an OS-assigned loopback port).
// ---------------------------------------------------------------------------

async fn spawn_blackbox(pool: &MySqlPool) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    // Raised governor ceilings keep the auth-heavy flow deterministic
    // (same per-test-router rationale as `throttle.rs`); 429 coverage
    // itself stays in T30.
    let settings = Settings {
        throttle_global_per_min: 10_000,
        throttle_auth_per_min: 10_000,
        ..Default::default()
    };
    let app = crate::helpers::app_on_pool_with(pool, settings);
    // Ephemeral port by default: this suite and `e2e_rest` run in parallel
    // in the same test binary, so a fixed port would make one fail.
    let (listener, addr) = super::wire::bind_blackbox().await;
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("black-box serve runs");
    });
    let mut probe = BbClient {
        host: addr.ip().to_string(),
        port: addr.port(),
        jar: Vec::new(),
    };
    for _ in 0..100 {
        if let Some(response) = probe.try_request("GET", "/health", None).await
            && response.status == 200
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let response = probe.request("GET", "/health", None).await;
    assert_eq!(response.status, 200, "black-box target is up");
    (addr, handle)
}

// ---------------------------------------------------------------------------
// The slice: auth → projects → boards.
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn e2e_auth_projects_boards_blackbox(pool: MySqlPool) {
    let target = Target::current();
    let (addr, server) = spawn_blackbox(&pool).await;
    let mut anon = BbClient {
        host: addr.ip().to_string(),
        port: addr.port(),
        jar: Vec::new(),
    };
    let mut owner = BbClient {
        host: addr.ip().to_string(),
        port: addr.port(),
        jar: Vec::new(),
    };

    // 0. Liveness + readiness identify the target over the wire.
    let response = anon.request("GET", "/health", None).await;
    assert_transport(&response, "GET /health");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.json("GET /health"),
        serde_json::json!({"status": "ok"})
    );
    let response = anon.request("GET", "/ready", None).await;
    assert_transport(&response, "GET /ready");
    assert_eq!(
        response.status,
        200,
        "readiness: {}",
        response.json("GET /ready")
    );

    // 1. Anonymous guards: every verb 401s without a session.
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/auth/login",
            Some(r#"{"email":"a@b.co","password":"Str0ng!pass"}"#),
        ),
        ("GET", "/api/v1/auth/me", None),
        ("GET", "/api/v1/projects", None),
        ("POST", "/api/v1/projects", Some(r#"{"name":"Nope"}"#)),
        ("PATCH", "/api/v1/projects/1", Some(r#"{"name":"Nope"}"#)),
        ("DELETE", "/api/v1/projects/1", None),
        ("GET", "/api/v1/boards", None),
        ("POST", "/api/v1/boards", Some(r#"{"name":"Nope"}"#)),
        ("GET", "/api/v1/boards/1", None),
        ("PATCH", "/api/v1/boards/1", Some(r#"{"name":"Nope"}"#)),
        ("DELETE", "/api/v1/boards/1", None),
    ] {
        let response = anon.request(method, path, body).await;
        assert_transport(&response, &format!("anon {method} {path}"));
        let failure = response.json(&format!("anon {method} {path}"));
        assert_eq!(response.status, 401, "{method} {path}: {failure}");
        assert_error(
            target,
            &response,
            &failure,
            "unauthorized",
            &format!("{method} {path}"),
        );
    }

    // 2. Register → 201 {data:{id,email,role}} + cookie + Location.
    let before_sessions = session_ids(&pool).await;
    let response = owner
        .request(
            "POST",
            "/api/v1/auth/register",
            Some(r#"{"email":"e2e-owner@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    assert_transport(&response, "register");
    let registered = response.json("register");
    assert_eq!(response.status, 201, "register: {registered}");
    let owner_id = registered["data"]["id"]
        .as_i64()
        .expect("register returns id");
    assert!(owner_id > 0);
    assert_eq!(registered["data"]["email"], "e2e-owner@example.com");
    assert_eq!(registered["data"]["role"], "user");
    if target.is_rust() {
        assert_no_legacy_keys(&registered, "register");
        assert_eq!(response.header("location"), Some("/api/v1/auth/me"));
        assert!(!owner.jar.is_empty(), "register sets a session cookie");
    }
    // DB re-read (Rust target only — the `#[sqlx::test]` pool is the Rust
    // DB; a Nest target would persist to its own database): user row +
    // exactly 6 seeded labels + Argon2id hash.
    if target.is_rust() {
        let stored: (String, String) =
            sqlx::query_as("SELECT `email`, `password` FROM `users` WHERE `id` = ?")
                .bind(owner_id)
                .fetch_one(&pool)
                .await
                .expect("registered user re-reads");
        assert_eq!(stored.0, "e2e-owner@example.com");
        assert!(
            stored.1.starts_with("$argon2id$v=19$m=19456,"),
            "Argon2id hash, got prefix: {}",
            stored.1.chars().take(30).collect::<String>()
        );
        let labels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
            .bind(owner_id)
            .fetch_one(&pool)
            .await
            .expect("seeded labels re-read");
        assert_eq!(labels, 6, "register seeds exactly 6 labels");
        // Register mints a session (set difference — never an absolute count).
        let after_register = session_ids(&pool).await;
        assert_eq!(
            after_register.len(),
            before_sessions.len() + 1,
            "register mints exactly one session row"
        );
    }

    // 3. Duplicate register → 409; row count unchanged (DB re-read).
    let response = anon
        .request(
            "POST",
            "/api/v1/auth/register",
            Some(r#"{"email":"e2e-owner@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    assert_transport(&response, "dup register");
    let failure = response.json("dup register");
    assert_eq!(response.status, 409, "dup register: {failure}");
    assert_error(target, &response, &failure, "conflict", "dup register");
    if target.is_rust() {
        let dupes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `users` WHERE `email` = ?")
            .bind("e2e-owner@example.com")
            .fetch_one(&pool)
            .await
            .expect("dup re-read");
        assert_eq!(dupes, 1, "duplicate register inserts no row");
    }

    // 4. Weak register input → Rust 400 `errors[]` (new strictness — Nest had
    //    no pipes on auth, so Nest asserts the behavior class only).
    for (payload, what) in [
        (
            r#"{"email":"not-an-email","password":"Str0ng!pass"}"#,
            "bad email",
        ),
        (
            r#"{"email":"weak@example.com","password":"short"}"#,
            "weak password",
        ),
    ] {
        let response = anon
            .request("POST", "/api/v1/auth/register", Some(payload))
            .await;
        assert_transport(&response, &format!("register {what}"));
        let failure = response.json(&format!("register {what}"));
        if target.is_rust() {
            assert_eq!(response.status, 400, "{what}: {failure}");
            assert_problem(&response, &failure, "validation-error", what);
            assert_errors_nonempty(target, &failure, what);
        } else {
            assert!(
                (400..500).contains(&response.status),
                "{what}: Nest behavior 4xx: {failure}"
            );
        }
    }

    // 5. Login → 200 + session rotation (pre-cookie != post-cookie).
    let pre_cookie = owner.cookie_header().expect("pre-login jar");
    let before_login = session_ids(&pool).await;
    let response = owner
        .request(
            "POST",
            "/api/v1/auth/login",
            Some(r#"{"email":"e2e-owner@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    assert_transport(&response, "login");
    let logged_in = response.json("login");
    assert_eq!(response.status, 200, "login: {logged_in}");
    assert_eq!(logged_in["data"]["email"], "e2e-owner@example.com");
    if target.is_rust() {
        assert_no_legacy_keys(&logged_in, "login");
    }
    let post_cookie = owner.cookie_header().expect("post-login jar");
    if target.is_rust() {
        assert_ne!(post_cookie, pre_cookie, "login rotates the session id");
        let after_login = session_ids(&pool).await;
        assert!(
            after_login.len() >= before_login.len(),
            "login keeps session rows (rotation, no leak assert beyond non-shrink)"
        );
    }

    // 6. Wrong password vs unknown email: identical 401 shape (no oracle).
    let response = anon
        .request(
            "POST",
            "/api/v1/auth/login",
            Some(r#"{"email":"e2e-owner@example.com","password":"Wr0ng!pass"}"#),
        )
        .await;
    let bad_body = response.json("bad login");
    assert_eq!(response.status, 401, "bad login: {bad_body}");
    assert_error(target, &response, &bad_body, "unauthorized", "bad login");
    let response = anon
        .request(
            "POST",
            "/api/v1/auth/login",
            Some(r#"{"email":"nobody@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    let unknown_body = response.json("unknown login");
    assert_eq!(response.status, 401, "unknown login: {unknown_body}");
    assert_error(
        target,
        &response,
        &unknown_body,
        "unauthorized",
        "unknown login",
    );
    if target.is_rust() {
        for key in ["type", "title", "status"] {
            assert_eq!(
                bad_body[key], unknown_body[key],
                "no enumeration oracle ({key})"
            );
        }
    }

    // 7. `me` with the login jar → 200; anonymous → 401.
    let response = owner.request("GET", "/api/v1/auth/me", None).await;
    assert_transport(&response, "me");
    let me = response.json("me");
    assert_eq!(response.status, 200, "me: {me}");
    assert_eq!(me["data"]["email"], "e2e-owner@example.com");
    assert_eq!(me["data"]["role"], "user");
    let response = anon.request("GET", "/api/v1/auth/me", None).await;
    let anon_me = response.json("anon me");
    assert_eq!(response.status, 401, "anon me: {anon_me}");
    assert_error(target, &response, &anon_me, "unauthorized", "anon me");

    // 8. Deprecated `/api/*` alias serves the same handlers.
    let response = anon
        .request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"e2e-owner@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    assert_transport(&response, "alias login");
    assert_eq!(
        response.status,
        200,
        "alias login: {}",
        response.json("alias login")
    );
    let response = anon.request("GET", "/api/auth/me", None).await;
    assert_eq!(
        response.status,
        200,
        "alias me: {}",
        response.json("alias me")
    );

    // 9. Projects: POST Alpha → 201 + Location; DB re-read finds the row.
    let response = owner
        .request("POST", "/api/v1/projects", Some(r#"{"name":"Alpha"}"#))
        .await;
    assert_transport(&response, "create Alpha");
    let created = response.json("create Alpha");
    assert_eq!(response.status, 201, "create Alpha: {created}");
    let alpha_id = created["data"]["id"].as_i64().expect("project id");
    if target.is_rust() {
        assert_no_legacy_keys(&created, "create project");
        assert_eq!(
            response.header("location"),
            Some(format!("/api/v1/projects/{alpha_id}").as_str())
        );
        assert_eq!(created["data"]["board_count"], 0);
        let stored: (String, i64) =
            sqlx::query_as("SELECT `name`, `user_id` FROM `projects` WHERE `id` = ?")
                .bind(alpha_id)
                .fetch_one(&pool)
                .await
                .expect("created project re-reads");
        assert_eq!(stored.0, "Alpha");
        assert_eq!(stored.1, owner_id);
    }

    // 10. More projects: Beta + quote payload stored literally.
    for name in [r#"{"name":"Beta"}"#, r#"{"name":"O'Brien"}"#] {
        let response = owner.request("POST", "/api/v1/projects", Some(name)).await;
        assert_eq!(
            response.status,
            201,
            "create {name}: {}",
            response.json("create")
        );
    }
    if target.is_rust() {
        let quote_name: String =
            sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `name` = ?")
                .bind("O'Brien")
                .fetch_one(&pool)
                .await
                .expect("quote payload re-reads literally");
        assert_eq!(quote_name, "O'Brien");
    }

    // 11. Empty project name → Rust 400 `errors[]` (Nest pipes: 400 too —
    //     projects mutations were piped; shape still diverges).
    let response = owner
        .request("POST", "/api/v1/projects", Some(r#"{"name":""}"#))
        .await;
    let failure = response.json("empty project");
    assert_eq!(response.status, 400, "empty project: {failure}");
    assert_error(
        target,
        &response,
        &failure,
        "validation-error",
        "empty project",
    );
    assert_errors_nonempty(target, &failure, "empty project");

    // 12. Boards: create linked to Alpha → 201 + 3 default columns (one tx).
    let board_payload = if target.is_rust() {
        format!(r#"{{"name":"Board One","project_id":{alpha_id}}}"#)
    } else {
        // Nest behavior-class path uses the legacy camelCase key.
        format!(r#"{{"name":"Board One","projectId":{alpha_id}}}"#)
    };
    let response = owner
        .request("POST", "/api/v1/boards", Some(&board_payload))
        .await;
    assert_transport(&response, "create board");
    let board_one = response.json("create board");
    assert_eq!(response.status, 201, "create board: {board_one}");
    let board_one_id = board_one["data"]["id"].as_i64().expect("board id");
    if target.is_rust() {
        assert_no_legacy_keys(&board_one, "create board");
        assert_eq!(
            response.header("location"),
            Some(format!("/api/v1/boards/{board_one_id}").as_str())
        );
        let columns: Vec<(String, i64)> = board_one["data"]["columns"]
            .as_array()
            .expect("create nests columns")
            .iter()
            .map(|column| {
                (
                    column["name"].as_str().expect("column name").to_string(),
                    column["position"].as_i64().expect("column position"),
                )
            })
            .collect();
        assert_eq!(
            columns,
            vec![
                ("To Do".to_string(), 0),
                ("In Progress".to_string(), 1),
                ("Done".to_string(), 2),
            ]
        );
    } else {
        assert_eq!(
            board_one["data"]["columns"]
                .as_array()
                .expect("columns")
                .len(),
            3
        );
    }
    // DB re-read: board row + exactly 3 dense columns (Rust target only).
    if target.is_rust() {
        let column_rows: Vec<(String, i32)> = sqlx::query_as(
            "SELECT `name`, `position` FROM `columns` WHERE `board_id` = ? ORDER BY `position` ASC",
        )
        .bind(board_one_id)
        .fetch_all(&pool)
        .await
        .expect("created columns re-read");
        assert_eq!(column_rows.len(), 3, "tx created exactly 3 columns");
        assert_eq!(
            column_rows.iter().map(|row| row.1).collect::<Vec<_>>(),
            vec![0, 1, 2],
            "columns dense 0..N"
        );
    }

    // Second board in Alpha + one standalone board.
    let response = owner
        .request(
            "POST",
            "/api/v1/boards",
            Some(&format!(
                r#"{{"name":"Board Two","project_id":{alpha_id}}}"#
            )),
        )
        .await;
    assert_eq!(
        response.status,
        201,
        "create Board Two: {}",
        response.json("create")
    );
    let board_two_id = response.json("create")["data"]["id"]
        .as_i64()
        .expect("board id");
    let response = owner
        .request("POST", "/api/v1/boards", Some(r#"{"name":"Solo"}"#))
        .await;
    assert_eq!(
        response.status,
        201,
        "create Solo: {}",
        response.json("create")
    );
    let solo_id = response.json("create")["data"]["id"]
        .as_i64()
        .expect("board id");

    // 13. Projects list carries REAL `board_count` (Rust `Page`; Nest
    //     behavior: `data` contains the rows, `boardCount` stub 0).
    let response = owner
        .request("GET", "/api/v1/projects?sort=name&order=asc", None)
        .await;
    assert_transport(&response, "projects list");
    let listed = response.json("projects list");
    assert_eq!(response.status, 200, "projects list: {listed}");
    if target.is_rust() {
        for key in ["data", "page", "limit", "total"] {
            assert!(listed.get(key).is_some(), "Page carries `{key}`: {listed}");
        }
        assert_eq!(listed["total"], 3);
        let counts: Vec<(String, i64)> = listed["data"]
            .as_array()
            .expect("data array")
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().expect("name").to_string(),
                    item["board_count"].as_i64().expect("board_count"),
                )
            })
            .collect();
        assert_eq!(
            counts,
            vec![
                ("Alpha".to_string(), 2),
                ("Beta".to_string(), 0),
                ("O'Brien".to_string(), 0),
            ]
        );
    } else {
        let names: Vec<&str> = listed["data"]
            .as_array()
            .expect("data array")
            .iter()
            .map(|item| item["name"].as_str().expect("name"))
            .collect();
        assert!(
            names.contains(&"Alpha"),
            "Nest list contains Alpha: {listed}"
        );
    }

    // 14. PATCH Alpha rename → 200; second GET confirms (no echo).
    let response = owner
        .request(
            "PATCH",
            &format!("/api/v1/projects/{alpha_id}"),
            Some(r#"{"name":"Alpha Renamed"}"#),
        )
        .await;
    let updated = response.json("patch project");
    assert_eq!(response.status, 200, "patch project: {updated}");
    assert_eq!(updated["data"]["name"], "Alpha Renamed");
    if target.is_rust() {
        let renamed: String = sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `id` = ?")
            .bind(alpha_id)
            .fetch_one(&pool)
            .await
            .expect("patched project re-reads");
        assert_eq!(renamed, "Alpha Renamed");
    }
    let response = owner
        .request("GET", "/api/v1/projects?q=Renamed", None)
        .await;
    let filtered = response.json("q filter");
    assert_eq!(response.status, 200, "q filter: {filtered}");
    if target.is_rust() {
        assert_eq!(filtered["data"][0]["name"], "Alpha Renamed");
    } else {
        // Nest has no `q` filter (unknown query ignored): behavior-class only.
        assert!(
            filtered["data"].as_array().is_some(),
            "Nest list shape: {filtered}"
        );
    }

    // 15. Board detail → 200 nested `position ASC` (second-GET re-read).
    let response = owner
        .request("GET", &format!("/api/v1/boards/{board_one_id}"), None)
        .await;
    assert_transport(&response, "board detail");
    let detail = response.json("board detail");
    assert_eq!(response.status, 200, "board detail: {detail}");
    let columns = detail["data"]["columns"]
        .as_array()
        .expect("detail nests columns");
    assert_eq!(columns.len(), 3, "detail carries the 3 columns");
    // Ordering IS contract on Rust (`position ASC` hardening); Nest loads
    // the relation unordered, so the Nest path asserts membership only.
    let positions: Vec<i64> = columns
        .iter()
        .map(|column| column["position"].as_i64().expect("position"))
        .collect();
    if target.is_rust() {
        assert_eq!(positions, vec![0, 1, 2], "columns ordered position ASC");
    } else {
        let mut sorted = positions.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![0, 1, 2],
            "Nest detail carries 3 columns: {detail}"
        );
    }
    if target.is_rust() {
        assert_no_legacy_keys(&detail, "board detail");
    }

    // 16. Board list + filters (Rust `Page`; Nest behavior on `data`).
    let response = owner.request("GET", "/api/v1/boards", None).await;
    let page = response.json("boards list");
    assert_eq!(response.status, 200, "boards list: {page}");
    if target.is_rust() {
        assert_eq!(page["total"], 3);
    } else {
        assert!(
            page["data"].as_array().expect("data").len() >= 3,
            "Nest list contains boards: {page}"
        );
    }
    let response = owner
        .request(
            "GET",
            &format!("/api/v1/boards?project_id={alpha_id}"),
            None,
        )
        .await;
    let filtered = response.json("project filter");
    assert_eq!(response.status, 200, "project filter: {filtered}");
    if target.is_rust() {
        assert_eq!(filtered["data"].as_array().expect("data").len(), 2);
    } else {
        // Nest reads `projectId` (camelCase): the snake_case key is ignored,
        // so the behavior-class assertion is membership, not the count.
        let names: Vec<&str> = filtered["data"]
            .as_array()
            .expect("data")
            .iter()
            .map(|item| item["name"].as_str().expect("name"))
            .collect();
        assert!(
            names.contains(&"Board One"),
            "Nest filter behavior: {filtered}"
        );
    }
    // Garbage `project_id` → Rust 400 (new strictness; Nest silently ignored
    // the filter — behavior-class path accepts the 200 list instead).
    let response = owner
        .request("GET", "/api/v1/boards?project_id=abc", None)
        .await;
    let garbage = response.json("garbage project_id");
    if target.is_rust() {
        assert_eq!(response.status, 400, "garbage project_id: {garbage}");
        assert_problem(&response, &garbage, "invalid-query", "garbage project_id");
    } else {
        assert_eq!(response.status, 200, "Nest ignores the filter: {garbage}");
    }
    // Deleted `/boards/archived` duplicate → Rust 400 `:id` int-parse fail.
    let response = owner.request("GET", "/api/v1/boards/archived", None).await;
    let failure = response.json("deleted /archived");
    if target.is_rust() {
        assert_eq!(response.status, 400, "deleted /archived: {failure}");
        assert_problem(&response, &failure, "invalid-path", "deleted /archived");
    } else {
        assert_eq!(response.status, 200, "Nest serves the duplicate: {failure}");
    }

    // 17. Board state: bad enum → 400; archive flips idempotent; re-read.
    let response = owner
        .request(
            "PATCH",
            &format!("/api/v1/boards/{solo_id}"),
            Some(r#"{"view_mode":"grid"}"#),
        )
        .await;
    let failure = response.json("bad view_mode");
    assert_eq!(response.status, 400, "bad view_mode: {failure}");
    assert_error(
        target,
        &response,
        &failure,
        "validation-error",
        "bad view_mode",
    );
    assert_errors_nonempty(target, &failure, "bad view_mode");
    for _ in 0..2 {
        let response = owner
            .request(
                "PATCH",
                &format!("/api/v1/boards/{solo_id}"),
                Some(r#"{"is_archived":true}"#),
            )
            .await;
        let patched = response.json("archive flip");
        assert_eq!(response.status, 200, "archive flip: {patched}");
        if target.is_rust() {
            assert_eq!(patched["data"]["is_archived"], true);
        }
    }
    if target.is_rust() {
        let archived_flag: i8 =
            sqlx::query_scalar("SELECT `is_archived` FROM `boards` WHERE `id` = ?")
                .bind(solo_id)
                .fetch_one(&pool)
                .await
                .expect("archived board re-reads");
        assert_eq!(archived_flag, 1);
    }
    // Default list hides archived (Rust exact; Nest behavior on `data`).
    let response = owner.request("GET", "/api/v1/boards", None).await;
    let page = response.json("hide archived");
    assert_eq!(response.status, 200);
    if target.is_rust() {
        assert_eq!(page["total"], 2, "archived hidden: {page}");
    }
    let response = owner
        .request("GET", "/api/v1/boards?archived=true", None)
        .await;
    let page = response.json("show archived");
    assert_eq!(response.status, 200);
    if target.is_rust() {
        assert_eq!(page["total"], 3, "archived included: {page}");
    }

    // 18-20. Board end-of-life. Rust: active delete → 409 (survives),
    // archive-flip idempotent, archived delete → 204, verb routes 404.
    // Nest (mirrors `backend/test/boards.e2e-spec.ts` behavior): verb routes
    // archive/restore → 200, deletes → 200 + `message`, archived list shows
    // the row. The Nest branch consumes Board Two; Solo stays for step 22
    // on both targets.
    if target.is_rust() {
        // 18. DELETE active board → 409 state conflict (board survives).
        let response = owner
            .request("DELETE", &format!("/api/v1/boards/{board_two_id}"), None)
            .await;
        let failure = response.json("delete active");
        assert_eq!(response.status, 409, "delete active: {failure}");
        assert_problem(&response, &failure, "conflict", "delete active");
        let response = owner
            .request("GET", &format!("/api/v1/boards/{board_two_id}"), None)
            .await;
        assert_eq!(response.status, 200, "active board survives the 409");

        // 19. Archive, then DELETE → 204; re-GET 404; columns cascade gone.
        let response = owner
            .request(
                "PATCH",
                &format!("/api/v1/boards/{board_two_id}"),
                Some(r#"{"is_archived":true}"#),
            )
            .await;
        assert_eq!(
            response.status,
            200,
            "pre-delete archive: {}",
            response.json("archive")
        );
        let response = owner
            .request("DELETE", &format!("/api/v1/boards/{board_two_id}"), None)
            .await;
        assert_transport(&response, "delete archived");
        assert_eq!(
            response.status,
            204,
            "delete archived: {}",
            String::from_utf8_lossy(&response.body)
        );
        assert!(response.body.is_empty(), "204 carries no body");
        let response = owner
            .request("GET", &format!("/api/v1/boards/{board_two_id}"), None)
            .await;
        let failure = response.json("re-get deleted");
        assert_eq!(response.status, 404, "re-get deleted: {failure}");
        assert_problem(&response, &failure, "not-found", "re-get deleted");
        let columns_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM `columns` WHERE `board_id` = ?")
                .bind(board_two_id)
                .fetch_one(&pool)
                .await
                .expect("cascade re-read");
        assert_eq!(columns_left, 0, "columns cascade gone");

        // 20. Deleted verb routes → 404 from the router (Solo still present).
        for route in ["archive", "restore", "permanent"] {
            for method in ["GET", "PATCH", "DELETE"] {
                let body = if method == "GET" { None } else { Some("{}") };
                let response = owner
                    .request(method, &format!("/api/v1/boards/{solo_id}/{route}"), body)
                    .await;
                let failure = response.json(&format!("{method} verb {route}"));
                assert_eq!(response.status, 404, "{method} verb {route}: {failure}");
                assert_problem(
                    &response,
                    &failure,
                    "not-found",
                    &format!("{method} verb {route}"),
                );
            }
        }
    } else {
        // Nest behavior-class flow: archive → archived list → restore →
        // delete (200s), row gone from the list.
        let response = owner
            .request(
                "PATCH",
                &format!("/api/boards/{board_two_id}/archive"),
                None,
            )
            .await;
        let archived = response.json("nest archive");
        assert_eq!(response.status, 200, "nest archive: {archived}");
        assert_eq!(archived["data"]["is_archived"], true);
        let response = owner.request("GET", "/api/boards/archived", None).await;
        let list = response.json("nest archived list");
        assert_eq!(response.status, 200, "nest archived list: {list}");
        assert!(
            list["data"]
                .as_array()
                .expect("data")
                .iter()
                .any(|item| item["id"] == board_two_id),
            "archived board listed: {list}"
        );
        let response = owner
            .request(
                "PATCH",
                &format!("/api/boards/{board_two_id}/restore"),
                None,
            )
            .await;
        assert_eq!(
            response.status,
            200,
            "nest restore: {}",
            response.json("restore")
        );
        let response = owner
            .request("DELETE", &format!("/api/boards/{board_two_id}"), None)
            .await;
        assert_eq!(
            response.status,
            200,
            "nest delete: {}",
            response.json("delete")
        );
        let response = owner.request("GET", "/api/boards", None).await;
        let list = response.json("nest list after delete");
        assert_eq!(response.status, 200);
        assert!(
            list["data"]
                .as_array()
                .expect("data")
                .iter()
                .all(|item| item["id"] != board_two_id),
            "deleted board absent: {list}"
        );
    }

    // 21. Cross-user isolation: intruder owns nothing of the owner's.
    let mut intruder = BbClient {
        host: addr.ip().to_string(),
        port: addr.port(),
        jar: Vec::new(),
    };
    let response = intruder
        .request(
            "POST",
            "/api/v1/auth/register",
            Some(r#"{"email":"e2e-intruder@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    assert_eq!(
        response.status,
        201,
        "intruder register: {}",
        response.json("register")
    );
    for (method, path) in [
        ("GET", format!("/api/v1/boards/{board_one_id}")),
        ("PATCH", format!("/api/v1/boards/{board_one_id}")),
        ("DELETE", format!("/api/v1/boards/{board_one_id}")),
        ("PATCH", format!("/api/v1/projects/{alpha_id}")),
        ("DELETE", format!("/api/v1/projects/{alpha_id}")),
    ] {
        let body = if method == "PATCH" {
            Some(r#"{"name":"Hijack"}"#)
        } else {
            None
        };
        let response = intruder.request(method, &path, body).await;
        let failure = response.json(&format!("intruder {method} {path}"));
        assert_eq!(response.status, 404, "intruder {method} {path}: {failure}");
        assert_error(
            target,
            &response,
            &failure,
            "not-found",
            &format!("intruder {method} {path}"),
        );
    }
    // Failed hijack left the rows untouched (DB re-read, Rust only).
    if target.is_rust() {
        let owner_name: String = sqlx::query_scalar("SELECT `name` FROM `projects` WHERE `id` = ?")
            .bind(alpha_id)
            .fetch_one(&pool)
            .await
            .expect("owner project survives hijack");
        assert_eq!(owner_name, "Alpha Renamed");
    }

    // 22. DELETE Solo → target delete status (Solo is archived on Rust;
    //     Nest deletes the active board with 200 per its spec).
    let response = owner
        .request("DELETE", &format!("/api/v1/boards/{solo_id}"), None)
        .await;
    assert_eq!(
        response.status,
        target.delete_status(),
        "delete Solo: {}",
        String::from_utf8_lossy(&response.body)
    );
    // DELETE Alpha project → target delete status, board cascade proven via
    // second GET (board detail 404) + SELECT (Rust) or list (Nest).
    let response = owner
        .request("DELETE", &format!("/api/v1/projects/{alpha_id}"), None)
        .await;
    assert_eq!(
        response.status,
        target.delete_status(),
        "delete Alpha: {}",
        String::from_utf8_lossy(&response.body)
    );
    if target.is_rust() {
        let response = owner
            .request("GET", &format!("/api/v1/boards/{board_one_id}"), None)
            .await;
        let failure = response.json("cascaded board");
        assert_eq!(response.status, 404, "cascaded board: {failure}");
        assert_problem(&response, &failure, "not-found", "cascaded board");
        let boards_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM `boards` WHERE `project_id` = ?")
                .bind(alpha_id)
                .fetch_one(&pool)
                .await
                .expect("project cascade re-read");
        assert_eq!(boards_left, 0, "project boards cascade-deleted");
        let response = owner.request("GET", "/api/v1/projects", None).await;
        let page = response.json("projects after delete");
        assert_eq!(page["total"], 2, "Alpha gone: {page}");
    }

    // 23. Logout → target delete status. Rust: empty body + cleared cookie
    //     + session row destroyed (set difference) + the pre-logout jar 401s
    //     on re-read. Repeated logouts never 429 — logout is unthrottled
    //     through the live governor (safe under any limits).
    let ids_before_logout = if target.is_rust() {
        session_ids(&pool).await
    } else {
        Vec::new()
    };
    if target.is_rust() {
        assert!(!ids_before_logout.is_empty());
    }
    let response = owner.request("POST", "/api/v1/auth/logout", None).await;
    assert_transport(&response, "logout");
    assert_eq!(
        response.status,
        target.delete_status(),
        "logout: {}",
        String::from_utf8_lossy(&response.body)
    );
    if target.is_rust() {
        assert!(response.body.is_empty(), "204 carries no body");
        assert!(
            response
                .headers
                .get("set-cookie")
                .is_some_and(|values| values.iter().any(|cookie| cookie.contains("Max-Age=0"))),
            "logout clears the cookie: {:?}",
            response.headers.get("set-cookie")
        );
        // The logout `Set-Cookie` clear emptied the owner jar client-side.
        assert!(owner.jar.is_empty(), "cleared jar drops the cookie");
        let ids_after_logout = session_ids(&pool).await;
        let removed: Vec<&String> = ids_before_logout
            .iter()
            .filter(|id| !ids_after_logout.contains(id))
            .collect();
        assert_eq!(removed.len(), 1, "logout destroys exactly its row");
        // The pre-logout cookie value is anonymous now (second-GET re-read).
        let (old_name, old_value) = post_cookie
            .split_once('=')
            .expect("login set a name=value cookie");
        let mut stale = BbClient {
            host: addr.ip().to_string(),
            port: addr.port(),
            jar: vec![(old_name.to_string(), old_value.to_string())],
        };
        let response = stale.request("GET", "/api/v1/auth/me", None).await;
        let failure = response.json("stale jar me");
        assert_eq!(response.status, 401, "stale jar me: {failure}");
        assert_problem(&response, &failure, "unauthorized", "stale jar me");
    }
    let response = owner
        .request(
            "POST",
            "/api/v1/auth/login",
            Some(r#"{"email":"e2e-owner@example.com","password":"Str0ng!pass"}"#),
        )
        .await;
    assert_eq!(
        response.status,
        200,
        "re-login: {}",
        response.json("re-login")
    );
    let fresh_cookie = owner.cookie_header().expect("fresh jar");
    assert_ne!(
        fresh_cookie, post_cookie,
        "post-logout login mints a new session"
    );
    let response = owner.request("GET", "/api/v1/auth/me", None).await;
    assert_eq!(
        response.status,
        200,
        "fresh jar works: {}",
        response.json("me")
    );
    // Unthrottled-logout proof: 6 anonymous logouts still pass.
    for _ in 0..6 {
        let response = anon.request("POST", "/api/v1/auth/logout", None).await;
        assert_ne!(response.status, 429, "logout must never 429");
        assert_eq!(
            response.status,
            target.delete_status(),
            "anonymous logout: {}",
            String::from_utf8_lossy(&response.body)
        );
    }

    server.abort();
    // NO TEARDOWN WIPE/DROP: the shared store must survive until every
    // suite in this process is done.
}
