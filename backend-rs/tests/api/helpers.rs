//! Shared integration-test helpers (T07 owns this file).
//!
//! # Ownership (normative for T10/T25/T50)
//! Import from here — never redefine: `login_as`, `ensure_default_labels`,
//! `clear_sessions`, `test_app`, fixture loading order. T10/T25 call prod
//! `backend_rs::common::label_seed::ensure_default_labels` only through the
//! [`ensure_default_labels`] wrapper below; the wrapper exists so the
//! call-site contract (executor-generic, `(pool_or_tx, user_id)` shape) is
//! documented in one place.
//!
//! # DB strategy (cost control, per 10-architecture.md Tests/CI)
//! - Migration/unicode tests use `#[sqlx::test(migrations, fixtures)]`
//!   per-test DBs (parallel, isolated). The test at the bottom of this
//!   file is the pattern reference — read its NOTE first: the baseline's
//!   fail-fast `CREATE SCHEMA tower_sessions` allows exactly one migrator
//!   setup per `cargo test` process on a shared server.
//! - Resource suites (T20+) share ONE database and wrap each test in a
//!   transaction with rollback per test (per-test `CREATE DATABASE` on MySQL
//!   is minutes at ~150 tests). Fixture files insert fixed ids in FK order
//!   (`users -> projects -> boards -> columns -> cards -> labels/card_labels
//!   -> checklists -> items -> notes/tags`), so a shared-DB suite loads them
//!   once in the order listed in [`FIXTURE_ORDER`] and rolls back per test.
//!   Fixed ids + isolated DBs (or tx rollback) = parallel-safe.
//!
//! # Cookie-jar persistence decision
//! `axum-test::TestServer` is NOT a dev-dependency (T03 pins only
//! `tower::util` + `oneshot`; `Cargo.toml` is outside T07 touch scope, so no
//! new dev-dep is added here). [`login_as`] therefore drives the app via
//! `tower::ServiceExt::oneshot` and propagates cookies MANUALLY: every
//! `set-cookie` value is stored and re-sent as one joined `cookie` header
//! (see [`JarClient`]). T11 may switch to `TestServer` cookie-jar storage
//! when auth routes land; manual propagation stays valid either way.
//!
//! # Sessions between suites
//! [`clear_sessions`] truncates the store-owned `tower_sessions.session`
//! table. Suites that touch auth call it in setup so expired/leftover rows
//! never leak across suites.

use axum::{
    body::Body,
    http::{header, HeaderMap, Request, Response},
    Router,
};
use backend_rs::{build_state, config::Settings, create_app};
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

/// Test settings: `NODE_ENV=test`, minimal Argon2 cost for speed.
pub fn test_settings() -> Settings {
    Settings {
        node_env: "test".to_string(),
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

/// Test-only wrapper around prod
/// `backend_rs::common::label_seed::ensure_default_labels`.
/// Returns rows inserted (6 on first call, 0 when already seeded).
pub async fn ensure_default_labels(pool: &MySqlPool, user_id: i32) -> Result<u64, sqlx::Error> {
    backend_rs::common::label_seed::ensure_default_labels(pool, user_id).await
}

/// Truncate store-owned session rows between suites (auth isolation).
pub async fn clear_sessions(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM `tower_sessions`.`session`")
        .execute(pool)
        .await
        .map(|_| ())
}

/// Authenticated cookie jar with MANUAL `Set-Cookie` propagation
/// (see module docs: no `axum-test` dev-dep in T07 scope).
#[allow(dead_code)] // wired fully in T11 when auth routes land
#[derive(Debug, Default)]
pub struct JarClient {
    cookies: Vec<String>,
}

#[allow(dead_code)] // wired fully in T11 when auth routes land
impl JarClient {
    /// Collect every `set-cookie` response value into the jar.
    pub fn store_from(&mut self, headers: &HeaderMap) {
        for value in headers.get_all(header::SET_COOKIE) {
            if let Ok(cookie) = value.to_str() {
                let pair = cookie.split(';').next().unwrap_or("").trim();
                if !pair.is_empty() && !self.cookies.contains(&pair.to_string()) {
                    self.cookies.push(pair.to_string());
                }
            }
        }
    }

    /// Render the jar as one `Cookie` header value.
    pub fn cookie_header(&self) -> String {
        self.cookies.join("; ")
    }
}

/// Log in via `POST /api/v1/auth/login` and return the cookie jar.
/// Requires T11 auth routes; until then this is scaffolding only.
#[allow(dead_code)] // wired fully in T11 when auth routes land
pub async fn login_as(app: Router, email: &str, password: &str) -> JarClient {
    let body = serde_json::json!({ "email": email, "password": password }).to_string();
    let request = Request::post("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("login request builds");
    let response = app.oneshot(request).await.expect("login oneshot");
    let mut jar = JarClient::default();
    jar.store_from(response.headers());
    jar
}

/// Send a request carrying the jar's cookies (manual propagation).
#[allow(dead_code)] // wired fully in T11 when auth routes land
pub async fn authed(app: Router, jar: &JarClient, request: Request<Body>) -> Response<Body> {
    let (mut parts, body) = request.into_parts();
    if !jar.cookies.is_empty() {
        parts.headers.insert(
            header::COOKIE,
            jar.cookie_header()
                .parse()
                .expect("jar cookies form a valid header"),
        );
    }
    app.oneshot(Request::from_parts(parts, body))
        .await
        .expect("authed oneshot")
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
/// NOTE (load-bearing, see T52 proposal in the T07 report): these two ACs
/// share ONE `#[sqlx::test]` setup deliberately. The baseline's first
/// statement is fail-fast `CREATE SCHEMA tower_sessions` (no `IF NOT EXISTS`,
/// T02 design) on a server-global schema, so two concurrent migrator setups
/// race (loser gets 1007) and EVERY later setup fails once the schema exists.
/// Exactly one migrator setup per `cargo test` process therefore succeeds;
/// the teardown below restores the server-fresh precondition for the next
/// run. Resource suites (T20+) sidestep this entirely via the shared-DB +
/// tx-rollback strategy (migrations run once). If this test aborts before
/// teardown, recover with: `DROP SCHEMA tower_sessions;`
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
    // Session-table access proof: truncate works on the store-owned schema.
    clear_sessions(&pool).await.unwrap();

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

    // Teardown: restore the T02 server-fresh precondition (baseline
    // `CREATE SCHEMA tower_sessions` fails when it exists). Required for
    // repeat `cargo test` runs on a shared server.
    sqlx::query("DROP SCHEMA IF EXISTS `tower_sessions`")
        .execute(&pool)
        .await
        .unwrap();
}
