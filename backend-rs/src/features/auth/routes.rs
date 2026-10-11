//! auth handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::post,
    Json,
};
use garde::Validate;
use tower_sessions::Session;

use crate::{
    common::{label_seed::ensure_default_labels, validation::normalize_email},
    error::{AppError, Internal},
    state::AppState,
};

use super::{
    store::{find_user_by_email, find_user_by_id, hash_password, insert_user_row},
    types::{RegisterInput, RegisterResponse, UserData},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`, so
/// this route serves `POST /api/v1/auth/register`.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new().route("/auth/register", post(register))
}

/// `POST /api/v1/auth/register{email,password} → 201 {data:{id,email,role}}`
/// + `Set-Cookie` + `Location: /api/v1/auth/me`.
///
/// * `REGISTRATION_ENABLED=false` → `403` (checked before validation, like
///   Nest — role-denied, never 404).
/// * The email is normalized (`trim` + lowercase) BEFORE validation, so
///   padded mixed-case input registers instead of 400ing.
/// * Bad email / weak / overlong password → `400` problem+json `errors[]`.
/// * Dup email → `409` (pre-check plus the `1062` race, which the shared
///   `Db` arm maps to `409`).
/// * The Argon2id hash runs BEFORE `tx.begin()`; the tx holds only `INSERT
///   user` + the single multi-row `INSERT labels` (no network `.await`
///   while it is open).
/// * `session.cycle_id()` establishes a fresh authenticated session
///   (fixation defense — register logs the user in, like Nest). Session keys
///   are `user_id`/`email`/`role`, matching the T11 `AuthSession` contract.
#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterInput,
    responses(
        (status = 201, description = "User registered", body = RegisterResponse),
        (status = 400, description = "Validation failed"),
        (status = 403, description = "Registration closed"),
        (status = 409, description = "Email already registered")
    )
)]
#[tracing::instrument(skip(state, session, input))]
pub async fn register(
    State(state): State<AppState>,
    session: Session,
    Json(mut input): Json<RegisterInput>,
) -> Result<impl IntoResponse, AppError> {
    if !state.settings.registration_enabled {
        return Err(AppError::Forbidden);
    }
    input.email = normalize_email(&input.email);
    input.validate().map_err(AppError::Validation)?;

    if find_user_by_email(&state.pool, &input.email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict);
    }

    let password_hash = hash_password(&input.password, state.settings.hash_cost)?;

    let mut tx = state.pool.begin().await.map_err(AppError::Db)?;
    let user_id = insert_user_row(&mut *tx, &input.email, &password_hash).await?;
    ensure_default_labels(&mut *tx, user_id)
        .await
        .map_err(AppError::Db)?;
    let user = find_user_by_id(&mut *tx, user_id).await?;
    tx.commit().await.map_err(AppError::Db)?;

    // Session persistence failures carry no payload (`Internal` is a unit
    // struct); only a static message is logged server-side — never session
    // JSON, the password, or its hash.
    session
        .insert("user_id", user.id.get())
        .await
        .map_err(|_| {
            tracing::error!("register: failed to persist session");
            AppError::Internal(Internal)
        })?;
    session.insert("email", &user.email).await.map_err(|_| {
        tracing::error!("register: failed to persist session");
        AppError::Internal(Internal)
    })?;
    session.insert("role", &user.role).await.map_err(|_| {
        tracing::error!("register: failed to persist session");
        AppError::Internal(Internal)
    })?;
    session.cycle_id().await.map_err(|_| {
        tracing::error!("register: failed to rotate session id");
        AppError::Internal(Internal)
    })?;

    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, "/api/v1/auth/me")],
        Json(RegisterResponse {
            data: UserData {
                id: user.id,
                email: user.email,
                role: user.role,
            },
        }),
    ))
}
