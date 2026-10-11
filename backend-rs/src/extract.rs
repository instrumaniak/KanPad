//! Custom extractors. Validation itself is `axum-valid` `Valid(Json/Query)`
//! (T08); only [`PaginationParams`] lives here in T04 (`AuthSession` is T11).

use axum::{
    extract::{FromRequestParts, Query},
    http::request::Parts,
};
use serde::Deserialize;

use crate::error::AppError;

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
