# CONTRACT-CHANGES.md — NestJS → Rust rewrite: intentional wire deviations

Written in T01 (contract freeze). Normative companions:
`specs/api-rewrite-rs-specs/20-api-contract.md` + `21-api-standard-shapes.md`
(the two agree; if they ever disagree, fix the disagreement — neither wins).
Contract changes after this freeze go only via T52
(spec + utoipa + `openapi.json` + typegen in the same PR).

Base path for every route below is `/api/v1/…` with the pre-version `/api/…`
kept as a deprecated same-handler alias for one release (dual-mount, NOT
redirects), then removed.

## 1. Keep vs break

| Area | Nest today | Rust rewrite | Keep or break |
|---|---|---|---|
| Transport | cookie session (`connect.sid`, 24h, `credentials:include`) | same mechanism (`tower-sessions` 0.14 + sqlx store, sliding 24h, `credentials:include`) | **keep** |
| Error shape | `{statusCode, message: string\|string[], error}` | RFC9457 `application/problem+json {type, title, status, detail, instance, errors?}` (stable `type` URIs, always-set `instance`) | **break (adopted)** |
| Validation status | 400 (per-endpoint pipes; register/login unvalidated) | 400 everywhere, with `errors[]` field→message map | **keep status, break shape** |
| Lists | `{data, total = data.length}` (projects/boards) / bare `{data: []}` (rest) | `Page<T> = {data, page, limit, total}` via count query, `?page&limit` strict-400 (`page ≥ 1`, `1 ≤ limit ≤ 100`, no clamp) | **break (adopted)** |
| Versioning | unversioned `/api/*` | `/api/v1/*` (+ deprecated `/api/*` alias one release) | **break (adopted)** |
| Naming | mixed (`boardCount`, `movedCount`, `tagId(s)`, `labelId`, `targetColumnId`, `?projectId=`) | `snake_case` everywhere (table §2) | **break (adopted)** |
| Deletes / logout | `200 {message}` (labels `DELETE` already 204) | `204` no body for ALL deletes + `POST logout` | **break (adopted)** |
| Success envelopes | `{data, message?: string}` / `{message}` | `201 {data}+Location` / `200 {data}` or `200 Page<T>` / `204`; `message` dropped everywhere | **break (dropped)** |
| Verb URLs | `/:id/archive`, `/:id/restore`, `/:id/permanent`, `/:id/sort`, `/:id/move-all` | state-field `PATCH {is_archived}`, `PATCH {sort_order}`, `POST /:id/move-cards` | **break (dropped)** |
| Throttle | global 10/min configured, never enforced (no `APP_GUARD`); logout throttled 5/min | 120 req/min per IP global + 5/min on `register`/`login` only, `/api/*` only; logout unthrottled; `/health` + `/ready` never throttled | **keep, re-scoped** |
| Password hashing | bcrypt cost-10 | Argon2id (`m=19MB, t=2, p=1`) for new hashes; bcrypt verifies pre-cutover hashes only, with same-request rehash to Argon2id | **break (adopted)** |
| Sessions across cutover | — | fresh store-owned schema + new `SESSION_SECRET` → old `connect.sid` cookies 401 → **force re-login** (no row migration) | **break** |

## 2. `snake_case` rename table

| Nest | Rewrite |
|---|---|
| `boardCount` | `board_count` (now computed for real, was stub `0`) |
| `movedCount` | `moved_count` |
| `tagId` / `tagIds` | `tag_id` / `tag_ids` |
| `labelId` | `label_id` |
| `targetColumnId` | `target_column_id` |
| `?projectId=` | `?project_id=` |
| `sort {order}` (column sort body) | `{sort_order: asc\|desc}` |
| card label alias body `{labelId}` (legacy) | `{label_id}` (legacy `{labelId}` accepted one release, then removed) |

## 3. `204` deletes (all, no body)

`DELETE projects/:id`, `DELETE boards/:id`, `DELETE columns/:id`,
`DELETE cards/:id`, `DELETE cards/:id/labels/:label_id`,
`DELETE labels/:id`, `DELETE tags/:id`,
`DELETE checklists/:id`, `DELETE checklist-items/:id`,
`DELETE notes/:id`, plus `POST /api/v1/auth/logout`.
Active-board delete → `409` state conflict (no `permanent` route).

## 4. Deleted routes (no implementation; T31 asserts absence)

| Deleted | Replacement | Absence status |
|---|---|---|
| `GET /boards/archived` | `GET /api/v1/boards?archived=true` | 400 (`:id` int-parse fail) |
| `PATCH /boards/:id/archive`, `/:id/restore`, `DELETE /:id/permanent` | `PATCH /api/v1/boards/:id {is_archived}` / plain `DELETE` | 404 |
| `GET /boards/:boardId/notes` | `GET /api/v1/notes?board_id=` | 404 |
| `PATCH /columns/:id/sort` | `PATCH /api/v1/columns/:id {sort_order}` | 404 |
| `POST /columns/:id/move-all` | `POST /api/v1/columns/:id/move-cards` | 404 |

## 5. Added routes

- `GET /api/v1/cards/:cardId/checklists → 200 {data[]}` (small-N, unpaginated).
- `PUT /api/v1/cards/:id/labels/:label_id → 200 {data: Card}` idempotent
  (keeps deprecated `POST /:id/labels` alias one release).
- `GET /health` (liveness `{status:"ok"}`, no DB) + `GET /ready` (readiness
  `SELECT 1` → `200`/`503`), both outside `/api/*`; retires `GET api/health`.
- `/swagger-ui` + `/api-docs/openapi.json`; `302 /api/docs → /swagger-ui`.

## 6. New strictness (400s the Nest build never emitted)

Auth `register`/`login` DTO decorators are dead code in Nest (no pipes on those
handlers — weak inputs reach the service today); GET query DTOs are mostly
unvalidated in Nest. The rewrite validates them (garde via `axum-valid`,
all failures 400 + `errors[]`); frontend T41* must pre-sanitize or accept
the surfaced 400:

- `register`: email + password strength (`≥ 8 chars + digit/symbol`) + password
  byte cap `≤ 72`; `login`: non-empty + byte cap only (never strength — legacy
  weak passwords must reach 401, not 400).
- `GET /api/v1/boards?project_id=abc` → 400 (Nest `parseInt` → `NaN`, filter
  silently ignored).
- `POST /api/v1/boards` blank-after-trim name → 400 (Nest `create-board.dto`
  lacks `IsNotEmpty`, `""` creates today). Same trim + min-length-1 rule on
  project/column/tag/label names, checklist-item text, board-update name.
- `?page=0`, `?limit>100`, non-integers → 400 strict (retires the notes-service
  `Math.min(limit ?? 50, 100)` silent clamp).
- Checklist-item `position`: int `≥ 0` (negatives/floats → 400).
- Notes: at most one of `board_id|project_id|card_id` (multiple → 400);
  `POST` unknown `tag_ids` → 404 (create aligns with update, which already
  throws; create silently dropped them).
- Tags update gains 409 dup-name + trim + not-empty (missing today).
- Label/tag names collide case-insensitively under `utf8mb4_unicode_ci`
  (`'Work'` vs `'work'` → 409); asserted in tests.

## 7. Ordering / query hardening

- `GET /api/v1/boards/:id` columns ordered `position ASC` (Nest loads the
  `columns` relation with no `ORDER BY`).
- move-cards source select `ORDER BY position ASC` (Nest loads source unordered;
  target order must mirror source order).
- Position writes are set-based range `UPDATE`s with `SELECT … FOR UPDATE`
  (never full-column reload+save); explicit card positions insert-with-open-gap
  (never verbatim writes that duplicate); out-of-range requests are clamped and
  the **clamped** value is written; cross-column move without position appends
  at `max+1`; column `DELETE` closes the gap in the same tx; card `DELETE`
  closes the gap in the same tx.
- Notes list: `LIKE %search%` on title with `%`/`_`/`\` escaping (bound params
  only), `ORDER updated_at DESC`, two-step IDs-then-enrich +
  `COUNT(DISTINCT n.id)` with a shared WHERE builder (never `COUNT(*)` over
  the tags join); list SELECTs exclude TEXT (`content` for notes, `description`
  for cards — `has_description` / `checklist_progress` summaries only).

## 8. Byte-level deltas (no parity owed)

- **bcrypt truncation:** passwords capped at **72 bytes** (byte-length check,
  not chars — multibyte would silently collide); bcrypt fallback truncates to
  72 bytes before verify (Nest/bcryptjs semantics).
- **Dates:** `due_date` must be ISO-8601 valid-date-or-null (JS `new Date()`
  parsing differs — ISO-8601 is pinned).
- **Email:** garde email validator approximates validator.js `isEmail`;
  borderline addresses may differ.
- **Unknown JSON fields:** ignored (serde default, no `deny_unknown_fields`) —
  matches Nest (pipes use `transform` without `whitelist`).
- **Compression:** no 1024-byte threshold (Nest `@fastify/compress` parity delta,
  accepted). Origin timeout → **503** + `Retry-After` (504 is proxy semantics).

## 9. No `/api/users` routes (see T13)

`U1 /api/users` stays **unmounted** in the rewrite — user helpers are internal
only. There is no user-listing or user-admin surface in v1.

## 10. Auth notes

- `register → 201 {data:{id,email,role}} + Set-Cookie + Location: /api/v1/auth/me`;
  `REGISTRATION_ENABLED=false → 403`; duplicate email → 409; user-create +
  6-label seed in **one transaction**; `session.cycle_id()` on register, every
  login, and privilege change (fixation defense).
- `me` + admin routes re-lookup the user row (deleted → 401, demoted → 403);
  otherwise the session payload is trusted without per-request DB lookup.
- Unknown-email login runs a dummy Argon2 verify (bcrypt-path timing delta
  accepted one release); bad credentials → 401 same shape (no enumeration).
- `served-by: nest|rust` response-header plan: implemented in T05, asserted
  in T51 (cutover detection).

## 11. GDPR erasure runbook

No self-service erasure endpoint ships in v1. Erasure is manual, owner-run:

1. `DELETE FROM users WHERE id = ?` — FK cascades remove projects, boards,
   columns, cards, labels, checklists, notes, tags owned by the user.
2. Purge rows for that user from `tower_sessions.session` (session payloads
   carry `user_id`/`email`; delete by payload match or `expiry`-sweep all rows
   and force global re-login if payload matching is unavailable).
3. Backup-retention window: erasure covers live DB + session store; time-boxed
   backups age out per the retention policy — record the request date and
   confirm the next post-window restore contains no residue.
4. Optional hardening (out of scope for the rewrite, noted for T52): scoped
   `DELETE /api/v1/auth/me` self-erasure or a `delete-user` CLI subcommand
   reusing the same cascade + session purge.
