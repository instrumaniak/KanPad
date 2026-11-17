# 33-notifications — Wave 2 design: in-app notification inbox

> **Design-only** (no implementation in T027; implementation tasks are authored by taskpad
> after design review — `30-collaboration.md` §6 rows K/L). Constitution: `00-overview.md`.
> Sibling: `32-comments-mentions.md` (mentions fan out here), `31-realtime-sse.md`
> (`notification.*` events drive the badge), `30-collaboration.md` §5 (`activity_log`
> contract vs this table — do not conflate: **`activity_log` is the audit trail, `notifications`
> is a delivery surface**).
>
> *(Amendment 2026-11-16: new file. Notifications were undesigned — a mention without an
> inbox is a dead end; every reference product (Plane, GitLab, Trello, Liveblocks) pairs
> @mention with an in-app inbox. Email delivery is explicitly deferred.)*

## Goals (from the product plan)

"Who needs to know, right now, about what changed." Org-wide (100+ users): every user gets a
per-user, unread-first inbox with a header badge; mentions always notify; comments on
resources you watch notify you; you are never notified about your own action.

## Non-goals

Email/SMTP/digest delivery (future PRD "Vision" — the schema leaves a `channel` seam, §10),
push (web/mobile), webhooks, notification grouping/stacking UI (v1 renders flat; stacking
is a later view concern, §9 note), admin read-access to personal inboxes (privacy — platform
admin audits `activity_log`, not inboxes), notification content PII, async workers (all
in-tx, no queue — S4).

## 1. Domain model & schema

Block starts at **`0008_*`** (notifications after `0007_comments`); both engines; forward-only.

```sql
CREATE TABLE `notifications` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `recipient_user_id` INT NOT NULL,
  `actor_user_id` INT NULL,                      -- NULL = system/deleted actor
  `actor_key_id` INT NULL,                       -- MCP attribution (Wave 3), FK → api_keys SET NULL
  `via` ENUM('browser','mcp','cli') NOT NULL DEFAULT 'browser',
  `type` VARCHAR(16) NOT NULL,                   -- 'mention' | 'comment' | 'shared'
  `workspace_id` INT NULL,                       -- tenancy predicate for scoping reads
  `subject_type` VARCHAR(16) NULL,               -- 'card' | 'note' | 'board' | 'project' | 'workspace'
  `subject_id` INT NULL,
  `comment_id` INT NULL,                         -- set for mention/comment rows
  `metadata` JSON NULL,                          -- bounded; NO content ever (§5); reserved for future channel/prefs
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `read_at` DATETIME(6) NULL,
  INDEX `IDX_notifications_recipient_unread` (`recipient_user_id`, `read_at`, `id`),
  INDEX `IDX_notifications_recipient_created` (`recipient_user_id`, `created_at`),
  INDEX `IDX_notifications_subject` (`subject_type`, `subject_id`),
  PRIMARY KEY (`id`),
  CONSTRAINT `CHK_notifications_type` CHECK (`type` IN ('mention','comment','shared')),
  CONSTRAINT `FK_notifications_recipient` FOREIGN KEY (`recipient_user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
  CONSTRAINT `FK_notifications_actor` FOREIGN KEY (`actor_user_id`) REFERENCES `users` (`id`) ON DELETE SET NULL,
  CONSTRAINT `FK_notifications_comment` FOREIGN KEY (`comment_id`) REFERENCES `comments` (`id`) ON DELETE CASCADE
  -- no FK on subject_* — polymorphic; resolution at read time through authz
) ENGINE=InnoDB ...;

CREATE TABLE `notification_watches` (            -- subscribe/unsubscribe per resource
  `user_id` INT NOT NULL,
  `workspace_id` INT NOT NULL,
  `subject_type` VARCHAR(16) NOT NULL,
  `subject_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`user_id`, `subject_type`, `subject_id`),
  INDEX `IDX_watches_subject` (`subject_type`, `subject_id`),
  CONSTRAINT `CHK_watches_subject_type` CHECK (`subject_type` IN ('card','note','board','project')),
  CONSTRAINT `FK_watches_user` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB ...;
```

- **`metadata` is never used to carry content** — display text is resolved at read time by
  joining the subject *through authz* (§5). If the user lost access, the row renders
  "An item you no longer have access to" — never stale titles or bodies (no PII retention,
  no existence oracle to anyone, consistent with the 404 convention).
- **Auto-purge:** read notifications older than **90 days** are purged by a *lazy sweep* —
  every inbox-list request deletes ≤500 expired read rows for that recipient inside the
  request (single indexed DELETE, sub-millisecond), plus an optional documented manual
  purge runbook for bulk. **Unread rows are never purged.** No background jobs (S4).
- `notifications` is **not** referenced by `activity_log` — the two subsystems share only
  the transaction boundary. `activity_log` must stay free of it (audit ≠ delivery).
- **`actor_key_id` FK:** the column is a plain `INT NULL` here. The FK to `api_keys`
  (`ON DELETE SET NULL`) is added inline **if** MCP lands first, or by the migration that
  follows MCP's `0004_api_keys` otherwise — the exact FK declaration is fixed at task
  authoring time, when migration ordering is known (never hand-edit an applied version).

## 2. Fan-out rules (deterministic; same tx as the mutation)

| Trigger (mutation) | Notification type | Recipients |
|---|---|---|
| Comment created | `comment` | All `notification_watches` rows for the subject, **minus the actor** |
| Comment created | `mention` | Validated mention set (`32` §4), **minus the actor** |
| Being mentioned | `mention` | The mentioned user (already covered above; single insert) |
| Grant added (`POST …/members`, share endpoints) | `shared` | The newly-granted user, **minus the actor** |
| Card assigned (future — no assignee in schema) | `assigned` | — (column reserved for the future assignee feature; do not build) |

- **Actor exclusion** is absolute — you never get notified of your own actions.
- **Dedupe:** no two unread rows with same `(recipient_user_id, type, comment_id)` — enforced
  by an existence check inside the tx (portable, no MySQL-only syntax); race-safe because
  the tx serializes on the comment row (`SELECT ... FOR UPDATE` on the subject, consistent
  with the house locking pattern).
- **Recipient cap:** any fan-out > **200** recipients (pathological watch graph) is rejected
  with `409` `notification-fanout-too-large` — fail loudly, never a partial send.
- **Rate limits:** share/notify paths ride the global per-IP bucket plus the `32` comment
  buckets. No new governor surfaces in v1.
- All notification inserts happen **inside the mutation transaction** — either the comment
  and its inbox rows commit together, or nothing. Client perceives < 1 extra ms.

## 3. Watch model (`notification_watches`)

- **Auto-subscribe triggers:** creating a comment; being mentioned; receiving a `shared`
  grant (for that resource). **Auto-unsubscribe:** removing own watch (explicit toggle);
  losing access (membership removed — the watch row is soft-ignored by filtering on access,
  and can be hard-deleted by the membership teardown; see `30` §5).
- **Manual watch:** card/board/note "watch" toggle in the UI →
  `PUT /api/v1/watches/{subject_type}/{subject_id}` / `DELETE …` — idempotent, self-scoped,
  no extra authz beyond read access (read needed to watch — no existence oracle).
- **Watch ≠ grant:** watch is a read-side subscription, not access. Losing board access
  hides both event delivery and the watch toggle.

## 4. API surface (additive; T52 same-PR)

| Method & path | Purpose |
|---|---|
| `GET /api/v1/notifications?unread=&page&limit&type=` | `Page<T>` inbox; `unread=true` default filter toggle; `type` filter (`mention`/`comment`/`shared`) |
| `GET /api/v1/notifications/unread-count` | `{data: {unread: int}}` — drives the badge; indexed COUNT |
| `POST /api/v1/notifications/{id}/read` | Mark one read; `204`; foreign/missing id → 404 |
| `POST /api/v1/notifications/read-all` | Mark all read for caller; `204` |
| `DELETE /api/v1/notifications/{id}` | Dismiss (delete own row); `204` |
| `PUT/DELETE /api/v1/watches/{subject_type}/{subject_id}` | Watch/unwatch; `204` both |

- JSON snake_case; `Page<T>`; ids are `INT`; all endpoints session-authz-scoped by
  `recipient_user_id` predicate (no cross-user read, ever; 404-no-oracle).
- `unread-count` must be **one** indexed query: `SELECT COUNT(*) FROM notifications WHERE
  recipient_user_id = ? AND read_at IS NULL`.

## 5. Read-time resolution & rendering (no content coupling)

- List rows return ids + enum + timestamps only. The client renders display text by joining
  with data it already has or fetches lazily: actor email (from `users` via a sanitized
  `GET /api/v1/users?ids=` — **new endpoint, returns `{id, email}` only for users sharing a
  workspace with the caller**; never a global directory), subject title (from an authorized
  `GET` of the subject), board name.
- Rows whose subject is no longer accessible render as *"An item you no longer have access
  to"* — authz wins; the notification itself (timestamp, type) stays visible so the inbox
  never leaks by silence, but reveals nothing either.
- Deep link per row: `card` → `/boards/{board_id}?card={id}&comment={comment_id}` (resolve
  board via authorized join); `note` → `/notes/{id}`; `shared` → the granted resource.

## 6. Realtime integration (SSE — `31-realtime-sse.md`)

- The mutation tx inserts notification rows; post-commit the bus publishes
  `notification.new` with `workspace_id` set. Each receiving browser maps it to
  `invalidateQueries(['notifications', 'notifications-unread-count'])` — the badge updates
  within one frame. No payload beyond ids; the client renders from authorized reads.
- Fallback ladder (polling) applies equally — the badge is eventually-consistent, never
  wrong-by-more-than-10 s.

## 7. Frontend notes (expectations)

- Header bell (Radix `DropdownMenu`/`Popover`) with badge (`99+` cap), inbox panel with
  **All / Mentions** tabs (each with its own unread count — Plane-style), rows = actor
  avatar/email, one-line summary ("X mentioned you in card Y"), relative time, unread dot;
  row click marks read + deep-links with comment highlight (`32` §7).
- Per-row actions on hover: mark read/unread, dismiss. Footer actions: "Mark all as read".
- Settings: per-type mute — deferred to v2 (O-18); v1 ships no mutes.
- Skeletons on first paint; `unread-count` failure → badge hidden, never blocks the app.

## 8. Security & privacy

- Inbox is **private to its recipient** — no API returns another user's notifications; not
  even platform admin (admin audits `activity_log` — `30` §5, O-6). Workspace admins likewise.
- No content in rows → an inbox dump (backup leak) reveals only ids + timestamps.
- `actor_key_id` + `via` carried for parity with Wave 3 (an agent-written comment notifies
  a user with `via='mcp'`; UI shows "via agent key" tooltip — attribution without exposure).
- Watching is not a back-channel: watch on unreachable resource = no stream (§3).
- Purge is read-rows-only; `unread` survives indefinitely (durability of obligations).

## 9. Performance & capacity (100+ users)

- Inbox read: `(recipient_user_id, read_at, id)` → O(log n + page). Unread count: same index.
- Fan-out write cost: watchers per subject avg < 10 → < 1 ms typical; cap 200 worst case.
- Purge sweep ≤ 500 rows on inbox open — amortized O(1).
- `notification.*` bus events: O(viewers on workspace) — negligible.
- Growth: 100 users × ~50 comment-events/day ≈ 5k notification rows/day; 90-day read-purge
  bounds the table to ≲ 450k rows — well within a single indexed MySQL table. Partitioning
  deliberately **not** introduced (operational cost > benefit at this scale).
- Rendering cost client-side: lists of ≤ 100 rows; React Compiler handles memoization;
  infinite scroll optional (page-button parity with the house `Page<T>` UX is acceptable v1).

## 10. Contract & amendment targets (T52 same-PR)

- New: `notifications` + `notification_watches` tables (`specs/api-rewrite-rs-specs/30-db-schema.md`), the six
  endpoints + `GET /api/v1/users?ids=` (`specs/api-rewrite-rs-specs/20-api-contract.md`), `schema.d.ts` regen, problem
  slug `notification-fanout-too-large` (409), `notification` page types.
- Frontend: `frontend/CONVENTIONS.md` (inbox hook + badge placement), Radix popover usage.
- **Ordering:** collab M1 (workspaces) → `0007_comments` (`32`) → this file's block —
  forward-only, checksums, `sqlx prepare` regen in the same PR.
- **Email seam:** additive `channel ENUM('in_app') DEFAULT 'in_app'` can be added later with a
  dispatcher — no v1 coupling to SMTP; document seam in this file only.

## 11. Open calls (decide at T027 review)

- **O-17:** lazy inbox purge (default **yes**) vs no auto-purge at all.
- **O-18:** mute-able `comment` notifications (default: **defer to v2**; v1 ships no mutes).
- **O-19:** `shared` notifications on workspace membership grant (default **yes** — it
  closes the "who added me?" loop).
- **O-20:** badge shows exact count vs capped `99+` (default **99+ cap**).
- **O-21:** mention notifications re-arm on re-mention (default **no** — dedupe stands).

## 12. Test plan (TDD)

- Mutation → inbox row co-committed: create comment → `GET /api/v1/notifications` for the
  mentioned user shows exactly one `mention`; rollback injection (fail `notifications`
  insert) → comment absent (no orphans).
- Actor exclusion: actor never appears in own fan-out. Dedupe: double @mention in one body →
  one row. Watch: watcher without mention receives `comment` row; watcher who can't read
  (access revoked) receives none.
- `unread-count` matches DB count; `read`/`read-all`/`dismiss` mutate correctly; re-read
  404s on foreign ids (no oracle).
- Fan-out cap: subject with 201 watchers → 409, zero rows written (tx proof).
- Purge: seed read rows older than 90 days + recent unread; list → old-read rows gone,
  unread preserved; DB-assert via query.
- SSE integration (with `31`): `notification.new` observed by recipient's stream; non-recipient
  stream silent. E2E: A comments on watched card → B badge increments within 5 s → B opens →
  deep link scrolls with highlight.
