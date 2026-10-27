//! Emoji gate for the live charset conversion (T02b).
//!
//! Gate string `'কানপ্যাড 🚀 𐍈'` (Bengali BMP + astral emoji + astral
//! `U+10348`) round-trips through `boards.name` / `cards.title` /
//! `notes.content` / `checklist_items.text` via the authenticated HTTP API
//! (register → live cookie jar), then re-reads via second GET + direct
//! `SELECT` and asserts byte-identical. Fails on a `latin1` store (chars
//! become `?`), passes after `0002_convert_to_utf8mb4.sql`.
//!
//! Also asserts the wire charset (`@@character_set_client=utf8mb4`) and the
//! `’ U+2019` spot-check (`HEX=E28099`, per `31-live-unicode-migration.md`
//! §31.2 — proves bytes are stored as utf8mb4, not `92`/cp1252).
//!
//! Single `#[sqlx::test]` setup (mirrors T21a): one isolated per-test DB,
//! migrations applied from `./migrations` (0001 + 0002).
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.

use super::helpers::{app_on_pool, authed_request, json_body, json_of, register, send};
use axum::http::StatusCode;
use sqlx::MySqlPool;

/// Gate string: Bengali (BMP, 3-byte) + rocket emoji (astral, 4-byte) +
/// Gothic letter hwair (astral, 4-byte). Any latin1 link in the chain
/// corrupts at least one of these.
const GATE: &str = "কানপ্যাড 🚀 𐍈";

#[sqlx::test(migrations = "./migrations")]
async fn unicode_emoji_gate_roundtrip(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Wire charset is utf8mb4 (per 31 §31.2: DATABASE_URL carries
    //    `?charset=utf8mb4`; the smoke assertion pins it).
    let client_charset: String = sqlx::query_scalar("SELECT @@character_set_client")
        .fetch_one(&pool)
        .await
        .expect("charset var reads");
    assert_eq!(client_charset, "utf8mb4", "connection must speak utf8mb4");

    // 1. Authenticate (register mints user + live session jar).
    let (jar, _) = register(app(), "unicode-gate@example.com").await;

    // 2. `boards.name` round-trip via POST + detail GET + direct SELECT.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/boards",
            Some(&jar),
            Some(json_body(&serde_json::json!({"name": GATE}))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create board: {created}");
    assert_eq!(created["data"]["name"], GATE);
    let board_id = created["data"]["id"].as_i64().expect("board id");
    let column_id = created["data"]["columns"][0]["id"]
        .as_i64()
        .expect("default column id");

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}"),
            Some(&jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "board detail: {detail}");
    assert_eq!(detail["data"]["name"], GATE, "board GET re-reads gate");
    let stored_board: String = sqlx::query_scalar("SELECT `name` FROM `boards` WHERE `id` = ?")
        .bind(board_id)
        .fetch_one(&pool)
        .await
        .expect("board row re-reads");
    assert_eq!(stored_board, GATE, "board SELECT re-reads gate");

    // 3. `cards.title` round-trip via POST + GET + direct SELECT.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&jar),
            Some(json_body(
                &serde_json::json!({"title": GATE, "column_id": column_id}),
            )),
        ),
    )
    .await;
    let card = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create card: {card}");
    assert_eq!(card["data"]["title"], GATE);
    let card_id = card["data"]["id"].as_i64().expect("card id");

    let (status, _, bytes) = send(
        app(),
        authed_request("GET", &format!("/api/v1/cards/{card_id}"), Some(&jar), None),
    )
    .await;
    let card_detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "card detail: {card_detail}");
    assert_eq!(card_detail["data"]["title"], GATE, "card GET re-reads gate");
    let stored_card: String = sqlx::query_scalar("SELECT `title` FROM `cards` WHERE `id` = ?")
        .bind(card_id)
        .fetch_one(&pool)
        .await
        .expect("card row re-reads");
    assert_eq!(stored_card, GATE, "card SELECT re-reads gate");

    // 4. `checklist_items.text` round-trip: checklist + item via the API,
    //    then direct SELECT re-read.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/cards/{card_id}/checklists"),
            Some(&jar),
            Some(json_body(&serde_json::json!({"title": GATE}))),
        ),
    )
    .await;
    let checklist = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create checklist: {checklist}");
    assert_eq!(checklist["data"]["title"], GATE);
    let checklist_id = checklist["data"]["id"].as_i64().expect("checklist id");

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            &format!("/api/v1/checklists/{checklist_id}/items"),
            Some(&jar),
            Some(json_body(&serde_json::json!({"text": GATE}))),
        ),
    )
    .await;
    let item = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create item: {item}");
    assert_eq!(item["data"]["text"], GATE);
    let item_id = item["data"]["id"].as_i64().expect("item id");
    let stored_item: String =
        sqlx::query_scalar("SELECT `text` FROM `checklist_items` WHERE `id` = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("item row re-reads");
    assert_eq!(stored_item, GATE, "item SELECT re-reads gate");

    // 5. `notes.content` round-trip via POST + GET + direct SELECT.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&jar),
            Some(json_body(
                &serde_json::json!({"title": GATE, "content": GATE}),
            )),
        ),
    )
    .await;
    let note = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create note: {note}");
    assert_eq!(note["data"]["content"], GATE);
    let note_id = note["data"]["id"].as_i64().expect("note id");

    let (status, _, bytes) = send(
        app(),
        authed_request("GET", &format!("/api/v1/notes/{note_id}"), Some(&jar), None),
    )
    .await;
    let note_detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "note detail: {note_detail}");
    assert_eq!(
        note_detail["data"]["content"], GATE,
        "note GET re-reads gate"
    );
    let stored_note: String = sqlx::query_scalar("SELECT `content` FROM `notes` WHERE `id` = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .expect("note row re-reads");
    assert_eq!(stored_note, GATE, "note SELECT re-reads gate");

    // 6. `’ U+2019` spot-check: stored as E28099 (utf8mb4), not 92/cp1252.
    sqlx::query("UPDATE `cards` SET `description` = 'it’s' WHERE `id` = ?")
        .bind(card_id)
        .execute(&pool)
        .await
        .expect("description update writes");
    let hex: String = sqlx::query_scalar("SELECT HEX(`description`) FROM `cards` WHERE `id` = ?")
        .bind(card_id)
        .fetch_one(&pool)
        .await
        .expect("HEX spot-check reads");
    assert!(
        hex.contains("E28099"),
        "U+2019 must store as E28099 (utf8mb4), got {hex}"
    );
    assert!(
        !hex.contains("92") || hex.contains("E28099"),
        "must not be cp1252 0x92: {hex}"
    );

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
