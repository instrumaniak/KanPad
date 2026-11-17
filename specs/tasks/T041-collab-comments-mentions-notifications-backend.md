# T041: Collab: comments + mentions + notifications backend

> **STATUS: pending.** Phase 2. Wave 2 / row K. **Closes S5** (HIGH strategic —
> no comments/mentions anywhere; they were PRD "Vision" items promoted to Wave 2
> by user decision 2026-11-16) and delivers the in-app notification inbox
> backend (`33`). May land as **two sequential PRs** (0007 comments →
> 0008 notifications) if session size demands — one task, two PRs, both must
> pass the DoD.

## Goal

Backend for comments, @mentions and the in-app inbox, per
`32-comments-mentions.md` and `33-notifications.md` — **all writes in ONE
transaction with `activity_log`** (`32` §6):

**Schema (0007 / 0008; collab block must precede these — forward-only,
checksums; renumber at authoring time; never reuse applied versions):**
- `comments` — `subject_type` ENUM-ish `card|note`, `parent_id` (one-level
  replies), `author_user_id` SET NULL, `body` VARCHAR(10000), `edited_at`,
  `removed_at` (soft delete — body destroyed at write time, row retained),
  `client_token` with `UNIQUE (author_user_id, client_token)`, indexes per
  `32` §1. **No FK to cards/notes** (polymorphic subject).
- `comment_mentions` — PK (comment_id, mentioned_user_id).
- `notifications` + `notification_watches` — exact shapes per `33` §1; both
  engines; `metadata` native JSON, never content; `actor_key_id` plain `INT
  NULL` (FK inline if MCP's 0004 has landed, else added by the following
  migration — **never hand-edit an applied version**).

**Comment API (`32` §2):** paged lists (`GET /cards/{id}/comments`,
`GET /notes/{id}/comments`, `order=asc`, nested one-level replies) ·
`POST …/comments` with **`client_token` idempotency** (duplicate → `200` with
the original, never a duplicate row) · `PATCH /comments/{id}` (author-only;
`edited_at` set; LWW by design — **no CAS on comments**) ·
`DELETE /comments/{id}` (author, or workspace admin/owner for audited
moderation delete; **soft delete only**, body → `''`) · cap **500 comments per
subject** → `409 too-many-comments` · create rate **30/min/user** (dedicated
bucket) · mentions **100/user/hour** + **1000/user/day** (in-tx count query).

**@mentions (`32` §4):** stored form `@[user:id]`; write-time validation in the
create/update tx — parse (unique set, ≤ **20** → else 400), each id must exist
**and hold read access to the subject** (same `authz` rule; a mention of someone
who lost access between autocomplete and send is rejected — never downgraded);
self-mention allowed (de-duped by notification dedupe + actor exclusion);
`@all` → 400 (not supported v1).

**Notifications (`33`):** fanout in the mutation tx — `mention` rows for the
validated set (minus actor), `comment` rows for subject watchers (minus actor),
`shared` rows on grant (wired by the second of {T035, T041} to land — see
retrofit rule); **actor exclusion absolute**; **dedupe**: no two unread rows
with same `(recipient_user_id, type, comment_id)` (existence check in-tx;
race-safe via `FOR UPDATE` on the comment row); **recipient cap 200** →
`409 notification-fanout-too-large`, zero rows written (tx proof).

**Watch model (`33` §3):** auto-subscribe on comment create / mention / granted
`shared`; `PUT/DELETE /api/v1/watches/{subject_type}/{subject_id}` — idempotent,
self-scoped, read-access required to watch (no oracle); access loss soft-ignores
watch rows (filtered on access); membership teardown hard-deletes watch rows.

**Inbox API (`33` §4):** `GET /api/v1/notifications` (`Page<T>`, `unread`,
`type` filter) · `GET /api/v1/notifications/unread-count` (**one** indexed
COUNT) · `POST /{id}/read` (204; foreign → 404) · `POST /read-all` (204) ·
`DELETE /{id}` (204, own rows only) · **lazy purge**: each inbox list call
deletes ≤500 expired **read** rows > **90 days** for that recipient (single
indexed DELETE); unread rows never purged; no background jobs (S4).

**Read-time resolution (`33` §5):** no content in rows; the client resolves
actor email via **new** `GET /api/v1/users?ids=` (returns `{id, email}` only
for users sharing a workspace with the caller — never a global directory).

**Cascade rule (`32` §8 — required):** card/note delete endpoints delete the
subject's comments (+mentions) **in the same transaction** (polymorphic, no
FK) — extend the delete handlers; deletion leaks orphaned rows otherwise.

## Depends On

T032 (kernel), T033 (C), T034 (D) — comment stores call the live authz kernel
for card/note resources; T040 (J) — the bus exists for `notification.*`.

## Phase

2

## Critical

No — but it is the largest Wave 2 task; if the session cannot cover both
features, land 0007 (comments + mentions + activity coupling) first, then
0008 (notifications + watches + purge + inbox API) as a second PR under this
same task file.

## Spec References

- `specs/arch-hardening-specs/32-comments-mentions.md` — the full contract:
  §1 schema (comments, comment_mentions; no FK to subjects; one-level
  threading; soft delete; 500-cap), §2 API surface (idempotency, LWW,
  rate budgets), §3 **permission matrix** (platform admin: no silent read;
  workspace owner/admin: moderation delete; member/editor: post; **viewer:
  read-only — 403**, **O-7 default**), §4 mention validation, §5 rendering
  (frontend's concern — T042; backend stores markdown subset plain),
  §6 same-tx activity coupling (`comment.create/update/delete`; metadata =
  field names + `mentioned: <n>`), §7 frontend expectations (T042), §8
  migration/backfill + **cascade-delete rule**, §9 test plan, §10 contracts,
  §11 open calls (**O-12 edit window: none; O-13 client_token idempotency:
  yes; O-15 cap 500: yes; O-16 moderator-delete copy** — all defaults pending
  T027 ratification).
- `specs/arch-hardening-specs/33-notifications.md` — §1 schema (notifications,
  notification_watches; `metadata` JSON, never content; 90-day lazy purge;
  unread never purged), §2 fanout rules (actor exclusion; dedupe; 200-cap →
  409), §3 watch model (auto/manual subscribe), §4 API surface, §5 read-time
  resolution + `GET /api/v1/users?ids=`, §6 SSE integration (`notification.new`
  post-commit via T040 bus), §8 privacy (recipient-private), §10 contract
  amendments, §11 open calls (**O-17 lazy purge: yes; O-18 mutes: v2 defer;
  O-19 shared notify: yes; O-20 99+ cap: client-side, T042; O-21 re-mention
  re-arm: no**).
- `specs/arch-hardening-specs/30-collaboration.md` — §2 authz kernel usage,
  §5 activity log (co-commit rule).
- `specs/arch-hardening-specs/00-overview.md` — findings **S5**, **S6**
  (notification.new riding SSE), **S1**; global DoD; D-7.
- `specs/api-rewrite-rs-specs/20-api-contract.md` + `30-db-schema.md` —
  **amendment targets** (T52): comment/notification/watch/users?ids endpoints,
  problem slugs (`too-many-comments`, `notification-fanout-too-large`),
  `Page<T>` envelope reuse. OpenAPI + `schema.d.ts` same-PR.
- `backend-rs/tests/README.md`; CI matrix legs (both engines).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/000N_collab_comments.sql` | **NEW** — `comments` + `comment_mentions` |
| `backend-rs/migrations/000N_collab_notifications.sql` | **NEW** — `notifications` + `notification_watches` |
| `backend-rs/src/features/comments/` | **NEW** — routes + store: CRUD (soft delete, nesting, caps, client_token idempotency, rate buckets), mention parse/validate, in-tx fanout + activity co-insert |
| `backend-rs/src/features/notifications/` | **NEW** — inbox routes (list/unread-count/read/read-all/dismiss), watch routes (PUT/DELETE), lazy purge, `GET /api/v1/users?ids=` |
| `backend-rs/src/features/cards|notes/` | extend **delete** handlers: same-tx delete of subject's comments + mentions |
| `backend-rs/src/features/members/` (T035) | `shared` fanout on grant — **second-landed rule** (this task wires it if T035 landed first; else T035 does) |
| `backend-rs/src/lib.rs` | register routes + OpenAPI paths + rate-limit buckets (`comments_create`, `mentions_*`) |
| `backend-rs/tests/api/comments.rs` | **NEW** — full matrix (below) |
| `backend-rs/tests/api/notifications.rs` | **NEW** — fanout/dedupe/cap/purge/actor-exclusion/watch matrix |
| `specs/api-rewrite-rs-specs/{20-api-contract.md,30-db-schema.md}` | **AMEND** (T52 same-PR) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** frontend (T042), email/webhook/push delivery
(out of scope), `activity_log` schema changes (T037 owns), global
`/api/v1/users` directory (does not exist — `users?ids=` only), comment CAS
(`32` §2 — LWW by design), note-body mention expansion (never).

## Implementation Steps

1. **Migrations first** (both engines; renumber if collab block moved); verify
   `migrate info` pending=0; FK decision for `actor_key_id` resolved by what
   has landed (MCP 0004 or not) — record in §Notes.
2. **Comments schema + store**: DTOs (garde; `body` ≤ 10 000 chars), nesting
   validation (parent must be a top-level comment **in the same subject**;
   reply-to-reply → 400 `errors[]`), workspace_id resolved server-side from the
   subject (never accepted from the client — mass-assignment safe), soft
   delete (body → `''`, `removed_at` set), per-subject count in-tx → 409 over 500.
3. **`client_token` idempotency** (red-first): same `(author, token)` replay →
   200 + original comment; NULL tokens never collide; different author +
   same token → independent rows (unique index scoped by author).
4. **Mention pipeline** (in-tx): tokenize → dedupe → ≤20 → existence +
   read-access check per id (one batched query) → `400 errors[]` on any
   invalid; else insert `comment_mentions` rows.
5. **Fanout** (in-tx, `33` §2): mention rows (minus actor) + comment rows for
   watchers (minus actor); `FOR UPDATE` on the comment row for race-safe
   dedupe; 200-recipient cap → 409 with **zero rows written** (rollback proof).
6. **Activity co-commit**: `comment.create|update|delete` rows via the T037
   helper — metadata = `{subject_type, subject_id, field_names:['body'],
   mentioned: <n>}`; rollback injection (fail fanout) → comment **and**
   activity row both absent (test).
7. **Watches**: auto-subscribe on create/mention/shared; `PUT/DELETE
   /watches/…` (read-access required; idempotent 204; teardown hard-delete is
   wired with T035's membership removal — second-landed rule); watcher who
   lost access → no notification (filter on access in fanout query — test).
8. **Inbox**: list (Page<T>; `unread=true` default-toggle; `type=` filter;
   purge sweep in the list call: `DELETE … WHERE recipient_user_id = ? AND
   read_at IS NOT NULL AND read_at < NOW() - INTERVAL 90 DAY LIMIT 500` — verify
   on both engines) · `unread-count` (one indexed COUNT) · mark read/read-all/
   dismiss (foreign id → 404; dismiss scoped to own rows) ·
   `GET /api/v1/users?ids=` (shared-workspace scoping; `{id,email}` only).
9. **Card/note delete cascade**: extend the delete handlers (same tx: collect
   comment ids → null their mentions → delete comments → delete subject). Wire
   `shared` fanout if T035 landed first (second-landed rule; else the wiring is
   a step here for T035's grant endpoints — check the seam note).
10. **Rate limits**: dedicated `comment_create` bucket (30/min/user) and
    mention budgets (100/h, 1000/day) as count queries in-tx; overage → 429 +
    `Retry-After`.
11. Contract (T52) regen; both engines; `sqlx prepare --check`; persistence
    re-read everywhere; lesson.

## Constraints

- **One transaction** for comment + mentions + activity + notifications —
  all-or-nothing, proven by rollback-injection tests.
- **Actor exclusion absolute** (never notify yourself); **dedupe** stands
  (O-21 default: re-mention does not re-arm).
- **Never content in `notifications.metadata`** or in `activity_log` — ids,
  enums, timestamps only (leak surface for backups, `33` §1/§8).
- Viewers **read but never post** (O-7 default) — enforced by the kernel,
  tested cross-user.
- 404-no-oracle on every path; recipients only see their own inbox — not
  platform admin, not workspace admin (O-6).
- Labels: `notification-fanout-too-large`, `too-many-comments` — new problem
  slugs under `kanpad.dev/problems/`, RFC 9457.
- Open calls O-7/O-12…O-21 defaults stand pending T027 — **if the review
  overrides any (it decides the permission matrix and fanout caps), amend this
  task before implementing.**
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Migrations green both engines; FK ordering rule (vs MCP 0004–0006)
      recorded in §Notes; never hand-edited `_sqlx_migrations`.
- [ ] Comment matrix green: create → 201 + Location; idempotent replay → 200
      same id; nesting rules (reply-to-reply → 400); 501st comment → 409;
      soft delete keeps thread intact with body destroyed; author-only edit;
      moderation delete (workspace admin) → 200 + activity row; plain member →
      403/404 per matrix. Closes **S5** (schema/API half; UI in T042).
- [ ] Mention validation: foreign-access member → 400 `errors[]`; unknown id →
      400; ≤20 enforced (21st → 400); valid multi-mention fanout co-commits.
- [ ] Fanout: `mention` + `comment` rows exact (assert via DB re-read + second
      GET); actor excluded; dedupe on double-mention (one row); 201 watchers →
      409 + **zero rows written** (tx proof); `shared` on grant wired (record
      second-landed outcome).
- [ ] Rollback injection: comment absent + activity absent when fanout fails.
- [ ] Inbox: `unread-count` matches DB COUNT; read/read-all/dismiss mutate;
      foreign id → 404; lazy purge deletes expired **read** rows, keeps unread
      (seed test); `GET /users?ids=` returns only shared-workspace users.
- [ ] Cascade: deleting a card/note removes its comments + mentions in the
      same tx (black-box test proves no orphans).
- [ ] Cross-user negative tests on every endpoint (B foreign card/note/inbox)
      — standing rule.
- [ ] T52 same-PR: contract + `30-db-schema.md` + `schema.d.ts` regen; both
      engines green; `sqlx prepare --check` clean; lesson appended to
      `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — include the 500-cap evidence, the
dedupe query shape, the 2nd-landed `shared`-fanout outcome, the FK decision
vs MCP, and the cascade-delete test evidence)

## Lessons

(pending)
