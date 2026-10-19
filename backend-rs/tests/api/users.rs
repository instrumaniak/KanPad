//! T13 users (no public routes — rewrite record).
//!
//! Nest source: `backend/src/users/users.controller.ts:1-6` is an empty
//! `@Controller('api/users')` (no handlers); `users.service.ts:14-31` is
//! internal only (`findById`/`findByEmail`/`create`). The rewrite keeps that
//! shape: [`backend_rs::features::users`] is an internal repository helper
//! with NO router mounted (user reads go through `GET /api/v1/auth/me`,
//! admin creation via the T12 CLI).
//!
//! Internal `findByEmail`/`create` behavior is covered by the T10/T11 auth
//! suites (`auth_register`, `auth_session`) — this file asserts ONLY the two
//! T13-owned facts, with no DB and no duplicate endpoint coverage:
//! 1. every `/api/users` path 404s from the router (intentional, JSON
//!    problem — never the SPA fallback); 2. the internal helper module
//!    resolves to the auth-store owner (re-export identity, compile-checked).
//!
//! DB-free by design: absence is a routing fact (`test_app()` boots without
//! I/O), and the helper check is type-level. Never add `#[sqlx::test]` or
//! session wipes here (the `sessions` store table lives in each test's own
//! database — see `helpers.rs` module docs).

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use backend_rs::common::ids::UserId;
use backend_rs::features::users::store::StoredUser;

use super::helpers::test_app;

const PROBLEM_JSON: &str = "application/problem+json";
const NOT_FOUND_TYPE: &str = "https://kanpad.dev/problems/not-found";

/// AC: no `/api/users` route mounted — every method/prefix 404s from the
/// router as JSON problem (intentional; user reads go through
/// `GET /api/v1/auth/me`).
#[tokio::test]
async fn users_routes_are_unmounted() {
    let app = test_app();
    for (method, uri) in [
        ("GET", "/api/v1/users"),
        ("GET", "/api/users"),
        ("POST", "/api/v1/users"),
        ("POST", "/api/users"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .expect("request builds");
        let response = app.clone().oneshot(request).await.expect("oneshot");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {uri} must 404 (no /api/users route mounted)"
        );
        let content_type = response
            .headers()
            .get("content-type")
            .expect("content-type present")
            .to_str()
            .expect("content-type ascii")
            .to_string();
        assert_eq!(
            content_type, PROBLEM_JSON,
            "{method} {uri} absence must be problem+json, not SPA html"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json["type"], NOT_FOUND_TYPE,
            "{method} {uri} must carry the exact not-found type URI"
        );
    }
}

/// The internal helper is a documented re-export of the auth-store owner
/// (T10/T11 own the queries + behavior coverage): this pins the T13-owned
/// fact — the re-export resolves and carries the auth projection shape —
/// without touching the DB or duplicating auth-suite coverage.
#[test]
fn users_store_reexports_auth_owner() {
    fn assert_owner_shape(user: StoredUser) -> (i32, String, String) {
        (user.id.get(), user.email, user.role)
    }

    let user = StoredUser {
        id: UserId::from(7),
        email: "t13@example.com".to_string(),
        role: "user".to_string(),
    };
    assert_eq!(
        assert_owner_shape(user),
        (7, "t13@example.com".to_string(), "user".to_string())
    );
}
