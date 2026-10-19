//! T08 validation + transport core (DB-free probe tests).
//!
//! Every catalogue rule from `20-api-contract.md` fails with **400**
//! problem+json carrying an `errors[]` field→message map (never 422/500).
//! Transport mapping (`PathRejection`/`QueryRejection`/oversized-body wrap,
//! unknown-field tolerance) is proven over HTTP against probe routes that use
//! the real feature DTOs through [`ValidJson`]/[`ValidQuery`].

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, StatusCode},
    middleware,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use backend_rs::{
    common::{ids::BoardId, validation::MAX_PASSWORD_BYTES},
    error::{capture_request_context, AppError},
    extract::{ValidJson, ValidPath, ValidQuery},
    features::{
        auth::types::{LoginInput, RegisterInput},
        boards::types::{BoardListQuery, CreateBoardInput, UpdateBoardInput},
        cards::types::{CreateCardInput, UpdateCardInput},
        checklists::types::{
            CreateChecklistInput, CreateChecklistItemInput, UpdateChecklistInput,
            UpdateChecklistItemInput,
        },
        columns::types::{CreateColumnInput, MoveCardsInput, SortColumnInput, UpdateColumnInput},
        labels::types::{CreateLabelInput, UpdateLabelInput},
        notes::types::{CreateNoteInput, NoteListQuery, UpdateNoteInput},
        projects::types::{CreateProjectInput, UpdateProjectInput},
        tags::types::{CreateTagInput, UpdateTagInput},
    },
};
use garde::Validate;
use serde_json::{json, Value};
use tower::ServiceExt;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

use super::helpers::PROBLEM_BASE;

/// Request-id header name (tower-http keeps its own copy private).
const X_REQUEST_ID: header::HeaderName = header::HeaderName::from_static("x-request-id");

async fn v_register(
    ValidJson(body): ValidJson<RegisterInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"email": body.email}))))
}

async fn v_login(
    ValidJson(body): ValidJson<LoginInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"email": body.email}))))
}

async fn v_create_project(
    ValidJson(body): ValidJson<CreateProjectInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"name": body.name}))))
}

async fn v_update_project(
    ValidJson(body): ValidJson<UpdateProjectInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"name": body.name}))))
}

async fn v_create_board(
    ValidJson(body): ValidJson<CreateBoardInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"name": body.name}))))
}

async fn v_update_board(
    ValidJson(_body): ValidJson<UpdateBoardInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_boards_list(
    ValidQuery(query): ValidQuery<BoardListQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"project_id": query.project_id})))
}

async fn v_create_column(
    ValidJson(body): ValidJson<CreateColumnInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"name": body.name}))))
}

async fn v_update_column(
    ValidJson(body): ValidJson<UpdateColumnInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"name": body.name}))))
}

async fn v_sort_column(
    ValidJson(body): ValidJson<SortColumnInput>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"sort_order": body.sort_order})))
}

async fn v_move_cards(ValidJson(body): ValidJson<MoveCardsInput>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"target_column_id": body.target_column_id})))
}

async fn v_create_card(
    ValidJson(body): ValidJson<CreateCardInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"title": body.title}))))
}

async fn v_update_card(
    ValidJson(_body): ValidJson<UpdateCardInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_create_label(
    ValidJson(body): ValidJson<CreateLabelInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"color": body.color}))))
}

async fn v_update_label(
    ValidJson(_body): ValidJson<UpdateLabelInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_create_tag(
    ValidJson(body): ValidJson<CreateTagInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"name": body.name}))))
}

async fn v_update_tag(
    ValidJson(_body): ValidJson<UpdateTagInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_create_checklist(
    ValidJson(body): ValidJson<CreateChecklistInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"title": body.title}))))
}

async fn v_update_checklist(
    ValidJson(_body): ValidJson<UpdateChecklistInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_create_checklist_item(
    ValidJson(body): ValidJson<CreateChecklistItemInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"text": body.text}))))
}

async fn v_update_checklist_item(
    ValidJson(_body): ValidJson<UpdateChecklistItemInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_create_note(
    ValidJson(body): ValidJson<CreateNoteInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::CREATED, Json(json!({"title": body.title}))))
}

async fn v_update_note(
    ValidJson(_body): ValidJson<UpdateNoteInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

async fn v_notes_list(
    ValidQuery(query): ValidQuery<NoteListQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        json!({"ok": true, "has_type": query.note_type.is_some()}),
    ))
}

/// Real path shape (`/api/v1/boards/{id}`) with the shared `PathRejection →
/// 400 problem+json` mapping: resource handlers take [`ValidPath`] (a raw
/// `Path<T>` rejection would bypass [`AppError`] and render axum's
/// plain-text 400).
async fn get_board(ValidPath(id): ValidPath<BoardId>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({"id": id.get()})))
}

fn probe_router() -> Router {
    Router::new()
        .route("/v/register", post(v_register))
        .route("/v/login", post(v_login))
        .route("/v/projects", post(v_create_project))
        .route("/v/projects-update", post(v_update_project))
        .route("/v/boards", post(v_create_board))
        .route("/v/boards-update", post(v_update_board))
        .route("/v/boards-list", get(v_boards_list))
        .route("/v/columns", post(v_create_column))
        .route("/v/columns-update", post(v_update_column))
        .route("/v/columns-sort", post(v_sort_column))
        .route("/v/columns-move", post(v_move_cards))
        .route("/v/cards", post(v_create_card))
        .route("/v/cards-update", post(v_update_card))
        .route("/v/labels", post(v_create_label))
        .route("/v/labels-update", post(v_update_label))
        .route("/v/tags", post(v_create_tag))
        .route("/v/tags-update", post(v_update_tag))
        .route("/v/checklists", post(v_create_checklist))
        .route("/v/checklists-update", post(v_update_checklist))
        .route("/v/checklist-items", post(v_create_checklist_item))
        .route("/v/checklist-items-update", post(v_update_checklist_item))
        .route("/v/notes", post(v_create_note))
        .route("/v/notes-update", post(v_update_note))
        .route("/v/notes-list", get(v_notes_list))
        .route("/api/v1/boards/{id}", get(get_board))
        .layer(middleware::from_fn(capture_request_context))
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
        .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
}

async fn read_problem(response: Response) -> (StatusCode, HeaderMap, Value) {
    let status = response.status();
    let headers = response.headers().clone();
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .expect("problem response must set Content-Type")
        .to_str()
        .expect("Content-Type must be valid ASCII")
        .to_owned();
    assert_eq!(
        content_type, "application/problem+json",
        "error responses must be problem+json"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).expect("problem body must be valid JSON");
    (status, headers, json)
}

fn assert_problem_shape(body: &Value, status: u16, type_uri: &str) {
    assert_eq!(body["status"], json!(status), "body: {body}");
    assert_eq!(body["type"], json!(type_uri), "body: {body}");
    assert!(body["title"].is_string(), "title must be set: {body}");
    assert!(body["detail"].is_string(), "detail must be set: {body}");
    assert!(body["instance"].is_string(), "instance must be set: {body}");
}

/// Messages reported for one field inside `errors[]`.
fn errors_for(body: &Value, field: &str) -> Vec<String> {
    body["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("errors[] must be set: {body}"))
        .iter()
        .filter(|entry| entry["field"] == json!(field))
        .filter_map(|entry| entry["message"].as_str().map(str::to_owned))
        .collect()
}

fn assert_field_error(body: &Value, field: &str) {
    assert_problem_shape(body, 400, &format!("{PROBLEM_BASE}/validation-error"));
    assert!(
        !errors_for(body, field).is_empty(),
        "errors[] must contain field {field:?}: {body}"
    );
}

async fn post_json(uri: &str, body: &str) -> Response {
    probe_router()
        .oneshot(
            Request::post(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_uri(uri: &str) -> Response {
    probe_router()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn post_problem(uri: &str, body: &str) -> (StatusCode, Value) {
    let (status, _, json) = read_problem(post_json(uri, body).await).await;
    (status, json)
}

/// Direct `Validate` failure → `errors[]` contains the field.
fn direct_field_error<T>(value: &T, field: &str)
where
    T: Validate<Context = ()>,
{
    let report = value.validate().expect_err("must fail validation");
    let errors: Vec<(String, String)> = report
        .iter()
        .map(|(path, error)| (path.to_string(), error.message().to_owned()))
        .collect();
    assert!(
        errors.iter().any(|(path, _)| path == field),
        "errors[] must contain field {field:?}: {errors:?}"
    );
}

#[tokio::test]
async fn register_rejects_bad_email_over_http() {
    for email in [
        "not-an-email",
        "",
        "a b@c.com",
        "user@.com",
        &format!("{}@example.com", "u".repeat(65)),
        &format!("{}@example.com", "x".repeat(250)),
    ] {
        let body =
            serde_json::to_string(&json!({"email": email, "password": "P@ssw0rd1"})).unwrap();
        let (status, json) = post_problem("/v/register", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "email: {email}");
        assert_field_error(&json, "email");
    }
    // Known divergence (recorded for T52/CONTRACT-CHANGES): garde's parser
    // accepts single-label domains (`a@b`) while validator.js `isEmail`
    // requires a TLD. The shared `validate_email` owns this behavior.
    assert!(
        RegisterInput {
            email: "a@b".to_string(),
            password: "P@ssw0rd1".to_string(),
        }
        .validate()
        .is_ok(),
        "single-label domain documents the isEmail divergence"
    );
}

#[tokio::test]
async fn register_rejects_weak_password_over_http() {
    for password in ["short1!", "password", "1234567", "        "] {
        let body = serde_json::to_string(&json!({"email": "u@example.com", "password": password}))
            .unwrap();
        let (status, json) = post_problem("/v/register", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "password: {password:?}");
        assert_field_error(&json, "password");
    }
}

#[tokio::test]
async fn register_rejects_multibyte_password_over_72_bytes() {
    // 38 chars (passes strength via the digit) but 74 bytes: the byte cap —
    // never bare garde `length` — must fire instead of a silent truncation.
    let password = format!("a1{}", "é".repeat(36));
    assert_eq!(password.chars().count(), 38);
    assert!(password.len() > MAX_PASSWORD_BYTES);
    let body =
        serde_json::to_string(&json!({"email": "u@example.com", "password": password})).unwrap();
    let (status, json) = post_problem("/v/register", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "password");
    // Strength alone passes here, so the failure is the byte cap.
    direct_field_error(
        &RegisterInput {
            email: "u@example.com".to_string(),
            password,
        },
        "password",
    );
}

#[tokio::test]
async fn register_ignores_unknown_json_fields() {
    let response = post_json(
        "/v/register",
        r#"{"email":"u@example.com","password":"P@ssw0rd1","unknown_field":1,"nested":{"x":[]}}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"email": "u@example.com"})
    );
}

#[tokio::test]
async fn register_oversized_body_is_400_invalid_json() {
    let big = format!(
        "{{\"email\":\"u@example.com\",\"password\":\"{}\"}}",
        "a".repeat(2 * 1024 * 1024 + 1)
    );
    let (status, _, body) = read_problem(post_json("/v/register", &big).await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEM_BASE}/invalid-json"));
}

#[tokio::test]
async fn register_malformed_and_missing_content_type() {
    let (status, _, body) = read_problem(post_json("/v/register", r#"{"email": "#).await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEM_BASE}/invalid-json"));

    let response = probe_router()
        .oneshot(
            Request::post("/v/register")
                .body(Body::from(r#"{"email":"u@example.com","password":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, _, body) = read_problem(response).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_problem_shape(&body, 415, &format!("{PROBLEM_BASE}/invalid-json"));
}

#[tokio::test]
async fn login_weak_but_wellformed_password_passes_validation() {
    // Legacy weak passwords must reach 401 in the handler (T11), never a
    // 400-strength rejection: login validates non-empty + byte cap only.
    for password in ["weakpass", "password", "12345678", "p@ss"] {
        let body = serde_json::to_string(&json!({"email": "u@example.com", "password": password}))
            .unwrap();
        let response = post_json("/v/login", &body).await;
        assert_eq!(response.status(), StatusCode::OK, "password: {password:?}");
    }
    assert!(LoginInput {
        email: "u@example.com".to_string(),
        password: "weakpass".to_string(),
    }
    .validate()
    .is_ok());
}

#[tokio::test]
async fn login_rejects_empty_and_overlong_password() {
    let (status, json) = post_problem(
        "/v/login",
        &serde_json::to_string(&json!({"email": "u@example.com", "password": ""})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "password");

    let overlong = "a".repeat(MAX_PASSWORD_BYTES + 1);
    let (status, json) = post_problem(
        "/v/login",
        &serde_json::to_string(&json!({"email": "u@example.com", "password": overlong})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "password");
}

#[tokio::test]
async fn board_name_blank_and_bad_hex_fail() {
    let (status, json) = post_problem(
        "/v/boards",
        &serde_json::to_string(&json!({"name": "   "})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "name");

    for color in ["red", "#zzzzzz", "#12", ""] {
        let (status, json) = post_problem(
            "/v/boards",
            &serde_json::to_string(&json!({"name": "Board", "background_color": color})).unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "color: {color:?}");
        assert_field_error(&json, "background_color");
    }

    for color in ["#0079BF", "#fff", "0079BF"] {
        let response = post_json(
            "/v/boards",
            &serde_json::to_string(&json!({"name": "Board", "background_color": color})).unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "color: {color:?}");
    }
}

#[tokio::test]
async fn board_path_param_abc_is_400_problem_json() {
    let (status, headers, body) = read_problem(get_uri("/api/v1/boards/abc").await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEM_BASE}/invalid-path"));
    assert_eq!(
        headers.get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    assert!(
        headers.contains_key(header::HeaderName::from_static("x-request-id")),
        "early rejections carry x-request-id"
    );

    let response = get_uri("/api/v1/boards/7").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"id": 7})
    );
}

#[tokio::test]
async fn boards_list_rejects_garbage_project_id() {
    let (status, _, body) = read_problem(get_uri("/v/boards-list?project_id=abc").await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEM_BASE}/invalid-query"));

    let (status, _, body) = read_problem(get_uri("/v/boards-list?project_id=0").await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&body, "project_id");
}

#[tokio::test]
async fn notes_list_rejects_bad_type_and_ids() {
    // Wire name is `?type=`; the DTO field is `note_type` (garde 0.22
    // renders Rust idents into `errors[]`, so `r#type` would leak).
    let (status, _, body) = read_problem(get_uri("/v/notes-list?type=bogus").await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&body, "note_type");

    let (status, _, body) = read_problem(get_uri("/v/notes-list?tag_id=abc").await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_problem_shape(&body, 400, &format!("{PROBLEM_BASE}/invalid-query"));

    let (status, _, body) = read_problem(get_uri("/v/notes-list?board_id=0").await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&body, "board_id");

    let response = get_uri("/v/notes-list?type=board&tag_id=2").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn card_due_date_must_be_iso8601_or_null() {
    for due in ["not-a-date", "2026-02-30", "2026-13-01", ""] {
        let (status, json) = post_problem(
            "/v/cards",
            &serde_json::to_string(&json!({"title": "T", "column_id": 1, "due_date": due}))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "due_date: {due:?}");
        assert_field_error(&json, "due_date");
    }
    for due in [
        json!("2026-01-01T00:00:00.000Z"),
        json!("2026-01-01"),
        Value::Null,
    ] {
        let response = post_json(
            "/v/cards",
            &serde_json::to_string(&json!({"title": "T", "column_id": 1, "due_date": due}))
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "due_date: {due}");
    }
}

#[tokio::test]
async fn column_move_sort_and_checklist_position_rules() {
    let (status, json) = post_problem(
        "/v/columns-move",
        &serde_json::to_string(&json!({"target_column_id": 0})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "target_column_id");

    for order in ["up", "ASC", ""] {
        let (status, json) = post_problem(
            "/v/columns-sort",
            &serde_json::to_string(&json!({"sort_order": order})).unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "order: {order:?}");
        assert_field_error(&json, "sort_order");
    }

    let (status, json) = post_problem(
        "/v/checklist-items",
        &serde_json::to_string(&json!({"text": "Buy milk", "position": -1})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "position");

    // Zero is a legal checklist-item position; card positions stay
    // unclamped at the DTO layer (negative card positions clamp in store).
    assert!(CreateChecklistItemInput {
        text: "Buy milk".to_string(),
        position: Some(0),
    }
    .validate()
    .is_ok());
    assert!(
        CreateCardInput {
            title: "T".to_string(),
            column_id: 1,
            position: Some(-5),
            description: None,
            due_date: None,
        }
        .validate()
        .is_ok(),
        "card position clamps in store, never 400s"
    );
}

#[tokio::test]
async fn label_color_enum_and_tag_freeform_color() {
    let (status, json) = post_problem(
        "/v/labels",
        &serde_json::to_string(&json!({"name": "Work", "color": "teal"})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "color");

    for color in ["red", "orange", "yellow", "green", "blue", "purple"] {
        let response = post_json(
            "/v/labels",
            &serde_json::to_string(&json!({"name": "Work", "color": color})).unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "color: {color}");
    }

    // Tag color is free-form (≤20 chars, server defaults to teal).
    let response = post_json(
        "/v/tags",
        &serde_json::to_string(&json!({"name": "urgent", "color": "teal"})).unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let (status, json) = post_problem(
        "/v/tags",
        &serde_json::to_string(&json!({"name": "urgent", "color": "x".repeat(21)})).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_field_error(&json, "color");
}

#[tokio::test]
async fn no_invalid_input_yields_422_or_500() {
    let bad_bodies = [
        ("/v/register", r#"{"email":"bad","password":"x"}"#),
        ("/v/login", r#"{"email":"bad","password":""}"#),
        ("/v/projects", r#"{"name":"   "}"#),
        ("/v/boards", r#"{"name":"","background_color":"nope"}"#),
        ("/v/boards-update", r#"{"view_mode":"grid"}"#),
        ("/v/columns", r#"{"name":"   "}"#),
        ("/v/columns-sort", r#"{"sort_order":"sideways"}"#),
        ("/v/columns-move", r#"{"target_column_id":-2}"#),
        (
            "/v/cards",
            r#"{"title":"","column_id":0,"due_date":"yesterday"}"#,
        ),
        ("/v/labels", r#"{"name":"","color":"chartreuse"}"#),
        ("/v/tags", r#"{"name":""}"#),
        ("/v/checklists", r#"{"title":"  "}"#),
        ("/v/checklist-items", r#"{"text":"","position":-9}"#),
        (
            "/v/notes",
            r#"{"title":"","content":"","board_id":0,"tag_ids":[0]}"#,
        ),
    ];
    for (uri, body) in bad_bodies {
        let response = post_json(uri, body).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "every validation failure is 400: {uri} {body}"
        );
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("must set Content-Type")
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(content_type, "application/problem+json", "uri: {uri}");
    }
}

#[tokio::test]
async fn update_dtos_trim_and_required_rules() {
    // Project update requires a non-blank name (DTO emits 400, not a
    // service-level error).
    direct_field_error(&UpdateProjectInput { name: None }, "name");
    direct_field_error(
        &UpdateProjectInput {
            name: Some("   ".to_string()),
        },
        "name",
    );
    assert!(UpdateProjectInput {
        name: Some("New".to_string()),
    }
    .validate()
    .is_ok());

    // Optional update fields: absent is fine, blank is 400.
    direct_field_error(
        &UpdateBoardInput {
            name: Some("  ".to_string()),
            background_color: None,
            project_id: None,
            view_mode: None,
            is_archived: None,
        },
        "name",
    );
    direct_field_error(
        &UpdateBoardInput {
            name: None,
            background_color: None,
            project_id: None,
            view_mode: Some("grid".to_string()),
            is_archived: None,
        },
        "view_mode",
    );
    assert!(UpdateBoardInput {
        name: None,
        background_color: None,
        project_id: None,
        view_mode: None,
        is_archived: Some(true),
    }
    .validate()
    .is_ok());

    direct_field_error(
        &UpdateTagInput {
            name: Some(String::new()),
            color: None,
        },
        "name",
    );
    direct_field_error(
        &UpdateLabelInput {
            name: Some("  ".to_string()),
            color: None,
        },
        "name",
    );
    direct_field_error(
        &UpdateCardInput {
            title: Some("  ".to_string()),
            column_id: None,
            position: None,
            description: None,
            due_date: None,
        },
        "title",
    );
    direct_field_error(
        &UpdateChecklistItemInput {
            text: Some(String::new()),
            is_completed: None,
        },
        "text",
    );
}

#[tokio::test]
async fn length_caps_match_nest_dtos() {
    direct_field_error(
        &CreateProjectInput {
            name: "x".repeat(256),
        },
        "name",
    );
    direct_field_error(
        &CreateLabelInput {
            name: "x".repeat(51),
            color: "red".to_string(),
        },
        "name",
    );
    direct_field_error(
        &CreateTagInput {
            name: "x".repeat(51),
            color: None,
        },
        "name",
    );
    direct_field_error(
        &CreateCardInput {
            title: "T".to_string(),
            column_id: 1,
            position: None,
            description: Some("x".repeat(10_001)),
            due_date: None,
        },
        "description",
    );
    direct_field_error(
        &CreateNoteInput {
            title: "T".to_string(),
            content: String::new(),
            board_id: None,
            project_id: None,
            card_id: None,
            tag_ids: None,
        },
        "content",
    );
    direct_field_error(
        &CreateNoteInput {
            title: "T".to_string(),
            content: "x".repeat(65_536),
            board_id: None,
            project_id: None,
            card_id: None,
            tag_ids: None,
        },
        "content",
    );
    direct_field_error(
        &CreateNoteInput {
            title: "T".to_string(),
            content: "ok".to_string(),
            board_id: None,
            project_id: None,
            card_id: None,
            tag_ids: Some(vec![1, 0]),
        },
        "tag_ids",
    );
    assert!(CreateNoteInput {
        title: "T".to_string(),
        content: "x".repeat(65_535),
        board_id: Some(2),
        project_id: None,
        card_id: None,
        tag_ids: Some(vec![1, 2]),
    }
    .validate()
    .is_ok());
}
