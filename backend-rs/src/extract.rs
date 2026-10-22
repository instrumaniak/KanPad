//! Custom extractors. Validation itself is garde derives on the feature DTOs
//! (T08); this module holds the state-generic validated-body/query wrappers
//! plus [`PaginationParams`] (T04; `AuthSession` is T11).
//!
//! [`ValidJson`] / [`ValidQuery`] are the `Valid(Json<T>)` /
//! `Valid(Query<T>)` extractors from `20-api-contract.md`: they run the
//! inner axum extractor (content-type, 2 MiB body limit, type parsing —
//! failures map to the shared `invalid-json` / `invalid-query` arms) and
//! then `garde::Validate::validate` with the unit context (failures map to
//! `AppError::Validation` → 400 problem+json with `errors[]`).
//!
//! Note on `axum-valid`: **the dependency was REMOVED (T63)** — it had ZERO
//! references in this crate, and it dragged a *second, complete* axum 0.7.9
//! (plus `axum-core 0.4.5`, `matchit 0.7.3`, the `validator` 0.18 framework
//! and a second `garde` 0.20) into the tree beside the real axum 0.8.9.
//! These hand-rolled extractors stay: they are behavior-identical AND
//! state-generic (`Garde<E>`'s `Context: FromRef<State>` bound has no blanket
//! `FromRef<S> for ()`, so it is not a drop-in even at a newer version).
//! Do NOT re-add the crate: the extract-then-validate sequence it provided
//! is 30 lines above, and `garde` is already a direct dependency.

use axum::{
    extract::{FromRequest, FromRequestParts, Json, Query, Request},
    http::request::Parts,
};
use garde::Validate;
use serde::{Deserialize, de::DeserializeOwned};
use tower_sessions::Session;

use crate::{common::ids::UserId, error::AppError, state::AppState};

/// Strict pagination query (`?page=&limit=`).
///
/// Shared `page=1, limit=50` defaults with manual strict checks (`page >= 1`,
/// `1 <= limit <= 100`). Out-of-range or non-integer values return 400
/// problem+json with `errors[]` in the exact validation shape — never a
/// silent clamp (retires the notes-service `Math.min(limit, 100)` behavior).
/// Unknown query keys are ignored (serde default, matching Nest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaginationParams {
    /// 1-based page number.
    pub page: u32,
    /// Items per page.
    pub limit: u32,
}

impl PaginationParams {
    /// Default page when `?page=` is absent.
    pub const DEFAULT_PAGE: u32 = 1;
    /// Default limit when `?limit=` is absent.
    pub const DEFAULT_LIMIT: u32 = 50;
    /// Hard ceiling; larger values are 400, never clamped.
    pub const MAX_LIMIT: u32 = 100;
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: Self::DEFAULT_PAGE,
            limit: Self::DEFAULT_LIMIT,
        }
    }
}

/// Raw query with stringly values so every malformed shape (`abc`, `1.5`,
/// `-1`, empty) funnels into the same strict 400 path instead of axum's
/// `QueryRejection`.
#[derive(Debug, Deserialize)]
struct RawPaginationQuery {
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    limit: Option<String>,
}

fn parse_field(
    raw: Option<String>,
    field: &'static str,
    report: &mut garde::Report,
) -> Option<u32> {
    match raw {
        None => None,
        Some(value) => match value.parse::<u32>() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                report.append(
                    garde::Path::new(field),
                    garde::Error::new("must be an integer"),
                );
                None
            }
        },
    }
}

impl<S> FromRequestParts<S> for PaginationParams
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(raw) = Query::<RawPaginationQuery>::from_request_parts(parts, state)
            .await
            .map_err(AppError::Query)?;

        let mut report = garde::Report::new();
        let page = parse_field(raw.page, "page", &mut report).unwrap_or(Self::DEFAULT_PAGE);
        let limit = parse_field(raw.limit, "limit", &mut report).unwrap_or(Self::DEFAULT_LIMIT);

        if page < 1 {
            report.append(
                garde::Path::new("page"),
                garde::Error::new("must be at least 1"),
            );
        }
        if limit < 1 {
            report.append(
                garde::Path::new("limit"),
                garde::Error::new("must be at least 1"),
            );
        }
        if limit > Self::MAX_LIMIT {
            report.append(
                garde::Path::new("limit"),
                garde::Error::new("must be at most 100"),
            );
        }

        if report.is_empty() {
            Ok(Self { page, limit })
        } else {
            Err(AppError::Validation(report))
        }
    }
}

/// Validated JSON body: `Valid(Json<T>)` in `20-api-contract.md` terms.
///
/// Handlers take `ValidJson<CreateBoardInput>` etc. instead of `Json<T>`.
/// Unknown JSON fields are ignored (serde default, matching Nest — no
/// `deny_unknown_fields`); wrong content-type / malformed JSON / over-limit
/// bodies map to the shared `invalid-json` arm (over-limit included: the
/// 2 MiB `DefaultBodyLimit` surfaces as the catch-all 400, never bare 413).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidJson<T>(pub T);

impl<T> ValidJson<T> {
    /// Unwrap to the validated body.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for ValidJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> FromRequest<S> for ValidJson<T>
where
    T: DeserializeOwned + Validate<Context = ()>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(AppError::Json)?;
        value.validate().map_err(AppError::Validation)?;
        Ok(Self(value))
    }
}

/// Validated query string: `Valid(Query<T>)` in `20-api-contract.md` terms.
///
/// Type-parse failures (`?tag_id=abc`) never reach garde — axum rejects
/// them as 400 `invalid-query` through the shared arm; garde rule failures
/// (unknown enum value, `0` id) map to `AppError::Validation` → 400
/// problem+json with `errors[]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidQuery<T>(pub T);

impl<T> ValidQuery<T> {
    /// Unwrap to the validated query.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for ValidQuery<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> FromRequestParts<S> for ValidQuery<T>
where
    T: DeserializeOwned + Validate<Context = ()>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(AppError::Query)?;
        value.validate().map_err(AppError::Validation)?;
        Ok(Self(value))
    }
}

/// Strict path params (`GET /api/v1/boards/{id}` and friends).
///
/// Handlers take `ValidPath<BoardId>` instead of `Path<BoardId>`: a raw
/// `Path<T>` rejection bypasses `AppError` (axum renders its own plain-text
/// 400), while this wrapper routes int-parse failures through
/// `AppError::Path` → 400 problem+json (`invalid-path`, never plain-text).
/// Resource routers (T21+) must use this wrapper for every `{id}` param.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidPath<T>(pub T);

impl<T> ValidPath<T> {
    /// Unwrap to the parsed path params.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for ValidPath<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> FromRequestParts<S> for ValidPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Path(value) = axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(AppError::Path)?;
        Ok(Self(value))
    }
}

/// Authenticated session (`AuthSession`, T11).
///
/// Requires the tower-sessions cookie to carry `user_id (> 0)` + `email` +
/// `role` (unifies Nest's `SessionGuard` userId-only check and `me`'s triple
/// check). Anything else — anonymous, expired row (the sqlx store's `load`
/// filters `expiry_date > now`, so expired sessions hydrate empty), or a
/// cookie that fails HMAC verification under the current `SESSION_SECRET`
/// (wrong/pre-cutover secret → anonymous) — rejects with `401`
/// `AppError::Unauthorized` through the shared problem+json renderer.
///
/// The payload is trusted without a per-request DB lookup (same as Nest's
/// `@CurrentUser()`), EXCEPT `GET /api/v1/auth/me` + admin routes, which
/// MUST re-lookup the user row (deleted → 401, demoted → 403).
/// Session-load failures also map to `401` (never a 500 oracle for session
/// state); the DB re-lookup in `me` is where outages surface as `503`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    /// `users.id` (`INT` ↔ `i32` newtype, always `> 0`).
    pub user_id: UserId,
    /// Normalized session email.
    pub email: String,
    /// Session role (`user` / `admin`).
    pub role: String,
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Unauthorized)?;
        let user_id: Option<i32> = session
            .get("user_id")
            .await
            .map_err(|_| AppError::Unauthorized)?;
        let email: Option<String> = session
            .get("email")
            .await
            .map_err(|_| AppError::Unauthorized)?;
        let role: Option<String> = session
            .get("role")
            .await
            .map_err(|_| AppError::Unauthorized)?;
        match (user_id, email, role) {
            (Some(id), Some(email), Some(role))
                if id > 0 && !email.is_empty() && !role.is_empty() =>
            {
                Ok(Self {
                    user_id: UserId::from(id),
                    email,
                    role,
                })
            }
            _ => Err(AppError::Unauthorized),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::{FromRequestParts, Request},
        response::IntoResponse,
    };

    async fn extract(uri: &str) -> Result<PaginationParams, AppError> {
        let (mut parts, _) = Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap()
            .into_parts();
        PaginationParams::from_request_parts(&mut parts, &()).await
    }

    #[tokio::test]
    async fn defaults_apply_without_query() {
        assert_eq!(
            extract("/items").await.unwrap(),
            PaginationParams { page: 1, limit: 50 }
        );
    }

    #[tokio::test]
    async fn out_of_range_is_strict_400_with_errors() {
        for uri in ["/items?page=0", "/items?limit=0", "/items?limit=101"] {
            let error = extract(uri).await.expect_err("must reject");
            let response = error.into_response();
            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        }
        // `page` has no ceiling: large pages are legal (possibly empty data).
        assert_eq!(
            extract("/items?page=999999&limit=100").await.unwrap(),
            PaginationParams {
                page: 999_999,
                limit: 100
            }
        );
    }

    #[tokio::test]
    async fn non_integers_reject() {
        for uri in [
            "/items?page=abc",
            "/items?page=-1",
            "/items?page=1.5",
            "/items?limit=",
        ] {
            assert!(
                extract(uri).await.is_err(),
                "must reject non-integer: {uri}"
            );
        }
    }
}
