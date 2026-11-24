# T048: Backend: parameterized embed limit + board cards list endpoint

## Goal

Add backend pagination primitives needed by the T022 amendment: parameterized column card embed limit and a board-scoped paginated cards list endpoint (`GET /api/v1/boards/{boardId}/cards`) for list view.

1. **Parameterized embed limit.** `GET /api/v1/boards/{boardId}/columns` accepts `embed_limit` query param (integer, default `10`, min `1`, max `100`) controlling how many cards are embedded per column. Missing param uses default `10`. Invalid values return 400 `validation-error` problem+json with `errors[]` containing an `embed_limit` field error. Keep additive `total_cards` authoritative (total cards in column for current scope, not limited by `embed_limit`).
2. **Board-scoped paginated cards list (`GET /api/v1/boards/{boardId}/cards`).** Add this endpoint returning a paginated, server-sorted board card list for list view. Default `limit=20`, max `limit=100`, supports `page,limit,sort,order`, deterministic `id` tiebreaker, ownership 404 no-oracle, shared pagination window ceiling. Response includes list-view fields plus `column_name`.

## Depends On

T022

## Phase

1

## Critical

No

## Spec References

- `specs/tasks/T022-pagination-u-x-page-ceilings.md` (amendment split to T048-T051; staged N=50 embed + `total_cards`; page ceiling/window semantics)
- `specs/tasks/T044-board-column-height-containment-visual.md` (naming collision: do **not** reuse persisted `card_limit`)
- `specs/tasks/T049-backend-server-side-board-card-search-and-filters.md` (follow-on filter/search extension point)
- `specs/tasks/T050-frontend-board-infinite-scroll-and-server-driven-filters.md` (frontend consumer; coordinates 10/20 targets)
- `backend-rs/src/common/pagination.rs`, `backend-rs/src/extract.rs` (`PaginationParams`, `MAX_LIMIT`, `MAX_PAGE_WINDOW`)
- `backend-rs/src/common/card_summary.rs` (`fetch_board_cards_limited`, `fetch_board_card_totals`, `CardSummary`)
- `backend-rs/src/features/columns/routes.rs`, `backend-rs/src/features/cards/routes.rs`
- `backend-rs/src/docs.rs`, `frontend/package.json` (`gen:api -- --check`)
- AGENTS: MySQL 8.4 + MariaDB 11.4, additive contract same-PR, TDD, no DB-persistence-after-mutation assertions for read-only GET endpoints.

## Behavior Change / Rollout

- **Intentional behavior change:** T022 staged columns embed default is `50` (`EMBED_CARD_LIMIT=50`). T048 changes the default embedded cards per column to `10` when `embed_limit` is omitted. This reduces initial payload and aligns with T050's 10/20 infinite-scroll targets.
- **Coordination:** land with frontend awareness. If T050 is not in the same release, either coordinate release timing or temporarily keep backend default compatible until frontend explicitly requests `embed_limit=10`; if deviating from default-10, update this task/spec and T050 before implementation. Do not silently change default without noting impact on existing board hydration, load-more math, and E2E fixtures.
- **Additive wire shape:** adding `embed_limit` query param and the board-scoped paginated cards list endpoint is additive; default-value change is behavioral, not a wire-schema break.

## API Contract

### A) Columns embed limit

- Endpoint: `GET /api/v1/boards/{boardId}/columns`
- New query param: `embed_limit`
  - Type: integer
  - Default: `10` when omitted
  - Min: `1`
  - Max: `100` (named constant; justify as payload/DB bound - max 100 cards * N columns * summary payload)
  - Invalid: non-integer, empty, `<1`, `>100`, overflow/unsupported format => 400 `validation-error` problem+json with `errors[]` including field `embed_limit`
- Validation should use a typed query DTO / `ValidQuery`-compatible pattern so error shape matches existing validation failures. Unknown query params remain ignored per serde default unless existing extractor behavior differs.
- Response shape unchanged except behavior: `ColumnResponse.cards` contains first `embed_limit` cards by existing stable card order (`sort_key ASC, id ASC` per column); `total_cards` remains authoritative total for the column scope.

### B) Board-scoped paginated cards list (`GET /api/v1/boards/{boardId}/cards`)

- Endpoint: `GET /api/v1/boards/{boardId}/cards`
- Query params:
  - `page`: integer, default `1`, min `1`
  - `limit`: integer, default `20`, min `1`, max `100` (`PaginationParams::MAX_LIMIT`)
  - `sort`: enum, default `updated_at`; allowed `created_at | updated_at | due_date | title`
  - `order`: enum, default `desc`; allowed `asc | desc`
- Validation: invalid page/limit/sort/order => 400 `validation-error` with `errors[]`. Type-parse failures should follow existing query/path validation conventions; final shape must be problem+json with `errors[]` for validation failures.
- Window ceiling: enforce same overflow-safe rule as `PaginationParams`: `u64(page) * u64(limit) <= MAX_PAGE_WINDOW` (10_000), else 400 validation-error on `page` ("outside the allowed window") or equivalent existing field/message convention.
- Ownership: missing/foreign board => 404, no-oracle (never 403/data leak).
- Ordering: whitelist sort keys to static SQL fragments; never interpolate user input. Deterministic `id` tiebreaker in same direction as primary sort.
  - `created_at`, `updated_at`, `title`: `ORDER BY <field> <dir>, k.id <dir>`
  - `due_date`: define explicit NULL ordering and test on MySQL 8.4 + MariaDB 11.4. Recommended: `due_date IS NULL` handling so NULLs sort last for `asc` and last for `desc` unless product specifies otherwise; document chosen behavior in OpenAPI description and tests. Do not leave NULL ordering implicit.
- Pagination: offset-based, stable across same filter/sort snapshot; clients dedupe by id as needed.

### C) Board cards list response DTO

- Return `Page<BoardCardListItem>` (new DTO; do not mutate shared `CardSummary` unless impact is fully audited and explicitly chosen).
- `BoardCardListItem` fields (minimum): `id`, `title`, `has_description`, `column_id`, `column_name`, `position`, `due_date`, `created_at`, `updated_at`, `labels`, `checklist_progress?`.
  - `column_name`: from joined `columns.name`.
  - `position`: keep semantics explicit; recommended same per-column computed position semantics as existing card summaries (0-based in column sort-key order), not board-page index. If changed, document and update consumers/tests.
  - No `description` TEXT selected.
- Register new schema and `Page<BoardCardListItem>` alias in `backend-rs/src/docs.rs`; add utoipa path/params/responses.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/features/columns/{routes.rs,store.rs,types.rs}` | Add typed `embed_limit` query param, validate/bound/default, plumb into `load_summaries`/`fetch_board_cards_limited`; update utoipa params/response schema. |
| `backend-rs/src/common/card_summary.rs` | Reuse parameterized limited/totals helpers; add/adjust shared helpers only if needed for the board cards list without duplicating TEXT-free summary selection. |
| `backend-rs/src/features/cards/{routes.rs,store.rs,types.rs}` (recommended owner) | Add `GET /api/v1/boards/{boardId}/cards` handler, query DTO, board-card list DTO, store count/page queries with board ownership, static sort mapping, column join. Alternative owner `boards` is allowed only if ownership/DTO boundaries are kept clear; pick one before implementation. |
| `backend-rs/src/docs.rs` | Register new path, query/response schemas, and `Page<BoardCardListItem>` alias; ensure columns list has documented query/response where missing. |
| `backend-rs/openapi.json`, `frontend/src/lib/api/schema.d.ts` | Regenerate additive contract (`gen:api` / `gen:api -- --check`). |
| Backend tests (`backend-rs/tests/api/...`) | Add coverage for embed_limit validation/default/bounds/total_cards authority, board cards list pagination/sort/tiebreaker/ownership/404/400/window ceiling, MySQL 8.4 + MariaDB 11.4. |

## Implementation Steps

1. **Owner decision (locked):** route owner for `GET /api/v1/boards/{boardId}/cards` is **`backend-rs/src/features/cards/{routes.rs,store.rs,types.rs}`** (recommended). Do **not** split ownership across `boards/*`; document this choice in the PR/task notes. Avoid split ownership.
2. **TDD red - embed limit:** tests for missing default, `1`, `100`, `0`, `101`, non-integer/empty/invalid, and 400 validation-error `errors[]` with `embed_limit`. Add test proving `total_cards` remains authoritative when embedded cards are limited.
3. **TDD red - board cards list:** tests for default `page=1,limit=20,sort=updated_at,order=desc`; allowed sorts/orders; deterministic id tiebreaker; `column_name`; ownership 404 no-oracle; invalid sort/order/page/limit; window ceiling; empty page shape.
4. **Columns embed param:** add typed query DTO (e.g. `ColumnListQuery { embed_limit }`) with named default/max constants. Parse/validate; pass to `load_summaries` -> `fetch_board_cards_limited`; keep `total_cards` from `fetch_board_card_totals`.
5. **Board cards list query/DTO:** add query DTO with validated allowlists/defaults and shared window ceiling check. Add `BoardCardListItem` DTO and `Page<BoardCardListItem>` response. Keep `CardSummary` unchanged unless explicitly justified.
6. **Board cards list store/routes:** prove board ownership via `check_board_owned`. Use shared count+page WHERE/filter builder shape so T049 can extend filters/search without rewriting pagination/sort validation. Select column name via join; batch labels/progress for page IDs; no TEXT selection. Map sort allowlist to static SQL fragments with same-direction `id` tiebreaker and **locked `due_date` NULL ordering** (see Constraints).
7. **Contract/docs:** add utoipa `params`/`responses` for columns list and the new board cards list endpoint; register schemas/page alias in `docs.rs`; regenerate OpenAPI/schema; run `gen:api -- --check`.
8. **Gates:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic`, relevant backend tests, both DB engines (MySQL 8.4, MariaDB 11.4 tmpfs), `cargo sqlx prepare --check` if SQL/cache affected.

## Constraints

- Additive contract only; same-PR OpenAPI/schema/typegen update.
- No `card_limit` query param name; avoid T044 collision.
- No silent clamping for invalid pagination/embed params; use strict 400 validation-error `errors[]`.
- Shared overflow-safe window ceiling applies to the board-scoped paginated cards list endpoint; reuse `MAX_PAGE_WINDOW` semantics.
- Deterministic ordering with id tiebreaker for all sorts; no dynamic ORDER BY interpolation.
- Ownership checks: missing/foreign board returns 404, never data leak.
- MySQL 8.4 + MariaDB 11.4 compatible SQL only.
- GET endpoints: no DB-persistence-after-mutation assertion required; verify read results via API/DB setup as appropriate.
- **Locked (board cards list):** `due_date` NULL ordering is **`due_date IS NULL` last in both `asc` and `desc`** (e.g. `ORDER BY (due_date IS NULL) ASC, due_date <dir>, id <dir>`), implemented deterministically and covered by tests on both engines; document this policy in OpenAPI/route docs and tests.
- **Locked (board cards list):** `BoardCardListItem.position` is the **per-column computed position in stable column sort-key order (0-based), matching existing column-embed `CardSummary.position` semantics** (not a board-page offset). Do not leave ambiguous.

## Acceptance Criteria

- [ ] `GET /boards/{boardId}/columns?embed_limit=n` defaults to `10` when omitted, accepts `1..=100`, rejects invalid values with 400 `validation-error` `errors[]` (`embed_limit` field), and keeps `total_cards` authoritative.
- [ ] `GET /api/v1/boards/{boardId}/cards` returns `Page<BoardCardListItem>` with `page,limit,total`, default `page=1,limit=20,sort=updated_at,order=desc`, allowed sort/order validation, deterministic same-direction `id` tiebreaker, **locked `due_date` NULLs-last ordering in both directions**, and fields including `column_name`. `position` follows the locked per-column semantics above.
- [ ] The board-scoped paginated cards list endpoint enforces ownership/no-oracle 404 and shared `MAX_PAGE_WINDOW` ceiling using overflow-safe math.
- [ ] OpenAPI/docs/schema are regenerated and include new params/path/schemas/page alias; `gen:api -- --check` passes.
- [ ] Backend tests cover embed limit, board cards list pagination/sort/tiebreaker/ownership/400/404/window ceiling (including `due_date` NULL-last behavior), and pass on MySQL 8.4 and MariaDB 11.4 tmpfs.

## Notes

- **T049 extension point:** filters/search params for the board cards list and column endpoints land in T049. Design query DTOs and store count/page query builders so filter params extend cleanly; `total_cards`/`Page.total` semantics become filtered-authoritative in T049.
- **DTO caution:** `CardSummary` is already used by column embed and `GET /columns/{colId}/cards`; adding `column_name` there has contract ripple. Prefer a dedicated board list DTO.
- **Position semantics (locked):** `BoardCardListItem.position` is per-column (0-based, stable column order) for consistency with `CardSummary`; document in OpenAPI/DTO docs.
- **Test placement:** follow `backend-rs/tests/README.md` (explicit helper imports, no forbidden helper shadowing, no blanket session SQL). Use `#[sqlx::test(migrations = "./migrations")]` patterns; name new suites clearly (e.g. `columns_embed_limit.rs`, `cards_board_list.rs`).
- **Frontend impact:** T050 consumes this API; schema regen will affect frontend types. Coordinate default 10/20 behavior before release.
