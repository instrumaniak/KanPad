//! T60 metrics wiring: router-level HTTP tracking, the SEPARATE `/metrics`
//! router (never mounted on the main app), and the pool series. Every
//! assertion is on the RENDERED Prometheus text (version 0.0.4).
//!
//! Harness rule: request plumbing comes from `super::helpers`, imported
//! EXPLICITLY (a glob import would let a leftover local definition shadow
//! the shared one silently — the E0255 compile error is the guard). This
//! suite re-declares NO helper and therefore needs no sanctioned exception.

use super::helpers::{self, send, test_settings};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};

/// The route label is the MATCHED PATTERN (`/api/v1/cards/{id}`), never the
/// raw path: two requests for DIFFERENT card ids must fold into ONE series
/// with count 2, and the raw ids must appear nowhere in the exposition.
#[tokio::test]
async fn route_label_is_the_matched_pattern_not_the_raw_path() {
    let state = backend_rs::state::AppState::from_settings(test_settings());
    let app = backend_rs::create_app(state.clone());
    let scrape = backend_rs::metrics_router(state);

    for id in ["999", "888"] {
        let (status, _, _) = send(
            app.clone(),
            Request::get(format!("/api/v1/cards/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "card {id} without a session cookie must 401 (no DB needed)"
        );
    }

    let (status, headers, body) = send(
        scrape,
        Request::get("/metrics").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the scrape endpoint is public");
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .expect("scrape sets Content-Type")
        .to_str()
        .expect("Content-Type is ASCII");
    assert!(
        content_type.starts_with("text/plain; version=0.0.4"),
        "{content_type}"
    );
    let text = String::from_utf8(body).expect("Prometheus text is UTF-8");
    assert!(
        text.contains(
            "kanpad_http_requests_total{method=\"GET\",route=\"/api/v1/cards/{id}\",status=\"401\"} 2"
        ),
        "both requests must fold into ONE series keyed by the matched route \
         PATTERN: {text}"
    );
    assert!(
        !text.contains("/api/v1/cards/999"),
        "the raw path must never become a series label (cardinality bomb): {text}"
    );
}

/// A request that matched no route is labelled `"unknown"` — the raw path
/// (which any client can mint) still never becomes a series label.
#[tokio::test]
async fn unmatched_request_is_labelled_unknown() {
    let state = backend_rs::state::AppState::from_settings(test_settings());
    let app = backend_rs::create_app(state.clone());
    let scrape = backend_rs::metrics_router(state);

    let (status, _, _) = send(
        app,
        Request::get("/api/v1/definitely-not-a-route")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "JSON 404 from the versioned fallback"
    );

    let (status, _, body) = send(
        scrape,
        Request::get("/metrics").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8(body).expect("Prometheus text is UTF-8");
    assert!(
        text.contains("route=\"unknown\""),
        "an unmatched request must be labelled \"unknown\": {text}"
    );
    assert!(
        !text.contains("definitely-not-a-route"),
        "the raw path must never become a series label: {text}"
    );
}

/// The scrape router has NO tracking middleware: two consecutive scrapes
/// must leave ZERO request series behind (a scrape never records itself).
#[tokio::test]
async fn a_scrape_does_not_record_itself() {
    let state = backend_rs::state::AppState::from_settings(test_settings());
    let scrape = backend_rs::metrics_router(state);

    for round in 1..=2 {
        let (status, _, body) = send(
            scrape.clone(),
            Request::get("/metrics").body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "scrape round {round}");
        if round == 2 {
            let text = String::from_utf8(body).expect("Prometheus text is UTF-8");
            assert!(
                !text.contains("kanpad_http_requests_total{"),
                "zero app requests were served — no requests_total SERIES \
                 line may exist at all: {text}"
            );
            assert!(
                !text.contains("route=\"/metrics\""),
                "no series may name the scrape route: {text}"
            );
        }
    }
}

/// `/metrics` is served ONLY by the separate [`backend_rs::metrics_router`]:
/// the main app (with its SPA fallback) must 404 it — fail closed.
#[tokio::test]
async fn metrics_are_never_mounted_on_the_main_app() {
    let (status, _, _) = send(
        helpers::test_app(),
        Request::get("/metrics").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "/metrics must 404 on the main app — it exists only on the separate \
         METRICS_PORT router"
    );
}

/// Pool gauges come from the live pool and the acquire histogram from
/// `AppState::acquire_timed`: one acquire (then release) must yield size 1,
/// idle 1, and acquire-count 1.
#[sqlx::test]
async fn pool_gauges_and_acquire_histogram_track_the_pool(pool: sqlx::MySqlPool) {
    let state = backend_rs::state::AppState::with_pool(test_settings(), pool);
    let connection = state.acquire_timed().await.expect("acquire");
    drop(connection);
    // Dropping a `PoolConnection` returns it to the pool ASYNCHRONOUSLY
    // (sqlx schedules the return), so `num_idle()` lags the drop by a task
    // hop — wait for the connection to actually land back before scraping,
    // so the idle gauge asserts its real value, not a scheduling artefact.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.pool.num_idle() == 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let (status, _, body) = send(
        backend_rs::metrics_router(state),
        Request::get("/metrics").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the scrape endpoint is public");
    let text = String::from_utf8(body).expect("Prometheus text is UTF-8");
    for expected in [
        "kanpad_db_pool_size 1",
        "kanpad_db_pool_idle 1",
        "kanpad_db_pool_acquire_duration_seconds_count 1",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in {text}");
    }
}
