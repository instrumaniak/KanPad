//! Shared integration-test helpers (T07 owns this file).
//!
//! # Ownership (NORMATIVE — import, never redefine)
//!
//! **Scope: the IN-PROCESS suites** — the ones that drive an `axum::Router`
//! through `tower::ServiceExt::oneshot` (everything except the two
//! black-box suites, which are covered by the `super::wire` section below).
//!
//! An in-process suite gets its request plumbing from HERE. Do NOT
//! re-declare any of these per suite; a local copy shadows the shared one
//! silently and drifts. The complete forbidden-local set is:
//!
//! **App / settings** — `app_on_pool` · `app_on_pool_with` · `test_app` ·
//! `test_settings` · `ensure_default_labels`
//!
//! **Request plumbing** — `send` · `send_ref` · `send_json` ·
//! `authed_request` · `json_body` · `json_of`
//!
//! **Assertions** — `assert_problem` · `assert_problem_fields` ·
//! `assert_errors_nonempty` · `assert_no_legacy_keys`
//!
//! **Cookies** — `cookie_jar` · `cookie_jar_from`
//!
//! **Scenario builders** — `register` · `create_project` · `create_board` ·
//! `create_board_json`
//!
//! **Constants** — `PROBLEM_JSON` · `PROBLEM_BASE` · `FIXTURE_ORDER`
//!
//! (`e2e_base_url` / `e2e_target_is_nest` are the black-box suites' knobs —
//! they belong here, but only the black-box suites may call them.)
//!
//! ## Sanctioned exceptions
//!
//! A suite MAY define a local helper when it is genuinely suite-specific —
//! but it must NOT reuse a name from the forbidden set above, and it MUST be
//! listed here. The two current exceptions:
//!
//! - `throttle.rs` — DB-free governor probes. It injects an explicit
//!   [`axum::extract::ConnectInfo`] peer per request and drives ONE router
//!   repeatedly, so it uses the borrowed [`send_ref`] (not [`send`]) and
//!   keeps its own request builders. Everything it can share, it shares.
//! - `e2e_auth_projects_boards.rs` — a target-aware
//!   `assert_errors_nonempty(target, body, what)` adapter over
//!   [`assert_errors_nonempty`], because the Nest column asserts a different
//!   body shape than the Rust column. It delegates to the shared core and
//!   imports it aliased (`assert_errors_nonempty_fields`).
//!
//! ## Import EXPLICITLY, never glob
//!
//! `use super::helpers::*` is forbidden: an explicit `use` that collides
//! with a leftover local definition is a compile error (E0255), whereas a
//! glob import lets the local silently win. That difference is the
//! regression guard for this rule.
//!
//! # Black-box suites: `super::wire`
//! The two black-box suites (T50a/T50c) drive the real app over a raw TCP
//! socket, so they cannot use the in-process plumbing above. Their
//! transport-independent pieces live in [`super::wire`]: `WireResponse` ·
//! `parse_response` · `dechunk` · `header_first` · `cookie_jar` (wire
//! flavor) · `assert_problem` (wire flavor) · `bind_blackbox`.
//! (`WireResponse::json` is the wire-flavored body parser; the in-process
//! counterpart is [`json_of`].)
//!
//! `wire::assert_problem` delegates to [`assert_problem_fields`] and
//! `wire::cookie_jar` delegates to [`cookie_jar_from`] — the SAME cores the
//! in-process suites use, so the two transports cannot drift. Import from
//! `super::wire` explicitly, by the same rule as above.
//!
//! What deliberately stays PER-SUITE: the socket client itself. T50a speaks
//! async `tokio::net::TcpStream` and keeps a name/value jar; T50c speaks a
//! blocking `std::net::TcpStream` and derives the jar from the response
//! headers on demand. They are not interchangeable, so they are not shared.
//!
//! T10/T25 call prod `backend_rs::common::label_seed::ensure_default_labels`
//! only through the [`ensure_default_labels`] wrapper below; the wrapper
//! exists so the call-site contract (executor-generic, `(pool_or_tx,
//! user_id)` shape) is documented in one place.
//!
//! # DB strategy (cost control, per 10-architecture.md Tests/CI)
//! - Suites use `#[sqlx::test(migrations = "./migrations")]` per-test
//!   databases (parallel, isolated). The test at the bottom of this file is
//!   the pattern reference — read its NOTE.
//! - Fixture files insert fixed ids in FK order (`users -> projects ->
//!   boards -> columns -> cards -> labels/card_labels -> checklists ->
//!   items -> notes/tags`), so a suite loads them once in the order listed in
//!   [`FIXTURE_ORDER`].
//!
//! # Sessions between suites
//! The store-owned `sessions` table lives in each test's OWN database (see
//! `30-db-schema.md` deviation 11 — it is NOT the store's default
//! `tower_sessions` schema). Isolation is therefore structural: parallel
//! suites can never see each other's rows, so there is nothing to wipe and
//! no coordinated-delete dance. [`app_on_pool`] reads the database name from
//! the pool's connect options via `AppState::with_pool`, so no suite needs
//! `SELECT DATABASE()` or hand-wires the store.
//!
//! Every suite still mints its own user + jar; store assertions should use
//! id-set difference or scope their SQL to rows they minted (`WHERE id = ?`).
//! NEVER reintroduce a global `DELETE FROM sessions`, an `UPDATE`-all, or a
//! schema drop in any suite's setup/teardown.

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::Router;
use backend_rs::state::AppState;
use backend_rs::{build_state, config::Settings, create_app};
use serde_json::Value;
use sqlx::MySqlPool;
use tower::ServiceExt;

/// Fixture load order (FK order). `#[sqlx::test(fixtures(..))]` lists must
/// follow this sequence; shared-DB suites replay it the same way.
pub const FIXTURE_ORDER: [&str; 12] = [
    "users",
    "projects",
    "boards",
    "columns",
    "cards",
    "labels",
    "card_labels",
    "checklists",
    "checklist_items",
    "notes",
    "tags",
    "note_tags",
];

/// `Content-Type` every problem+json error must carry.
pub const PROBLEM_JSON: &str = "application/problem+json";

/// Base URI of the problem-type slugs asserted by [`assert_problem`].
pub const PROBLEM_BASE: &str = "https://kanpad.dev/problems";

/// Test settings: `APP_ENV=test`, minimal Argon2 cost for speed.
pub fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_string(),
        hash_cost: 1,
        port: 3001,
        // T05 added Settings fields with `Default` (compile-compat only).
        ..Default::default()
    }
}

/// Boot the app in-process via `lib::create_app`/`build_state`
/// (never spawns the session sweeper — only `main.rs` does).
pub fn test_app() -> Router {
    create_app(build_state(test_settings()))
}

// ---------------------------------------------------------------------------
// HTTP harness (the single definition of every suite's request plumbing)
// ---------------------------------------------------------------------------

/// Build the app bound to a `#[sqlx::test]` pool: prod settings, prod
/// session-store wiring, but the pool the fixture migrator actually created.
///
/// `AppState::with_pool` reads the database name straight from the pool's
/// connect options, so suites never wire the store by hand and never query
/// `SELECT DATABASE()`.
///
/// In-process via `create_app` — never spawns the session sweeper.
pub fn app_on_pool(pool: &MySqlPool) -> Router {
    app_on_pool_with(pool, Settings::default())
}

/// [`app_on_pool`] with a settings override — for suites that boot the app
/// with one flag flipped (registration gate, `SESSION_SECRET`, `APP_ENV`).
/// The session store still follows the pool's database, so overrides never
/// cost a suite its store wiring.
pub fn app_on_pool_with(pool: &MySqlPool, settings: Settings) -> Router {
    create_app(AppState::with_pool(settings, pool.clone()))
}

/// Serialize a JSON payload for a request body.
pub fn json_body(value: &Value) -> String {
    value.to_string()
}

/// Build a request, optionally carrying a cookie jar and a JSON body.
pub fn authed_request(
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

/// Drive one request through the router, returning status + headers + raw
/// body (parse with [`json_of`] when the response is JSON).
pub async fn send(app: Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    send_ref(&app, request).await
}

/// [`send`], but the router is BORROWED — for suites that drive the same app
/// repeatedly (e.g. `throttle.rs`'s flood probes, which must keep one
/// governor state alive across many requests).
pub async fn send_ref(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request completes");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body reads");
    (status, headers, bytes.to_vec())
}

/// [`send`], pre-parsed — for suites whose every assertion is on JSON.
pub async fn send_json(app: Router, request: Request<Body>) -> (StatusCode, HeaderMap, Value) {
    let (status, headers, bytes) = send(app, request).await;
    (status, headers, json_of(&bytes))
}

/// Parse a response body as JSON.
pub fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("response is JSON")
}

/// Core of the cookie-jar extraction: the `name=value` pair of every
/// `Set-Cookie` value, joined as one `Cookie` header value.
///
/// Shared by [`cookie_jar`] (axum `HeaderMap`) and `super::wire::cookie_jar`
/// (raw wire headers) — one core, so the two transports cannot drift on how
/// a jar is built.
pub fn cookie_jar_from<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .map(|cookie| cookie.split(';').next().unwrap_or("").trim())
        .filter(|pair| !pair.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// First `name=value` pair of every `Set-Cookie`, joined as a `Cookie` header.
pub fn cookie_jar(headers: &HeaderMap) -> String {
    cookie_jar_from(
        headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok()),
    )
}

/// Assert a problem+json error carries `slug`, the right status, and a
/// non-empty `instance`.
///
/// `what` labels the failing step in the panic message — every call site
/// passes one so a failure names the scenario, not just the line.
pub fn assert_problem(
    status: StatusCode,
    headers: &HeaderMap,
    body: &Value,
    slug: &str,
    what: &str,
) {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .unwrap_or_else(|| panic!("{what}: error must set Content-Type"))
        .to_str()
        .expect("Content-Type is ASCII");
    assert_problem_fields(content_type, body, status.as_u16(), slug, what);
}

/// Transport-independent core of the problem+json assertion.
///
/// Shared by the in-process suites ([`assert_problem`], which reads the
/// `Content-Type` from an axum `HeaderMap`) and the black-box wire suites
/// (`super::wire::assert_problem`, which reads it off the raw HTTP response).
/// One core, so the two transports can never drift on what a problem body
/// must contain.
pub fn assert_problem_fields(
    content_type: &str,
    body: &Value,
    status: u16,
    slug: &str,
    what: &str,
) {
    assert_eq!(content_type, PROBLEM_JSON, "{what}: status {status}");
    assert_eq!(
        body["type"],
        format!("{PROBLEM_BASE}/{slug}"),
        "{what}: status {status}"
    );
    assert_eq!(body["status"], status, "{what}");
    assert!(
        body["instance"].as_str().is_some_and(|s| !s.is_empty()),
        "{what}: instance always set"
    );
}

/// Assert a validation-class problem carries a non-empty `errors[]`.
pub fn assert_errors_nonempty(body: &Value, what: &str) {
    assert!(
        body["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()),
        "{what} must carry errors[]: {body}"
    );
}

/// Assert the body carries none of the legacy Nest response keys, at any
/// depth (a raw-substring scan, so nested `description`/`content` also fail).
pub fn assert_no_legacy_keys(body: &Value, what: &str) {
    let raw = body.to_string();
    for key in ["message", "description", "content"] {
        assert!(
            body.get(key).is_none(),
            "{what} must not carry top-level `{key}`: {body}"
        );
    }
    assert!(
        !raw.contains("\"description\""),
        "{what} must not carry `description` anywhere: {body}"
    );
    assert!(
        !raw.contains("\"content\""),
        "{what} must not carry `content` anywhere: {body}"
    );
}

/// Register a fresh user with a live cookie jar; returns `(jar, user_id)`.
pub async fn register(app: Router, email: &str) -> (String, i64) {
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

/// Create a project under `jar`; returns its id.
pub async fn create_project(app: Router, jar: &str, name: &str) -> i64 {
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

/// Create a board under `jar` from a caller-built payload; returns its id.
pub async fn create_board(app: Router, jar: &str, payload: &Value) -> i64 {
    create_board_json(app, jar, payload).await["data"]["id"]
        .as_i64()
        .expect("board id")
}

/// [`create_board`] returning the whole `data` envelope — for suites that
/// assert on `background_color` / `project_id` off the create response.
pub async fn create_board_json(app: Router, jar: &str, payload: &Value) -> Value {
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
    body
}

/// Test-only wrapper around prod
/// `backend_rs::common::label_seed::ensure_default_labels`.
/// Returns rows inserted (6 on first call, 0 when already seeded).
pub async fn ensure_default_labels(pool: &MySqlPool, user_id: i32) -> Result<u64, sqlx::Error> {
    backend_rs::common::label_seed::ensure_default_labels(pool, user_id).await
}

// ---------------------------------------------------------------------------
// Pattern-reference tests (run with `cargo test -p backend-rs --test api helpers`).
// Per-test isolated DBs via #[sqlx::test]; needs DATABASE_URL at runtime.
// ---------------------------------------------------------------------------

/// No-DB smoke proof that `test_app()` boots via `lib::create_app` /
/// `build_state` (also keeps `test_settings` live for future suites).
#[tokio::test]
async fn helpers_test_app_serves_health() {
    use axum::body::to_bytes;
    use axum::http::StatusCode;

    let response = test_app()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, serde_json::json!({"status": "ok"}));
}

/// AC: `ensure_default_labels` seeds exactly the 6 defaults idempotently
/// (second call = no dup, still 6 rows).
/// AC: fixtures insert fixed ids in FK order and load cleanly together.
///
/// NOTE (load-bearing): the `sessions` store table lives in the SAME
/// database as the pool (see `AppState::with_pool`), so under
/// `#[sqlx::test]` it is PER-TEST — every suite gets its own database and
/// no suite can reach another's sessions. There is therefore NO global
/// wipe helper at all (rs-fix-session-isolation retired T11's single
/// coordinated wipe; a blanket `DELETE`/`UPDATE`-all/`DROP` would only
/// reach that test's own rows — while racing the test's own live jars and
/// 401ing them). NEVER wipe or drop the store from any suite: scope
/// session mutations by `WHERE id = ?` instead. The baseline DDL for
/// `sessions` is fail-fast `CREATE TABLE` (no `IF NOT EXISTS`) — safe
/// because each `#[sqlx::test]` migrates its own fresh database, so there
/// is no cross-process race and no "server-fresh" teardown is needed.
#[sqlx::test(
    migrations = "./migrations",
    fixtures(
        "../fixtures/users.sql",
        "../fixtures/projects.sql",
        "../fixtures/boards.sql",
        "../fixtures/columns.sql",
        "../fixtures/cards.sql",
        "../fixtures/labels.sql",
        "../fixtures/card_labels.sql",
        "../fixtures/checklists.sql",
        "../fixtures/checklist_items.sql",
        "../fixtures/notes.sql",
        "../fixtures/tags.sql",
        "../fixtures/note_tags.sql"
    )
)]
async fn helpers_seeded_fixtures_suite(pool: MySqlPool) {
    // NO session wipe here: the `sessions` table lives in this test's own
    // database (see NOTE above) — global wipes are retired entirely, and
    // this suite holds no session jars anyway.

    let expected_counts = FIXTURE_ORDER.map(|table| {
        let expected = match table {
            "users" => 2,
            "projects" => 3,
            "boards" => 4,
            "columns" => 4,
            "cards" => 3,
            "labels" => 7,
            "card_labels" => 2,
            "checklists" => 2,
            "checklist_items" => 2,
            "notes" => 2,
            "tags" => 2,
            "note_tags" => 1,
            other => panic!("FIXTURE_ORDER lists unknown fixture: {other}"),
        };
        (table, expected)
    });
    for (table, expected) in expected_counts {
        let sql = format!("SELECT COUNT(*) FROM `{table}`");
        let count: i64 = sqlx::query_scalar(&sql).fetch_one(&pool).await.unwrap();
        assert_eq!(count, expected, "fixture row count for {table}");
    }

    // Spot-check FK integrity across the chain: card 41 -> column 31 ->
    // board 21 -> project 11 -> user 1, carrying 2 labels.
    let owner: String = sqlx::query_scalar(
        "SELECT u.`email` FROM `users` u \
         JOIN `projects` p ON p.`user_id` = u.`id` \
         JOIN `boards` b ON b.`project_id` = p.`id` \
         JOIN `columns` c ON c.`board_id` = b.`id` \
         JOIN `cards` k ON k.`column_id` = c.`id` \
         WHERE k.`id` = 41",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, "owner@example.com");

    let card_labels: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `card_labels` WHERE `card_id` = 41")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(card_labels, 2);

    // Idempotency on a label-free user (user 99 is NOT in users.sql).
    sqlx::query(
        "INSERT INTO `users` (`id`, `email`, `password`) VALUES (99, 'seed@example.com', 'x')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let first = ensure_default_labels(&pool, 99).await.unwrap();
    assert_eq!(first, 6, "first seed must insert exactly 6 rows");

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT `name`, `color` FROM `labels` WHERE `user_id` = 99 ORDER BY `id`")
            .fetch_all(&pool)
            .await
            .unwrap();
    let expected: Vec<(String, String)> = backend_rs::common::label_seed::DEFAULT_LABELS
        .iter()
        .map(|(name, color)| ((*name).to_string(), (*color).to_string()))
        .collect();
    assert_eq!(rows, expected, "seeded rows must match prod DEFAULT_LABELS");

    let second = ensure_default_labels(&pool, 99).await.unwrap();
    assert_eq!(second, 0, "second seed must insert 0 rows (idempotent)");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = 99")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 6, "still exactly 6 rows after re-seed");

    // NO TEARDOWN WIPE/DROP (see NOTE above).
}

// ---------------------------------------------------------------------------
// T50a black-box e2e env-switch (T50a owns ONLY these two fns in this file).
// `E2E_BASE_URL` PINS the black-box port (default: none — the suite binds an
// OS-assigned ephemeral port); `E2E_TARGET=rust|nest` selects the per-target
// expectation params in `e2e_auth_projects_boards.rs` (Nest = behavior-class
// assertions only — the contract diverges, so one assertion set cannot pass
// on both).
// ---------------------------------------------------------------------------

/// The operator-pinned black-box base URL (`E2E_BASE_URL`), if any.
///
/// `None` means "no pin": the black-box suites then bind an OS-assigned
/// EPHEMERAL port (see [`super::wire::bind_blackbox`]). That default is
/// deliberate — the two black-box suites run in parallel inside the SAME test
/// binary, so a fixed default port made one of them fail with `AddrInUse`;
/// an ephemeral port also means a dev server already holding `:3001` cannot
/// break the suite.
pub fn e2e_base_url() -> Option<String> {
    std::env::var("E2E_BASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Whether black-box suites should use Nest behavior-class expectations
/// (`E2E_TARGET=nest`) instead of the default exact Rust-contract assertions.
pub fn e2e_target_is_nest() -> bool {
    std::env::var("E2E_TARGET").is_ok_and(|value| value.eq_ignore_ascii_case("nest"))
}
