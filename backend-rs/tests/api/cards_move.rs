//! `PATCH /api/v1/cards/:id` move/reorder + fuzz (T24b).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T24a): all scenarios run
//! sequentially in one test with distinct emails. Registers owner + intruder
//! via `POST /api/v1/auth/register` (real Argon2id hashes, live cookie jars)
//! and creates boards via the API; cards are seeded via direct SQL.
//!
//! NO session wipe here: `tower_sessions.session` is server-global. This
//! suite holds only its own jars, so concurrent suites are unaffected.
//!
//! Covered (red first: the T24a handler IGNORES `column_id`/`position`, so
//! every move below returns 200 with the position unchanged):
//! intra-column reorder + clamp (`clamp_position` unit: negative → 0,
//! over-large → `N-1`, clamped value WRITTEN); cross-column move with explicit
//! position (clamped to `M`) and without position (appends `max+1`); both
//! columns re-read dense `0..N` after every move; scalar+move in one PATCH;
//! foreign `column_id` → `404` (never 403); `column_id: 0` → `400`
//! validation-error; garbage id → `400 invalid-path`; 500-random-move
//! sequential fuzz with an exact-order oracle; concurrent `tokio::spawn` fuzz
//! with an invariant oracle (dense + unique + `COUNT` unchanged + each card
//! once — no exact-order assert under concurrency). Every 4xx asserts
//! problem+json + exact `type` URI; every mutation re-reads state via second
//! GET or direct `SELECT`.
//!
//! Contract: `20-api-contract.md` §`cards` (`PATCH /:id` scalar + move/reorder
//! with set-based range `UPDATE`s + `SELECT … FOR UPDATE` in a short tx;
//! same-column = one range shift + point update; cross-column = close-gap +
//! open-gap + point update; clamp-then-write; cross-column without position
//! appends `max+1`) + `21-api-standard-shapes.md` cards row; schema:
//! `30-db-schema.md` `cards(id,title,column_id CASCADE,position,description
//! TEXT NULL,due_date NULL,timestamps)`.

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
    let response = app.oneshot(request).await.expect("cards-move oneshot");
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

/// Card id for a seeded title (direct `SELECT` re-read).
async fn card_id_by_title(pool: &MySqlPool, title: &str) -> i64 {
    sqlx::query_scalar("SELECT `id` FROM `cards` WHERE `title` = ?")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("seeded card id")
}
/// Column order re-read: `(card_id, position)` in `position ASC`.
async fn column_order(pool: &MySqlPool, column_id: i64) -> Vec<(i64, i64)> {
    sqlx::query_as(
        "SELECT `id`, `position` FROM `cards` WHERE `column_id` = ? ORDER BY `position` ASC",
    )
    .bind(column_id)
    .fetch_all(pool)
    .await
    .expect("column order re-reads")
}

/// Assert a column is dense `0..N` and return its card ids in order.
async fn assert_dense(pool: &MySqlPool, column_id: i64, what: &str) -> Vec<i64> {
    let order = column_order(pool, column_id).await;
    let positions: Vec<i64> = order.iter().map(|(_, position)| *position).collect();
    let dense: Vec<i64> = (0..positions.len() as i64).collect();
    assert_eq!(positions, dense, "dense 0..N ({what})");
    order.into_iter().map(|(id, _)| id).collect()
}

/// Deterministic xorshift64* RNG (no extra deps; fixed seed = reproducible).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0, "rng bound must be positive");
        (self.next() % bound as u64) as usize
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn cards_move_reorder_fuzz_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous PATCH is 401 (AuthSession guard).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/cards/1",
            None,
            Some(json_body(&serde_json::json!({"position": 0}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::UNAUTHORIZED, "anon: {failure}");
    assert_problem(status, &headers, &failure, "unauthorized");

    // 1. Register owner + intruder; owner creates one board (3 default
    //    columns); intruder creates a foreign board + card.
    let (owner_jar, _) = register(app(), "move-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "move-intruder@example.com").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Move Board"}))),
        ),
    )
    .await;
    let created_board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create board: {created_board}");
    let board_id = created_board["data"]["id"].as_i64().expect("board id");
    let board_columns = created_board["data"]["columns"]
        .as_array()
        .expect("board nests columns");
    assert_eq!(board_columns.len(), 3);
    let col_a = board_columns[0]["id"].as_i64().expect("col A id");
    let col_b = board_columns[1]["id"].as_i64().expect("col B id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({"name": "Theirs"}))),
        ),
    )
    .await;
    let foreign_board = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "foreign board: {foreign_board}"
    );
    let foreign_col = foreign_board["data"]["columns"][0]["id"]
        .as_i64()
        .expect("foreign column id");
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&intruder_jar),
            Some(json_body(&serde_json::json!({
                "title": "Intruder card", "column_id": foreign_col,
            }))),
        ),
    )
    .await;
    let intruder_card_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "intruder card: {intruder_card_body}"
    );
    let foreign_card = intruder_card_body["data"]["id"]
        .as_i64()
        .expect("foreign card id");

    // 2. Seed cards via SQL: 4 in A (0..3), 2 in B (0..1).
    for (title, column, position) in [
        ("A0", col_a, 0),
        ("A1", col_a, 1),
        ("A2", col_a, 2),
        ("A3", col_a, 3),
        ("B0", col_b, 0),
        ("B1", col_b, 1),
    ] {
        sqlx::query("INSERT INTO `cards` (`title`, `column_id`, `position`) VALUES (?, ?, ?)")
            .bind(title)
            .bind(column)
            .bind(position)
            .execute(&pool)
            .await
            .expect("seed card");
    }
    let a0 = card_id_by_title(&pool, "A0").await;
    let a1 = card_id_by_title(&pool, "A1").await;
    let a2 = card_id_by_title(&pool, "A2").await;
    let a3 = card_id_by_title(&pool, "A3").await;

    // Helper: PATCH a move, assert 200 + the WRITTEN position/column, then
    // re-read both columns dense via SELECT (never trust the echo alone).
    #[allow(clippy::too_many_arguments)]
    async fn patch_move(
        app: Router,
        pool: &MySqlPool,
        jar: &str,
        card_id: i64,
        payload: Value,
        expect_column: i64,
        expect_position: i64,
        what: &str,
    ) -> Value {
        let (status, _, bytes) = send(
            app,
            authed_request(
                "PATCH",
                &format!("/api/v1/cards/{card_id}"),
                Some(jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let moved = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "{what}: {moved}");
        assert!(moved.get("message").is_none(), "no message key: {moved}");
        assert_eq!(moved["data"]["id"], card_id, "{what}: id");
        assert_eq!(
            moved["data"]["column_id"], expect_column,
            "{what}: column written"
        );
        assert_eq!(
            moved["data"]["position"], expect_position,
            "{what}: clamped position written"
        );
        // Second GET proves persistence (not an echo).
        let (status, _, bytes) = send(
            app_on_pool(pool),
            authed_request("GET", &format!("/api/v1/cards/{card_id}"), Some(jar), None),
        )
        .await;
        let detail = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "{what} re-get: {detail}");
        assert_eq!(detail["data"]["position"], expect_position);
        assert_eq!(detail["data"]["column_id"], expect_column);
        moved
    }

    // 3. Intra-column: A0 0→2 → [A1,A2,A0,A3], dense on re-read.
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a0,
        serde_json::json!({"position": 2}),
        col_a,
        2,
        "intra 0→2",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "intra 0→2").await,
        vec![a1, a2, a0, a3]
    );

    // 4. Intra-column: A3 3→0 → [A3,A1,A2,A0], dense on re-read.
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a3,
        serde_json::json!({"position": 0}),
        col_a,
        0,
        "intra 3→0",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "intra 3→0").await,
        vec![a3, a1, a2, a0]
    );

    // 5. Intra-column clamp low: A0 (pos 3) → -50 clamps to 0 and WRITES 0.
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a0,
        serde_json::json!({"position": -50}),
        col_a,
        0,
        "intra clamp low",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "intra clamp low").await,
        vec![a0, a3, a1, a2]
    );

    // 6. Intra-column clamp high: A2 (pos 3) → 999 clamps to N-1 = 3.
    //    Same-position write is a no-op shift; order stable, still dense.
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a2,
        serde_json::json!({"position": 999}),
        col_a,
        3,
        "intra clamp high",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "intra clamp high").await,
        vec![a0, a3, a1, a2]
    );

    // 7. Cross-column with explicit position: A1 (pos 2) → B@0.
    //    A closes its gap, B opens one: B = [A1,B0,B1], A dense.
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a1,
        serde_json::json!({"column_id": col_b, "position": 0}),
        col_b,
        0,
        "cross with position",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "cross source gap").await,
        vec![a0, a3, a2]
    );
    let b0 = card_id_by_title(&pool, "B0").await;
    let b1 = card_id_by_title(&pool, "B1").await;
    assert_eq!(
        assert_dense(&pool, col_b, "cross target gap").await,
        vec![a1, b0, b1]
    );

    // 8. Cross-column WITHOUT position appends max+1: A2 → B (M=3 → pos 3).
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a2,
        serde_json::json!({"column_id": col_b}),
        col_b,
        3,
        "cross append max+1",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "append source").await,
        vec![a0, a3]
    );
    assert_eq!(
        assert_dense(&pool, col_b, "append target").await,
        vec![a1, b0, b1, a2]
    );

    // 9. Cross-column clamp high: A3 (pos 1) → B@999 clamps to M = 4.
    patch_move(
        app(),
        &pool,
        &owner_jar,
        a3,
        serde_json::json!({"column_id": col_b, "position": 999}),
        col_b,
        4,
        "cross clamp high",
    )
    .await;
    assert_eq!(
        assert_dense(&pool, col_a, "cross clamp source").await,
        vec![a0]
    );
    assert_eq!(
        assert_dense(&pool, col_b, "cross clamp target").await,
        vec![a1, b0, b1, a2, a3]
    );

    // 10. Scalar + move in one PATCH: title and position both persist.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{b0}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "B0 renamed", "position": 4,
            }))),
        ),
    )
    .await;
    let combined = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "scalar+move: {combined}");
    assert_eq!(combined["data"]["title"], "B0 renamed");
    assert_eq!(combined["data"]["position"], 4);
    let stored: (String, i32) =
        sqlx::query_as("SELECT `title`, `position` FROM `cards` WHERE `id` = ?")
            .bind(b0)
            .fetch_one(&pool)
            .await
            .expect("scalar+move re-reads");
    assert_eq!(stored, ("B0 renamed".to_string(), 4));
    assert_eq!(
        assert_dense(&pool, col_b, "scalar+move").await,
        vec![a1, b1, a2, a3, b0]
    );

    // 11. Deprecated `/api/*` alias serves the same move handler: B1 → A
    //     (empty A has 1 card; appends max+1 = 1).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/cards/{b1}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"column_id": col_a}))),
        ),
    )
    .await;
    let aliased = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "alias move: {aliased}");
    assert_eq!(aliased["data"]["column_id"], col_a);
    assert_eq!(aliased["data"]["position"], 1);
    assert_eq!(
        assert_dense(&pool, col_a, "alias source").await,
        vec![a0, b1]
    );
    assert_eq!(
        assert_dense(&pool, col_b, "alias target").await,
        vec![a1, a2, a3, b0]
    );

    // 12. Errors: foreign column → 404 (never 403); missing card → 404;
    //     garbage id → 400 invalid-path; column_id 0 → 400 validation-error;
    //     wrong-typed position → 400 invalid-json. Rejected moves write
    //     nothing (SELECT re-read).
    let before_a = column_order(&pool, col_a).await;
    let before_b = column_order(&pool, col_b).await;
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{a0}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"column_id": foreign_col}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign column: {failure}");
    assert_problem(status, &headers, &failure, "not-found");
    for id in [foreign_card.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/cards/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"position": 0}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "patch {id}: {failure}");
        assert_problem(status, &headers, &failure, "not-found");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/cards/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"position": 0}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{a0}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"column_id": 0}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "column_id 0: {failure}");
    assert_problem(status, &headers, &failure, "validation-error");
    assert_errors_nonempty(&failure, "column_id 0");
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/cards/{a0}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"position": "top"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "string position: {failure}"
    );
    assert_problem(status, &headers, &failure, "invalid-json");
    assert_eq!(
        column_order(&pool, col_a).await,
        before_a,
        "rejected moves write nothing (A)"
    );
    assert_eq!(
        column_order(&pool, col_b).await,
        before_b,
        "rejected moves write nothing (B)"
    );

    // 13. Sequential fuzz: 500 random moves, exact-order oracle. A local
    //     model mirrors the contract clamp semantics (same-column clamp to
    //     N-1, cross-column clamp to M, missing position appends M); the
    //     final DB order must match the model exactly and the total COUNT
    //     must be unchanged.
    let total_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards`")
        .fetch_one(&pool)
        .await
        .expect("fuzz COUNT baseline");
    // Model: column id → card ids in order.
    let mut model_a = assert_dense(&pool, col_a, "fuzz baseline A").await;
    let mut model_b = assert_dense(&pool, col_b, "fuzz baseline B").await;
    let mut rng = Rng(0xC0FFEE_12345678);
    for step in 0..500 {
        let all: Vec<i64> = model_a.iter().chain(model_b.iter()).copied().collect();
        let card = all[rng.below(all.len())];
        let in_a = model_a.contains(&card);
        // Random target column; random requested position in [-3, M+3]
        // (deliberately out of range half the time to exercise clamping);
        // cross-column moves omit `position` every 4th step to exercise the
        // append-`max+1` path.
        let cross = rng.below(2) == 0;
        let (target, source) = if cross {
            if in_a {
                (col_b, col_a)
            } else {
                (col_a, col_b)
            }
        } else if in_a {
            (col_a, col_a)
        } else {
            (col_b, col_b)
        };
        let target_len = if target == col_a {
            model_a.len()
        } else {
            model_b.len()
        } as i64;
        let same = source == target;
        let omit_position = !same && step % 4 == 3;
        let requested: Option<i64> = if omit_position {
            None
        } else {
            Some(rng.below((target_len + 7) as usize) as i64 - 3)
        };
        // Contract clamp: same-column → N-1, cross-column → M, append M.
        let upper = if same {
            (if target == col_a {
                model_a.len()
            } else {
                model_b.len()
            } as i64
                - 1)
            .max(0)
        } else {
            target_len
        };
        let final_pos = requested.map(|p| p.clamp(0, upper)).unwrap_or(target_len);
        // Apply to the model (remove + insert = the range-shift semantics).
        {
            let from = if in_a { &mut model_a } else { &mut model_b };
            let idx = from
                .iter()
                .position(|id| *id == card)
                .expect("card in model");
            from.remove(idx);
        }
        {
            let to = if target == col_a {
                &mut model_a
            } else {
                &mut model_b
            };
            to.insert(final_pos as usize, card);
        }
        // Always send both keys on cross-column moves; same-column moves
        // send `position` only (exercises the position-only path).
        let mut payload = serde_json::Map::new();
        if !same {
            payload.insert("column_id".to_string(), serde_json::json!(target));
        }
        if !omit_position {
            payload.insert(
                "position".to_string(),
                serde_json::json!(requested.expect("requested set")),
            );
        }
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/cards/{card}"),
                Some(&owner_jar),
                Some(json_body(&Value::Object(payload))),
            ),
        )
        .await;
        let moved = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "fuzz step {step}: {moved}");
        assert_eq!(moved["data"]["column_id"], target, "fuzz step {step}");
        assert_eq!(moved["data"]["position"], final_pos, "fuzz step {step}");
        // Dense re-read every 50 steps keeps the fuzz honest mid-flight.
        if step % 50 == 49 {
            assert_eq!(
                assert_dense(&pool, col_a, &format!("fuzz step {step} A")).await,
                model_a,
                "fuzz step {step} model A"
            );
            assert_eq!(
                assert_dense(&pool, col_b, &format!("fuzz step {step} B")).await,
                model_b,
                "fuzz step {step} model B"
            );
        }
    }
    assert_eq!(
        assert_dense(&pool, col_a, "fuzz final A").await,
        model_a,
        "500-move fuzz exact order (A)"
    );
    assert_eq!(
        assert_dense(&pool, col_b, "fuzz final B").await,
        model_b,
        "500-move fuzz exact order (B)"
    );
    let total_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `cards`")
        .fetch_one(&pool)
        .await
        .expect("fuzz COUNT re-read");
    assert_eq!(total_after, total_before, "fuzz moves never lose cards");

    // 14. Concurrent fuzz: 8 `tokio::spawn` workers × 25 random moves each
    //     against the same two columns. No exact-order assert under
    //     concurrency — the invariant oracle is: per-column dense 0..N,
    //     unique positions per column, total COUNT unchanged, every card
    //     exactly once. A 500 (InnoDB deadlock 1213 under contention) retries
    //     the same move boundedly; every move must eventually return 200 or
    //     the test fails (retries never mask a persistent 500).
    let total_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `cards` WHERE `column_id` IN (?, ?)")
            .bind(col_a)
            .bind(col_b)
            .fetch_one(&pool)
            .await
            .expect("concurrent COUNT baseline");
    // Owned cards only: the intruder's card must never be touched (its PATCH
    // is a 404 by design, asserted in §12).
    let owned_ids: Vec<i64> =
        sqlx::query_scalar("SELECT `id` FROM `cards` WHERE `column_id` IN (?, ?) ORDER BY `id`")
            .bind(col_a)
            .bind(col_b)
            .fetch_all(&pool)
            .await
            .expect("concurrent card set");
    let worker = |worker_id: usize| {
        let pool = pool.clone();
        let jar = owner_jar.clone();
        let ids = owned_ids.clone();
        let app = app_on_pool(&pool);
        tokio::spawn(async move {
            let mut rng = Rng(0xDEAD_BEEF_0000 + worker_id as u64);
            let mut moved_ok = 0;
            for _ in 0..25 {
                // Re-read the live card set per move: ids never change, only
                // positions/columns do, so any card + any column is valid.
                let card = ids[rng.below(ids.len())];
                let target = if rng.below(2) == 0 { col_a } else { col_b };
                let omit = rng.below(4) == 3;
                let payload = if omit {
                    serde_json::json!({"column_id": target})
                } else {
                    serde_json::json!({"column_id": target, "position": rng.below(9) as i64 - 2})
                };
                let mut attempts = 0;
                loop {
                    attempts += 1;
                    let (status, _, bytes) = send(
                        app.clone(),
                        authed_request(
                            "PATCH",
                            &format!("/api/v1/cards/{card}"),
                            Some(&jar),
                            Some(json_body(&payload)),
                        ),
                    )
                    .await;
                    if status == StatusCode::OK {
                        let moved = json_of(&bytes);
                        assert_eq!(moved["data"]["id"], card);
                        moved_ok += 1;
                        break;
                    }
                    // Transient deadlock only: any other status fails loudly.
                    assert_eq!(
                        status,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "worker {worker_id}: unexpected {status}: {}",
                        String::from_utf8_lossy(&bytes)
                    );
                    assert!(
                        attempts < 10,
                        "worker {worker_id}: move never landed (card {card})"
                    );
                }
            }
            moved_ok
        })
    };
    let handles: Vec<_> = (0..8).map(worker).collect();
    let mut total_moves = 0;
    for handle in handles {
        total_moves += handle.await.expect("worker joins");
    }
    assert_eq!(total_moves, 8 * 25, "every concurrent move eventually 200s");
    // Invariant oracle via one SELECT over both columns.
    let rows: Vec<(i64, i64, i32)> = sqlx::query_as(
        "SELECT `id`, `column_id`, `position` FROM `cards` WHERE `column_id` IN (?, ?) ORDER BY `column_id`, `position`",
    )
    .bind(col_a)
    .bind(col_b)
    .fetch_all(&pool)
    .await
    .expect("concurrent oracle re-read");
    assert_eq!(rows.len() as i64, total_before, "COUNT unchanged");
    let mut seen: Vec<i64> = rows.iter().map(|(id, _, _)| *id).collect();
    seen.sort_unstable();
    let mut expected = owned_ids.clone();
    expected.sort_unstable();
    assert_eq!(seen, expected, "each card exactly once");
    for column in [col_a, col_b] {
        let mut positions: Vec<i32> = rows
            .iter()
            .filter(|(_, col, _)| *col == column)
            .map(|(_, _, position)| *position)
            .collect();
        positions.sort_unstable();
        let dense: Vec<i32> = (0..positions.len() as i32).collect();
        assert_eq!(positions, dense, "concurrent dense 0..N (col {column})");
        let len = positions.len();
        let distinct: std::collections::HashSet<i32> = positions.into_iter().collect();
        assert_eq!(distinct.len(), len, "unique positions (col {column})");
    }
    let _ = board_id;

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
