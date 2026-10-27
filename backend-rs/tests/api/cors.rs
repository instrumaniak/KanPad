//! CORS preflight guard (T55, `54` B3) + the cross-origin / CSP guards
//! (T56, `54` B4 + B7).
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
//! that makes an EMPTY production allowlist workable for a same-origin SPA).
//!
//! **The CSP guard (T56).** The Rust service serves the SPA itself, so it
//! MUST carry a `Content-Security-Policy`. The policy's `script-src` allows
//! the SPA's inline theme bootstrap by its **hash** — computed at boot from
//! the `public/index.html` the server actually serves, never hand-written.
//!
//! DB-free: a preflight never reaches the handler (the CORS layer answers it),
//! and the cross-origin probes target a path with no route (an allowed
//! request therefore ends on the API's JSON 404, a rejected one on the
//! cross-origin layer's 403 — both before any DB work).

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use backend_rs::{build_state, config::Settings, create_app, create_app_with_csp};
use tower::ServiceExt;

use super::helpers::{assert_problem, json_of, send, test_app, test_settings};

/// The dev-default allowlisted origin (`config::DEFAULT_CORS_ORIGIN`).
const ORIGIN: &str = "http://localhost:5173";

/// Every verb the API serves. `PUT` is the one that drifted — it is here
/// because `PUT /api/v1/cards/{id}/labels/{label_id}` is a real route.
const SERVED_VERBS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

/// The served SPA index, as a FIXTURE (`backend-rs/public/` is not in git —
/// the Dockerfile copies `frontend/dist/` into it before the image build, so
/// the file does not exist in a dev tree). The fixture mirrors the real
/// `frontend/index.html`: ONE inline theme bootstrap + ONE external module
/// script.
const SPA_INDEX_FIXTURE: &str = include_str!("../fixtures/spa-index.html");

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

/// The `Content-Security-Policy` header of a response, if present.
fn csp_of(response: &Response) -> Option<String> {
    response
        .headers()
        .get("content-security-policy")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// The value of one `;`-separated CSP directive (everything AFTER its name —
/// `directive(p, "default-src") == Some("'self'")`).
///
/// Assertions are made PER DIRECTIVE (`script-src`, `connect-src`, …) rather
/// than on the whole policy string: `'unsafe-inline'` is legitimate in
/// `style-src` and fatal in `script-src`, so a substring check over the whole
/// header cannot tell the two apart.
fn directive<'a>(policy: &'a str, name: &str) -> Option<&'a str> {
    policy.split(';').map(str::trim).find_map(|part| {
        let mut words = part.split_whitespace();
        if words.next() != Some(name) {
            return None;
        }
        // `part` starts with `name`, so this slice is a char boundary.
        Some(part[name.len()..].trim())
    })
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

/// The `Origin`/`Host` fallback: a same-origin SPA works with an **EMPTY**
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

/// ONE canonical list feeds THREE consumers: the CORS allowlist, the
/// cross-origin trusted set and the CSP's `connect-src`. If they could
/// disagree, the API would answer a preflight for an origin it then
/// rejects — or advertise an origin in the CSP it does not serve.
///
/// The configured entry is deliberately **NOT** in canonical form (uppercase
/// scheme + host). A `Settings` built by hand never passes boot validation,
/// so this is exactly the shape a mis-written config would take — and if any
/// of the three consumers read the RAW list instead of the canonicalizer, the
/// uppercase spelling would leak into its output and the assertions below
/// would see it.
#[tokio::test]
async fn one_canonical_list_feeds_cors_cross_origin_and_csp() {
    const RAW: &str = "HTTPS://APP.EXAMPLE.COM";
    const CANONICAL: &str = "https://app.example.com";
    let settings = Settings {
        cors_origins: RAW.to_string(),
        ..test_settings()
    };
    let app = create_app(build_state(settings));

    // 1. the CSP's connect-src names the CANONICAL origin (not the raw one).
    let response = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let csp = csp_of(&response).expect("the CSP header must be present");
    let connect_src = directive(&csp, "connect-src").expect("connect-src is part of the policy");
    let sources: Vec<&str> = connect_src.split_whitespace().collect();
    assert!(
        sources.contains(&CANONICAL),
        "the CSP's connect-src must name the canonical origin: {csp}"
    );
    assert!(
        !sources.contains(&RAW),
        "the raw (non-canonical) entry must never reach the CSP: {csp}"
    );

    // 2. the CORS layer allows the canonical origin.
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

    // 3. the cross-origin layer trusts it (a cross-site request from it is
    //    therefore allowed).
    let request = probe("POST", Some("cross-site"), Some(CANONICAL), None);
    let (status, _, _) = send(app, request).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the cross-origin layer must trust the canonical origin"
    );
}

// ---------------------------------------------------------------------------
// T56 — the CSP header
// ---------------------------------------------------------------------------

/// The CSP header must be on a 200 **and** on a 404.
#[tokio::test]
async fn csp_header_is_present_on_a_200_and_on_a_404() {
    let app = test_app();
    let ok = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let policy = csp_of(&ok).expect("a 200 must carry the CSP header");
    assert_eq!(
        directive(&policy, "default-src"),
        Some("'self'"),
        "unexpected policy: {policy}"
    );
    assert_eq!(
        directive(&policy, "frame-ancestors"),
        Some("'none'"),
        "unexpected policy: {policy}"
    );
    assert_eq!(
        directive(&policy, "base-uri"),
        Some("'self'"),
        "unexpected policy: {policy}"
    );
    assert_eq!(
        directive(&policy, "form-action"),
        Some("'self'"),
        "unexpected policy: {policy}"
    );
    // `style-src 'unsafe-inline'` is REQUIRED (the frontend uses inline
    // `style={{…}}` props); a browser ignores `'unsafe-inline'` in
    // `script-src` when a hash is present, but the directive must not carry
    // it at all — nor `'unsafe-eval'`.
    let style_src = directive(&policy, "style-src").expect("style-src is part of the policy");
    assert!(
        style_src
            .split_whitespace()
            .any(|src| src == "'unsafe-inline'"),
        "the frontend needs inline styles: {policy}"
    );
    let script_src = directive(&policy, "script-src").expect("script-src is part of the policy");
    assert!(
        !script_src.contains("'unsafe-inline'"),
        "script-src must not allow inline scripts: {policy}"
    );
    assert!(
        !script_src.contains("'unsafe-eval'"),
        "script-src must not allow eval: {policy}"
    );

    let missing = app
        .oneshot(
            Request::get("/api/v1/definitely-not-a-route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert!(
        csp_of(&missing).is_some(),
        "a 404 must carry the CSP header too"
    );
}

/// `connect-src` must be `'self'` **plus the CORS allowlist** — in the
/// shipped dev topology the SPA is served by Vite at `http://localhost:5173`
/// and calls the API at `http://localhost:3001` (cross-origin), so
/// `'self'` alone would have the browser block every API call.
#[tokio::test]
async fn csp_connect_src_lists_the_cors_allowlist() {
    let app = test_app();
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let policy = csp_of(&response).expect("the CSP header must be present");
    let connect_src = directive(&policy, "connect-src").expect("connect-src is part of the policy");
    let sources: Vec<&str> = connect_src.split_whitespace().collect();
    assert!(
        sources.contains(&"'self'"),
        "connect-src must always allow the serving origin: {policy}"
    );
    assert!(
        sources.contains(&ORIGIN),
        "connect-src must name the allowlisted origin {ORIGIN}: {policy}"
    );
}

/// The inline theme bootstrap is allowed by its **hash**, computed from the
/// served index — never a hand-maintained constant.
///
/// The expected hash below is derived **out of band** (`openssl dgst -sha256
/// -binary | base64` over the exact bytes between the fixture's `<script>`
/// tags), NOT by calling the implementation again: recomputing with the code
/// under test would assert only that it agrees with itself. This way an EDIT
/// to the fixture's inline script fails the test — which is the whole point
/// of the drift gate ("drift fails the test, not the page").
const EXPECTED_INLINE_SCRIPT_HASH: &str = "'sha256-QMHClxD2saan54ttd7m7mAvax7fxuvMeq2qVTMbRZ4k='";

#[tokio::test]
async fn csp_script_src_carries_the_hash_of_the_served_inline_script() {
    let hashes = backend_rs::inline_script_hashes(SPA_INDEX_FIXTURE.as_bytes());
    assert_eq!(
        hashes.len(),
        1,
        "the fixture has exactly ONE inline script (the external module \
         script is covered by script-src 'self')"
    );
    assert_eq!(
        hashes[0], EXPECTED_INLINE_SCRIPT_HASH,
        "the recomputed hash of the fixture's inline script must match the \
         independently derived one — an edited script must fail HERE"
    );
    let app = create_app_with_csp(build_state(test_settings()), hashes.clone());
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let policy = csp_of(&response).expect("the CSP header must be present");
    let script_src = directive(&policy, "script-src").expect("script-src is part of the policy");
    let sources: Vec<&str> = script_src.split_whitespace().collect();
    assert!(
        sources.contains(&"'self'"),
        "script-src must still allow the served bundles: {policy}"
    );
    assert!(
        sources.contains(&EXPECTED_INLINE_SCRIPT_HASH),
        "the CSP's script-src must carry the inline script's hash \
         {EXPECTED_INLINE_SCRIPT_HASH}: {policy}"
    );
}

/// The Swagger UI is a developer surface, not the SPA: it is EXCLUDED from
/// the CSP.
#[tokio::test]
async fn csp_excludes_the_swagger_ui() {
    let app = test_app();
    let response = app
        .oneshot(Request::get("/swagger-ui").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(
        csp_of(&response).is_none(),
        "the Swagger UI must be excluded from the CSP"
    );
}
