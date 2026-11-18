//! T020: the shared `with_tx_retry` kernel, proven end to end with
//! always-fire SIGNAL-trigger injectors.
//!
//! The six inline retry loops (card create, card move, checklist-item
//! create/update, column update, bulk move-cards) move onto the shared
//! `common::tx_retry::with_tx_retry` helper (behavior-preserving), and the
//! previously SINGLE-SHOT write transactions gain the same protection. This
//! suite pins the single-shot side of that contract:
//!
//! - an injected `1213` on a single-shot write path (`DELETE /api/v1/cards/{id}`)
//!   is retried to success (before T020: the handler surfaces a 500);
//! - an always-firing `1213` / `1205` exhausts the bounded retry
//!   (`MAX_ATTEMPTS = 5`) and renders **503 + `Retry-After: 2`** with the
//!   `service-unavailable` slug — never the generic 500 — and the body
//!   carries no driver/SQL text (before T020: 500);
//! - a scoped `1062` (`for key 'UQ_columns_board_sort_key'`) exhausts the
//!   retry and stays **409** (class 23 integrity violation, not a
//!   serialization failure) — while an unrelated `1062` is NOT retried at all;
//! - retry exhaustion leaves the database untouched (the rollback is honoured);
//! - a bounded `1213` burst (two injected failures, then clean) recovers
//!   and the write lands exactly once;
//! - `create_column` — the one un-retried transaction that re-keys rows
//!   (serialized `LIMIT 1 FOR UPDATE` append + full-board rebalance) —
//!   recovers from injected `1213`s and returns 503 + `Retry-After: 2` on
//!   exhaustion.
//!
//! Fault mode: the injectors are `BEFORE DELETE` / `BEFORE INSERT` triggers
//! backed by a MyISAM fault table (transactional rollback must NOT resurrect
//! the flag — the T61 precedent from `cards_move.rs`). `Counted(n)` fires
//! the fault exactly `n` times then goes quiet (the recovery tests); `Always`
//! fires on every attempt (the exhaustion tests). The `fired` counter is the
//! attempt counter: reading it after the request proves the bounded retry
//! count and that no attempt was skipped (or doubled).
//!
//! Each `#[sqlx::test]` gets its own database (dropped at test end), so the
//! fault triggers and MyISAM tables die with it and cannot leak across
//! suites. Nothing to wipe.
//!
//! Rollback evidence: the always-fire abort happens at the ONLY write of the
//! `delete_card`/`create_column` paths, so `survivors == 1` /
//! `board_column_count == 3` alone cannot distinguish "rolled back" from
//! "never written". The `exhaustion_rolls_back_in_tx_writes` test below
//! covers the gap: it fails a `PATCH /notes/{id}` at its LAST statement
//! (the tag re-link INSERT), after an in-tx UPDATE and DELETE have already
//! executed — exhaustion must restore BOTH.

use super::helpers::{
    app_on_pool, assert_problem, authed_request, json_body, json_of, register, send,
};
use axum::http::StatusCode;
use axum::http::header;
use serde_json::json;
use sqlx::MySqlPool;

/// How many times the injected fault fires before going quiet.
enum FaultMode {
    /// Fire exactly `n` times, then pass through (the recovery tests).
    Counted(u32),
    /// Fire on every attempt (the exhaustion tests).
    Always,
}

/// Install a fault injector: a `timing` trigger on `table` that records the
/// transaction's real isolation level into `probe_table`, increments the
/// MyISAM fault counter, and raises `errno` — but only while the per-mode
/// budget remains (`Counted`) or unconditionally (`Always`).
///
/// MyISAM on purpose: the counter must survive the handler rolling its
/// failed transaction back. The trigger is a test-local literal whatever the
/// table holds; identifiers are never user input.
#[allow(clippy::too_many_arguments)]
async fn install_fault_injector(
    pool: &MySqlPool,
    table: &str,
    timing: &str,
    fault_table: &str,
    probe_table: &str,
    trigger: &str,
    sqlstate: &str,
    errno: u16,
    message_text: &str,
    mode: FaultMode,
) {
    const MESSAGE_TEXT_MAX: usize = 128; // SIGNAL MESSAGE_TEXT limit
    let escaped = message_text.replace('\'', "''");
    assert!(
        escaped.len() <= MESSAGE_TEXT_MAX,
        "MESSAGE_TEXT too long for SIGNAL: {escaped}"
    );
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE `{fault_table}` (`fired` BIGINT NOT NULL) ENGINE=MyISAM"
    )))
    .execute(pool)
    .await
    .expect("fault table");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO `{fault_table}` VALUES (0)"
    )))
    .execute(pool)
    .await
    .expect("fault table seed");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE `{probe_table}` (`isolation` VARCHAR(32) NOT NULL) ENGINE=MyISAM"
    )))
    .execute(pool)
    .await
    .expect("probe table");
    // CREATE TRIGGER is NOT preparable (MySQL error 1295): `raw_sql` sends one
    // COM_QUERY, while `query()` would go through COM_STMT_PREPARE.
    // MariaDB exposes no per-transaction ISOLATION_LEVEL view (verified on
    // 11.4), so the probe records the expected constant there — the helper
    // sets READ COMMITTED before `begin()` (code-proven) — while MySQL
    // reports the real level. On MariaDB the probe records the constant, so
    // the isolation assertion (delete-recovery test) validates the code
    // path, not the wire — runtime isolation evidence is MySQL-only.
    let version: String = sqlx::query_scalar("SELECT VERSION()")
        .fetch_one(pool)
        .await
        .expect("server version");
    let isolation_expr = if version.contains("MariaDB") {
        "SELECT 'READ-COMMITTED'"
    } else {
        "SELECT ISOLATION_LEVEL FROM performance_schema.events_transactions_current \
         WHERE THREAD_ID = PS_CURRENT_THREAD_ID()"
    };
    // The `fired` budget: `Counted(n)` fires while `fired < n`; `Always`
    // always fires. Both increment the counter — it doubles as the attempt
    // counter the tests assert on.
    let budget = match mode {
        FaultMode::Counted(n) => format!("(SELECT `fired` FROM `{fault_table}`) < {n}"),
        FaultMode::Always => "TRUE".to_string(),
    };
    let create = format!(
        "CREATE TRIGGER `{trigger}` {timing} ON `{table}` FOR EACH ROW \
         BEGIN \
           IF {budget} THEN \
             INSERT INTO `{probe_table}` (`isolation`) \
               {isolation_expr}; \
             UPDATE `{fault_table}` SET `fired` = `fired` + 1; \
             SIGNAL SQLSTATE '{sqlstate}' SET MYSQL_ERRNO = {errno}, MESSAGE_TEXT = '{escaped}'; \
           END IF; \
         END"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(create))
        .execute(pool)
        .await
        .expect("fault trigger");
}

/// How many times the injected fault fired (the attempt counter).
async fn fire_count(pool: &MySqlPool, fault_table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT CAST(`fired` AS SIGNED) FROM `{fault_table}`"
    )))
    .fetch_one(pool)
    .await
    .expect("fault counter")
}

/// Isolation levels the trigger recorded (MariaDB reports the constant the
/// helper sets before `begin()`; MySQL reports the observed level).
async fn probe_isolation(pool: &MySqlPool, probe_table: &str) -> Vec<String> {
    let raw: Vec<String> = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
        "SELECT `isolation` FROM `{probe_table}`"
    )))
    .fetch_all(pool)
    .await
    .expect("probe rows");
    raw.into_iter()
        .map(|value| value.replace('-', " ").to_uppercase())
        .collect()
}

/// Create one board (3 default columns) for `jar` and one card in its first
/// column; returns (first_column_id, card_id).
async fn create_board_and_card(pool: &MySqlPool, jar: &str, name: &str) -> (i64, i64) {
    let (status, _, bytes) = send(
        app_on_pool(pool),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(jar),
            Some(json_body(&json!({ "name": name }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board: {created}");
    let column = created["data"]["columns"][0]["id"]
        .as_i64()
        .expect("first column id");
    let (status, _, bytes) = send(
        app_on_pool(pool),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(jar),
            Some(json_body(&json!({
                "title": "T020 victim",
                "column_id": column,
            }))),
        ),
    )
    .await;
    let card = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "card: {card}");
    let card_id = card["data"]["id"].as_i64().expect("card id");
    (column, card_id)
}

/// Column ids of a board (used by the create_column persistence checks).
async fn board_column_count(pool: &MySqlPool, board_id: i64) -> i64 {
    sqlx::query_scalar("SELECT CAST(COUNT(*) AS SIGNED) FROM `columns` WHERE `board_id` = ?")
        .bind(board_id)
        .fetch_one(pool)
        .await
        .expect("column count")
}

/// A `1213` injected into the first TWO delete attempts must be retried to
/// success (200 family, here 204) — the row really disappears, the fault
/// counter reads 2, and the tx ran at READ COMMITTED. Before T020 the
/// single-shot `delete_card` surfaces a 500 and the row survives.
#[sqlx::test(migrations = "./migrations")]
async fn injected_1213_on_single_shot_delete_recovers(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-recover-delete@example.com").await;
    let (_, card_id) = create_board_and_card(&pool, &jar, "T020 delete recovery").await;
    install_fault_injector(
        &pool,
        "cards",
        "BEFORE DELETE",
        "t020_rec_fault",
        "t020_rec_probe",
        "t020_rec_trigger",
        "40001",
        1213,
        "injected deadlock",
        FaultMode::Counted(2),
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{card_id}"),
            Some(&jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "an injected 1213 burst must be retried, not surfaced: {bytes:?}"
    );
    assert_eq!(
        fire_count(&pool, "t020_rec_fault").await,
        2,
        "the injected 1213 fired exactly twice, then the retry succeeded"
    );
    assert_eq!(
        probe_isolation(&pool, "t020_rec_probe").await,
        vec!["READ COMMITTED".to_string(), "READ COMMITTED".to_string()],
        "the retried tx ran at READ COMMITTED"
    );
    let survivors: i64 =
        sqlx::query_scalar("SELECT CAST(COUNT(*) AS SIGNED) FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("card re-read");
    assert_eq!(survivors, 0, "the card was actually deleted (persistence)");
}

/// A `1213` injected into the first TWO create attempts must be retried to
/// success; the column lands and is visible afterwards. Before T020: 500.
#[sqlx::test(migrations = "./migrations")]
async fn injected_1213_on_single_shot_create_column_recovers(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-recover-column@example.com").await;
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 column recovery" }))),
        ),
    )
    .await;
    let board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board: {board}");
    let board_id = board["data"]["id"].as_i64().expect("board id");
    install_fault_injector(
        &pool,
        "columns",
        "BEFORE INSERT",
        "t020_cc_fault",
        "t020_cc_probe",
        "t020_cc_trigger",
        "40001",
        1213,
        "injected deadlock",
        FaultMode::Counted(2),
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 Column" }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "an injected 1213 burst must be retried, not surfaced: {created}"
    );
    assert_eq!(
        fire_count(&pool, "t020_cc_fault").await,
        2,
        "the injected 1213 fired exactly twice, then the retry succeeded"
    );
    assert_eq!(
        board_column_count(&pool, board_id).await,
        4,
        "the retried create persisted its column"
    );
}

/// An ALWAYS-firing `1213` on a single-shot write exhausts the bounded retry
/// and renders **503 `service-unavailable` + `Retry-After: 2`** — never the
/// generic 500, never driver/SQL text in the body — and the card survives.
/// Before T020: 500.
#[sqlx::test(migrations = "./migrations")]
async fn always_firing_1213_on_single_shot_delete_renders_503(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-exhaust-delete@example.com").await;
    let (_, card_id) = create_board_and_card(&pool, &jar, "T020 delete exhaustion").await;
    install_fault_injector(
        &pool,
        "cards",
        "BEFORE DELETE",
        "t020_del_fault",
        "t020_del_probe",
        "t020_del_trigger",
        "40001",
        1213,
        "injected deadlock",
        FaultMode::Always,
    )
    .await;

    let (status, headers, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{card_id}"),
            Some(&jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "exhausted lock retries render 503, not 500: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "service-unavailable",
        "exhausted lock retries render service-unavailable",
    );
    assert_eq!(
        headers
            .get(header::RETRY_AFTER)
            .expect("Retry-After must be set"),
        "2",
        "Retry-After carries the 2 s busy floor"
    );
    let raw = String::from_utf8_lossy(&bytes);
    assert!(!raw.contains("1213"), "driver errno leaked: {raw}");
    assert!(!raw.contains("deadlock"), "driver text leaked: {raw}");
    assert_eq!(
        fire_count(&pool, "t020_del_fault").await,
        5,
        "the retry budget is bounded at MAX_ATTEMPTS = 5"
    );
    let survivors: i64 =
        sqlx::query_scalar("SELECT CAST(COUNT(*) AS SIGNED) FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("card re-read");
    assert_eq!(survivors, 1, "the rolled-back delete left the card intact");
}

/// An ALWAYS-firing `1205` (lock wait timeout, SQLSTATE `HY000`) on a
/// single-shot write exhausts the bounded retry and renders **503 +
/// `Retry-After: 2`**. Before T020: 500. (A real blocked `1205` still rides
/// the 30 s `TimeoutLayer` — the injected fault raises the error directly.)
#[sqlx::test(migrations = "./migrations")]
async fn always_firing_1205_on_single_shot_delete_renders_503(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-exhaust-1205@example.com").await;
    let (_, card_id) = create_board_and_card(&pool, &jar, "T020 1205 exhaustion").await;
    install_fault_injector(
        &pool,
        "cards",
        "BEFORE DELETE",
        "t020_1205_fault",
        "t020_1205_probe",
        "t020_1205_trigger",
        "HY000",
        1205,
        "injected lock wait timeout",
        FaultMode::Always,
    )
    .await;

    let (status, headers, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "DELETE",
            &format!("/api/v1/cards/{card_id}"),
            Some(&jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "exhausted lock-wait retries render 503, not 500: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "service-unavailable",
        "exhausted lock-wait retries render service-unavailable",
    );
    assert_eq!(
        headers
            .get(header::RETRY_AFTER)
            .expect("Retry-After must be set"),
        "2",
        "Retry-After carries the 2 s busy floor"
    );
    let raw = String::from_utf8_lossy(&bytes);
    assert!(!raw.contains("1205"), "driver errno leaked: {raw}");
    assert!(!raw.contains("HY000"), "driver SQLSTATE leaked: {raw}");
    assert!(!raw.contains("lock wait"), "driver text leaked: {raw}");
    assert!(!raw.contains("40001"), "driver SQLSTATE leaked: {raw}");
    assert_eq!(
        fire_count(&pool, "t020_1205_fault").await,
        5,
        "the retry budget is bounded at MAX_ATTEMPTS = 5"
    );
    let survivors: i64 =
        sqlx::query_scalar("SELECT CAST(COUNT(*) AS SIGNED) FROM `cards` WHERE `id` = ?")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("card re-read");
    assert_eq!(survivors, 1, "the rolled-back delete left the card intact");
}

/// An ALWAYS-firing `1213` on `create_column` — the only un-retried
/// transaction that re-keys rows — exhausts the bounded retry and renders
/// **503 + `Retry-After: 2`**, leaving the board untouched. Before T020: 500.
#[sqlx::test(migrations = "./migrations")]
async fn always_firing_1213_on_create_column_renders_503(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-exhaust-column@example.com").await;
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 column exhaustion" }))),
        ),
    )
    .await;
    let board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board: {board}");
    let board_id = board["data"]["id"].as_i64().expect("board id");
    install_fault_injector(
        &pool,
        "columns",
        "BEFORE INSERT",
        "t020_c503_fault",
        "t020_c503_probe",
        "t020_c503_trigger",
        "40001",
        1213,
        "injected deadlock",
        FaultMode::Always,
    )
    .await;

    let (status, headers, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 Exhausted" }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "exhausted lock retries render 503, not 500: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "service-unavailable",
        "exhausted create_column retries render service-unavailable",
    );
    assert_eq!(
        headers
            .get(header::RETRY_AFTER)
            .expect("Retry-After must be set"),
        "2",
        "Retry-After carries the 2 s busy floor"
    );
    let raw = String::from_utf8_lossy(&bytes);
    assert!(!raw.contains("1213"), "driver errno leaked: {raw}");
    assert!(!raw.contains("deadlock"), "driver text leaked: {raw}");
    assert_eq!(
        fire_count(&pool, "t020_c503_fault").await,
        5,
        "the retry budget is bounded at MAX_ATTEMPTS = 5"
    );
    assert_eq!(
        board_column_count(&pool, board_id).await,
        3,
        "no column was created (the retried tx never applied)"
    );
}

/// A scoped `1062` on `UQ_columns_board_sort_key` is retried (LIKE a lock
/// conflict), but exhaustion stays **409** (class 23, not a serialization
/// failure — the unique index is doing its job). Before T020 this site did
/// not retry at all (fired once); the budget must now be consumed.
#[sqlx::test(migrations = "./migrations")]
async fn always_firing_scoped_1062_on_create_column_stays_409(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-1062-column@example.com").await;
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 scoped 1062" }))),
        ),
    )
    .await;
    let board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board: {board}");
    let board_id = board["data"]["id"].as_i64().expect("board id");
    install_fault_injector(
        &pool,
        "columns",
        "BEFORE INSERT",
        "t020_1062_fault",
        "t020_1062_probe",
        "t020_1062_trigger",
        "23000",
        1062,
        "Duplicate entry 'a0-b' for key 'UQ_columns_board_sort_key'",
        FaultMode::Always,
    )
    .await;

    let (status, headers, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 Dup" }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "scoped-1062 exhaustion stays 409, never 503/500: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "conflict",
        "scoped-1062 exhaustion stays conflict",
    );
    let raw = String::from_utf8_lossy(&bytes);
    assert!(
        !raw.contains("UQ_columns"),
        "driver constraint name leaked: {raw}"
    );
    assert!(
        !raw.contains("Duplicate entry"),
        "driver text leaked: {raw}"
    );
    assert_eq!(
        fire_count(&pool, "t020_1062_fault").await,
        5,
        "the scoped 1062 consumed the full retry budget before surfacing"
    );
    assert_eq!(
        board_column_count(&pool, board_id).await,
        3,
        "the scoped-1062 path left the board untouched"
    );
}

/// An UNRELATED `1062` (a unique index that is not `UQ_*_sort_key`) is NOT
/// retried: 409 immediately, the retry budget untouched.
#[sqlx::test(migrations = "./migrations")]
async fn always_firing_unrelated_1062_is_not_retried(pool: MySqlPool) {
    let (jar, _) = register(app_on_pool(&pool), "t020-unrelated-1062@example.com").await;
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 unrelated 1062" }))),
        ),
    )
    .await;
    let board = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board: {board}");
    let board_id = board["data"]["id"].as_i64().expect("board id");
    install_fault_injector(
        &pool,
        "columns",
        "BEFORE INSERT",
        "t020_u1062_fault",
        "t020_u1062_probe",
        "t020_u1062_trigger",
        "23000",
        1062,
        "Duplicate entry 'work' for key 'UQ_labels_user_name'",
        FaultMode::Always,
    )
    .await;

    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&jar),
            Some(json_body(&json!({ "name": "T020 Dup" }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "an unrelated 1062 renders 409 without burning the retry budget: {body}"
    );
    assert_eq!(
        fire_count(&pool, "t020_u1062_fault").await,
        1,
        "unrelated unique violations are never retried"
    );
    assert_eq!(
        board_column_count(&pool, board_id).await,
        3,
        "the unrelated-1062 path left the board untouched"
    );
}

/// Create a tag row directly (suite-local seed, like `notes.rs`).
async fn seed_tag(pool: &MySqlPool, user_id: i64, name: &str) -> i64 {
    let result = sqlx::query("INSERT INTO `tags` (`name`, `user_id`) VALUES (?, ?)")
        .bind(name)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("seed tag");
    i64::try_from(result.last_insert_id()).expect("tag id fits")
}

/// Retry exhaustion rolls back writes that happened EARLIER in the same
/// transaction — the only exhaustion test whose fault is not on the first
/// write. `PATCH /notes/{id}{title, tag_ids}` merges, applies the patch
/// (UPDATE `notes`), clears tag links (`DELETE note_tags`), then re-links
/// (`INSERT note_tags`) — where the always-firing `1213` is injected. Every
/// attempt fails at the last step with the earlier writes pending;
/// exhaustion must restore BOTH the original title and the original tag set.
#[sqlx::test(migrations = "./migrations")]
async fn exhaustion_rolls_back_in_tx_writes(pool: MySqlPool) {
    let (jar, user_id) = register(app_on_pool(&pool), "t020-rollback@example.com").await;
    let tag_a = seed_tag(&pool, user_id, "t020-rollback-a").await;
    let tag_b = seed_tag(&pool, user_id, "t020-rollback-b").await;
    let (status, _, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&jar),
            Some(json_body(&json!({
                "title": "T020 original",
                "content": "body",
                "tag_ids": [tag_a],
            }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "note: {created}");
    let note_id = created["data"]["id"].as_i64().expect("note id");
    install_fault_injector(
        &pool,
        "note_tags",
        "BEFORE INSERT",
        "t020_rb_fault",
        "t020_rb_probe",
        "t020_rb_trigger",
        "40001",
        1213,
        "injected deadlock",
        FaultMode::Always,
    )
    .await;

    let (status, headers, bytes) = send(
        app_on_pool(&pool),
        authed_request(
            "PATCH",
            &format!("/api/v1/notes/{note_id}"),
            Some(&jar),
            Some(json_body(&json!({
                "title": "T020 retried title",
                "tag_ids": [tag_b],
            }))),
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "exhaustion renders 503: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "service-unavailable",
        "exhausted notes patch renders service-unavailable",
    );
    assert_eq!(
        fire_count(&pool, "t020_rb_fault").await,
        5,
        "the retry budget is bounded at MAX_ATTEMPTS = 5"
    );
    // The earlier in-tx writes must have rolled back too.
    let title: String = sqlx::query_scalar("SELECT `title` FROM `notes` WHERE `id` = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .expect("title re-read");
    assert_eq!(
        title, "T020 original",
        "the title update rolled back with the tx"
    );
    let tags: Vec<i64> = sqlx::query_scalar("SELECT `tag_id` FROM `note_tags` WHERE `note_id` = ?")
        .bind(note_id)
        .fetch_all(&pool)
        .await
        .expect("tag links re-read");
    assert_eq!(
        tags,
        vec![tag_a],
        "the tag re-link rolled back with the tx (the original link survives)"
    );
}
