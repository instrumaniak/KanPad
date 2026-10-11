//! auth handlers: extract -> store -> `Result<Json<_>, AppError>`. No SQL here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json,
};
use garde::Validate;
use tower_sessions::Session;

use crate::{
    common::{label_seed::ensure_default_labels, validation::normalize_email},
    error::{AppError, Internal},
    extract::AuthSession,
    state::AppState,
};

use super::{
    store::{
        dummy_verify, find_auth_row_by_email, find_user_by_email, find_user_by_id, hash_password,
        insert_user_row, update_password_hash, verify_argon2, verify_bcrypt_truncated,
    },
    types::{LoginInput, LoginResponse, MeResponse, RegisterInput, RegisterResponse, UserData},
};

/// Feature router (no `with_state` inside; the caller applies state once).
/// Mounted under `/api/v1` (+ the deprecated `/api` alias) by `lib.rs`.
pub fn router() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
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
    // JSON, the password, or its hash. Shared with login-success so the two
    // entry points cannot drift (fixation defense: register logs in).
    establish_session(&session, &user.email, user.id.get(), &user.role).await?;

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

/// `POST /api/v1/auth/login{email,password} → 200 {data:{id,email,role}}`,
/// no `message` key.
///
/// * The email is normalized (`trim` + lowercase) BEFORE validation.
/// * Validation is non-empty + 72-byte cap only (never strength): legacy
///   weak-but-wellformed passwords reach auth (200 on match, 401 on
///   mismatch), never 400.
/// * Unknown email runs [`dummy_verify`] (same Argon2 work as a real check)
///   then 401s with the identical problem shape — no enumeration oracle.
/// * Verify order: Argon2 → on miss legacy bcrypt (cost-10 `$2a$`/`$2b$`,
///   password truncated to 72 bytes) → on bcrypt success rehash to Argon2id
///   + update the row in the same request. Never mints bcrypt.
/// * `session.cycle_id()` on every login (fixation defense). Session keys
///   are `user_id`/`email`/`role`, matching the `AuthSession` contract.
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginInput,
    responses(
        (status = 200, description = "User logged in", body = LoginResponse),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Invalid credentials")
    )
)]
#[tracing::instrument(skip(state, session, input))]
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Json(mut input): Json<LoginInput>,
) -> Result<impl IntoResponse, AppError> {
    input.email = normalize_email(&input.email);
    input.validate().map_err(AppError::Validation)?;

    let Some(row) = find_auth_row_by_email(&state.pool, &input.email).await? else {
        dummy_verify(state.settings.hash_cost);
        return Err(AppError::Unauthorized);
    };

    if !verify_argon2(&input.password, &row.password_hash)
        && !verify_bcrypt_truncated(&input.password, &row.password_hash)
    {
        return Err(AppError::Unauthorized);
    }
    // Legacy-bcrypt success (Argon2 missed but bcrypt hit): upgrade to
    // Argon2id in the same request. Argon2-success skips the write.
    if !row.password_hash.starts_with("$argon2") {
        let upgraded = hash_password(&input.password, state.settings.hash_cost)?;
        update_password_hash(&state.pool, row.user.id.get(), &upgraded).await?;
    }

    establish_session(&session, &row.user.email, row.user.id.get(), &row.user.role).await?;

    Ok(Json(LoginResponse {
        data: UserData {
            id: row.user.id,
            email: row.user.email,
            role: row.user.role,
        },
    }))
}

/// `POST /api/v1/auth/logout → 204` (no body).
///
/// `flush()` destroys the store row AND clears the in-memory id so the
/// middleware emits a cookie-removal (`Max-Age=0`); a bare `delete()` would
/// leave data behind and the session would be re-saved. Anonymous logout
/// still 204s (never 4xx on the destroy path). Deliberately unthrottled
/// (T30 scopes governor to login/register only — logging out must never
/// 429).
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses((status = 204, description = "User logged out"))
)]
#[tracing::instrument(skip(session))]
pub async fn logout(session: Session) -> Result<impl IntoResponse, AppError> {
    session.flush().await.map_err(|_| {
        tracing::error!("logout: failed to destroy session");
        AppError::Internal(Internal)
    })?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/auth/me → 200 {data:{id,email,role}}` else 401.
///
/// The ONLY auth read that re-looks-up the user row (plus admin routes):
/// deleted → 401; role drift (demoted/promoted since login) → 403 after
/// rotating the session id (privilege-change fixation hygiene). Returns the
/// fresh row values, never the stale session payload.
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current user", body = MeResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Role changed since login")
    )
)]
#[tracing::instrument(skip(state, session))]
pub async fn me(
    State(state): State<AppState>,
    auth: AuthSession,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let row = match find_user_by_id(&state.pool, auth.user_id.get()).await {
        Ok(row) => row,
        Err(AppError::Db(sqlx::Error::RowNotFound)) => return Err(AppError::Unauthorized),
        Err(other) => return Err(other),
    };
    if row.role != auth.role {
        session.cycle_id().await.map_err(|_| {
            tracing::error!("me: failed to rotate session id on role change");
            AppError::Internal(Internal)
        })?;
        return Err(AppError::Forbidden);
    }
    Ok(Json(MeResponse {
        data: UserData {
            id: row.id,
            email: row.email,
            role: row.role,
        },
    }))
}

/// Persist `user_id`/`email`/`role` and rotate the session id (fixation
/// defense). Shared by register-success and login-success so the two entry
/// points cannot drift. Failures carry no payload (`Internal` is a unit
/// struct — never session JSON, password, or hash).
async fn establish_session(
    session: &Session,
    email: &str,
    user_id: i32,
    role: &str,
) -> Result<(), AppError> {
    session.insert("user_id", user_id).await.map_err(|_| {
        tracing::error!("auth: failed to persist session");
        AppError::Internal(Internal)
    })?;
    session.insert("email", email).await.map_err(|_| {
        tracing::error!("auth: failed to persist session");
        AppError::Internal(Internal)
    })?;
    session.insert("role", role).await.map_err(|_| {
        tracing::error!("auth: failed to persist session");
        AppError::Internal(Internal)
    })?;
    session.cycle_id().await.map_err(|_| {
        tracing::error!("auth: failed to rotate session id");
        AppError::Internal(Internal)
    })?;
    Ok(())
}
