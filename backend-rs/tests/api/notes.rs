//! `POST /api/v1/notes` + `PATCH|GET|DELETE /api/v1/notes/:id` +
//! `GET /api/v1/notes?search&type&tag_id&board_id&page&limit` (T27a).
//!
//! Single `#[sqlx::test]` setup on purpose (mirrors T24a): all scenarios run
//! sequentially in one test with distinct emails. Registers owner + intruder
//! via `POST /api/v1/auth/register` (real Argon2id hashes, live cookie jars),
//! creates project/board/card targets via the API, and seeds tags via SQL
//! (tag routes land in T27b — no HTTP path exists yet).
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`), so no other suite can reach it. Never issue a
//! blanket `DELETE`/`UPDATE`/`DROP` against it: scope by `WHERE id = ?`.
//!
//! Covered (red first: notes routes do not exist yet):
//!
//! `POST {title,content,board_id?|project_id?|card_id?,tag_ids?} returns 201
//! {data} plus Location`; `GET /:id → 200` (explicit `content` select round-trip);
//! `PATCH → 200`; `DELETE → 204`; `GET / → 200 Page<NoteSummary>` (no
//! `content` key anywhere, `COUNT(DISTINCT n.id)` over the tags join, two-step
//! IDs-then-enrich, `ORDER updated_at DESC`);
//! `?search&type&tag_id&board_id&page&limit` combos; `%`/`_`/`'`/`\` search
//! payloads escaped + stored literally; missing/foreign tag → `404`;
//! multiple FKs → `400`; overlong content → `400 errors[]`; foreign id → `404`
//! (never 403); no `GET /boards/:boardId/notes` duplicate (404 from router);
//! every 4xx asserts problem+json + exact `type` URI; every mutation re-reads
//! state via second GET or direct `SELECT`.
//!
//! Contract: `specs/api-rewrite-rs-specs/20-api-contract.md` notes section
//! (list excludes `content` TEXT; `type` derived from which FK is set; POST
//! enforces at most one FK; unknown `tag_ids` → 404 aligning create with
//! update) + `21-api-standard-shapes.md` notes row (`tagIds→tag_ids`,
//! `Page<NoteSummary>`, deleted `/boards/:boardId/notes`).

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board_json,
    json_body, json_of, register, send,
};
use axum::Router;
use axum::http::{StatusCode, header};
use serde_json::Value;
use sqlx::MySqlPool;

async fn create_tag(pool: &MySqlPool, user_id: i64, name: &str) -> i64 {
    let result = sqlx::query("INSERT INTO `tags` (`name`, `user_id`) VALUES (?, ?)")
        .bind(name)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("seed tag");
    i64::try_from(result.last_insert_id()).expect("tag id fits")
}

/// Board + its first default column (notes attach to both), from the create
/// response. Notes-specific, so it stays local rather than in `helpers`.
async fn create_board_with_column(app: Router, jar: &str, name: &str) -> (i64, i64) {
    let body = create_board_json(app, jar, &serde_json::json!({"name": name})).await;
    let board_id = body["data"]["id"].as_i64().expect("board id");
    let col_id = body["data"]["columns"][0]["id"]
        .as_i64()
        .expect("column id");
    (board_id, col_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn notes_crud_list_flow(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    // 0. Anonymous access is 401 on every notes route (AuthSession guard).
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/notes",
            Some(json_body(
                &serde_json::json!({"title": "Nope", "content": "Nope"}),
            )),
        ),
        (
            "PATCH",
            "/api/v1/notes/1",
            Some(json_body(&serde_json::json!({"title": "Nope"}))),
        ),
        ("GET", "/api/v1/notes", None),
        ("GET", "/api/v1/notes/1", None),
        ("DELETE", "/api/v1/notes/1", None),
    ] {
        let (status, headers, bytes) = send(app(), authed_request(method, path, None, body)).await;
        let failure = json_of(&bytes);
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}: {failure}"
        );
        assert_problem(
            status,
            &headers,
            &failure,
            "unauthorized",
            "Anonymous access is 401 on every notes route (AuthSession…",
        );
    }

    // 1. Register owner + intruder; owner creates project/board/card targets
    //    via the API; intruder creates a foreign board for 404 probes.
    let (owner_jar, owner_id) = register(app(), "notes-owner@example.com").await;
    let (intruder_jar, intruder_id) = register(app(), "notes-intruder@example.com").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/projects",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"name": "Notes project"}))),
        ),
    )
    .await;
    let project_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create project: {project_body}"
    );
    let project_id = project_body["data"]["id"].as_i64().expect("project id");
    let (board_id, col_id) = create_board_with_column(app(), &owner_jar, "Notes board").await;
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/cards",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"title": "Notes card", "column_id": col_id}),
            )),
        ),
    )
    .await;
    let card_body = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create card: {card_body}");
    let card_id = card_body["data"]["id"].as_i64().expect("card id");
    let (foreign_board_id, _) = create_board_with_column(app(), &intruder_jar, "Theirs").await;
    // Intruder owns one note (for foreign-note 404s).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&intruder_jar),
            Some(json_body(
                &serde_json::json!({"title": "Intruder note", "content": "Theirs"}),
            )),
        ),
    )
    .await;
    let intruder_note_body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::CREATED,
        "intruder note: {intruder_note_body}"
    );
    let foreign_note = intruder_note_body["data"]["id"]
        .as_i64()
        .expect("foreign note id");
    // Tags are SQL-seeded (tag routes land in T27b).
    let tag_a = create_tag(&pool, owner_id, "Owner Tag A").await;
    let tag_b = create_tag(&pool, owner_id, "Owner Tag B").await;
    let foreign_tag = create_tag(&pool, intruder_id, "Intruder Tag").await;

    // 2. Create general note → 201 + Location; DB re-read + GET detail prove
    //    the `content` round-trip (never an echo).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "General note",
                "content": "# Hello",
                "tag_ids": [tag_a],
            }))),
        ),
    )
    .await;
    let created = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "create note: {created}");
    assert!(
        created.get("message").is_none(),
        "no message key: {created}"
    );
    let note_id = created["data"]["id"].as_i64().expect("note id");
    assert_eq!(created["data"]["title"], "General note");
    assert_eq!(created["data"]["content"], "# Hello");
    assert!(created["data"]["board_id"].is_null());
    assert!(created["data"]["project_id"].is_null());
    assert!(created["data"]["card_id"].is_null());
    assert_eq!(created["data"]["tags"][0]["id"], tag_a);
    assert_eq!(created["data"]["tags"][0]["name"], "Owner Tag A");
    let location = headers
        .get(header::LOCATION)
        .expect("201 sets Location")
        .to_str()
        .expect("Location is ASCII");
    assert_eq!(location, format!("/api/v1/notes/{note_id}"));
    let stored: (String, String, i64) =
        sqlx::query_as("SELECT `title`, `content`, `user_id` FROM `notes` WHERE `id` = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .expect("created note re-reads");
    assert_eq!(
        stored,
        ("General note".to_string(), "# Hello".to_string(), owner_id)
    );
    let link_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `note_tags` WHERE `note_id` = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .expect("tag links re-read");
    assert_eq!(link_count, 1);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/notes/{note_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let detail = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "get detail: {detail}");
    assert_eq!(detail["data"]["id"], note_id);
    assert_eq!(detail["data"]["content"], "# Hello");

    // 3. Board-linked note with duplicated tag_ids persists one row per tag.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Board note",
                "content": "Board body",
                "board_id": board_id,
                "tag_ids": [tag_a, tag_b, tag_a],
            }))),
        ),
    )
    .await;
    let board_note = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "board note: {board_note}");
    let board_note_id = board_note["data"]["id"].as_i64().expect("board note id");
    assert_eq!(board_note["data"]["board_id"], board_id);
    let link_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM `note_tags` WHERE `note_id` = ?")
            .bind(board_note_id)
            .fetch_one(&pool)
            .await
            .expect("dedup re-read");
    assert_eq!(link_count, 2, "tag_ids dedup persists single rows");
    // Project + card notes round out the `type` filter matrix.
    for (title, key, target) in [
        ("Project note", "project_id", project_id),
        ("Card note", "card_id", card_id),
    ] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/notes",
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({
                    "title": title, "content": "Body", key: target,
                }))),
            ),
        )
        .await;
        let body = json_of(&bytes);
        assert_eq!(status, StatusCode::CREATED, "create {title}: {body}");
        assert_eq!(body["data"][key], target);
    }

    // 4. LIKE-special titles are stored literally (injection regression).
    for title in ["100% _coverage_ \\ path", "O'Brien"] {
        let (status, _, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/notes",
                Some(&owner_jar),
                Some(json_body(
                    &serde_json::json!({"title": title, "content": "Body"}),
                )),
            ),
        )
        .await;
        let body = json_of(&bytes);
        assert_eq!(status, StatusCode::CREATED, "special {title:?}: {body}");
        let stored_title: String = sqlx::query_scalar("SELECT `title` FROM `notes` WHERE `id` = ?")
            .bind(body["data"]["id"].as_i64().expect("special note id"))
            .fetch_one(&pool)
            .await
            .expect("special title re-reads literally");
        assert_eq!(stored_title, title);
    }

    // 5. List → 200 Page: no `content` key anywhere (TEXT never selected),
    //    `total` is exact despite the tags-join fanout (board note × 2 tags).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/notes?page=1&limit=50",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let page = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "list notes: {page}");
    assert!(page.get("message").is_none(), "no message key: {page}");
    assert_eq!(page["page"], 1);
    assert_eq!(page["limit"], 50);
    let items = page["data"].as_array().expect("data[]");
    assert_eq!(items.len(), 6, "all owner notes listed: {page}");
    assert_eq!(
        page["total"].as_u64().expect("total"),
        6,
        "COUNT(DISTINCT n.id) is exact under join fanout: {page}"
    );
    for item in items {
        assert!(
            item.get("content").is_none(),
            "list never selects TEXT: {item}"
        );
        assert!(item["title"].is_string(), "title present: {item}");
        assert!(item["tags"].is_array(), "tags[] present: {item}");
        assert!(item["created_at"].is_string() && item["updated_at"].is_string());
    }
    let updated: Vec<&str> = items
        .iter()
        .map(|item| item["updated_at"].as_str().expect("updated_at"))
        .collect();
    let mut sorted = updated.clone();
    sorted.sort_unstable();
    sorted.reverse();
    assert_eq!(updated, sorted, "list ordered updated_at DESC: {page}");
    // Pagination slices + strict range errors.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/notes?page=1&limit=2",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let sliced = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "paged list: {sliced}");
    assert_eq!(sliced["data"].as_array().expect("data[]").len(), 2);
    assert_eq!(sliced["total"], 6);
    for uri in ["/api/v1/notes?page=0", "/api/v1/notes?limit=500"] {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", uri, Some(&owner_jar), None)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "bad {uri}: {failure}");
        assert_problem(status, &headers, &failure, "validation-error", "List");
        assert_errors_nonempty(&failure, uri);
    }

    // 6. Filter matrix: search/type/tag_id/board_id combos.
    async fn filtered(app: Router, jar: &str, query: &str) -> Value {
        let (status, _, bytes) = send(
            app,
            authed_request("GET", &format!("/api/v1/notes{query}"), Some(jar), None),
        )
        .await;
        let body = json_of(&bytes);
        assert_eq!(status, StatusCode::OK, "filter {query}: {body}");
        body
    }
    let body = filtered(app(), &owner_jar, "?search=Board").await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["title"], "Board note");
    for (query, expected) in [
        ("?type=general", 3usize),
        ("?type=board", 1usize),
        ("?type=project", 1usize),
        ("?type=card", 1usize),
    ] {
        let body = filtered(app(), &owner_jar, query).await;
        assert_eq!(
            body["data"].as_array().expect("data[]").len(),
            expected,
            "filter {query}"
        );
        assert_eq!(body["total"], expected as u64, "filter {query} total");
    }
    let body = filtered(app(), &owner_jar, &format!("?tag_id={tag_b}")).await;
    assert_eq!(body["total"], 1, "tag filter is exact: {body}");
    assert_eq!(body["data"][0]["title"], "Board note");
    let body = filtered(app(), &owner_jar, &format!("?board_id={board_id}")).await;
    assert_eq!(body["total"], 1, "board filter: {body}");
    assert_eq!(body["data"][0]["title"], "Board note");
    let body = filtered(
        app(),
        &owner_jar,
        "?search=note&type=general&page=1&limit=50",
    )
    .await;
    assert!(
        body["total"].as_u64().expect("total") >= 1,
        "combo filter: {body}"
    );
    for item in body["data"].as_array().expect("data[]") {
        assert!(item["board_id"].is_null() && item["project_id"].is_null());
    }
    // Filter validation: bad enum → 400 errors[]; garbage ints → 400
    // invalid-query; range violation → 400 errors[].
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/notes?type=evil", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "bad type: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "validation-error",
        "Filter matrix: search/type/tag_id/board_id combos",
    );
    assert_errors_nonempty(&failure, "bad type");
    for uri in ["/api/v1/notes?tag_id=abc", "/api/v1/notes?board_id=abc"] {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", uri, Some(&owner_jar), None)).await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "garbage {uri}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "invalid-query",
            "Filter matrix: search/type/tag_id/board_id combos",
        );
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/notes?tag_id=0", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "tag_id 0: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "validation-error",
        "Filter matrix: search/type/tag_id/board_id combos",
    );

    // 7. LIKE wildcards are escaped: a bare `%`/`_` matches only literal
    //    carriers, `'`/`\` neither break the query nor widen it.
    for (query, titles) in [
        ("?search=%", vec!["100% _coverage_ \\ path"]),
        ("?search=_", vec!["100% _coverage_ \\ path"]),
        ("?search=O'Brien", vec!["O'Brien"]),
        ("?search=\\", vec!["100% _coverage_ \\ path"]),
    ] {
        let body = filtered(app(), &owner_jar, query).await;
        let found: Vec<&str> = body["data"]
            .as_array()
            .expect("data[]")
            .iter()
            .map(|item| item["title"].as_str().expect("title"))
            .collect();
        assert_eq!(found, titles, "escaped filter {query}: {body}");
    }

    // 8. Create validation: multiple FKs → 400; missing/foreign tag → 404;
    //    foreign board/project/card → 404; overlong/empty content and blank
    //    title → 400 errors[]; wrong JSON types → 400 invalid-json.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Two links", "content": "Body",
                "board_id": board_id, "project_id": project_id,
            }))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "multi-FK: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "validation-error",
        "Create validation: multiple FKs",
    );
    assert_errors_nonempty(&failure, "multi-FK");
    for (payload, what) in [
        (
            serde_json::json!({"title": "Ghost tag", "content": "Body", "tag_ids": [999_999]}),
            "missing tag",
        ),
        (
            serde_json::json!({"title": "Foreign tag", "content": "Body", "tag_ids": [foreign_tag]}),
            "foreign tag",
        ),
        (
            serde_json::json!({"title": "Hijack", "content": "Body", "board_id": foreign_board_id}),
            "foreign board",
        ),
        (
            serde_json::json!({"title": "Hijack", "content": "Body", "project_id": 999_999}),
            "missing project",
        ),
        (
            serde_json::json!({"title": "Hijack", "content": "Body", "card_id": 999_999}),
            "missing card",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/notes",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "Create validation: multiple FKs",
        );
    }
    for (payload, what) in [
        (
            serde_json::json!({"title": "Too long", "content": "a".repeat(65_536)}),
            "overlong content",
        ),
        (
            serde_json::json!({"title": "Empty body", "content": ""}),
            "empty content",
        ),
        (
            serde_json::json!({"title": "   ", "content": "Body"}),
            "whitespace title",
        ),
        (
            serde_json::json!({"title": "a".repeat(256), "content": "Body"}),
            "overlong title",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/notes",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "validation-error",
            "Create validation: multiple FKs",
        );
        assert_errors_nonempty(&failure, what);
    }
    for (payload, what) in [
        (
            serde_json::json!({"title": 123, "content": "Body"}),
            "numeric title",
        ),
        (serde_json::json!({"content": "Body"}), "missing title"),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "POST",
                "/api/v1/notes",
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "invalid-json",
            "Create validation: multiple FKs",
        );
    }
    // Boundary: exactly 65_535 chars is accepted.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/notes",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"title": "Max body", "content": "a".repeat(65_535)}),
            )),
        ),
    )
    .await;
    let boundary = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "65535 chars: {boundary}");
    let boundary_id = boundary["data"]["id"].as_i64().expect("boundary id");

    // 9. Deprecated `/api/*` alias serves the same create handler.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/notes",
            Some(&owner_jar),
            Some(json_body(
                &serde_json::json!({"title": "Alias note", "content": "Body"}),
            )),
        ),
    )
    .await;
    let aliased = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "alias create: {aliased}");
    assert_eq!(aliased["data"]["title"], "Alias note");

    // 10. PATCH scalars + tag replace → 200; re-GET + SELECT confirm.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/notes/{note_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({
                "title": "Renamed note",
                "content": "Updated body",
                "tag_ids": [tag_b],
            }))),
        ),
    )
    .await;
    let patched = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "patch note: {patched}");
    assert_eq!(patched["data"]["id"], note_id);
    assert_eq!(patched["data"]["title"], "Renamed note");
    assert_eq!(patched["data"]["content"], "Updated body");
    assert_eq!(patched["data"]["tags"].as_array().expect("tags[]").len(), 1);
    assert_eq!(patched["data"]["tags"][0]["id"], tag_b);
    let stored: (String, String) =
        sqlx::query_as("SELECT `title`, `content` FROM `notes` WHERE `id` = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .expect("patched note re-reads");
    assert_eq!(
        stored,
        ("Renamed note".to_string(), "Updated body".to_string())
    );
    let link_ids: Vec<i64> =
        sqlx::query_scalar("SELECT `tag_id` FROM `note_tags` WHERE `note_id` = ?")
            .bind(note_id)
            .fetch_all(&pool)
            .await
            .expect("replaced links re-read");
    assert_eq!(link_ids, vec![tag_b]);
    // Empty {} PATCH is a no-op 200.
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/notes/{note_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({}))),
        ),
    )
    .await;
    let noop = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty patch: {noop}");
    assert_eq!(noop["data"]["title"], "Renamed note");
    // PATCH detach: explicit null clears the board link (back to general).
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            &format!("/api/v1/notes/{board_note_id}"),
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"board_id": null}))),
        ),
    )
    .await;
    let detached = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "detach link: {detached}");
    assert!(detached["data"]["board_id"].is_null());
    let stored_board: Option<i64> =
        sqlx::query_scalar("SELECT `board_id` FROM `notes` WHERE `id` = ?")
            .bind(board_note_id)
            .fetch_one(&pool)
            .await
            .expect("detached link re-reads");
    assert_eq!(stored_board, None);
    // PATCH validation: multi-FK merge, foreign link, overlong content.
    for (payload, slug, what) in [
        (
            serde_json::json!({"board_id": board_id, "project_id": project_id}),
            "validation-error",
            "patch multi-FK",
        ),
        (
            serde_json::json!({"content": "a".repeat(65_536)}),
            "validation-error",
            "patch overlong content",
        ),
        (
            serde_json::json!({"board_id": foreign_board_id}),
            "not-found",
            "patch foreign board",
        ),
        (
            serde_json::json!({"tag_ids": [999_999]}),
            "not-found",
            "patch missing tag",
        ),
    ] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/notes/{note_id}"),
                Some(&owner_jar),
                Some(json_body(&payload)),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        let expected = if slug == "validation-error" {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::NOT_FOUND
        };
        assert_eq!(status, expected, "{what}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            slug,
            "PATCH scalars + tag replace",
        );
    }
    // PATCH foreign/missing → 404 (never 403); garbage id → 400.
    for id in [foreign_note.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "PATCH",
                &format!("/api/v1/notes/{id}"),
                Some(&owner_jar),
                Some(json_body(&serde_json::json!({"title": "Hijack"}))),
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "patch {id}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "PATCH scalars + tag replace",
        );
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "PATCH",
            "/api/v1/notes/abc",
            Some(&owner_jar),
            Some(json_body(&serde_json::json!({"title": "Hijack"}))),
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "PATCH scalars + tag replace",
    );
    // GET foreign/missing → 404 (never 403); garbage id → 400.
    for id in [foreign_note.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "GET",
                &format!("/api/v1/notes/{id}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "get {id}: {failure}");
        assert_problem(
            status,
            &headers,
            &failure,
            "not-found",
            "PATCH scalars + tag replace",
        );
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request("GET", "/api/v1/notes/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(
        status,
        &headers,
        &failure,
        "invalid-path",
        "PATCH scalars + tag replace",
    );

    // 11. No `GET /boards/:boardId/notes` duplicate: the path 404s from the
    //     router (use `?board_id=` instead).
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/notes"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "deleted duplicate route: {failure}"
    );
    assert_problem(
        status,
        &headers,
        &failure,
        "not-found",
        "No `GET /boards/:boardId/notes` duplicate: the path 404s…",
    );

    // 12. DELETE → 204 empty body; re-GET 404s + `SELECT COUNT` = 0.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/notes/{boundary_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "delete note: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        bytes.is_empty(),
        "204 carries an empty body: {}",
        String::from_utf8_lossy(&bytes)
    );
    drop(headers);
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/notes/{boundary_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "re-get deleted: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "DELETE");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `notes` WHERE `id` = ?")
        .bind(boundary_id)
        .fetch_one(&pool)
        .await
        .expect("deleted row re-read");
    assert_eq!(remaining, 0);
    // Deleting twice → 404; foreign/missing → 404 (never 403); garbage → 400.
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "DELETE",
            &format!("/api/v1/notes/{boundary_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "delete twice: {failure}");
    assert_problem(status, &headers, &failure, "not-found", "DELETE");
    for id in [foreign_note.to_string(), "999999".to_string()] {
        let (status, headers, bytes) = send(
            app(),
            authed_request(
                "DELETE",
                &format!("/api/v1/notes/{id}"),
                Some(&owner_jar),
                None,
            ),
        )
        .await;
        let failure = json_of(&bytes);
        assert_eq!(status, StatusCode::NOT_FOUND, "delete {id}: {failure}");
        assert_problem(status, &headers, &failure, "not-found", "DELETE");
    }
    let (status, headers, bytes) = send(
        app(),
        authed_request("DELETE", "/api/v1/notes/abc", Some(&owner_jar), None),
    )
    .await;
    let failure = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "garbage id: {failure}");
    assert_problem(status, &headers, &failure, "invalid-path", "DELETE");

    // NO TEARDOWN WIPE/DROP (see module docs): the shared store must survive
    // until every suite in this process is done.
}
