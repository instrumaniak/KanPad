# 21 — API standard shapes (companion to 20; both files agree)

Audit basis: all 11 `backend/src/**/*.controller.ts` (52 handlers) + 16 DTOs,
checked 2026-10-11. Standards applied: plural nouns, no verbs in URLs,
`GET collection / POST collection / GET|PATCH|DELETE item`, `201+Location`
create, `200` update, `204` delete (no body), `Page{data,page,limit,total}`
lists, RFC9457 `problem+json` errors, one naming case, `/api/v1` versioning.
This file holds the Nest→rewrite endpoint map; `20-api-contract.md` holds the
same contract condensed for utoipa work. If the two ever disagree, fix the
disagreement — neither silently wins.

## 21.1 Global rules

* Base: `/api/v1/*`. Keep `/api/*` as a deprecated alias one release (dual-mount,
  same handlers — NOT redirects, so method + body survive untouched), then
  remove. No unversioned new routes. Versioning is implemented once in T05
  (router mount); resource tasks (T20–T27) register `v1` routes only and get
  the alias free.
* Case: `snake_case` everywhere (keys, bodies, queries). Rename outliers:
  `boardCount→board_count`, `movedCount→moved_count`, `tagId→tag_id`,
  `tagIds→tag_ids`, `labelId→label_id`, `targetColumnId→target_column_id`,
  `?projectId=→?project_id=`. Record table in `CONTRACT-CHANGES.md`; typegen
  catches frontend (`40-typegen.md` verbatim rule is retired).
* Envelopes: creates `201 {data}+Location`, reads/updates `200 {data}` or
  `200 Page<T>`, deletes `204` (no body — adopts `11 E1`, retires the
  `20-api-contract.md:12` `200`-typed-delete split). No `message` strings.
* Errors: RFC9457 per `11 E1` (stable `type` URIs, always-`instance` with
  `x-request-id`). Validation→`400+errors[]`, `401` unauth, `403` role-denial only, `404` no-oracle for foreign IDs (detached-label delete is 404, not 409), `409` conflicts + wrong-state
  deletes (active-board delete), `429+Retry-After` throttle.
* Lists: `?page&limit` strict-400 (`page≥1`, `1≤limit≤100`), `?sort&order`
  from a per-resource whitelist (never `format!` values), `?q=` search where
  listed. `total` via count query, never `data.length`.
* Health (outside governor, no auth): `GET /health` liveness (`{status:"ok"}`,
  no DB) + `GET /ready` readiness (`SELECT 1` → `200`/`503`). Both outside
  `/api/*`. Retires `GET api/health` (the only health route in source — no `/health` or `/api/ready` exist); canonical is `/health` + `/ready` outside `/api`.
* Idempotency: label attach `PUT` (keep `POST` alias one release, document
  deprecated); archive/restore via state field (idempotent); empty move-cards =
  `200 {moved_count:0}`, not an error.

## 21.2 Endpoint map (Nest → rewrite)

Auth (`auth.controller.ts:24-70`):

| Nest | Rewrite | Notes |
|---|---|---|
| `POST /api/auth/register` → `201 {data,message}` | `POST /api/v1/auth/register` → `201 {data}+Location: /api/v1/auth/me` + cookie | drop `message`; `403` closed, `409` dup; tx user+labels |
| `POST /api/auth/login` → `200 {data,message}` | `POST /api/v1/auth/login` → `200 {data}` + cookie | 5/min; bad→`401` same shape (no enumeration) |
| `POST /api/auth/logout` → `200 {message}` (throttled!) | `POST /api/v1/auth/logout` → `204` + clear cookie, **unthrottled** | throttling logout can lock users out — fix |
| `GET /api/auth/me` → `200 {data}` (triple check) | `GET /api/v1/auth/me` → `200 {data}` | unify to guard rule + MUST re-lookup row (deleted → 401, demoted → 403); old cookies → `401` |

Projects (`projects.controller.ts:40-102`):

| Nest | Rewrite |
|---|---|
| `GET /api/projects` → `{data,total=len}`, `boardCount:0` stub | `GET /api/v1/projects?page&limit&q&sort&order` → `Page<Project{board_count real}>` |
| `POST /api/projects {name}` → `201 {data,message}` | `POST /api/v1/projects` → `201 {data}+Location` |
| `PATCH /api/projects/:id` (manual empty-check) | `PATCH /api/v1/projects/:id` → `200`; empty→`400+errors[]` via DTO |
| `DELETE /api/projects/:id` → `200 {message}` | `DELETE /api/v1/projects/:id` → `204` (cascade boards) |

Boards (`boards.controller.ts:45-246`):

| Nest | Rewrite |
|---|---|
| `GET /api/boards?projectId&archived` (manual `parseInt`, NaN ignored) | `GET /api/v1/boards?project_id&archived&sort&order&page&limit` → `Page<Board>`; garbage→`400` |
| `GET /api/boards/archived` (duplicate, matchit hack) | **deleted** — use `?archived=true` |
| `POST /api/boards` (non-atomic) | `POST /api/v1/boards` → `201+Location`; board + 3 columns (`To Do/In Progress/Done` 0/1/2) in one tx; foreign `project_id`→`404` |
| `GET /api/boards/:id` (board+columns, no cards) | `GET /api/v1/boards/:id` → `200` unpaginated (`position ASC`), keep shape |
| `PATCH /api/boards/:id` | `PATCH /api/v1/boards/:id {name?,background_color?,project_id?\|null,view_mode?}` → `200` |
| `DELETE /api/boards/:id` → `200 {message}` | `DELETE /api/v1/boards/:id` → `204` (hard) |
| `PATCH /:id/archive`, `/:id/restore` (verbs) | **deleted** — `PATCH /api/v1/boards/:id {is_archived:true\|false}` (idempotent) |
| `DELETE /:id/permanent` → `200`, `403` unless archived | **deleted** — covered by `DELETE`; active-board delete→`409` (state conflict, not `403`) |

Columns (`columns.controller.ts:44-169`):

| Nest | Rewrite |
|---|---|
| `GET /api/boards/:boardId/columns` (embeds cards) | `GET /api/v1/boards/:boardId/columns` → `200 {data[]}` unpaginated `position ASC`, keep embed |
| `POST /api/boards/:boardId/columns` | `POST ...` → `201+Location`, `MAX(position)+1` via `SELECT ... FOR UPDATE` in tx, default `New Column` |
| `PATCH /api/columns/:id` | `PATCH /api/v1/columns/:id {name}` → `200` |
| `DELETE /api/columns/:id` (no gap-close/tx) | `DELETE /api/v1/columns/:id` → `204`, cascade cards + set-based gap-close (`position - 1` range) in tx |
| `PATCH /api/columns/:id/sort {order}` (verb) | `PATCH /api/v1/columns/:id {sort_order:asc\|desc}` → `200` (rewrite by `created_at`) |
| `POST /api/columns/:id/move-all {targetColumnId}` (verb phrase) | `POST /api/v1/columns/:id/move-cards {target_column_id}` → `200 {moved_count}`; same-board/same-id→`400`, empty→`200 {moved_count:0}`; append at `MAX+1` in tx with source `ORDER BY position ASC` (target mirrors source order); set-based, no reload+save loop | sub-path `move-cards` (plain Axum route — NOT a colon-suffix `:move-cards` custom verb; `matchit` treats `:` literally and `{id}:move-cards` would not match as intended) |

Cards (`cards.controller.ts:40-128`):

| Nest | Rewrite |
|---|---|
| `GET /api/columns/:col/cards` (bare array) | `GET /api/v1/columns/:col/cards?page&limit` → `Page<CardSummary>` (no TEXT; `has_description` + `checklist_progress?`) |
| `POST /api/cards {title,column_id,...}` (flat, explicit position writes verbatim → dupes) | `POST /api/v1/cards` → `201+Location` (keep flat body; explicit `position` = insert-with-open-gap, default `MAX+1` via `FOR UPDATE`) |
| `PATCH /api/cards/:id` (scalar+move, clamp bug) | `PATCH /api/v1/cards/:id` → `200`; set-based range `UPDATE`s + `FOR UPDATE` per `11 A3`; cross-column without position appends `max+1`; write clamped value |
| `GET /api/cards/:id` | `GET /api/v1/cards/:id` → `200 CardDetail{checklists{items},labels}` |
| `POST /api/cards/:id/labels {labelId}` → `200` | `PUT /api/v1/cards/:id/labels/:label_id` → `200 {data:Card}` idempotent (keep `POST` alias one release, deprecated — alias body `{label_id}`, legacy `{labelId}` also accepted) |
| `DELETE /api/cards/:id/labels/:labelId` → `200 {message}` | `DELETE ...` → `204`; unassigned→`404` |
| `DELETE /api/cards/:id` → `200 {message}` (bare, no gap-close) | `DELETE /api/v1/cards/:id` → `204` + gap-close in same tx (dense re-read) |

Labels / tags (`labels.controller.ts:33-77`, `tags.controller.ts:50-94`):

| Nest | Rewrite |
|---|---|
| `GET /api/labels`, `GET /api/tags` (bare arrays) | `GET /api/v1/labels`, `/tags?page&limit&q&sort&order` → `Page<T>`; labels lazy-seed defaults if empty |
| `POST` → `201 {data,message}` | `201 {data}+Location`; label `color` enum(6), tag `color` free-form ≤20 default `teal`; trim + per-user unique→`409`; whitespace-only→`400` |
| `PATCH` → `200 {data,message}` | `200 {data}`; tag update gains `409`+trim+not-empty (missing today) |
| labels `DELETE` → `204` (only correct one) / tags `DELETE` → `200 {message}` | both `DELETE` → `204` |

Checklists (`checklists.controller.ts:77-168`):

| Nest | Rewrite |
|---|---|
| `POST /api/cards/:cardId/checklists` | `POST /api/v1/cards/:cardId/checklists` → `201+Location` |
| (missing list) | **added** `GET /api/v1/cards/:cardId/checklists` → `200 {data[]}` (small-N, unpaginated, documented) |
| `GET/PATCH/DELETE /api/checklists/:id` | `GET /api/v1/checklists/:id` → `200`; `PATCH` → `200`; `DELETE` → `204` |
| `POST /api/checklists/:id/items` | `POST /api/v1/checklists/:id/items` → `201+Location`; `position` int `≥0` (reject negatives/floats `400`) |
| `PATCH/DELETE /api/checklist-items/:id` (stem mix) | `PATCH /api/v1/checklist-items/:id` → `200`; `DELETE` → `204` (lock stem; document nested alternative, don't ship both) |

Notes (`notes.controller.ts:67-138`):

| Nest | Rewrite |
|---|---|
| `GET /api/notes?search&type&tagId&boardId&page&limit` (full `content` in list, clamp, bare `@IsNumber`, multi-FK allowed, no LIKE escape) | `GET /api/v1/notes?search&type&tag_id&board_id&page&limit` → `Page<NoteSummary>` (no `content`; `type` derived from FK set, at most one FK else 400; int-validated, strict-400; `LIKE %q%` title with escaping + `push_bind`, `updated_at DESC`; two-step IDs-then-enrich + `COUNT(DISTINCT)`) |
| `POST /api/notes {…,tagIds?}` (drops unknown tags) | `POST /api/v1/notes` → `201+Location`; unknown `tag_ids`→`404` (match update); `content ≤65535` chars documented |
| `GET/PATCH/DELETE /api/notes/:id` | `GET` → `200` (explicit `content` select, `select:false` entity); `PATCH` → `200`; `DELETE` → `204` |
| `GET /api/boards/:boardId/notes` (duplicate) | **deleted** — use `GET /api/v1/notes?board_id=` |

Users / misc: `U1 /api/users` stays **unmounted** (internal helpers only, per `T13`); `S1` health replaced by §21.1 `/health` + `/ready`.

## 21.3 What executors must do

* New routes: add `GET cards/:id/checklists` (T26a), `PUT cards/:id/labels/:label_id`
  (T24c, `POST` alias deprecated), `/api/v1` prefix + `/api` dual-mount alias
  (T05 — resource tasks register `v1` routes only), `/health` + `/ready`
  outside `/api` (T05), `Location` on all `201`s (each resource task).
* Deleted routes (no implementation + `CONTRACT-CHANGES.md` entry; T31 pins per-route status): `/boards/archived` → 400 (`:id` int-parse fail), `/boards/:id/archive|/restore|/permanent` → 404, `/boards/:id/notes` → 404, `/columns/:id/sort` → 404, `/columns/:id/move-all` → 404. `T31` golden fixtures updated to assert absence with this table.
* Renames (same PR as resource, via T52): snake_case fields/queries above;
  `movedCount→moved_count` payload; column `sort{order}`→`{sort_order}`.
* Status fixes (same PR as resource): all deletes `204` (incl. checklist-item
  deletes); logout `204`; active-board delete `409` (no `permanent` route);
  empty move-cards `200 {moved_count:0}`.
