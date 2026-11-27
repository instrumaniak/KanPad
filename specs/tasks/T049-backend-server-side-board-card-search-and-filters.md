# T049: Backend: server-side board card search and filters

## Goal

Move board card search/filter to server-authoritative query params, applied consistently to board column embed, column card hydration, and the board-scoped paginated cards list endpoint (`GET /api/v1/boards/{boardId}/cards`).

## Depends On

- T022
- T048 (**hard prerequisite**: `embed_limit` and `GET /api/v1/boards/{boardId}/cards` must exist or be landed in the same change series before T049 can satisfy all acceptance criteria. Do not mark T049 complete while the board cards list endpoint is missing.)

## Phase

1

## Critical

No

## Spec References

- `specs/tasks/T022-pagination-u-x-page-ceilings.md` (amendment: server-side search/filter authoritative)
- `specs/tasks/T048-backend-parameterized-embed-limit-board-cards-list-endpoint.md` (board-scoped paginated cards list endpoint, `embed_limit`)
- `backend-rs/src/features/notes/store.rs` (`escape_like_pattern` precedent)
- `backend-rs/src/common/card_summary.rs` (board embed/totals/windowed query)
- `backend-rs/src/features/cards/{routes.rs,store.rs}` (column cards page/count)
- `backend-rs/src/extract.rs` (`PaginationParams`, validation error shape)
- AGENTS: MySQL 8.4 + MariaDB 11.4, additive contract same-PR, TDD, no raw SQL interpolation

## Locked Contract Decisions (Normative)

These decisions are implementation requirements. Do not reinterpret during coding; amend this spec first if a change is required.

### 1. Query params

All three read paths accept the same filter params:

| Param | Type | Validation / parsing | Semantics |
|---|---|---|---|
| `search` | optional string | Trim whitespace. Empty/whitespace after trim = no filter. Min 1 char after trim when present. Max 255 chars after trim. | Case-insensitive title search. Escape `%`, `_`, and `\` before binding. Use a shared LIKE-escape helper (reuse/move `notes::store::escape_like_pattern` or equivalent common helper; do not duplicate logic). Apply as `title LIKE ? ESCAPE '\\'` pattern (or equivalent bound pattern) under project collation. |
| `labels` | optional repeated `i32` query param | Repeated form only: `?labels=1&labels=2`. Each value must parse as `i32 > 0`. Dedupe IDs before applying. Max 20 distinct IDs. Empty/absent = no filter. Invalid integer/`<=0`/too many = 400 validation-error. | **AND** semantics: a card matches only if it has every selected label. Label IDs must be owned by the authenticated user and resolvable in scope; foreign/nonexistent/unauthorized IDs are `400 validation-error` with `errors[]` (no existence oracle, never 403). |
| `due_date` | optional enum string | Allowed values: `overdue`, `today`, `this_week`, `no_date`. Unknown/invalid = 400 validation-error. | Evaluated against **UTC**. `overdue`: `due_date IS NOT NULL AND due_date < start_of_today_utc`. `today`: `due_date IS NOT NULL AND due_date >= start_of_today_utc AND due_date < start_of_tomorrow_utc`. `this_week`: `due_date IS NOT NULL AND due_date >= start_of_today_utc AND due_date < start_of_next_week_utc`, where the window is a rolling 7 days in UTC: `start_of_next_week_utc = start_of_today_utc + 7 days` (upper bound exclusive; NOT next Monday / NOT a calendar Monday-start week). `no_date`: `due_date IS NULL`. Comparisons use stored `DATETIME` value as UTC timestamp. |
| `checklist` | optional enum string | Allowed values: `with`, `complete`, `incomplete`. Unknown/invalid = 400 validation-error. | `with`: card has >= 1 checklist. `complete`: card has >= 1 checklist AND total checklist items > 0 AND completed_items == total_items across the card's checklists. `incomplete`: card has >= 1 checklist AND (`total_items == 0 OR completed_items < total_items`). Cards with no checklists match only when `checklist` filter is absent. |

### 2. Filter composition and totals

- Different filter types compose with **AND**.
- `Page.total` for paginated card list endpoints is the **filtered authoritative** total.
- `ColumnResponse.total_cards` remains the **unfiltered authoritative** total for the column (existing meaning preserved).
- `ColumnResponse.matching_cards` is added as an additive `u64` field: **filtered authoritative** count of cards matching active filters in that column; equals `total_cards` when no filter is active.
- Column embed returns first `embed_limit` **matching** cards by stable order; `cards.len()` may be less than `matching_cards`.
- Do not overload existing field meanings. Any new fields are additive and same-PR OpenAPI/schema/typegen updated.

### 3. Affected read paths

Filters apply identically to:

1. `GET /api/v1/boards/{boardId}/columns?embed_limit=&search=&labels=&due_date=&checklist=` (embed top N matching cards per column; `total_cards` unfiltered, `matching_cards` filtered)
2. `GET /api/v1/columns/{colId}/cards?page=&limit=&search=&labels=&due_date=&checklist=` (`Page<CardSummary>.total` filtered)
3. `GET /api/v1/boards/{boardId}/cards?page=&limit=&sort=&order=&search=&labels=&due_date=&checklist=` (board-scoped paginated cards list, `Page.total` filtered)

### 4. Ownership / no-oracle / errors

- Missing/foreign board/column returns 404 no-oracle as today.
- Invalid filter shape/value/enum/range returns 400 problem+json `validation-error` with `errors[]`.
- Filter evaluation must be scoped to the authenticated owner’s resources; no foreign label/card/board existence leaks.
- `PaginationParams` window ceiling (`u64(page)*u64(limit) <= 10_000`) continues to apply.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/common/card_summary.rs` or `backend-rs/src/common/card_filters.rs` (new, if cleaner) | Shared filter input/type, validation helpers, LIKE escaping reuse, and bound SQL fragment/builder for MySQL 8.4 + MariaDB 11.4. No raw SQL interpolation. |
| `backend-rs/src/features/cards/{routes.rs,store.rs,types.rs}` | Add filter query DTO/extractor and apply filters to `GET /columns/{colId}/cards` count + page query. Preserve `sort_key ASC, id ASC`, offset position calculation, and `Page.total` filtered semantics. |
| `backend-rs/src/features/columns/{routes.rs,store.rs,types.rs}` | Add same filter params to `GET /boards/{boardId}/columns`; add `matching_cards` to `ColumnResponse`; apply filters before windowed embed; keep `total_cards` unfiltered. |
| `backend-rs/src/features/boards/{routes.rs,store.rs,types.rs}` or the board cards list owner (per T048) | Add same filter params to `GET /api/v1/boards/{boardId}/cards`; filtered `Page.total`; preserve sort/id tiebreaker. |
| `backend-rs/src/features/notes/store.rs` or common search helper | Reuse/move LIKE escaping helper without behavior change; update imports/tests as needed. |
| `backend-rs/src/docs.rs`, `backend-rs/openapi.json`, `frontend/src/lib/api/schema.d.ts` | Add query params/schemas/path params; regenerate contract in same PR; `gen:api -- --check`. |
| Backend tests | Filter semantics, parity, validation, ownership/no-oracle, pagination edges, MySQL 8.4 and MariaDB 11.4. |

## Implementation Steps

1. **Preflight (spec/contract check):** confirm T048 endpoint/`embed_limit` contract is present. **Query extraction (locked guidance):** prefer a **single validated query DTO** (via `ValidQuery<...>`) that combines pagination fields (`page`,`limit`) and filter fields (`search`,`labels` repeated/parsed, `due_date`,`checklist`) plus shared window ceiling checks, instead of mixing `PaginationParams` extractor with separate filter extractors (to avoid Axum extractor composition friction). If you must split, document the exact pattern in the PR; default to the single-DTO approach.
2. **TDD red - validation/parsing:** tests for `search` trim/empty/max/escaping, `labels` repeated parsing/dedupe/max/invalid/foreign, `due_date` enum, `checklist` enum, invalid values -> 400 validation-error `errors[]`.
3. **TDD red - semantics/parity:** failing tests for each filter, combinations, labels AND, due boundaries (UTC), checklist states including zero-item/multi-checklist, filtered totals, and parity across all three endpoints.
4. **Shared filter core:** introduce shared filter type and bound-parameter SQL builder/helpers. Centralize LIKE escaping reuse. Ensure board/column scoping and ownership constraints are preserved. Avoid N+1. **SQL planning (required):** for labels AND use a deterministic approach compatible with MySQL 8.4/MariaDB 11.4 (e.g. correlate with `card_labels` grouped by `card_id` having `COUNT(DISTINCT label_id)=N` for the requested set, within owner/board scope) and validate behavior; for checklist semantics, compute rollups via grouped aggregation across all checklists for a card (multi-checklist totals summed). Do not guess—note the chosen strategy in code comments/PR notes if non-obvious.
5. **Column cards page:** extend `count_cards_in_column`/`fetch_card_page` (or add filtered variants) to apply identical filters; keep stable ordering and `offset + index` positions over returned page.
6. **Column embed:** extend `fetch_board_cards_limited` and totals logic. Filters apply before `ROW_NUMBER()` window selection; `matching_cards` is filtered count, `total_cards` remains unfiltered count. Keep MySQL 8.4/MariaDB 11.4 compatible window SQL.
7. **Board cards list endpoint (`GET /api/v1/boards/{boardId}/cards`):** extend the T048 implementation to apply the same filters; keep deterministic sort + id tiebreaker and ownership/no-oracle.
8. **OpenAPI/docs/typegen:** add `ToSchema`/`IntoParams` query DTOs, utoipa `params(...)`, docs registrations/page aliases as needed; regenerate `openapi.json` and `frontend/src/lib/api/schema.d.ts`.
9. **Gates/evidence:** fmt, clippy, relevant backend tests, MySQL 8.4 and MariaDB 11.4 tmpfs runs, `cargo sqlx prepare --check` if SQL metadata/checks require it, and `gen:api -- --check`.

## Constraints

- Server-side filter is authoritative; client-side filtering is not a correctness substitute.
- Additive contract only; same-PR OpenAPI/schema/typegen update.
- No raw SQL interpolation for user input; all values bound. LIKE metacharacters escaped.
- No existence oracle: ownership failures remain 404 where applicable; foreign/unauthorized filter IDs are validation errors as specified.
- Deterministic ordering preserved: column cards/embed `sort_key ASC, id ASC`; the board cards list keeps its documented sort + id tiebreaker.
- MySQL 8.4 + MariaDB 11.4 compatible SQL only.
- DoS guardrails: `search` max 255 chars, max 20 label IDs, existing pagination window ceiling enforced.
- Preserve existing field meanings; add additive fields for filtered/unfiltered clarity.
- **Labels AND query strategy:** implement via a clear, deterministic subquery/grouped approach (no N+1). Validate with tests including multiple labels and edge cases.
- **Checklist aggregation:** roll up across all checklists for a card; `complete` requires `total_items > 0` and `completed_items == total_items`; `incomplete` requires presence of checklists and (`total_items == 0 OR completed_items < total_items`); cards with no checklists never match `with|complete|incomplete`.

## Acceptance Criteria

- [ ] `search`, `labels`, `due_date`, and `checklist` are accepted and applied consistently on `GET /boards/{boardId}/columns`, `GET /columns/{colId}/cards`, and `GET /api/v1/boards/{boardId}/cards`.
- [ ] `search` trims input, treats empty/whitespace as no filter, enforces max length, escapes `%`, `_`, `\`, and performs case-insensitive title matching.
- [ ] `labels` uses repeated-param AND semantics, dedupes IDs, enforces max/count/value validation, and rejects foreign/nonexistent/unauthorized label IDs with 400 validation-error `errors[]` (no oracle).
- [ ] `due_date` and `checklist` enums match the locked UTC/boundary/state semantics, including zero-item and multi-checklist cases.
- [ ] Paginated endpoints return filtered authoritative `Page.total`; column embed keeps `total_cards` unfiltered authoritative and adds additive `matching_cards` filtered authoritative. Embedded cards are first N matching cards.
- [ ] Invalid filter values/shapes/enums return 400 problem+json `validation-error` with `errors[]`; pagination window ceiling still applies; page beyond filtered total returns empty `data` with correct filtered total.
- [ ] Equivalent filters produce consistent matching semantics across all three endpoints (parity tests).
- [ ] OpenAPI/docs/schema regenerated in same PR; `gen:api -- --check` passes.
- [ ] Backend tests pass on MySQL 8.4 and MariaDB 11.4 tmpfs containers.

## Test Plan

- **Validation/parsing:** missing/empty/whitespace `search`; max length; invalid enum values; `labels` non-int, <=0, duplicates, >20, repeated params; unknown query shape errors mapped consistently.
- **Semantics:** case-insensitive search; LIKE metacharacters (`% _ \ ' "`); labels AND across multiple labels; due_date UTC boundaries for overdue/today/this_week/no_date; checklist with none/zero-item/complete/incomplete/multiple checklists.
- **Parity:** same filter set on columns embed, column cards page, and board-cards endpoint yields consistent IDs/counts/totals.
- **Pagination/totals:** filtered total < page window returns empty page; `page*limit` ceiling still 400; `matching_cards` vs `total_cards` behavior with/without filters; `embed_limit` + filters.
- **Security/ownership:** foreign label ID rejected without oracle; missing/foreign board/column still 404; no SQL interpolation; bound params only.
- **Portability/regression:** run on MySQL 8.4 and MariaDB 11.4 tmpfs containers; existing unfiltered behavior remains unchanged when no filter params are provided.

## Notes

- T048 defines the board-scoped paginated cards list endpoint (`GET /api/v1/boards/{boardId}/cards`) and `embed_limit`; T049 extends both with filters. Keep param naming collision-safe (`embed_limit` remains distinct from T044 persisted `card_limit`).
- T050 handles DnD behavior under filtered views. Backend must not assume filtered-relative positions are safe for mutation.
- If implementation discovers a contract conflict (e.g. extractor composition or existing client expectations for `total_cards`), amend this spec before coding further.
- Prefer a small shared filter module over duplicating WHERE logic in cards/columns/boards stores. Reuse existing `Page<T>`, `PaginationParams`, `ValidQuery`, and garde validation patterns.
