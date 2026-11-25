//! Shared card filter parsing and SQL composition (T049).
//!
//! All three card read paths (board column embed, column cards page, board
//! cards list) accept the same filter params. This module owns validation,
//! LIKE escaping, and bound-parameter WHERE fragment composition so semantics
//! cannot drift across endpoints.

use chrono::Datelike;
use sqlx::{MySql, QueryBuilder};

use crate::error::{AppError, Internal};

/// Max chars for `search` after trim.
pub const MAX_SEARCH_LEN: usize = 255;
/// Max distinct label IDs in a filter.
pub const MAX_LABEL_IDS: usize = 20;

/// Allowed `due_date` filter values.
pub const DUE_DATE_FILTERS: [&str; 4] = ["overdue", "today", "this_week", "no_date"];
/// Allowed `checklist` filter values.
pub const CHECKLIST_FILTERS: [&str; 3] = ["with", "complete", "incomplete"];

/// Card due-date filter (UTC semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueDateFilter {
    /// `due_date < start_of_today_utc`.
    Overdue,
    /// `due_date` in `[start_of_today_utc, start_of_tomorrow_utc)`.
    Today,
    /// `due_date` in `[start_of_today_utc, start_of_next_week_utc)` (Monday-start week).
    ThisWeek,
    /// `due_date IS NULL`.
    NoDate,
}

impl DueDateFilter {
    /// Parse a query value into a due-date filter.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "overdue" => Some(Self::Overdue),
            "today" => Some(Self::Today),
            "this_week" => Some(Self::ThisWeek),
            "no_date" => Some(Self::NoDate),
            _ => None,
        }
    }

    /// Canonical query string value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overdue => "overdue",
            Self::Today => "today",
            Self::ThisWeek => "this_week",
            Self::NoDate => "no_date",
        }
    }
}

/// Card checklist filter (aggregated across all checklists for a card).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecklistFilter {
    /// Card has >= 1 checklist.
    With,
    /// Card has >= 1 checklist, total items > 0, completed == total.
    Complete,
    /// Card has >= 1 checklist and (total items == 0 or completed < total).
    Incomplete,
}

impl ChecklistFilter {
    /// Parse a query value into a checklist filter.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "with" => Some(Self::With),
            "complete" => Some(Self::Complete),
            "incomplete" => Some(Self::Incomplete),
            _ => None,
        }
    }

    /// Canonical query string value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::With => "with",
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
        }
    }
}

/// Validated card filters. `None`/empty means no filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardFilters {
    /// Trimmed, non-empty search substring; SQL LIKE metacharacters escaped by
    /// [`escape_like_pattern`] at bind time.
    pub search: Option<String>,
    /// Distinct positive label IDs, AND semantics.
    pub labels: Vec<i32>,
    /// Optional due-date filter.
    pub due_date: Option<DueDateFilter>,
    /// Optional checklist rollup filter.
    pub checklist: Option<ChecklistFilter>,
}

impl CardFilters {
    /// Returns true when no filter is active.
    pub fn is_empty(&self) -> bool {
        self.search.is_none()
            && self.labels.is_empty()
            && self.due_date.is_none()
            && self.checklist.is_none()
    }
}

/// Escape a raw search substring for `LIKE`: `\` → `\\`, `%` → `\%`,
/// `_` → `\_` (backslash first), then wrapped in `%…%`. The pattern must be
/// bound (never interpolated) and used with `ESCAPE '\\'`.
pub fn escape_like_pattern(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len() + 2);
    escaped.push('%');
    for ch in raw.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('%');
    escaped
}

fn push_field_error(report: &mut garde::Report, field: &'static str, message: impl Into<String>) {
    report.append(garde::Path::new(field), garde::Error::new(message.into()));
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    Many(Vec<String>),
    One(String),
}

/// Deserialize a query param that may appear as a single string or repeated strings.
pub fn deserialize_string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::Many(values) => Ok(values),
        StringOrVec::One(value) => Ok(vec![value]),
    }
}

fn percent_decode_query(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = bytes[i + 1] as char;
                let lo = bytes[i + 2] as char;
                if let (Some(h), Some(l)) = (hi.to_digit(16), lo.to_digit(16)) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse raw query string into key-value pairs, preserving repeated keys.
pub fn parse_query_pairs(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut it = part.splitn(2, '=');
            let k = percent_decode_query(it.next().unwrap_or(""));
            let v = percent_decode_query(it.next().unwrap_or(""));
            (k, v)
        })
        .collect()
}

/// Parse/validate raw query filter fields into [`CardFilters`].
///
/// `labels_raw` are the repeated `labels` query values as strings so invalid
/// integers become validation errors with `errors[]`, not query parse failures.
pub fn parse_card_filters(
    search: Option<&str>,
    labels_raw: &[String],
    due_date: Option<&str>,
    checklist: Option<&str>,
) -> Result<CardFilters, garde::Report> {
    let mut report = garde::Report::new();

    let search = match search {
        None => None,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                let char_len = trimmed.chars().count();
                if char_len > MAX_SEARCH_LEN {
                    push_field_error(
                        &mut report,
                        "search",
                        format!("length must be at most {MAX_SEARCH_LEN} characters"),
                    );
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
        }
    };

    let mut labels: Vec<i32> = Vec::new();
    if !labels_raw.is_empty() {
        let mut seen = std::collections::HashSet::new();
        for raw in labels_raw {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let parsed = match raw.parse::<i32>() {
                Ok(value) => value,
                Err(_) => {
                    push_field_error(&mut report, "labels", "must be an integer");
                    continue;
                }
            };
            if parsed < 1 {
                push_field_error(&mut report, "labels", "must be at least 1");
                continue;
            }
            if seen.insert(parsed) {
                labels.push(parsed);
            }
        }
        if labels.len() > MAX_LABEL_IDS {
            push_field_error(
                &mut report,
                "labels",
                format!("must have at most {MAX_LABEL_IDS} distinct values"),
            );
        }
    }

    let due_date = match due_date {
        None => None,
        Some(raw) => match DueDateFilter::parse(raw) {
            Some(value) => Some(value),
            None => {
                push_field_error(
                    &mut report,
                    "due_date",
                    format!("due_date must be one of: {}", DUE_DATE_FILTERS.join(", ")),
                );
                None
            }
        },
    };

    let checklist = match checklist {
        None => None,
        Some(raw) => match ChecklistFilter::parse(raw) {
            Some(value) => Some(value),
            None => {
                push_field_error(
                    &mut report,
                    "checklist",
                    format!("checklist must be one of: {}", CHECKLIST_FILTERS.join(", ")),
                );
                None
            }
        },
    };

    if report.is_empty() {
        Ok(CardFilters {
            search,
            labels,
            due_date,
            checklist,
        })
    } else {
        Err(report)
    }
}

/// UTC date boundaries for due-date filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DueDateBoundaries {
    /// 00:00:00 UTC of the current day.
    pub start_of_today: chrono::NaiveDateTime,
    /// 00:00:00 UTC of the next day.
    pub start_of_tomorrow: chrono::NaiveDateTime,
    /// 00:00:00 UTC of next Monday (Monday-start week, 7-day window).
    pub start_of_next_week: chrono::NaiveDateTime,
}

/// Compute due-date boundaries from current UTC time.
pub fn due_date_boundaries_now() -> DueDateBoundaries {
    due_date_boundaries_at(chrono::Utc::now())
}

/// Compute due-date boundaries at a specific UTC instant.
pub fn due_date_boundaries_at(now: chrono::DateTime<chrono::Utc>) -> DueDateBoundaries {
    let today = now.date_naive();
    let start_of_today = today.and_hms_opt(0, 0, 0).expect("valid midnight");
    let start_of_tomorrow = (today + chrono::Days::new(1))
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    // Monday-start week, 7-day window: start_of_today .. start_of_next_week
    let weekday = today.weekday().num_days_from_monday();
    let start_of_week = (today - chrono::Days::new(u64::from(weekday)))
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    // Spec: "this_week: due_date IS NOT NULL AND due_date >= start_of_today_utc
    // AND due_date < start_of_next_week_utc, where week is Monday-start, 7-day
    // window in UTC". Upper bound is next Monday 00:00:00 UTC.
    let start_of_next_week = start_of_week + chrono::Days::new(7);
    DueDateBoundaries {
        start_of_today,
        start_of_tomorrow,
        start_of_next_week,
    }
}

/// Assert label IDs exist and are owned by the user. Any missing/foreign/
/// unauthorized ID is a validation error (no existence oracle).
pub async fn check_labels_visible_to_user(
    pool: &sqlx::MySqlPool,
    user_id: i32,
    label_ids: &[i32],
) -> Result<(), AppError> {
    if label_ids.is_empty() {
        return Ok(());
    }
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM `labels` WHERE `user_id` = ");
    builder.push_bind(user_id);
    builder.push(" AND `id` IN (");
    {
        let mut sep = builder.separated(", ");
        for id in label_ids {
            sep.push_bind(*id);
        }
    }
    builder.push(")");
    let matched: i64 = builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(AppError::Db)?;
    let expected = i64::try_from(label_ids.len()).map_err(|_| AppError::Internal(Internal))?;
    if matched != expected {
        let mut report = garde::Report::new();
        push_field_error(
            &mut report,
            "labels",
            "contains unknown or unauthorized label id",
        );
        return Err(AppError::from(report));
    }
    Ok(())
}

/// Append card-filter WHERE clauses to an existing builder.
///
/// The caller must have already pushed `WHERE <base scope>` and passes the
/// card table alias (`k` in current queries). All user values are bound.
pub fn push_card_filters(
    builder: &mut QueryBuilder<MySql>,
    card_alias: &str,
    filters: &CardFilters,
    due: DueDateBoundaries,
) {
    if let Some(search) = filters.search.as_deref() {
        builder.push(" AND ");
        builder.push(card_alias);
        builder.push(".`title` LIKE ");
        builder.push_bind(escape_like_pattern(search));
        builder.push(" ESCAPE '\\\\'");
    }

    if !filters.labels.is_empty() {
        // Label IDs are pre-validated as owned by the authenticated user (400 if
        // not, no oracle). Cards are already scoped to the owned board/column.
        builder.push(" AND ");
        builder.push(card_alias);
        builder
            .push(".`id` IN (SELECT cl.`card_id` FROM `card_labels` cl WHERE cl.`label_id` IN (");
        {
            let mut sep = builder.separated(", ");
            for id in &filters.labels {
                sep.push_bind(*id);
            }
        }
        builder.push(") GROUP BY cl.`card_id` HAVING COUNT(DISTINCT cl.`label_id`) = ");
        builder.push_bind(i64::try_from(filters.labels.len()).unwrap_or(i64::MAX));
        builder.push(")");
    }

    if let Some(due_date) = filters.due_date {
        match due_date {
            DueDateFilter::NoDate => {
                builder.push(" AND ");
                builder.push(card_alias);
                builder.push(".`due_date` IS NULL");
            }
            DueDateFilter::Overdue => {
                builder.push(" AND ");
                builder.push(card_alias);
                builder.push(".`due_date` IS NOT NULL AND ");
                builder.push(card_alias);
                builder.push(".`due_date` < ");
                builder.push_bind(due.start_of_today);
            }
            DueDateFilter::Today => {
                builder.push(" AND ");
                builder.push(card_alias);
                builder.push(".`due_date` IS NOT NULL AND ");
                builder.push(card_alias);
                builder.push(".`due_date` >= ");
                builder.push_bind(due.start_of_today);
                builder.push(" AND ");
                builder.push(card_alias);
                builder.push(".`due_date` < ");
                builder.push_bind(due.start_of_tomorrow);
            }
            DueDateFilter::ThisWeek => {
                builder.push(" AND ");
                builder.push(card_alias);
                builder.push(".`due_date` IS NOT NULL AND ");
                builder.push(card_alias);
                builder.push(".`due_date` >= ");
                builder.push_bind(due.start_of_today);
                builder.push(" AND ");
                builder.push(card_alias);
                builder.push(".`due_date` < ");
                builder.push_bind(due.start_of_next_week);
            }
        }
    }

    if let Some(checklist) = filters.checklist {
        // Correlated rollup across all checklists for the card.
        // Avoid derived-table correlation (not portable as LATERAL) by using
        // a correlated grouped EXISTS with HAVING.
        match checklist {
            ChecklistFilter::With => {
                builder.push(" AND EXISTS (SELECT 1 FROM `checklists` cx WHERE cx.`card_id` = ");
                builder.push(card_alias);
                builder.push(".`id`)");
            }
            ChecklistFilter::Complete => {
                builder.push(
                    " AND EXISTS (SELECT 1 FROM `checklists` clx \
                     LEFT JOIN `checklist_items` cix ON cix.`checklist_id` = clx.`id` \
                     WHERE clx.`card_id` = ",
                );
                builder.push(card_alias);
                builder.push(
                    ".`id` GROUP BY clx.`card_id` \
                     HAVING COUNT(cix.`id`) > 0 \
                     AND COUNT(CASE WHEN cix.`is_completed` <> 0 THEN 1 END) = COUNT(cix.`id`))",
                );
            }
            ChecklistFilter::Incomplete => {
                builder.push(
                    " AND EXISTS (SELECT 1 FROM `checklists` clx \
                     LEFT JOIN `checklist_items` cix ON cix.`checklist_id` = clx.`id` \
                     WHERE clx.`card_id` = ",
                );
                builder.push(card_alias);
                builder.push(
                    ".`id` GROUP BY clx.`card_id` \
                     HAVING COUNT(cix.`id`) = 0 \
                     OR COUNT(CASE WHEN cix.`is_completed` <> 0 THEN 1 END) < COUNT(cix.`id`))",
                );
            }
        }
    }
}

// --- garde validators for query DTOs (shared by cards/columns endpoints) ---

/// Garde validator for optional search filter.
pub fn validate_optional_search(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(raw) = value {
        let trimmed = raw.trim();
        if !trimmed.is_empty() && trimmed.chars().count() > MAX_SEARCH_LEN {
            return Err(garde::Error::new(format!(
                "length must be at most {MAX_SEARCH_LEN} characters"
            )));
        }
    }
    Ok(())
}

/// Garde validator for optional due-date filter enum.
pub fn validate_optional_due_date_filter(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(raw) = value
        && DueDateFilter::parse(raw).is_none()
    {
        return Err(garde::Error::new(format!(
            "due_date must be one of: {}",
            DUE_DATE_FILTERS.join(", ")
        )));
    }
    Ok(())
}

/// Garde validator for optional checklist filter enum.
pub fn validate_optional_checklist_filter(value: &Option<String>, _context: &()) -> garde::Result {
    if let Some(raw) = value
        && ChecklistFilter::parse(raw).is_none()
    {
        return Err(garde::Error::new(format!(
            "checklist must be one of: {}",
            CHECKLIST_FILTERS.join(", ")
        )));
    }
    Ok(())
}

/// Garde validator for repeated label ID filter values.
pub fn validate_labels_filter(value: &Vec<String>, _context: &()) -> garde::Result {
    if value.is_empty() {
        return Ok(());
    }
    let mut seen = std::collections::HashSet::new();
    let mut distinct = 0usize;
    for raw in value {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let parsed = raw
            .parse::<i32>()
            .map_err(|_| garde::Error::new("must be an integer"))?;
        if parsed < 1 {
            return Err(garde::Error::new("must be at least 1"));
        }
        if seen.insert(parsed) {
            distinct += 1;
        }
    }
    if distinct > MAX_LABEL_IDS {
        return Err(garde::Error::new(format!(
            "must have at most {MAX_LABEL_IDS} distinct values"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_like_pattern_escapes_metacharacters() {
        assert_eq!(escape_like_pattern("plain"), "%plain%");
        assert_eq!(escape_like_pattern("100%"), "%100\\%%");
        assert_eq!(escape_like_pattern("a_b"), "%a\\_b%");
        assert_eq!(escape_like_pattern("a\\b"), "%a\\\\b%");
        assert_eq!(escape_like_pattern(""), "%%");
    }

    #[test]
    fn parse_search_trims_and_rejects_long() {
        let f = parse_card_filters(Some("  hello  "), &[], None, None).unwrap();
        assert_eq!(f.search.as_deref(), Some("hello"));
        let f = parse_card_filters(Some("   "), &[], None, None).unwrap();
        assert_eq!(f.search, None);
        let long = "x".repeat(MAX_SEARCH_LEN + 1);
        assert!(parse_card_filters(Some(&long), &[], None, None).is_err());
    }

    #[test]
    fn parse_labels_dedupes_and_validates() {
        let f =
            parse_card_filters(None, &["1".into(), "2".into(), "1".into()], None, None).unwrap();
        assert_eq!(f.labels, vec![1, 2]);
        assert!(parse_card_filters(None, &["abc".into()], None, None).is_err());
        assert!(parse_card_filters(None, &["0".into()], None, None).is_err());
        let many: Vec<String> = (1..=21).map(|i| i.to_string()).collect();
        assert!(parse_card_filters(None, &many, None, None).is_err());
    }

    #[test]
    fn parse_enums() {
        assert!(parse_card_filters(None, &[], Some("bogus"), None).is_err());
        assert!(parse_card_filters(None, &[], None, Some("bogus")).is_err());
        let f = parse_card_filters(None, &[], Some("today"), Some("with")).unwrap();
        assert_eq!(f.due_date, Some(DueDateFilter::Today));
        assert_eq!(f.checklist, Some(ChecklistFilter::With));
    }

    #[test]
    fn due_date_boundaries_monday_week_window() {
        // 2026-11-25 is a Wednesday
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-25T12:34:56Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let b = due_date_boundaries_at(now);
        assert_eq!(b.start_of_today.to_string(), "2026-11-25 00:00:00");
        assert_eq!(b.start_of_tomorrow.to_string(), "2026-11-26 00:00:00");
        assert_eq!(b.start_of_next_week.to_string(), "2026-12-02 00:00:00");
    }
}
