//! notes store: free `async fn`s generic over `MySqlExecutor` (no traits).
//!
//! Ownership rides `notes.user_id` directly (a migration-only FK per
//! `30-db-schema.md` — no entity declares it): every single-note lookup carries
//! `AND n.user_id = ?`, so a foreign or missing id matches zero rows →
//! [`AppError::NotFound`] (via `RowNotFound`), never a 403 oracle. Linked
//! `board_id`/`project_id`/`card_id` targets are ownership-checked the same
//! way before the write runs (the DB FKs guard integrity, not isolation).
//!
//! At most one FK may be set (multiple → 400 [`AppError::Validation`]; none =
//! `general`): [`check_single_link`] is the single pure owner, called by both
//! the create and the update paths. Linked `tag_ids` are deduped in Rust and
//! ownership-checked by count (`tags` matching `user_id + id IN (...)` must
//! equal the unique request count — a missing OR foreign tag → 404, aligning
//! create with update per `20-api-contract.md` notes).
//!
//! List queries never `SELECT` the `content` MEDIUMTEXT (detail `GET /:id`
//! only, mirroring the cards-list `description` rule): the page runs in two
//! steps — `DISTINCT` ids under the shared filter, then enrichment — and the
//! `Page.total` rides `COUNT(DISTINCT n.id)` over the same `WHERE` (the tags
//! `LEFT JOIN` fans out, so a bare `COUNT(*)` would overcount). `LIKE`
//! patterns are built in Rust ([`escape_like_pattern`]: `\`/`%`/`_` escaped,
//! wrapped in `%…%`) and ride `push_bind` only — user text is never `push`ed —
//! with an explicit `ESCAPE` clause.
//!
//! `query!` `DATETIME` selects need NO type override: with the in-tree
//! session store (T65) the `sqlx` `time` feature is no longer enabled, so
//! `query!` maps a bare `DATETIME` to `chrono::NaiveDateTime` natively.
//! Nullable
//! selects override with the INNER type (sqlx wraps nullable columns in
//! `Option` itself). `COUNT(DISTINCT …)` decodes as `i64` (`LONGLONG`).
//!
//! One query per fetch fn (an executor value moves, so multi-select flows
//! live in the caller, which orchestrates pool/tx borrows).

use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlExecutor, QueryBuilder};

use crate::{
    common::ids::{BoardId, CardId, NoteId, ProjectId, TagId, UserId},
    error::{AppError, Internal},
};

use super::types::{NoteDetailResponse, NoteSummaryResponse, TagRef, format_timestamp};

/// One `notes` row (assembled by [`fetch_note_owned`]; tags ride separately
/// so the note select stays single-row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRow {
    /// `notes.id`.
    pub id: NoteId,
    /// Note title.
    pub title: String,
    /// `notes.content` (always set — `MEDIUMTEXT NOT NULL`; selected here and
    /// in detail only, never in lists).
    pub content: String,
    /// `notes.board_id` (`None` = unlinked).
    pub board_id: Option<BoardId>,
    /// `notes.project_id` (`None` = unlinked).
    pub project_id: Option<ProjectId>,
    /// `notes.card_id` (`None` = unlinked).
    pub card_id: Option<CardId>,
    /// `notes.user_id`.
    pub user_id: UserId,
    /// `notes.created_at`.
    pub created_at: NaiveDateTime,
    /// `notes.updated_at`.
    pub updated_at: NaiveDateTime,
}

/// At-most-one-FK rule (pure, single owner for create + update): more than one
/// set link → 400 `validation-error` with `errors[]`; none set = `general`
/// (valid). Runs before any SQL, so a violating request never opens a tx.
pub fn check_single_link(
    board_id: Option<i32>,
    project_id: Option<i32>,
    card_id: Option<i32>,
) -> Result<(), AppError> {
    let set = [board_id.is_some(), project_id.is_some(), card_id.is_some()]
        .into_iter()
        .filter(|present| *present)
        .count();
    if set > 1 {
        let mut report = garde::Report::new();
        report.append(
            garde::Path::new("board_id"),
            garde::Error::new("at most one of board_id, project_id, card_id may be set"),
        );
        return Err(AppError::from(report));
    }
    Ok(())
}

/// Escape a raw `?search=` substring for `LIKE`: `\` → `\\`, `%` → `\%`,
/// `_` → `\_` (backslash first so the introduced escapes are not re-escaped),
/// then wrapped in `%…%` for substring matching. The pattern rides `push_bind`
/// only (never `push`ed) alongside an explicit `ESCAPE` clause, so `%`/`_`/`'`
/// payloads match literally and `'` never breaks out of the string. Pure: no
/// SQL.
pub fn escape_like_pattern(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len() + 2);
    for ch in raw.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    format!("%{escaped}%")
}

/// Dedup tag ids in Rust (stable order): the `note_tags` composite PK would
/// 409 on a repeated id, so callers normalize before linking. Pure: no SQL.
pub fn dedup_tag_ids(tag_ids: &[i32]) -> Vec<i32> {
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::with_capacity(tag_ids.len());
    for id in tag_ids {
        if seen.insert(*id) {
            unique.push(*id);
        }
    }
    unique
}

/// Assert a board belongs to the caller. Foreign/missing id → `RowNotFound` →
/// 404 (no oracle, never 403). Runs inside the caller's tx so the check and
/// the write that follows are atomic.
#[tracing::instrument(skip(ex))]
pub async fn check_board_owned<'e, E>(ex: E, user_id: i32, board_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT `id` FROM `boards` WHERE `id` = ? AND `user_id` = ?",
        board_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert a project belongs to the caller (same 404 rule as
/// [`check_board_owned`]).
#[tracing::instrument(skip(ex))]
pub async fn check_project_owned<'e, E>(
    ex: E,
    user_id: i32,
    project_id: i32,
) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT `id` FROM `projects` WHERE `id` = ? AND `user_id` = ?",
        project_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert a card belongs to the caller via its grandparent board (cards carry
/// no `user_id`; same 404 rule as [`check_board_owned`]).
#[tracing::instrument(skip(ex))]
pub async fn check_card_owned<'e, E>(ex: E, user_id: i32, card_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!(
        "SELECT k.`id` FROM `cards` k \
          JOIN `columns` c ON c.`id` = k.`column_id` \
          JOIN `boards` b ON b.`id` = c.`board_id` \
          WHERE k.`id` = ? AND b.`user_id` = ?",
        card_id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Assert every requested tag exists AND belongs to the caller: the count of
/// matching rows must equal the (already deduped) request count, so a missing
/// OR foreign tag → 404 (no oracle, never 403). `COUNT(CASE…)` is unnecessary
/// here — plain `COUNT(*)` over the `IN` set. Empty input short-circuits (no
/// `IN ()` query emitted).
#[tracing::instrument(skip(ex))]
pub async fn check_tags_owned<'e, E>(ex: E, user_id: i32, tag_ids: &[i32]) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if tag_ids.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM `tags` WHERE `user_id` = ");
    builder.push_bind(user_id);
    builder.push(" AND `id` IN (");
    {
        let mut separated = builder.separated(", ");
        for id in tag_ids {
            separated.push_bind(*id);
        }
    }
    builder.push(")");
    let matched: i64 = builder
        .build_query_scalar()
        .fetch_one(ex)
        .await
        .map_err(AppError::Db)?;
    let expected = i64::try_from(tag_ids.len()).map_err(|_| AppError::Internal(Internal))?;
    if matched != expected {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Insert a note row; returns the new id (`last_insert_id` as `u64` →
/// `i32::try_from`, never a silent `as` cast per `30-db-schema.md`). A `1452`
/// FK failure (link deleted between the ownership check and the insert) maps
/// to 404, matching the check's outcome — never a 500 oracle.
#[tracing::instrument(skip(ex))]
#[allow(clippy::too_many_arguments)]
pub async fn insert_note<'e, E>(
    ex: E,
    user_id: i32,
    title: &str,
    content: &str,
    board_id: Option<i32>,
    project_id: Option<i32>,
    card_id: Option<i32>,
) -> Result<i32, AppError>
where
    E: MySqlExecutor<'e>,
{
    let result = sqlx::query!(
        "INSERT INTO `notes` (`title`, `content`, `board_id`, `project_id`, `card_id`, `user_id`) \
          VALUES (?, ?, ?, ?, ?, ?)",
        title,
        content,
        board_id,
        project_id,
        card_id,
        user_id
    )
    .execute(ex)
    .await
    .map_err(|error| match &error {
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("1452") =>
        {
            AppError::NotFound
        }
        _ => AppError::Db(error),
    })?;
    i32::try_from(result.last_insert_id()).map_err(|_| AppError::Internal(Internal))
}

/// Link (already deduped) tags to a note as ONE multi-row `INSERT` (small-N;
/// the caller dedups via [`dedup_tag_ids`] so the composite PK never 409s).
/// Empty input short-circuits (no `VALUES ()` query emitted). One query per
/// fn (the executor value moves — callers orchestrate pool/tx borrows).
#[tracing::instrument(skip(ex))]
pub async fn link_note_tags<'e, E>(ex: E, note_id: i32, tag_ids: &[i32]) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if tag_ids.is_empty() {
        return Ok(());
    }
    let mut builder: QueryBuilder<MySql> =
        QueryBuilder::new("INSERT INTO `note_tags` (`note_id`, `tag_id`) ");
    builder.push_values(tag_ids, |mut row, tag_id| {
        row.push_bind(note_id);
        row.push_bind(*tag_id);
    });
    builder.build().execute(ex).await.map_err(AppError::Db)?;
    Ok(())
}

/// Clear a note's tag set (the replace path runs this + [`link_note_tags`]
/// from the caller, one query per fn). The `note_tags` PK makes the delete
/// idempotent-safe.
#[tracing::instrument(skip(ex))]
pub async fn clear_note_tags<'e, E>(ex: E, note_id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("DELETE FROM `note_tags` WHERE `note_id` = ?", note_id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Fetch one owned note WITH its `content` (detail path only — lists never
/// select TEXT). Foreign/missing id → `RowNotFound` → 404 (no oracle).
#[tracing::instrument(skip(ex))]
pub async fn fetch_note_owned<'e, E>(ex: E, user_id: i32, id: i32) -> Result<NoteRow, AppError>
where
    E: MySqlExecutor<'e>,
{
    let row = sqlx::query!(
        "SELECT n.`id`, n.`title`, n.`content`, \
          n.`board_id` AS \"board_id: i32\", \
          n.`project_id` AS \"project_id: i32\", \
          n.`card_id` AS \"card_id: i32\", \
          n.`user_id` AS \"user_id: i32\", \
          n.`created_at`, \
          n.`updated_at` \
          FROM `notes` n WHERE n.`id` = ? AND n.`user_id` = ?",
        id,
        user_id
    )
    .fetch_one(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(NoteRow {
        id: NoteId::from(row.id),
        title: row.title,
        content: row.content,
        board_id: row.board_id.map(BoardId::from),
        project_id: row.project_id.map(ProjectId::from),
        card_id: row.card_id.map(CardId::from),
        user_id: UserId::from(row.user_id),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// One tag link for a note body.
#[derive(Debug, Clone)]
pub struct NoteTagRow {
    /// `tags.id`.
    pub tag_id: i32,
    /// Tag name.
    pub name: String,
    /// Tag color.
    pub color: String,
}

/// Tag links for one note (`tag id` order).
#[tracing::instrument(skip(ex))]
pub async fn fetch_note_tags<'e, E>(ex: E, note_id: i32) -> Result<Vec<NoteTagRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let rows = sqlx::query!(
        "SELECT t.`id` AS \"tag_id: i32\", t.`name`, t.`color` \
          FROM `note_tags` nt JOIN `tags` t ON t.`id` = nt.`tag_id` \
          WHERE nt.`note_id` = ? ORDER BY t.`id` ASC",
        note_id
    )
    .fetch_all(ex)
    .await
    .map_err(AppError::Db)?;
    Ok(rows
        .into_iter()
        .map(|row| NoteTagRow {
            tag_id: row.tag_id,
            name: row.name,
            color: row.color,
        })
        .collect())
}

/// Tag links for a page of notes: `(note_id, tag_id, name, color)`,
/// (`note_id`, `tag id` order). Empty input short-circuits.
pub type PageTagTuple = (i32, i32, String, String);

/// Tag links for the enriched ids (one query for the whole page, no dynamic
/// per-note selects). Empty input short-circuits (no `IN ()` query emitted).
#[tracing::instrument(skip(ex))]
pub async fn fetch_notes_tags<'e, E>(ex: E, note_ids: &[i32]) -> Result<Vec<PageTagTuple>, AppError>
where
    E: MySqlExecutor<'e>,
{
    if note_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
        "SELECT nt.`note_id`, t.`id`, t.`name`, t.`color` \
          FROM `note_tags` nt JOIN `tags` t ON t.`id` = nt.`tag_id` \
          WHERE nt.`note_id` IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for id in note_ids {
            separated.push_bind(*id);
        }
    }
    builder.push(") ORDER BY nt.`note_id` ASC, t.`id` ASC");
    builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)
}

/// Render a [`NoteRow`] + its tags for the wire. `content` is always `Some`
/// (the column is `MEDIUMTEXT NOT NULL`); the `Option` shape follows the
/// `20-api-contract.md` nullability table (emit-null, never omitted).
pub fn to_detail(row: NoteRow, tags: Vec<NoteTagRow>) -> NoteDetailResponse {
    NoteDetailResponse {
        id: row.id,
        title: row.title,
        content: Some(row.content),
        board_id: row.board_id,
        project_id: row.project_id,
        card_id: row.card_id,
        user_id: row.user_id,
        tags: tags
            .into_iter()
            .map(|tag| TagRef {
                id: TagId::from(tag.tag_id),
                name: tag.name,
                color: tag.color,
            })
            .collect(),
        created_at: format_timestamp(&row.created_at),
        updated_at: format_timestamp(&row.updated_at),
    }
}

/// List filters (resolved from `NoteListQuery`; pagination rides separately).
#[derive(Debug, Clone, Default)]
pub struct NoteFilter {
    /// Title substring (`None` = unfiltered; escaped in Rust, bound only).
    pub search: Option<String>,
    /// `type` filter (`general|board|project|card`, pre-validated enum).
    pub note_type: Option<String>,
    /// Tag filter (`None` = unfiltered).
    pub tag_id: Option<i32>,
    /// Board filter (`None` = unfiltered).
    pub board_id: Option<i32>,
}

/// Shared list `WHERE` builder (one owner for the count + ids selects, so the
/// two can never drift). Ownership is baked in (`n.user_id = ?`). The tags
/// `LEFT JOIN` in the `FROM` clause fans out (one row per tag), so the count
/// uses `COUNT(DISTINCT n.id)` and the ids select uses `DISTINCT` — never a
/// bare `COUNT(*)` (which would overcount multi-tag notes). `sort`/`order`
/// are fixed (`updated_at DESC, id DESC` — the `id` tiebreak keeps pagination
/// stable under equal timestamps); no user text is ever `push`ed.
fn push_note_filter(builder: &mut QueryBuilder<MySql>, user_id: i32, filter: &NoteFilter) {
    builder.push(" WHERE n.`user_id` = ");
    builder.push_bind(user_id);
    if let Some(search) = filter.search.as_deref() {
        builder.push(" AND n.`title` LIKE ");
        builder.push_bind(escape_like_pattern(search));
        builder.push(" ESCAPE '\\\\'");
    }
    match filter.note_type.as_deref() {
        Some("board") => {
            builder.push(" AND n.`board_id` IS NOT NULL");
        }
        Some("project") => {
            builder.push(" AND n.`project_id` IS NOT NULL");
        }
        Some("card") => {
            builder.push(" AND n.`card_id` IS NOT NULL");
        }
        Some("general") => {
            builder.push(
                " AND n.`board_id` IS NULL AND n.`project_id` IS NULL AND n.`card_id` IS NULL",
            );
        }
        _ => {}
    }
    if let Some(tag_id) = filter.tag_id {
        builder.push(" AND nt.`tag_id` = ");
        builder.push_bind(tag_id);
    }
    if let Some(board_id) = filter.board_id {
        builder.push(" AND n.`board_id` = ");
        builder.push_bind(board_id);
    }
}

/// Shared list `FROM` (one owner — the count and the ids select build from the
/// same fragments, so the tag join can never appear in one but not the other).
const fn note_list_from() -> &'static str {
    "FROM `notes` n LEFT JOIN `note_tags` nt ON nt.`note_id` = n.`id`"
}

/// `SELECT COUNT(DISTINCT n.id)` for the list `Page.total` (same `WHERE` as
/// the ids select — the `DISTINCT` absorbs the tags-join fanout; `COUNT(*)`
/// would overcount multi-tag notes). Decodes `LONGLONG` → `i64` → `u64` via
/// `try_from` (never a silent `as` cast).
#[tracing::instrument(skip(ex))]
pub async fn count_notes<'e, E>(ex: E, user_id: i32, filter: &NoteFilter) -> Result<u64, AppError>
where
    E: MySqlExecutor<'e>,
{
    let mut builder = QueryBuilder::new("SELECT COUNT(DISTINCT n.`id`) ");
    builder.push(note_list_from());
    push_note_filter(&mut builder, user_id, filter);
    let total: i64 = builder
        .build_query_scalar()
        .fetch_one(ex)
        .await
        .map_err(AppError::Db)?;
    u64::try_from(total).map_err(|_| AppError::Internal(Internal))
}

/// First list step: `DISTINCT` note ids under the shared filter,
/// `updated_at DESC, id DESC`, windowed by `LIMIT`/`OFFSET`. The `updated_at`
/// rides along so `DISTINCT` keeps the ordering column in the select list
/// (MySQL `DISTINCT` + `ORDER BY` discipline); callers keep the returned order
/// (it IS the page order) and enrich by id.
#[tracing::instrument(skip(ex))]
pub async fn fetch_note_ids<'e, E>(
    ex: E,
    user_id: i32,
    filter: &NoteFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<i32>, AppError>
where
    E: MySqlExecutor<'e>,
{
    let mut builder: QueryBuilder<MySql> =
        QueryBuilder::new("SELECT DISTINCT n.`id`, n.`updated_at` ");
    builder.push(note_list_from());
    push_note_filter(&mut builder, user_id, filter);
    builder.push(" ORDER BY n.`updated_at` DESC, n.`id` DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    // `QueryBuilder` (not `query!`): ids + filter bind dynamically. `DATETIME`
    // decodes straight into `chrono` here (the `time`-crate mapping quirk only
    // affects the `query!` macros).
    let rows: Vec<(i32, NaiveDateTime)> = builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(rows.into_iter().map(|(id, _)| id).collect())
}

/// One TEXT-free page row (decodes the [`fetch_note_summaries`] select).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotePageRow {
    /// `notes.id`.
    id: i32,
    /// `notes.title`.
    title: String,
    /// `notes.board_id` (`None` = unlinked).
    board_id: Option<i32>,
    /// `notes.project_id` (`None` = unlinked).
    project_id: Option<i32>,
    /// `notes.card_id` (`None` = unlinked).
    card_id: Option<i32>,
    /// `notes.user_id`.
    user_id: i32,
    /// `notes.created_at`.
    created_at: NaiveDateTime,
    /// `notes.updated_at`.
    updated_at: NaiveDateTime,
}

/// Second list step: TEXT-free summaries for the enriched ids (explicit column
/// list — `content` is never selected here). The `IN` order is unspecified, so
/// the caller restores page order in Rust ([`to_summaries`]). Empty input
/// short-circuits (no `IN ()` query emitted).
#[tracing::instrument(skip(ex))]
pub async fn fetch_note_summaries<'e, E>(
    ex: E,
    note_ids: &[i32],
) -> Result<Vec<NotePageRow>, AppError>
where
    E: MySqlExecutor<'e>,
{
    if note_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
        "SELECT n.`id`, n.`title`, n.`board_id`, n.`project_id`, n.`card_id`, \
          n.`user_id`, n.`created_at`, n.`updated_at` \
          FROM `notes` n WHERE n.`id` IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for id in note_ids {
            separated.push_bind(*id);
        }
    }
    builder.push(")");
    builder
        .build_query_as()
        .fetch_all(ex)
        .await
        .map_err(AppError::Db)
}

/// Assemble the enriched rows + tag links into wire summaries, restoring the
/// ids-step page order (`IN` returns rows unordered). Pure: no SQL.
pub fn to_summaries(
    ordered_ids: &[i32],
    rows: Vec<NotePageRow>,
    tags: Vec<PageTagTuple>,
) -> Vec<NoteSummaryResponse> {
    use std::collections::HashMap;
    let mut rows_by_id: HashMap<i32, NotePageRow> = HashMap::new();
    for row in rows {
        rows_by_id.insert(row.id, row);
    }
    let mut tags_by_note: HashMap<i32, Vec<TagRef>> = HashMap::new();
    for (note_id, tag_id, name, color) in tags {
        tags_by_note.entry(note_id).or_default().push(TagRef {
            id: TagId::from(tag_id),
            name,
            color,
        });
    }
    let mut summaries = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        if let Some(row) = rows_by_id.remove(id) {
            summaries.push(NoteSummaryResponse {
                id: NoteId::from(row.id),
                title: row.title,
                board_id: row.board_id.map(BoardId::from),
                project_id: row.project_id.map(ProjectId::from),
                card_id: row.card_id.map(CardId::from),
                user_id: UserId::from(row.user_id),
                tags: tags_by_note.remove(&row.id).unwrap_or_default(),
                created_at: format_timestamp(&row.created_at),
                updated_at: format_timestamp(&row.updated_at),
            });
        }
    }
    summaries
}

/// Partial note update (T27a `PATCH`): every scalar optional. FKs mirror the
/// DTO double `Option` — `None` leaves the link untouched, `Some(None)`
/// detaches (`NULL`), `Some(Some(..))` relinks (the caller ownership-checks
/// via `check_*_owned` first, so a foreign id never reaches the write). An
/// all-`None` patch is a no-op (returns without emitting `UPDATE`, so empty
/// `{}` bodies stay 200 without touching the row).
#[derive(Debug, Clone, Copy, Default)]
pub struct NotePatch<'a> {
    /// New `notes.title` (`None` = untouched).
    pub title: Option<&'a str>,
    /// New `notes.content` (`None` = untouched).
    pub content: Option<&'a str>,
    /// Board link change (`None` = untouched).
    pub board_id: Option<Option<i32>>,
    /// Project link change (`None` = untouched).
    pub project_id: Option<Option<i32>>,
    /// Card link change (`None` = untouched).
    pub card_id: Option<Option<i32>>,
}

impl NotePatch<'_> {
    /// Whether every field is untouched (no `UPDATE` to emit).
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.content.is_none()
            && self.board_id.is_none()
            && self.project_id.is_none()
            && self.card_id.is_none()
    }
}

/// Apply a [`NotePatch`] to a note row. The caller pre-reads via
/// [`fetch_note_owned`] for the 404 instead of trusting affected-rows (which
/// would misreport an idempotent no-change write as missing). `updated_at`
/// bumps via the column's `ON UPDATE CURRENT_TIMESTAMP(6)`.
#[tracing::instrument(skip(ex))]
pub async fn apply_note_patch<'e, E>(ex: E, id: i32, patch: NotePatch<'_>) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    if patch.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::new("UPDATE `notes` SET ");
    {
        // `Separated` inserts `", "` between EVERY push — including between a
        // `` `col` = `` prefix and its placeholder — so each value rides
        // `push_bind_unseparated` (the `push` prefix already consumed the
        // separator slot). Plain `push_bind` here emits `` `col` = , ? ``
        // (1064 at runtime; see T22 probe).
        let mut set = builder.separated(", ");
        if let Some(title) = patch.title {
            set.push("`title` = ");
            set.push_bind_unseparated(title);
        }
        if let Some(content) = patch.content {
            set.push("`content` = ");
            set.push_bind_unseparated(content);
        }
        if let Some(board_id) = patch.board_id {
            set.push("`board_id` = ");
            set.push_bind_unseparated(board_id);
        }
        if let Some(project_id) = patch.project_id {
            set.push("`project_id` = ");
            set.push_bind_unseparated(project_id);
        }
        if let Some(card_id) = patch.card_id {
            set.push("`card_id` = ");
            set.push_bind_unseparated(card_id);
        }
    }
    builder.push(" WHERE `id` = ");
    builder.push_bind(id);
    builder.build().execute(ex).await.map_err(AppError::Db)?;
    Ok(())
}

/// Hard-delete an owned note (`WHERE id` — the caller pre-reads via
/// [`fetch_note_owned`] for ownership + the 404). Tag links fan out via DB
/// `ON DELETE CASCADE` FKs — no manual cleanup loop.
#[tracing::instrument(skip(ex))]
pub async fn delete_note_row<'e, E>(ex: E, id: i32) -> Result<(), AppError>
where
    E: MySqlExecutor<'e>,
{
    sqlx::query!("DELETE FROM `notes` WHERE `id` = ?", id)
        .execute(ex)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_pattern_escapes_wildcards_and_backslash() {
        assert_eq!(escape_like_pattern("plain"), "%plain%");
        assert_eq!(escape_like_pattern("100%"), "%100\\%%");
        assert_eq!(escape_like_pattern("a_b"), "%a\\_b%");
        assert_eq!(escape_like_pattern("a\\b"), "%a\\\\b%");
        assert_eq!(escape_like_pattern("O'Brien"), "%O'Brien%");
        assert_eq!(escape_like_pattern(""), "%%");
    }

    #[test]
    fn single_link_allows_zero_or_one() {
        assert!(check_single_link(None, None, None).is_ok());
        assert!(check_single_link(Some(1), None, None).is_ok());
        assert!(check_single_link(None, Some(1), None).is_ok());
        assert!(check_single_link(None, None, Some(1)).is_ok());
    }

    #[test]
    fn single_link_rejects_multiples_as_validation_400() {
        for (board, project, card) in [
            (Some(1), Some(2), None),
            (Some(1), None, Some(3)),
            (None, Some(2), Some(3)),
            (Some(1), Some(2), Some(3)),
        ] {
            assert!(
                check_single_link(board, project, card).is_err(),
                "must reject {board:?}/{project:?}/{card:?}"
            );
        }
    }

    #[test]
    fn tag_ids_dedup_keeps_first_order() {
        assert_eq!(dedup_tag_ids(&[3, 1, 3, 2, 1]), vec![3, 1, 2]);
        assert!(dedup_tag_ids(&[]).is_empty());
    }
}
