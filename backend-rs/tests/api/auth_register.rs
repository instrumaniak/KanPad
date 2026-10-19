//! `POST /api/v1/auth/register` (T10).
//!
//! Single `#[sqlx::test]` setup on purpose: all register scenarios run
//! sequentially inside one test against one pool, with distinct emails per
//! scenario. The `sessions` store table lives in THIS test's own database
//! (the store follows the pool's database — see `AppState::with_pool`),
//! so no other suite can reach it: this suite never wipes it, and NOBODY
//! drops the schema (baseline DDL is `IF NOT EXISTS` since early-T52, so
//! the old fail-fast `DROP SCHEMA` hygiene is retired; a concurrent drop
//! would 500 in-flight suites with 1049 `Unknown database`). This suite's
//! cookie assertions need no live jars beyond the request that mints them.
//!
//! Covered: 201 + `{data:{id,email,role}}` + `Location: /api/v1/auth/me` +
//! `Set-Cookie` (+ `cycle_id` rotation proof); DB re-read finds the user +
//! exactly 6 labels + an Argon2id PHC hash (`m=19456,t=HASH_COST,p=1`, never
//! bcrypt); dup email -> 409 problem+json; bad email / weak / overlong
//! password -> 400 problem+json with `errors[]`;
//! `REGISTRATION_ENABLED=false` -> 403 problem+json.

use axum::{
    body::Body,
    http::{header, HeaderMap, Request, StatusCode},
    Router,
};
use backend_rs::config::Settings;
use sqlx::MySqlPool;

use super::helpers::{app_on_pool_with, assert_problem, cookie_jar, send_json};

/// This suite's register payload (its own email/password ordering).
///
/// Named `register_body`, not `json_body`: `json_body` is owned by
/// `super::helpers` (see that file's ownership header).
fn register_body(password: &str, email: &str) -> String {
    serde_json::json!({ "email": email, "password": password }).to_string()
}

/// Build the app on the `#[sqlx::test]` pool (in-process, no sweeper —
/// `create_app` never spawns one), with a registration-gate override.
fn app_with_registration(pool: &MySqlPool, registration_enabled: bool) -> Router {
    app_on_pool_with(
        pool,
        Settings {
            registration_enabled,
            ..Default::default()
        },
    )
}

fn post_register(email: &str, password: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder =
        Request::post("/api/v1/auth/register").header(header::CONTENT_TYPE, "application/json");
    if let Some(jar) = cookie {
        builder = builder.header(header::COOKIE, jar);
    }
    builder
        .body(Body::from(register_body(password, email)))
        .expect("register request builds")
}

fn raw_set_cookies(headers: &HeaderMap) -> String {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n")
}

#[sqlx::test(migrations = "./migrations")]
async fn register_full_flow(pool: MySqlPool) {
    let app = || app_with_registration(&pool, true);

    // 1. Happy path: mixed-case + padded email normalizes; 201 + Location +
    //    Set-Cookie; body carries no `message` key.
    let (status, headers, body) = send_json(
        app(),
        post_register("  NewUser@Example.COM ", "Str0ng!pass", None),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "happy path: {body}");
    assert_eq!(body["data"]["email"], "newuser@example.com");
    assert_eq!(body["data"]["role"], "user");
    let user_id = body["data"]["id"].as_i64().expect("id is a number");
    assert!(user_id > 0, "id must be positive");
    assert!(
        body.get("message").is_none(),
        "no `message` envelope key: {body}"
    );
    assert_eq!(
        headers.get(header::LOCATION).expect("201 sets Location"),
        "/api/v1/auth/me"
    );
    let jar_after_first = cookie_jar(&headers);
    assert!(
        !jar_after_first.is_empty(),
        "register must establish a session (Set-Cookie)"
    );
    let raw_first = raw_set_cookies(&headers);

    // DB re-read: user row + Argon2id PHC params + exactly 6 default labels.
    let row: (i64, String, String, String) = sqlx::query_as(
        "SELECT `id`, `email`, `role`, `password` FROM `users` WHERE `email` = 'newuser@example.com'",
    )
    .fetch_one(&pool)
    .await
    .expect("user row persists");
    assert_eq!(row.0, user_id);
    assert_eq!(row.1, "newuser@example.com");
    assert_eq!(row.2, "user");
    assert!(
        row.3.starts_with("$argon2id$v=19$m=19456,t=1,p=1$"),
        "Argon2id PHC with m=19456,t=HASH_COST(1),p=1, got: {}",
        &row.3[..row.3.len().min(40)]
    );
    assert!(
        !row.3.starts_with("$2a$") && !row.3.starts_with("$2b$"),
        "must not be a bcrypt hash"
    );
    let label_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
        .bind(user_id as i32)
        .fetch_one(&pool)
        .await
        .expect("label count reads");
    assert_eq!(label_count, 6, "register seeds exactly 6 default labels");

    // 2. Session fixation defense: registering again with the first session
    //    cookie rotates the session id (`cycle_id` on register).
    let (status, headers, _) = send_json(
        app(),
        post_register("second@example.com", "Str0ng!pass", Some(&jar_after_first)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "second register");
    let raw_second = raw_set_cookies(&headers);
    assert!(
        !raw_second.is_empty(),
        "second register must also set a session cookie"
    );
    assert_ne!(
        raw_second, raw_first,
        "session id must rotate on register (cycle_id)"
    );

    // 3. Duplicate email -> 409 problem+json; no second row, labels untouched.
    let (status, headers, body) = send_json(
        app(),
        post_register("newuser@example.com", "Str0ng!pass", None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "dup email: {body}");
    assert_problem(
        status,
        &headers,
        &body,
        "conflict",
        "Duplicate email -> 409 problem+json",
    );
    let dup_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `users` WHERE `email` = 'newuser@example.com'")
            .fetch_one(&pool)
            .await
            .expect("dup count reads");
    assert_eq!(dup_count, 1, "dup register must not insert a second row");
    let label_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ?")
        .bind(user_id as i32)
        .fetch_one(&pool)
        .await
        .expect("label count re-reads");
    assert_eq!(label_count, 6, "failed register must not reseed labels");

    // 4. Bad email / weak password -> 400 problem+json with `errors[]`.
    for (email, password) in [
        ("not-an-email", "Str0ng!pass"),
        ("weak@example.com", "password"),
        ("weak2@example.com", "short1!"),
    ] {
        let (status, headers, body) = send_json(app(), post_register(email, password, None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "input {email:?}: {body}");
        assert_problem(
            status,
            &headers,
            &body,
            "validation-error",
            "Bad email / weak password -> 400 problem+json with…",
        );
        let errors = body["errors"].as_array().expect("errors[] present");
        assert!(!errors.is_empty(), "errors[] must not be empty: {body}");
    }

    // 5. Overlong passwords -> 400 (bytes, not chars): 73 ASCII bytes, and a
    //    38-char / 75-byte multibyte input that passes strength on its own.
    let multibyte = format!("1{}", "é".repeat(37));
    assert_eq!(multibyte.len(), 75, "fixture sanity: bytes, not chars");
    let overlong: Vec<(String, String)> = vec![
        ("long1@example.com".to_string(), "a".repeat(71) + "1!"),
        ("long2@example.com".to_string(), multibyte.clone()),
    ];
    for (email, password) in &overlong {
        let (status, headers, body) = send_json(app(), post_register(email, password, None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "input {email:?}: {body}");
        assert_problem(
            status,
            &headers,
            &body,
            "validation-error",
            "Overlong passwords -> 400 (bytes, not chars): 73 ASCII…",
        );
    }

    // 6. Closed registration -> 403 problem+json (role-denied, not 404), and
    //    no user row is created.
    let closed = app_with_registration(&pool, false);
    let (status, headers, body) = send_json(
        closed,
        post_register("closed@example.com", "Str0ng!pass", None),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "closed registration: {body}");
    assert_problem(
        status,
        &headers,
        &body,
        "forbidden",
        "Closed registration -> 403 problem+json (role-denied, not…",
    );
    let closed_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `users` WHERE `email` = 'closed@example.com'")
            .fetch_one(&pool)
            .await
            .expect("closed count reads");
    assert_eq!(closed_count, 0, "closed register must not insert");

    // NO TEARDOWN WIPE/DROP: the shared session store must survive until
    // every suite in this process is done (see module docs).
}
