//! T049: server-side board card search and filters.

use super::helpers::{
    app_on_pool, assert_errors_nonempty, assert_problem, authed_request, create_board, json_of,
    register, send,
};
use axum::http::StatusCode;
use serde_json::json;
use sqlx::MySqlPool;

fn names(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("data[]")
        .iter()
        .map(|item| item["title"].as_str().expect("title").to_string())
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn cards_filters_validation_and_semantics(pool: MySqlPool) {
    let app = || app_on_pool(&pool);
    let (owner_jar, _) = register(app(), "filter-owner@example.com").await;
    let (intruder_jar, _) = register(app(), "filter-intruder@example.com").await;

    let board_id = create_board(app(), &owner_jar, &json!({"name": "Filter Board"})).await;
    let foreign_board_id = create_board(
        app(),
        &intruder_jar,
        &json!({"name": "Foreign Filter Board"}),
    )
    .await;

    // Get columns
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let cols = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "cols: {cols}");
    let col_a = cols["data"][0]["id"].as_i64().unwrap();
    let col_b = cols["data"][1]["id"].as_i64().unwrap();

    // Create labels (owner)
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&owner_jar),
            Some(json!({"name":"L1","color":"red"}).to_string()),
        ),
    )
    .await;
    let l1 = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "l1: {l1}");
    let l1_id = l1["data"]["id"].as_i64().unwrap();

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&owner_jar),
            Some(json!({"name":"L2","color":"blue"}).to_string()),
        ),
    )
    .await;
    let l2 = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "l2: {l2}");
    let l2_id = l2["data"]["id"].as_i64().unwrap();

    // Foreign label
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "POST",
            "/api/v1/labels",
            Some(&intruder_jar),
            Some(json!({"name":"LF","color":"green"}).to_string()),
        ),
    )
    .await;
    let lf = json_of(&bytes);
    assert_eq!(status, StatusCode::CREATED, "lf: {lf}");
    let lf_id = lf["data"]["id"].as_i64().unwrap();

    // Seed cards with SQL for deterministic setup.
    let now = chrono::Utc::now().naive_utc();
    let today_date = now.date();
    let today = today_date.and_hms_opt(12, 0, 0).unwrap();
    let overdue = (today_date - chrono::Days::new(2))
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let next_week = (today_date + chrono::Days::new(3))
        .and_hms_opt(12, 0, 0)
        .unwrap(); // within 7-day window

    async fn insert_card(
        pool: &MySqlPool,
        title: &str,
        col: i64,
        key: &str,
        due: Option<chrono::NaiveDateTime>,
    ) -> i64 {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query(
            "INSERT INTO `cards` (`title`,`column_id`,`sort_key`,`due_date`) VALUES (?,?,?,?)",
        )
        .bind(title)
        .bind(col)
        .bind(key)
        .bind(due)
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query_scalar::<_, i64>("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(&mut *conn)
            .await
            .unwrap()
    }

    let a1 = insert_card(&pool, "Alpha %_\\ task", col_a, "a0", None).await;
    let a2 = insert_card(&pool, "Beta", col_a, "a1", Some(today)).await;
    let b1 = insert_card(&pool, "Gamma", col_b, "a0", Some(overdue)).await;
    let b2 = insert_card(&pool, "Delta", col_b, "a1", Some(next_week)).await;

    // Labels: A1 has L1,L2; A2 has L1; B1 has L2
    for (card, label) in [(a1, l1_id), (a1, l2_id), (a2, l1_id), (b1, l2_id)] {
        sqlx::query("INSERT INTO `card_labels` (`card_id`,`label_id`) VALUES (?,?)")
            .bind(card)
            .bind(label)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Checklists: A2 complete (1/1), B1 incomplete (0/1), B2 has checklist but zero items
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("INSERT INTO `checklists` (`title`,`card_id`) VALUES ('CL',?)")
        .bind(a2)
        .execute(&mut *conn)
        .await
        .unwrap();
    let cl_a2: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO `checklist_items` (`text`,`is_completed`,`checklist_id`,`sort_key`) VALUES ('i',1,?,'a0')")
        .bind(cl_a2).execute(&mut *conn).await.unwrap();

    sqlx::query("INSERT INTO `checklists` (`title`,`card_id`) VALUES ('CL',?)")
        .bind(b1)
        .execute(&mut *conn)
        .await
        .unwrap();
    let cl_b1: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO `checklist_items` (`text`,`is_completed`,`checklist_id`,`sort_key`) VALUES ('i',0,?,'a0')")
        .bind(cl_b1).execute(&mut *conn).await.unwrap();

    sqlx::query("INSERT INTO `checklists` (`title`,`card_id`) VALUES ('CL',?)")
        .bind(b2)
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);

    // --- Validation errors ---
    let bad_cases = vec![
        (
            format!("/api/v1/boards/{board_id}/cards?search={}", "x".repeat(256)),
            "search too long",
            "search",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?due_date=bogus"),
            "bad due_date",
            "due_date",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?checklist=bogus"),
            "bad checklist",
            "checklist",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?labels=abc"),
            "bad label",
            "labels",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?labels=0"),
            "zero label",
            "labels",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?labels=-1"),
            "neg label",
            "labels",
        ),
        (
            format!(
                "/api/v1/boards/{board_id}/cards?{}",
                (1..=21)
                    .map(|i| format!("labels={i}"))
                    .collect::<Vec<_>>()
                    .join("&")
            ),
            "too many labels",
            "labels",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?labels={lf_id}"),
            "foreign label",
            "labels",
        ),
        (
            format!("/api/v1/boards/{board_id}/cards?labels=999999"),
            "missing label",
            "labels",
        ),
    ];
    for (path, what, field) in bad_cases {
        let (status, headers, bytes) =
            send(app(), authed_request("GET", &path, Some(&owner_jar), None)).await;
        let body = json_of(&bytes);
        eprintln!("{what} => {status} {body}");
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {body}");
        assert_problem(status, &headers, &body, "validation-error", what);
        assert_errors_nonempty(&body, what);
        assert!(
            body["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["field"] == field),
            "{what} field {field}: {body}"
        );
    }

    // same validation on column cards endpoint
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{col_a}/cards?due_date=bogus"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::BAD_REQUEST, "col bad due_date: {body}");
    assert_problem(
        status,
        &headers,
        &body,
        "validation-error",
        "col bad due_date",
    );

    // same validation on columns embed endpoint
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns?checklist=bogus"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "embed bad checklist: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "validation-error",
        "embed bad checklist",
    );

    // --- Search: case-insensitive, LIKE metacharacters escaped ---
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?search=%25_%5C"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "search meta: {body}");
    assert_eq!(names(&body), vec!["Alpha %_\\ task"]);
    assert_eq!(body["total"], 1);

    // search trims and empty is no filter
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?search=%20%20%20"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "search empty: {body}");
    assert_eq!(body["total"], 4);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?search=alpha"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "search ci: {body}");
    assert_eq!(body["total"], 1);

    // --- Labels AND semantics ---
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?labels={l1_id}&labels={l2_id}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "labels AND: {body}");
    assert_eq!(names(&body), vec!["Alpha %_\\ task"]);
    assert_eq!(body["total"], 1);

    // --- due_date filters ---
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?due_date=overdue&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "overdue: {body}");
    assert_eq!(names(&body), vec!["Gamma"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?due_date=today&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "today: {body}");
    assert_eq!(names(&body), vec!["Beta"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?due_date=this_week&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "this_week: {body}");
    // today + next_week (within 7 days) both qualify; overdue is 2 days ago so also? overdue is < start_of_today so not in this_week. today is >= start_of_today and < next_week bound - yes. next_week card is +3 days - within 7 day window, yes.
    let mut got = names(&body);
    got.sort();
    assert_eq!(got, vec!["Beta", "Delta"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?due_date=no_date&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "no_date: {body}");
    assert_eq!(names(&body), vec!["Alpha %_\\ task"]);

    // --- checklist filters ---
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?checklist=with&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "checklist with: {body}");
    assert_eq!(names(&body), vec!["Beta", "Delta", "Gamma"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?checklist=complete&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "checklist complete: {body}");
    assert_eq!(names(&body), vec!["Beta"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?checklist=incomplete&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "checklist incomplete: {body}");
    assert_eq!(names(&body), vec!["Delta", "Gamma"]);

    // --- Parity across 3 endpoints ---
    let filter = "search=&labels=&due_date=overdue&checklist=incomplete";
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?{filter}&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let board_list = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "parity board list: {board_list}");
    assert_eq!(names(&board_list), vec!["Gamma"]);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns?embed_limit=50&{filter}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let cols_body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "parity cols: {cols_body}");
    let mut embed_names: Vec<String> = cols_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| {
            c["cards"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| k["title"].as_str().unwrap().to_string())
        })
        .collect();
    embed_names.sort();
    assert_eq!(embed_names, vec!["Gamma"]);
    // total_cards unfiltered, matching_cards filtered
    let col_b_body = cols_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == col_b)
        .unwrap();
    assert_eq!(col_b_body["total_cards"], 2);
    assert_eq!(col_b_body["matching_cards"], 1);
    let col_a_body = cols_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == col_a)
        .unwrap();
    assert_eq!(col_a_body["total_cards"], 2);
    assert_eq!(col_a_body["matching_cards"], 0);

    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/columns/{col_b}/cards?{filter}"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let col_cards = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "parity col cards: {col_cards}");
    assert_eq!(names(&col_cards), vec!["Gamma"]);
    assert_eq!(col_cards["total"], 1);

    // --- Filtered pagination: page beyond filtered total returns empty data, correct total ---
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!(
                "/api/v1/boards/{board_id}/cards?search=Gamma&page=2&limit=1&sort=title&order=asc"
            ),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "page beyond: {body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"].as_array().unwrap().len(), 0);

    // --- No-filter behavior preserved ---
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "no filter: {body}");
    assert_eq!(body["total"], 4);
    assert_eq!(
        names(&body),
        vec!["Alpha %_\\ task", "Beta", "Delta", "Gamma"]
    );

    // --- Extra cases from review ---

    // empty labels param and duplicate labels treated as no-op/dedup
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?labels=&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "empty labels: {body}");
    assert_eq!(body["total"], 4);
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!(
                "/api/v1/boards/{board_id}/cards?labels={l1_id}&labels={l1_id}&sort=title&order=asc"
            ),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "dup labels: {body}");
    assert_eq!(names(&body), vec!["Alpha %_\\ task", "Beta"]);

    // window ceiling with filters
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?search=Alpha&page=101&limit=100"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "window ceiling filtered: {body}"
    );
    assert_problem(
        status,
        &headers,
        &body,
        "validation-error",
        "window ceiling filtered",
    );

    // foreign/missing column 404 with filters
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            "/api/v1/columns/999999/cards?search=Alpha",
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "missing col filter: {body}");
    assert_problem(status, &headers, &body, "not-found", "missing col filter");

    // embed_limit + filters: filtered matches exceed embed_limit
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/columns?embed_limit=1&checklist=with"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "embed+filter: {body}");
    let col_b_body = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == col_b)
        .unwrap();
    assert_eq!(col_b_body["total_cards"], 2);
    assert_eq!(col_b_body["matching_cards"], 2);
    assert_eq!(col_b_body["cards"].as_array().unwrap().len(), 1);

    // multi-checklist rollup: card with 2 checklists, one incomplete => incomplete
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("INSERT INTO `checklists` (`title`,`card_id`) VALUES ('CL2',?)")
            .bind(a2)
            .execute(&mut *conn)
            .await
            .unwrap();
        let cl_a2b: i64 = sqlx::query_scalar::<_, i64>("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO `checklist_items` (`text`,`is_completed`,`checklist_id`,`sort_key`) VALUES ('i',0,?,'a0')")
            .bind(cl_a2b).execute(&mut *conn).await.unwrap();
    }
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?checklist=complete&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "multi checklist complete: {body}");
    assert_eq!(names(&body), Vec::<String>::new());
    let (status, _, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{board_id}/cards?checklist=incomplete&sort=title&order=asc"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::OK, "multi checklist incomplete: {body}");
    let got = names(&body);
    assert!(
        got.contains(&"Beta".to_string()),
        "beta now incomplete: {body}"
    );

    // --- Ownership 404 no-oracle still holds ---
    let (status, headers, bytes) = send(
        app(),
        authed_request(
            "GET",
            &format!("/api/v1/boards/{foreign_board_id}/cards?search=Alpha"),
            Some(&owner_jar),
            None,
        ),
    )
    .await;
    let body = json_of(&bytes);
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign board: {body}");
    assert_problem(status, &headers, &body, "not-found", "foreign board");

    let _ = (b2, next_week);
}
