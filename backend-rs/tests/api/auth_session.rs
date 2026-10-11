//! `POST /api/v1/auth/login`, `POST /api/v1/auth/logout`, `GET /api/v1/auth/me`
//! + `AuthSession` guard + session cookie hardening (T11).
//!
//! Single `#[sqlx::test]` setup on purpose (the `tower_sessions.session`
//! store table is server-global, shared by every suite in this process): all
//! scenarios run sequentially in one test with distinct emails.
//!
//! Session-isolation rule (rs-fix-session-isolation): NO suite in this
//! process may `DELETE`/`UPDATE`-all/`DROP` the shared store. The old single
//! coordinated wipe at startup is retired — under parallel scheduling it
//! lands mid-flight of other suites' live jars and 401s them (3 flakes,
//! latest `boards_state` 401 at create). Cross-suite rows are harmless:
//! every suite mints its own user+jar, nothing asserts absolute store
//! counts, and logout asserts by id-set difference. Tests needing a
//! dirty/clean row scope their SQL to rows they minted (see
//! `login_fresh_session` + the `WHERE id = ?` expiry below). NOBODY drops
//! the shared schema (baseline DDL is `IF NOT EXISTS` since early-T52,
//! so the old fail-fast `DROP SCHEMA` hygiene is retired — a concurrent drop
//! 500s in-flight suites with 1049 `Unknown database 'tower_sessions'`).
//!
//! Covered (red first: login/logout/me routes do not exist yet):
//! login 200 (+ fixation rotation, pre-cookie != post-cookie), bad
//! password / unknown email -> 401 same problem shape (no enumeration
//! oracle), weak-but-wellformed password still reaches auth (200 on match,
//! 401 on mismatch — never 400 strength), overlong password -> 400 byte cap,
//! bcrypt cost-10 `$2b$` login verifies + rehashes to Argon2id in the same
//! request (DB re-read), logout 204 (row destroyed, cookie cleared,
//! id-set shrinks by exactly the logged-out row), me 200 `{data:{id,email,role}}` else 401, me re-lookup
//! (deleted -> 401, demoted -> 403 + rotation), expired session row (past
//! TIMESTAMP) -> 401, wrong `SESSION_SECRET` cookie -> 401 (force re-login),
//! cookie matrix dev (`connect.sid`, no Secure) vs prod (`__Host-`,
//! Secure+HttpOnly+SameSite=Lax+Path=/, no Domain), deprecated `/api/*`
//! alias serves the same handlers.

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Request, StatusCode},
    Router,
};
use backend_rs::{config::Settings, create_app, state::AppState};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::MySqlPool;
use tower::ServiceExt;
use tower_sessions_sqlx_store::MySqlStore;

const PROBLEM_JSON: &str = "application/problem+json";

/// Build the app on the `#[sqlx::test]` pool (in-process, no sweeper —
/// `create_app` never spawns one), with session-secret / env overrides.
fn app_on_pool(pool: &MySqlPool, session_secret: Option<&str>, node_env: Option<&str>) -> Router {
    let mut settings = Settings {
        ..Default::default()
    };
    if let Some(secret) = session_secret {
        settings.session_secret = SecretString::new(secret.to_string().into_boxed_str());
    }
    if let Some(env) = node_env {
        settings.node_env = env.to_string();
    }
    if settings.node_env == "production" {
        settings.cors_origins = "https://app.example.com".to_string();
    }
    let mut state = AppState::from_settings(settings);
    state.pool = pool.clone();
    state.sessions = MySqlStore::new(pool.clone());
    create_app(state)
}

fn json_login(email: &str, password: &str) -> String {
    serde_json::json!({ "email": email, "password": password }).to_string()
}

fn post(path: &str, email: &str, password: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(jar) = cookie {
        builder = builder.header(header::COOKIE, jar);
    }
    builder
        .body(Body::from(json_login(email, password)))
        .expect("auth request builds")
}

fn get(path: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::get(path);
    if let Some(jar) = cookie {
        builder = builder.header(header::COOKIE, jar);
    }
    builder.body(Body::empty()).expect("auth request builds")
}

fn post_empty(path: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::post(path);
    if let Some(jar) = cookie {
        builder = builder.header(header::COOKIE, jar);
    }
    builder.body(Body::empty()).expect("auth request builds")
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

fn raw_set_cookies(headers: &HeaderMap) -> String {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn send(app: Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app.oneshot(request).await.expect("auth oneshot");
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

/// Live ids of the server-global session store (`tower_sessions.session`
/// is shared by every suite in this process — compare SETS, never absolute
/// counts, so a concurrent suite's inserts can't flake the assertion; only
/// logout deletes rows, so exactly one id may vanish across a logout).
async fn session_ids(pool: &MySqlPool) -> Vec<String> {
    sqlx::query_scalar("SELECT `id` FROM `tower_sessions`.`session`")
        .fetch_all(pool)
        .await
        .expect("session ids read")
}

/// Log in once and return `(jar, store_row_id)` for exactly the session
/// that login minted.
///
/// The `tower_sessions.session` table is server-global (shared by every
/// suite in this process), so row ownership is resolved by id-set
/// difference around the login: `after − before` must be exactly one row.
/// If a concurrent suite minted alongside us (delta != 1) the attempt is
/// discarded WITHOUT touching any row and retried, so we can never
/// expire/delete a stranger's session. Retries converge immediately in
/// practice (the window is a single in-process roundtrip); the discarded
/// jars stay valid and are harmless leftovers.
async fn login_fresh_session(app: impl Fn() -> Router, pool: &MySqlPool) -> (String, String) {
    for _ in 0..25 {
        let before = session_ids(pool).await;
        let (status, headers, bytes) = send(
            app(),
            post(
                "/api/v1/auth/login",
                "login-user@example.com",
                "Str0ng!pass",
                None,
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "expiry-probe login: {}",
            json_of(&bytes)
        );
        let jar = cookie_jar(&headers);
        assert!(!jar.is_empty(), "probe login sets a session cookie");
        let after = session_ids(pool).await;
        let fresh: Vec<&String> = after.iter().filter(|id| !before.contains(id)).collect();
        if fresh.len() == 1 {
            return (jar, fresh[0].clone());
        }
        // Ambiguous window — a concurrent suite minted too. Discard this
        // jar untouched (its row stays valid) and retry.
    }
    panic!("could not isolate a freshly minted session row after retries");
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_session_full_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool, None, None);

    // 0. NO session wipe: the store table is server-global, so even this
    //    startup `DELETE` races parallel suites' live jars and 401s them
    //    mid-flight (3 flakes and counting — the "no live jars yet"
    //    assumption does not hold under cargo's parallel scheduling).
    //    Leftover rows are harmless: per-test DBs are fresh every process,
    //    every suite mints its own user+jar, and no assertion below counts
    //    absolute rows (logout uses id-set difference).

    // 1. Register (T10) establishes the pre-login session cookie; login
    //    rotates it (`cycle_id` fixation defense: pre != post).
    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/register",
            "login-user@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "register: {}", json_of(&bytes));
    let pre_cookie = cookie_jar(&headers);
    assert!(!pre_cookie.is_empty(), "register sets a session cookie");

    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "Str0ng!pass",
            Some(&pre_cookie),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "login: {body}");
    assert_eq!(body["data"]["email"], "login-user@example.com");
    assert_eq!(body["data"]["role"], "user");
    assert!(body["data"]["id"].as_i64().unwrap() > 0);
    assert!(body.get("message").is_none(), "no message key: {body}");
    let login_jar = cookie_jar(&headers);
    assert!(!login_jar.is_empty(), "login sets a session cookie");
    assert_ne!(
        login_jar, pre_cookie,
        "login must rotate the session id (cycle_id)"
    );
    // Fixation proof against the register cookie from step 1.
    let (re_status, re_headers, _) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "Str0ng!pass",
            Some(&pre_cookie),
        ),
    )
    .await;
    assert_eq!(re_status, StatusCode::OK);
    assert_ne!(
        cookie_jar(&re_headers),
        pre_cookie,
        "login must rotate the session id (cycle_id)"
    );

    // 2. Wrong password vs unknown email: identical 401 problem shape
    //    (no user-enumeration oracle beyond the accepted bcrypt delta).
    let (_, bad_headers, bad_bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "Wr0ng!pass",
            None,
        ),
    )
    .await;
    let bad_body = json_of(&bad_bytes);
    assert_problem(
        StatusCode::UNAUTHORIZED,
        &bad_headers,
        &bad_body,
        "unauthorized",
    );
    assert_eq!(bad_body["title"], "Unauthorized");

    let (_, unknown_headers, unknown_bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "nobody@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    let unknown_body = json_of(&unknown_bytes);
    assert_problem(
        StatusCode::UNAUTHORIZED,
        &unknown_headers,
        &unknown_body,
        "unauthorized",
    );
    for key in ["type", "title", "status"] {
        assert_eq!(
            bad_body[key], unknown_body[key],
            "unknown-email and wrong-password 401 must share shape (key {key})"
        );
    }
    assert!(unknown_body.get("errors").is_none(), "no errors[] on 401");

    // 3. Login has no strength gate: a weak-but-wellformed stored password
    //    still authenticates (200 on match, 401 on mismatch — never 400).
    let weak_hash =
        backend_rs::features::auth::store::hash_password("weakpass", 1).expect("weak hash mints");
    sqlx::query("UPDATE `users` SET `password` = ? WHERE `email` = 'login-user@example.com'")
        .bind(&weak_hash)
        .execute(&pool)
        .await
        .expect("weak password stored");
    let (status, _, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "weakpass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "weak match: {}", json_of(&bytes));
    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "alsoweak",
            None,
        ),
    )
    .await;
    let mismatch_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "weak mismatch: {mismatch_body}"
    );
    assert_problem(status, &headers, &mismatch_body, "unauthorized");
    // Restore a strong password for the remaining scenarios.
    let strong_hash =
        backend_rs::features::auth::store::hash_password("Str0ng!pass", 1).expect("hash mints");
    sqlx::query("UPDATE `users` SET `password` = ? WHERE `email` = 'login-user@example.com'")
        .bind(&strong_hash)
        .execute(&pool)
        .await
        .expect("strong password restored");
    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "re-login: {}", json_of(&bytes));
    let login_jar = cookie_jar(&headers);

    // 4. Login validation: bad email / empty / overlong password -> 400
    //    problem+json with `errors[]` (byte cap, not chars).
    for (email, password) in [
        ("not-an-email", "Str0ng!pass"),
        ("login-user@example.com", ""),
        ("login-user@example.com", &"a".repeat(73)),
    ] {
        let (status, headers, bytes) =
            send(app(), post("/api/v1/auth/login", email, password, None)).await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "input {email:?}: {failure}"
        );
        assert_problem(status, &headers, &failure, "validation-error");
        assert!(failure["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()));
    }

    // 5. Legacy bcrypt (cost-10 `$2b$`) verifies, then upgrades to Argon2id
    //    in the same request (DB re-read proves the upgrade).
    let bcrypt_hash = bcrypt::hash("LegacyPass1", 10).expect("cost-10 bcrypt mints");
    assert!(
        bcrypt_hash.starts_with("$2b$10$"),
        "cost-10 fixture shape: {bcrypt_hash}"
    );
    sqlx::query("INSERT INTO `users` (`email`, `password`) VALUES ('legacy@example.com', ?)")
        .bind(&bcrypt_hash)
        .execute(&pool)
        .await
        .expect("legacy bcrypt user inserted");
    let (status, _, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "legacy@example.com",
            "LegacyPass1",
            None,
        ),
    )
    .await;
    let legacy_body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "bcrypt login: {legacy_body}");
    assert_eq!(legacy_body["data"]["email"], "legacy@example.com");
    let stored: String =
        sqlx::query_scalar("SELECT `password` FROM `users` WHERE `email` = 'legacy@example.com'")
            .fetch_one(&pool)
            .await
            .expect("upgraded hash re-reads");
    assert!(
        stored.starts_with("$argon2id$v=19$m=19456,t=1,p=1$"),
        "bcrypt success must rehash to Argon2id, got: {}",
        &stored[..stored.len().min(40)]
    );
    assert_ne!(stored, bcrypt_hash, "hash must actually change");
    // Wrong legacy password still 401s (and does not upgrade).
    let (status, _, _) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "legacy@example.com",
            "LegacyPass2",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 6. `me` with the login cookie -> 200 `{data:{id,email,role}}`;
    //    anonymous -> 401 problem+json.
    let (status, _, bytes) = send(app(), get("/api/v1/auth/me", Some(&login_jar))).await;
    let me_body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "me: {me_body}");
    assert_eq!(me_body["data"]["email"], "login-user@example.com");
    assert_eq!(me_body["data"]["role"], "user");
    let (status, headers, bytes) = send(app(), get("/api/v1/auth/me", None)).await;
    let anon_body = json_of(&bytes);
    assert_eq!(status, StatusCode::UNAUTHORIZED, "anon me: {anon_body}");
    assert_problem(status, &headers, &anon_body, "unauthorized");

    // 7. Deprecated `/api/*` alias serves the same handlers.
    let (status, _, bytes) = send(
        app(),
        post(
            "/api/auth/login",
            "login-user@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alias login: {}", json_of(&bytes));
    let alias_jar = {
        let (_, headers, _) = send(
            app(),
            post(
                "/api/auth/login",
                "login-user@example.com",
                "Str0ng!pass",
                None,
            ),
        )
        .await;
        cookie_jar(&headers)
    };
    let (status, _, bytes) = send(app(), get("/api/auth/me", Some(&alias_jar))).await;
    assert_eq!(status, StatusCode::OK, "alias me: {}", json_of(&bytes));

    // 8. Dev/test cookie matrix: `connect.sid`, HttpOnly + SameSite=Lax +
    //    Path=/, no Secure, no Domain, no `__Host-` prefix.
    let raw_dev = {
        let (_, headers, _) = send(
            app(),
            post(
                "/api/v1/auth/login",
                "login-user@example.com",
                "Str0ng!pass",
                None,
            ),
        )
        .await;
        raw_set_cookies(&headers)
    };
    assert!(
        raw_dev.contains("connect.sid="),
        "dev cookie name: {raw_dev}"
    );
    assert!(
        !raw_dev.contains("__Host-"),
        "dev must not use __Host-: {raw_dev}"
    );
    for attr in ["HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(raw_dev.contains(attr), "dev must carry {attr}: {raw_dev}");
    }
    assert!(
        !raw_dev.contains("Secure"),
        "dev must not be Secure: {raw_dev}"
    );
    assert!(
        !raw_dev.contains("Domain="),
        "dev must not set Domain: {raw_dev}"
    );

    // 9. Prod cookie matrix: `__Host-connect.sid` + Secure, still
    //    HttpOnly + SameSite=Lax + Path=/, no Domain.
    let prod = app_on_pool(&pool, None, Some("production"));
    let (status, headers, bytes) = send(
        prod,
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "prod login: {}", json_of(&bytes));
    let raw_prod = raw_set_cookies(&headers);
    assert!(
        raw_prod.contains("__Host-connect.sid="),
        "prod cookie name: {raw_prod}"
    );
    for attr in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(
            raw_prod.contains(attr),
            "prod must carry {attr}: {raw_prod}"
        );
    }
    assert!(
        !raw_prod.contains("Domain="),
        "prod must not set Domain: {raw_prod}"
    );

    // 10. Expired session row (past TIMESTAMP — the store uses
    //     `timestamp(6)`, not epoch-ms) is ignored -> 401. Scoped to ONE row
    //     this suite minted (`WHERE id = ?`): the store is server-global, so
    //     the old bare `UPDATE ... session` (no WHERE) expired concurrent
    //     suites' live jars too — the same race class as the retired wipe.
    let (expiry_jar, expiry_id) = login_fresh_session(&app, &pool).await;
    sqlx::query(
        "UPDATE `tower_sessions`.`session` SET `expiry_date` = '2000-01-01 00:00:00.000000' WHERE `id` = ?",
    )
    .bind(&expiry_id)
    .execute(&pool)
    .await
    .expect("own session expired");
    let (status, headers, bytes) = send(app(), get("/api/v1/auth/me", Some(&expiry_jar))).await;
    let expired_body = json_of(&bytes);
    assert_eq!(status, StatusCode::UNAUTHORIZED, "expired: {expired_body}");
    assert_problem(status, &headers, &expired_body, "unauthorized");
    // Control: an unexpired jar must still validate, proving the expiry
    // touched only our row (what the old global UPDATE violated).
    let (status, _, bytes) = send(app(), get("/api/v1/auth/me", Some(&login_jar))).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpired jar survives scoped expiry: {}",
        json_of(&bytes)
    );
    // Re-login for the remaining scenarios.
    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "login-user@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "post-expiry login: {}",
        json_of(&bytes)
    );
    let login_jar = cookie_jar(&headers);

    // 11. Cookie signed with the wrong `SESSION_SECRET` fails verification
    //     (force re-login proof — no Nest-row seeding, codecs incompatible
    //     by design) -> 401.
    let other_secret = app_on_pool(&pool, Some("a-completely-different-secret-32bytes!!"), None);
    let (status, headers, bytes) =
        send(other_secret, get("/api/v1/auth/me", Some(&login_jar))).await;
    let rotated_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "wrong secret: {rotated_body}"
    );
    assert_problem(status, &headers, &rotated_body, "unauthorized");

    // 12. `me` re-lookup: deleted user -> 401.
    let (status, _, _bytes) = send(
        app(),
        post(
            "/api/v1/auth/register",
            "doomed@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "doomed@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "doomed login: {}", json_of(&bytes));
    let doomed_jar = cookie_jar(&headers);
    sqlx::query("DELETE FROM `users` WHERE `email` = 'doomed@example.com'")
        .execute(&pool)
        .await
        .expect("doomed user deleted");
    let (status, headers, bytes) = send(app(), get("/api/v1/auth/me", Some(&doomed_jar))).await;
    let deleted_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "deleted user me: {deleted_body}"
    );
    assert_problem(status, &headers, &deleted_body, "unauthorized");

    // 13. `me` re-lookup: demoted admin (stale session role) -> 403, with
    //     session rotation on the privilege change.
    let (status, _, _bytes) = send(
        app(),
        post(
            "/api/v1/auth/register",
            "boss@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    sqlx::query("UPDATE `users` SET `role` = 'admin' WHERE `email` = 'boss@example.com'")
        .execute(&pool)
        .await
        .expect("boss promoted");
    let (status, headers, bytes) = send(
        app(),
        post(
            "/api/v1/auth/login",
            "boss@example.com",
            "Str0ng!pass",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "boss login: {}", json_of(&bytes));
    assert_eq!(json_of(&bytes)["data"]["role"], "admin");
    let boss_jar = cookie_jar(&headers);
    let (status, _, bytes) = send(app(), get("/api/v1/auth/me", Some(&boss_jar))).await;
    assert_eq!(status, StatusCode::OK, "admin me: {}", json_of(&bytes));
    sqlx::query("UPDATE `users` SET `role` = 'user' WHERE `email` = 'boss@example.com'")
        .execute(&pool)
        .await
        .expect("boss demoted");
    let (status, headers, bytes) = send(app(), get("/api/v1/auth/me", Some(&boss_jar))).await;
    let demoted_body = json_of(&bytes);
    assert_eq!(status, StatusCode::FORBIDDEN, "demoted me: {demoted_body}");
    assert_problem(status, &headers, &demoted_body, "forbidden");
    assert_ne!(
        cookie_jar(&headers),
        boss_jar,
        "privilege change must rotate the session id"
    );

    // 14. Logout -> 204: destroys the row (id-set shrinks by exactly the
    //     logged-out row — set difference, not an absolute count, so a
    //     concurrent suite's inserts can't flake it; nothing else deletes
    //     rows) + clears the cookie; the old jar then 401s.
    let ids_before = session_ids(&pool).await;
    assert!(!ids_before.is_empty(), "sessions exist before logout");
    let (status, headers, bytes) =
        send(app(), post_empty("/api/v1/auth/logout", Some(&login_jar))).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "logout: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(bytes.is_empty(), "204 carries no body");
    let raw_logout = raw_set_cookies(&headers);
    assert!(
        !raw_logout.is_empty(),
        "logout must clear the cookie: {raw_logout}"
    );
    assert!(
        raw_logout.contains("Max-Age=0"),
        "logout must expire the cookie: {raw_logout}"
    );
    let ids_after = session_ids(&pool).await;
    let removed: Vec<&String> = ids_before
        .iter()
        .filter(|id| !ids_after.contains(id))
        .collect();
    assert_eq!(
        removed.len(),
        1,
        "logout must destroy exactly its row (concurrent adds are ignored by set difference)"
    );
    // DB re-read via a second call: the old jar is anonymous now.
    let (status, headers, bytes) = send(app(), get("/api/v1/auth/me", Some(&login_jar))).await;
    let logged_out_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "post-logout me: {logged_out_body}"
    );
    assert_problem(status, &headers, &logged_out_body, "unauthorized");
    // Anonymous logout still 204s (never 4xx on the destroy path).
    let (status, _, _) = send(app(), post_empty("/api/v1/auth/logout", None)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // NO TEARDOWN WIPE/DROP: the shared store must survive until every
    // suite in this process is done (a concurrent drop 500s in-flight
    // suites with 1049). Leftover rows are harmless: per-test DBs are
    // fresh every process and no suite asserts store emptiness.
}
