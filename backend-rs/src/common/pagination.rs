//! Shared `Page<T>` list envelope (`{data,page,limit,total}`).
//!
//! Every top-level list returns `Page<T>` via a real count query (never
//! `data.length`). Nested board/column children stay unpaginated. utoipa
//! composes per-item names (`Page_BoardResponse`); concrete `#[aliases]`
//! land in T40a alongside the DTOs.

use serde::Serialize;
use utoipa::ToSchema;

/// Paginated list envelope. `total` is the full count for the applied
/// filters, not the length of this page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Page<T> {
    /// Items on this page.
    pub data: Vec<T>,
    /// 1-based page number (echoes [`crate::extract::PaginationParams::page`]).
    pub page: u32,
    /// Items per page (echoes `limit`).
    pub limit: u32,
    /// Total items across all pages.
    pub total: u64,
}

impl<T> Page<T> {
    /// Build a page from the fetched items plus the separate count query.
    pub fn new(data: Vec<T>, page: u32, limit: u32, total: u64) -> Self {
        Self {
            data,
            page,
            limit,
            total,
        }
    }

    /// Empty page (lets handlers short-circuit without a select).
    pub fn empty(page: u32, limit: u32) -> Self {
        Self::new(Vec::new(), page, limit, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_serializes_snake_case_envelope() {
        let page = Page::new(vec!["a", "b"], 2, 10, 25);
        assert_eq!(
            serde_json::to_value(&page).unwrap(),
            json!({"data": ["a", "b"], "page": 2, "limit": 10, "total": 25})
        );
        assert_eq!(Page::<String>::empty(1, 50).total, 0);
    }
}
