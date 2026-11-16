# 32-comments-mentions — Wave 2 design: comments & @mentions

> **Design-only** (no implementation in T027; implementation tasks are authored by taskpad
> after design review — `30-collaboration.md` §6 rows K/L). Constitution: `00-overview.md`.
> Sibling: `31-realtime-sse.md` (invalidation), `33-notifications.md` (inbox fed by mentions),
> `30-collaboration.md` (§1 domain model, §2 authz kernel, §5 activity log).
>
> *(Amendment 2026-11-16: new file. Comments+mentions were "Vision (future)" in the PRD —
> promoted to Wave 2 scope by user decision 2026-11-16.)*

## Goals (from the product plan)

Cross-functional discussion happens **on the work itself**: employees comment on cards and
notes, @mention teammates, and the mentioned person discovers it in the inbox + badge.
Scope (review decision 2026-11-16): **card comments and note comments**, one-level replies.

## Non-goals

Email delivery of mentions (in-app only, see `33`), rich text/WYSIWYG (markdown subset like
notes), @mentions in note *bodies* (notes stay documents — mention tokens there are not
expanded), reactions/emoji votes, file attachments, comment search, per-card ACL,
edit history diff UI (activity_log records the change; the comment row stores current
content), anonymous comments.

## 1. Domain model & schema

Forward-only sqlx migration(s); the collab migration block starts at **`0007_*`** (MCP holds
`0004`–`0006`). If collab ships first, renumber at authoring time — never reuse or hand-edit
an applied version. Both engines (MySQL 8.4 / MariaDB 11.4); no MySQL-only syntax; no row-alias
upserts; `utf8mb4_unicode_ci`.

```sql
CREATE TABLE `comments` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `workspace_id` INT NOT NULL,                    -- denormalized for authz + list filtering;
                                                  -- backfilled server-side from the subject
  `subject_type` VARCHAR(10) NOT NULL,            -- 'card' | 'note' (CHECK constraint style)
  `subject_id` INT NOT NULL,
  `parent_id` INT NULL,                           -- top-level comment id for one-level replies
  `author_user_id` INT NULL,                      -- ON DELETE SET NULL → renders "Deleted user"
  `body` VARCHAR(10000) NOT NULL,                 -- markdown subset, see §5
  `edited_at` DATETIME(6) NULL,                   -- NULL = never edited
  `removed_at` DATETIME(6) NULL,                  -- soft delete: body replaced by placeholder
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  `client_token` VARCHAR(64) NULL,                 -- idempotency, see §2; NULL = no constraint
  UNIQUE INDEX `UQ_comments_author_token` (`author_user_id`, `client_token`),
  INDEX `IDX_comments_subject_created` (`subject_type`, `subject_id`, `created_at`),
  INDEX `IDX_comments_workspace_created` (`workspace_id`, `created_at`),
  INDEX `IDX_comments_parent` (`parent_id`),
  PRIMARY KEY (`id`),
  CONSTRAINT `CHK_comments_subject_type` CHECK (`subject_type` IN ('card','note')),
  CONSTRAINT `FK_comments_workspace` FOREIGN KEY (`workspace_id`) REFERENCES `workspaces` (`id`) ON DELETE CASCADE,
  CONSTRAINT `FK_comments_author` FOREIGN KEY (`author_user_id`) REFERENCES `users` (`id`) ON DELETE SET NULL,
  CONSTRAINT `FK_comments_parent` FOREIGN KEY (`parent_id`) REFERENCES `comments` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB ...;

CREATE TABLE `comment_mentions` (                  -- one row per @mention (render + notify)
  `comment_id` INT NOT NULL,
  `mentioned_user_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`comment_id`, `mentioned_user_id`),
  INDEX `IDX_comment_mentions_user` (`mentioned_user_id`, `created_at`),
  CONSTRAINT `FK_comment_mentions_comment` FOREIGN KEY (`comment_id`) REFERENCES `comments` (`id`) ON DELETE CASCADE,
  CONSTRAINT `FK_comment_mentions_user` FOREIGN KEY (`mentioned_user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB ...;
```

- **No foreign key to `cards`/`notes`** — the polymorphic subject is enforced by the app
  layer (existence + authz at read/write time, §3). `workspace_id` is mandatory for every
  list query; it is resolved server-side from the subject (board→workspace chain), never
  accepted from the client (mass-assignment safe).
- **One-level threading** (`parent_id` → top-level comment in the *same subject*, validated).
  Replies to replies are rejected with `400` (`errors[]`). Deeper nesting is deliberately
  out of scope. `ON DELETE CASCADE` on `parent_id` only fires on physical deletion paths
  (workspace purge) — the API never physically deletes a comment, so soft-deleted threads
  stay intact.
- **Soft delete only.** `removed_at` marks the row; `body` is replaced by `''` at write time
  when `removed_at` is set (content destroyed; who/when retained for moderation + audit).
  Hard delete is never exposed by the API.
- **Per-subject cap:** 500 comments per subject (counted in the transaction that writes;
  over cap → `409` problem `too-many-comments`). Guardrail, not a quota — Plane uses 1000.
- **Notes commentable:** `subject_type='note'` follows the note (the note already links to
  board/project/card — comments follow access to the *note*, not its container, for simplicity).

## 2. API surface (additive; OpenAPI + `schema.d.ts` regen in the same PR — T52)

| Method & path | Purpose | Authz |
|---|---|---|
| `GET /api/v1/cards/{id}/comments?page&limit` | Paged list (limit ≤ 100), top-level comments with nested one-level replies; `order=asc` default | card read (viewer+) |
| `GET /api/v1/notes/{id}/comments?page&limit` | Same for notes | note read |
| `POST /api/v1/cards/{id}/comments` `{body, parent_id?, client_token?}` | Create; `201` + `Location` | card write (workspace member / editor grant) — **viewers: 403** (O-7) |
| `POST /api/v1/notes/{id}/comments` | Create | note write |
| `PATCH /api/v1/comments/{id}` `{body}` | Author-only edit; sets `edited_at` | comment author (or workspace admin for moderation delete, never edit) |
| `DELETE /api/v1/comments/{id}` | Author or workspace admin/owner; soft delete | as above |
| `GET /api/v1/workspaces/{id}/members?search=` | Mention/share autocomplete — workspace members *who can read* the context; `search` matches email prefix/substring, limit 50 | caller must be able to read at least one shared resource in the workspace |

- **Conflict semantics:** create is idempotent when `client_token` (client-generated UUID,
  ≤64 chars) is provided — unique index `UQ_comments_author_token (author_user_id, client_token)`;
  duplicate → `200` with the original comment (never a duplicate; NULL tokens never collide).
  Optional header form `Idempotency-Key` rejected — body field, `client_token`, matching house
  DTO discipline (snake_case, validated by garde).
- **Edit conflicts:** last-writer-wins on your own comment (`edited_at` is informational);
  no optimistic CAS for comments — they are conversational, not documents. If two devices
  edit simultaneously the later write commits and `edited_at` reflects it (documented);
  the activity_log keeps both writes with diffs.
- **Rate limits:** comment create **30/min/user** (dedicated governor bucket, separate from
  the global 120/min — burst-friendly for CLI/MCP); mentions **100/user/hour** and
  **1000/user/day** (enforced in the tx via count query); overage → `429` + `Retry-After`.
- **Pagination & ordering:** `Page<T>` envelope; comments `order=asc` (conversation), activity
  `order=desc`; `total` is a real count. Foreign/absent subject → **404** (no oracle).

## 3. Permission matrix

| Role | Read comments | Post/Reply | Edit own | Delete own | Delete others' |
|---|---|---|---|---|---|
| Platform admin | audit via admin activity only (no silent read — O-6) | no | no | no | no (moderation delegates to workspace admin) |
| Workspace owner/admin | yes | yes | yes | yes | **yes (soft, audited)** |
| Workspace member | yes | yes | yes | yes | no |
| Resource `editor` grant | yes | yes | yes | yes | no |
| Resource `viewer` grant | yes | **no (403)** | no | no | no |
| Non-member | **404** | 404 | 404 | 404 | 404 |

Enforcement: `authz::require(conn, actor, ResourceRef::Card(id)|Note(id), Perm::Read|Write)`
(see `30-collaboration.md` §2) — one indexed query each; the comment store never re-implements
ownership SQL. Cross-user negative tests are standing ACs (user B → 404/403).

## 4. @mentions — resolution, validation, guardrails

- **Wire/stored form:** `@[user:123]` tokens in `body`. Display resolution at read time joins
  `users` (email as handle — **no display-name column exists**; UX renders `@local-part` in
  a chip, tooltip = full email). A future `display_name` column is backwards-compatible.
- **Composer:** typing `@` opens a Radix `DropdownMenu` fed by
  `GET /api/v1/workspaces/{id}/members?search=`; choosing inserts the token with the
  member's email as visible text. No free-text mention entry — the token is atomic.
- **Write-time validation (in the comment create/update transaction):**
  1. Parse tokens (unique set, ≤ **20** distinct mentions/comment — over → `400`).
  2. Each id must exist AND hold **read access to the subject** (workspace membership or
     explicit resource grant — the same `authz` rule used for readers; a member of the org
     who cannot see this card must not be mentionable on it).
  3. `@all` / `@team:<name>` are **not** supported in v1 (no groups model yet). A workspace
     announcement is a future feature; it requires an admin-only flag and a recipient cap
     (200) — record as future work.
  4. Invalid mentions → `400` with `errors[]` listing the offending handles (never a
     silent drop — the client already showed them a member list).
- **Self-mention:** allowed; no client warning; de-duped by the notification dedupe rule
  (§5) and the actor-exclusion rule (`33`).
- **Unauthorized-mention objection:** none — validation happens before commit; a mention of
  someone who lost access between autocomplete and send is rejected, not downgraded.
- **Linkification:** mention tokens render as links to the user's membership context
  (`/workspace/{id}/…` is not needed — link to `#user-{id}` chip tooltip only v1; deep
  user pages don't exist).

## 5. Rendering & sanitization

- Comment markdown subset: paragraphs, emphasis/strong, inline code, fenced code (reuse
  `rehype-highlight`), links (http(s)/mailto only), lists, line breaks, mentions.
  **No headings, no images, no mermaid, no raw HTML.**
- Sanitize with the existing pipeline: `DOMPurify.sanitize()` **and** `rehype-sanitize` with
  a strict schema (allow `a,p,strong,em,code,pre,ul,ol,li,br`, `class` on `code` for the
  highlight theme). Mention chips are injected as React nodes after sanitize (never
  HTML-built), so raw token text can never smuggle markup.
- Body length ≤ **10 000 chars** (garde range on the DTO, consistent with the 10 000-char
  ceiling used elsewhere in the stack).
- Edited comments render an "edited" badge (`edited_at != NULL`); removed comments render
  a fixed placeholder — never the old body.

## 6. Transactions & activity-log coupling (same-tx writes)

Every mutation in this feature runs in **one sqlx transaction** containing:

1. The `comments` row write (create/update/soft-delete).
2. For creates: `comment_mentions` rows (after §4 validation).
3. `activity_log` row(s): `comment.create | comment.update | comment.delete`,
   `subject_type='comment'`, `subject_id=<id>`, `metadata` = `{subject_type: 'card'|'note',
   subject_id: <id>, field_names: ['body']}` (names only — never values; MCP rule).
   Mention count goes in `metadata` (`mentioned: <n>`), not individual mention rows.
4. `notifications` rows — fans out inside the same tx (see `33`): `mention` rows for the
   validated mention set; `comment` rows for the watchers of the subject.

Rollback of any step rolls back everything — a comment can never exist without its audit row
or inbox entries. Transaction cost: ~4–8 small inserts — well within the current write path
budget (T026 input). No background jobs (S4) — synchronous by design.

## 7. Frontend notes (expectations, not implementation)

- Comment thread in the existing card detail panel (`features/cards/`) and a comments tab
  in the notes view; composer = textarea auto-growing, 10k counter (soft warn at 9k),
  Radix primitives (no hand-rolled popovers), `@` dropdown with keyboard navigation,
  submit on `Ctrl/Cmd+Enter`, optimistic post with temp id reconciliation, rollback on error.
- Deep links: `/boards/:id?card=123&comment=456` and `/notes/:id?comment=456` — scroll into
  view + 2 s highlight (fade). Authz applies; foreign targets 404 the page load as today.
- Mention chips, edited badge, delete confirm dialog (Radix `Dialog`), "Show more" pagination
  (infinite scroll past page 1 — reuse `useInfiniteQuery` pattern from cards), empty state
  ("Be the first to comment").

## 8. Migration & backfill

- Additive tables only; no existing rows touched. Self-hosted orgs only — no data migration.
- **Ordering:** `0007_comments` requires the collab domain block from `30-collaboration.md` §3
  (M1: `workspaces`, member tables, `activity_log`) to be applied first — these tables never
  land before it (forward-only, checksums). `0008_notifications` builds on `0007` (FK to
  `comments`). Both engines green in CI (new suite + existing `sql_portability` patterns);
  `sqlx prepare` regen in the same PR; `migrate info` pending = 0.
- Deleting a workspace cascades comments + mentions (audit durability of `activity_log`
  is independent — kept, per `30` §5; `actor_user_id` survives, subject links dangle by
  design). Deleting a card/note does **not** cascade automatically — `subject_*` is
  polymorphic with no FK — so the card/note delete endpoints must delete the subject's
  comments (and their mentions) **in the same transaction**, or deletion leaks orphaned
  rows; this is a required task-AC in row K (flag at T027 review). Deleting a user:
  `author_user_id` → NULL (content retained, attribution via `activity_log` retained).

## 9. Test plan (TDD)

- Cross-user matrix on every endpoint (B on foreign card/note → 404 read/POST; 403 viewer).
- Create → GET re-read **and** DB query assert (body, mentions rows, workspace_id derivation);
  idempotency (`client_token` replay → same id, no duplicate row).
- Mention validation: foreign-access member → 400; non-member id → 400; >20 tokens → 400;
  valid multi-mention fanout (notification rows via `33` test helpers).
- Edit: author-only, `edited_at` set; delete: soft (body empty, `removed_at` set, thread
  intact); moderation delete by workspace admin → 200 + activity row; by member → 404/403.
- XSS: body with `<script>`, `javascript:` links, broken token markup → sanitized output
  asserted in render test; token `@[user:abc]` (garbage) → untouched plain text.
- Cap: 501st comment → 409; mention rate-limit bucket trips at threshold.
- Rollback: inject failure on notification insert → comment absent + activity_log absent.
- E2E (Playwright, API-call setup): B mentions A → A's bell shows 1 within 5 s (SSE path
  from `31`), A opens, deep link scrolls, A replies.

## 10. Contract & amendments (T52 same-PR)

- `openapi.json` + `schema.d.ts` + utoipa DTOs in the same PRs as the routes; `Page<T>`
  response for lists; problem types reused (`too-many-comments` new slug under
  `https://kanpad.dev/problems/`, RFC 9457).
- Amends: `api-rewrite-rs-specs/20-api-contract.md` (comments section), `30-db-schema.md`
  (two new tables), `frontend/CONVENTIONS.md` (composer + mention chip patterns),
  `specs/mcp/90-open-calls.md` O-1 cross-ref (notes-as-comments was **rejected** — dedicated
  table, rationale in this file §1; no MCP files are amended by this design pass).

## 11. Open calls (decide at T027 review)

- **O-7 (from `30` §7, repeated):** viewer can read comments but cannot post/reply/react —
  default yes (matches Read/Write split).
- **O-12:** allow editing own comment without time window (default **yes** — `edited_at`
  badge + activity trail suffice; 15-min windows frustrate users and don't stop edits).
- **O-13:** `client_token` idempotency (default **yes**, body field; omitted = always create).
- **O-14:** mention bell-priority vs inbox tab grouping (default **All/Mentions tabs**,
  Plane-style; watcher comments land in "All").
- **O-15:** comment count cap per subject 500 (default yes).
- **O-16:** delete-by-moderator copy ("deleted by a moderator" vs "…by author") — copy
  decision only.
