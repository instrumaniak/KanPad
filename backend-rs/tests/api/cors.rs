//! CORS preflight guard (T55, `54` B3) + the cross-origin guard
//! (T56, `54` B4).
//!
//! **The bug the preflight guard exists to prevent.** `CorsLayer::allow_methods`
//! is a hand-written list — axum does not expose the registered route table,
//! so the list CANNOT be derived from the router. It drifted: `PUT` was
//! missing while `PUT /api/v1/cards/{id}/labels/{label_id}` **is** served and
//! the frontend **calls** it (`frontend/src/features/cards/cards.api.ts`).
//! `PUT` is not a CORS-safelisted method, so the browser sends a
//! **preflight**; the server answered `Access-Control-Allow-Methods` without
//! `PUT` and the browser blocked the request. In the shipped topology
//! (compose sets `VITE_API_URL=http://localhost:3001`, so the SPA at `:5173`
//! calls `:3001` **cross-origin** and the Vite proxy is bypassed) every other
//! verb worked — only label-attach failed, with a browser-console CORS error
//! instead of an application error.
//!
//! **Why a test and not a derived list.** The class of bug is "a hand-
//! maintained policy drifts". The policy cannot be derived from the router,
//! so the guard must be automated: every verb the API actually serves must
//! survive a CORS preflight.
//!
//! **The cross-origin guard (T56).** The API authenticates with a cookie, so
//! a browser attaches the session cookie to a cross-site request. The defence
//! is the Go 1.25 `CrossOriginProtection` algorithm (tower-http 0.7's
//! `CsrfLayer`): safe methods always pass, `Sec-Fetch-Site: same-origin|none`
//! passes, `cross-site|same-site` passes ONLY for a trusted `Origin`, a
//! request with neither header passes (curl/proxy/native), and an `Origin`
//! whose authority equals the request's effective host passes (the safety net
//! that makes an EMPTY production allowlist workable).
//!
//! DB-free: a preflight never reaches the handler (the CORS layer answers it),
//! and the cross-origin probes target a path with no route (an allowed
//! request therefore ends on the API's JSON 404, a rejected one on the
//! cross-origin layer's 403 — both before any DB work).

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use backend_rs::{build_state, config::Settings, create_app};
use tower::ServiceExt;

use super::helpers::{assert_problem, json_of, send, test_app, test_settings};

/// The dev-default allowlisted origin (`config::DEFAULT_CORS_ORIGIN`).
const ORIGIN: &str = "http://localhost:5173";

/// Every verb the API serves. `PUT` is the one that drifted — it is here
/// because `PUT /api/v1/cards/{id}/labels/{label_id}` is a real route.
const SERVED_VERBS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

/// A path with no route: an ALLOWED request ends on the API's JSON 404, a
/// REJECTED one on the cross-origin layer's 403 — both before any DB work.
const PROBE_PATH: &str = "/api/v1/__csrf_probe";

/// Send a CORS preflight for `method` and return the response.
async fn preflight(method: &str) -> axum::response::Response {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/cards/1/labels/2")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
        .body(Body::empty())
        .unwrap();
    test_app().oneshot(request).await.unwrap()
}

/// Build a cross-origin probe request.
fn probe(
    method: &str,
    sec_fetch_site: Option<&str>,
    origin: Option<&str>,
    host: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(PROBE_PATH);
    if let Some(site) = sec_fetch_site {
        builder = builder.header("sec-fetch-site", site);
    }
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    if let Some(host) = host {
        builder = builder.header(header::HOST, host);
    }
    builder.body(Body::empty()).expect("request builds")
}

/// Every served verb must survive a preflight (the drift guard).
#[tokio::test]
async fn preflight_allows_every_served_verb() {
    for method in SERVED_VERBS {
        let response = preflight(method).await;
        assert!(
            response.status().is_success(),
            "preflight for {method} must succeed, got {}",
            response.status()
        );
        let allow = response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            allow.contains(method),
            "CORS must allow {method}, got {allow:?}"
        );
    }
}

/// The preflight must echo the allowlisted origin and the credentials
/// permission — the SPA authenticates with a cookie, so
/// `Access-Control-Allow-Credentials` is load-bearing.
#[tokio::test]
async fn preflight_allows_the_configured_origin_with_credentials() {
    let response = preflight("POST").await;
    let allow_origin = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        allow_origin, ORIGIN,
        "the allowlisted origin must be echoed verbatim"
    );
    let credentials = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        credentials, "true",
        "the cookie session needs Access-Control-Allow-Credentials"
    );
}

/// A FOREIGN origin must NOT be granted access — the allowlist is explicit,
/// never `Any` (the layer pairs `allow_credentials(true)`, which browsers
/// reject with `*`).
#[tokio::test]
async fn preflight_does_not_grant_a_foreign_origin() {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/cards/1/labels/2")
        .header(header::ORIGIN, "https://evil.example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();
    let allow_origin = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        !allow_origin.contains("evil.example.com"),
        "a foreign origin must never be granted access, got {allow_origin:?}"
    );
}

/// The CORS preflight is answered by the CORS layer, NOT by a route: an
/// unknown path still gets a CORS answer (and never a 404 body).
#[tokio::test]
async fn preflight_is_answered_before_the_router() {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/definitely-not-a-route")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the CORS layer answers the preflight before routing"
    );
}

// ---------------------------------------------------------------------------
// T56 — the cross-origin check on mutating requests
// ---------------------------------------------------------------------------

/// One row of the cross-origin matrix.
struct OriginCase {
    /// What the row proves (the failure label).
    what: &'static str,
    /// HTTP method.
    method: &'static str,
    /// `Sec-Fetch-Site` header (absent = not sent).
    sec_fetch_site: Option<&'static str>,
    /// `Origin` header (absent = not sent).
    origin: Option<&'static str>,
    /// `Host` header (absent = not sent).
    host: Option<&'static str>,
    /// Whether the request must pass the cross-origin check.
    allowed: bool,
}

/// The Go 1.25 `CrossOriginProtection` matrix, step for step.
const ORIGIN_CASES: &[OriginCase] = &[
    OriginCase {
        what: "a safe GET is always allowed",
        method: "GET",
        sec_fetch_site: Some("cross-site"),
        origin: Some("https://evil.example.com"),
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "a safe HEAD is always allowed",
        method: "HEAD",
        sec_fetch_site: Some("cross-site"),
        origin: Some("https://evil.example.com"),
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "Sec-Fetch-Site: same-origin is allowed",
        method: "POST",
        sec_fetch_site: Some("same-origin"),
        origin: Some("https://app.example.com"),
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "Sec-Fetch-Site: none is allowed",
        method: "POST",
        sec_fetch_site: Some("none"),
        origin: None,
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "Sec-Fetch-Site: cross-site with an untrusted origin is rejected",
        method: "POST",
        sec_fetch_site: Some("cross-site"),
        origin: Some("https://evil.example.com"),
        host: None,
        allowed: false,
    },
    OriginCase {
        what: "Sec-Fetch-Site: same-site with an untrusted origin is rejected",
        method: "POST",
        sec_fetch_site: Some("same-site"),
        origin: Some("https://evil.example.com"),
        host: None,
        allowed: false,
    },
    OriginCase {
        what: "Sec-Fetch-Site: same-site with a TRUSTED origin is allowed",
        method: "POST",
        sec_fetch_site: Some("same-site"),
        origin: Some(ORIGIN),
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "no Sec-Fetch-Site and no Origin is allowed (curl/proxy/native)",
        method: "POST",
        sec_fetch_site: None,
        origin: None,
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "no Sec-Fetch-Site with an untrusted Origin is rejected",
        method: "POST",
        sec_fetch_site: None,
        origin: Some("https://evil.example.com"),
        host: None,
        allowed: false,
    },
    OriginCase {
        what: "a trusted Origin without Sec-Fetch-Site is allowed",
        method: "POST",
        sec_fetch_site: None,
        origin: Some(ORIGIN),
        host: None,
        allowed: true,
    },
    OriginCase {
        what: "an Origin whose authority equals the request host is allowed",
        method: "POST",
        sec_fetch_site: None,
        origin: Some("http://localhost:3001"),
        host: Some("localhost:3001"),
        allowed: true,
    },
    OriginCase {
        what: "an Origin whose authority differs from the request host is rejected",
        method: "POST",
        sec_fetch_site: None,
        origin: Some("http://evil.example.com:3001"),
        host: Some("localhost:3001"),
        allowed: false,
    },
];

/// The cross-origin matrix (table test).
///
/// An ALLOWED request must reach the router (the API's JSON 404 on the
/// probe path); a REJECTED one must be answered by the cross-origin layer
/// with a **403 problem+json** carrying the EXISTING `forbidden` problem
/// type — no new problem-type URI is introduced by this change.
#[tokio::test]
async fn cross_origin_matrix_is_enforced() {
    let app = test_app();
    for case in ORIGIN_CASES {
        let request = probe(case.method, case.sec_fetch_site, case.origin, case.host);
        let (status, headers, bytes) = send(app.clone(), request).await;
        if case.allowed {
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "{}: an allowed request must reach the router",
                case.what
            );
        } else {
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{}: a rejected request must be 403",
                case.what
            );
            let body = json_of(&bytes);
            assert_problem(status, &headers, &body, "forbidden", case.what);
        }
    }
}

/// `OPTIONS` is a SAFE method in the algorithm (step 1), so the cross-origin
/// layer always lets it through — the CORS layer, not this one, answers a
/// preflight. (`OPTIONS` on a path with no `OPTIONS` route ends on axum's
/// method fallback, which is NOT the cross-origin layer's 403, so the
/// assertion is deliberately about the ABSENCE of 403.)
#[tokio::test]
async fn a_safe_options_request_is_never_rejected_by_the_cross_origin_layer() {
    let app = test_app();
    let request = probe(
        "OPTIONS",
        Some("cross-site"),
        Some("https://evil.example.com"),
        None,
    );
    let (status, _, _) = send(app, request).await;
    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "OPTIONS is a safe method — the cross-origin layer must not reject it"
    );
}

/// The compose dev topology: the SPA at `http://localhost:5173` calls the API
/// at `http://localhost:3001` — a different PORT, so the browser sends
/// `Sec-Fetch-Site: same-site` (NOT `same-origin`). The request is allowed
/// because the origin is TRUSTED by default.
#[tokio::test]
async fn the_compose_dev_topology_same_site_origin_is_trusted() {
    let app = test_app();
    let request = probe("POST", Some("same-site"), Some(ORIGIN), None);
    let (status, _, _) = send(app, request).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the default-allowlisted dev origin must pass the cross-origin check"
    );
}

/// The `Origin`/`Host` fallback: an empty allowlist works with the
/// allowlist — the operator does not have to configure `CORS_ORIGINS` for
/// the production single-origin deployment to keep working.
#[tokio::test]
async fn same_origin_is_allowed_with_an_empty_allowlist() {
    let settings = Settings {
        cors_origins: String::new(),
        ..test_settings()
    };
    let app = create_app(build_state(settings));
    let request = probe("POST", Some("same-origin"), None, None);
    let (status, _, _) = send(app, request).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "same-origin must pass the cross-origin check with an empty allowlist"
    );
}

/// ONE canonical list feeds TWO consumers: the CORS allowlist and the
/// cross-origin trusted set. If they could disagree, the API would answer a
/// preflight for an origin it then rejects.
///
/// The configured entry is deliberately **NOT** in canonical form (uppercase
/// scheme + host). A `Settings` built by hand never passes boot validation,
/// so this is exactly the shape a mis-written config would take — and if any
/// of the two consumers read the RAW list instead of the canonicalizer, the
/// uppercase spelling would leak into its output and the assertions below
/// would see it.
#[tokio::test]
async fn one_canonical_list_feeds_cors_cross_origin_and_csrf() {
    const RAW: &str = "HTTPS://APP.EXAMPLE.COM";
    const CANONICAL: &str = "https://app.example.com";
    let settings = Settings {
        cors_origins: RAW.to_string(),
        ..test_settings()
    };
    let app = create_app(build_state(settings));

    // 1. the CORS layer allows the canonical origin.
    let preflight = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/cards/1/labels/2")
                .header(header::ORIGIN, CANONICAL)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let allow_origin = preflight
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        allow_origin, CANONICAL,
        "the CORS layer must allow the canonical origin"
    );

    // 2. the cross-origin layer trusts it (a cross-site request from it is
    //    therefore allowed).
    let request = probe("POST", Some("cross-site"), Some(CANONICAL), None);
    let (status, _, _) = send(app, request).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the cross-origin layer must trust the canonical origin"
    );
}
