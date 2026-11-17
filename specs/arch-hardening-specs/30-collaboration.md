# 30-collaboration — Wave 2 design (S1): workspaces, sharing, authorization

> **Design-only.** Implementation tasks are **T031–T042** (rows A–L, authored
> 2026-11-16 ahead of the review at user request — IDs assigned by taskpad at
> authoring time; T028/T029/T030 already exist: storybook, SQL observability,
> Sentry/OTel). They are **not started**; the design review (T027) still gates
> implementation and any O-7…O-21 override applies back to T031–T042 before work
> begins. The system today is architecturally
> single-user (`user_id` ownership on every table; no team/share/member/permission concept; admin =
> one CLI-minted superadmin, no admin panel, no activity log; last-write-wins). This
> file is the target model so implementation can be delegated task-by-task without
> re-litigating decisions. Constitution: `00-overview.md`.
>
> Sibling designs (D-7, 2026-11-16 — scope promoted from "deferred" to designed Wave 2
> features): realtime invalidation → `31-realtime-sse.md`; comments & mentions →
> `32-comments-mentions.md`; in-app notification inbox → `33-notifications.md`. Those
> files are the single owners of their designs; this file remains the owner of the domain
> model, authz kernel, migration strategy, concurrency and the activity log.
> *(Amendment 2026-11-16, pass 3: new sibling files + activity-log hardening in §5.)*

## Goals (from the product plan)

Org-wide (100+ users) cross-functional collaboration: employees and teams share boards,
projects and notes for project management, product development and brainstorming —
with governance (admin panel, activity log) and safe concurrent editing.

## Non-goals (this design)

Email notification delivery (in-app only — `33`; email/digest is a documented future seam),
file attachments, per-resource granular roles beyond the three below, orgs-below-workspaces
hierarchy, external/guest accounts, SSO (record as future work — self-hosted orgs often
want OIDC later; keep the session model pluggable), WebSockets/CRDT editing (D-7 chose SSE
+ cache invalidation — see `31`), card assignees (cards have none today; notification
fanout uses the watch model — `33`), note-body @mentions. Realtime itself is **in
scope** as of D-7 (`31-realtime-sse.md`).

## 1. Domain model

### 1.1 Core entities (additive; `user_id` becomes *creator*, not sole owner)

```
workspaces            id, name, slug UNIQUE, created_by → users, created_at, updated_at
workspace_members     workspace_id, user_id, role ENUM('owner','admin','member'),
                      UNIQUE(workspace_id, user_id), INDEX(user_id)
board_members         board_id, user_id, role ENUM('viewer','editor'),
                      UNIQUE(board_id, user_id), INDEX(user_id)
project_members       project_id, user_id, role ENUM('viewer','editor'),
                      UNIQUE(project_id, user_id), INDEX(user_id)
```

- **Every user gets a personal workspace on register** (auto-created, they are
  `owner`) — single-user behavior is unchanged: existing `user_id`-owned rows are
  associated to the creator's personal workspace during migration (§3).
- **Notes** follow their container (board/project) or are visible to the workspace when
  top-level (top-level notes belong to a workspace — add `workspace_id` nullable to
  `notes`, personal-workspace backfill) — **O-3 resolved 2026-11-16**.
- **Cards/checklists/labels/tags** stay scoped through their parent (board/column) — no
  per-card ACL (YAGNI at this scale; card visibility = board visibility).
- **Notes are commentable** (`comments.subject_type='note'`, see `32-comments-mentions.md`);
  comments follow access to the note itself, not its container.
- **Mentions** in comments are the only @mention surface; note bodies are not expanded.
- **Notification/watch rows** are per-user inbox state (`33-notifications.md`) — strictly
  separate from `activity_log` (audit) and from presence (ephemeral — `31-realtime-sse.md` §7).
- **Roles:** workspace `owner` (delete workspace, transfer), `admin` (manage members,
  read all workspace resources), `member` (default). Resource `editor`/`viewer`
  overrides/adds access for explicitly-shared resources.
- **Admin (platform):** keep the existing `users.role = 'admin'` superadmin (T12 CLI)
  for platform-level ops (registration toggle, user blocking) — distinct from workspace
  roles.

### 1.2 Access rule (the one sentence)

> A user may read a resource iff they are: its `created_by`, a member of its workspace
> (any role), or an explicit `*_members` grant on it (or on a parent — board grant
> covers its columns/cards/notes). Write requires `editor` (or workspace `member`+ on
> workspace-owned resources — **O-1 resolved 2026-11-16**: workspace `member` can edit
> shared workspace resources; `viewer` only via explicit grant).

Platform `admin` gets read for governance (activity log, user list) — **not** silent
write (auditability).

## 2. Authorization kernel (the chokepoint)

**One shared helper, used by every store** — never per-route ad-hoc checks (the review's
core maintainability requirement):

```
backend-rs/src/common/authz.rs (new)
  pub enum Perm { Read, Write, Admin }
  pub async fn require(conn, actor: &Actor, resource: ResourceRef, perm: Perm)
      -> Result<(), AppError>
  // ResourceRef = enum { Workspace(id), Board(id), Project(id), Note(id), ... }
  // resolves membership with ONE indexed query per check (batch variant for lists)
```

- **404 vs 403:** unauthorized *read* → **404** (existence hiding, matches current
  `AppError::NotFound` convention); authenticated-but-insufficient *write* → **403**
  (`AppError::Forbidden` exists). Document this in `20-api-contract.md` amendments.
- **List endpoints** filter by membership in SQL (`JOIN workspace_members …`) — no
  post-filtering in Rust (scalability: filter must be in the WHERE clause, indexed by
  `workspace_members.user_id` / `board_members.user_id`).
- **Extractors:** extend the existing `AuthUser`/session extractor with an
  `authorize(...)` method so route handlers stay one-liners; validation/errors reuse
  the problem+json renderer (no new problem types unless RFC requires).
- **Test rule:** every endpoint gets a **cross-user negative test** (user B → 404/403
  as appropriate) — this is the enforcement proof and becomes a CI grep-able pattern.
- **Applies to the new Wave 2 surfaces too** (D-7): comment/notification stores call
  `authz::require` with the same `ResourceRef` variants (`Note(id)`, `Card(id)`); the SSE
  connect handshake resolves the visible workspace set via the same indexed membership
  query (`SELECT workspace_id FROM workspace_members WHERE user_id = ?`); mention
  validation (`32` §4) uses the same rule for "can this user read the subject"; the
  presence heartbeat (`31` §7) re-uses board-read authz on every ping.
- **Membership changes** publish `member.revoked`/`member.added` bus events (`31` §5) and
  are written to `activity_log` in the same transaction (§5 below) — access removal is
  therefore enforced for live connections, list queries and mentions alike.

## 3. Migration strategy (expand → dual-read → enforce → contract)

Forward-only sqlx migrations, backup-gated (T011 makes the backup mechanical):

1. **Expand (M1):** create the four tables + indexes. No behavior change.
2. **Backfill (M2):** create a personal workspace per existing user; map existing
   `user_id`-owned rows to it (no row rewrites needed if access derives from
   `created_by` — decide: derive vs add `workspace_id` columns. **Default: add
   `workspace_id` to `boards`/`projects`/`notes`** (nullable → backfilled →
   non-null for new rows) so list filtering is a single indexed join, not an
   OR across `created_by`/membership.)
3. **Enforce (M3+tasks):** route-by-route: lists gain membership JOINs; detail/mutations
   call `authz::require`. Dual-read not strictly needed if enforce lands per-route with
   tests — **default: per-route enforcement with cross-user tests, single release
   per route, no config flag** (simpler; no flag drift). Rollback = redeploy of the
   previous release — the authz change is behavioural, not data;
   backup-restore stays the last-resort rollback per cutover doctrine, not the
   routine one.
4. **Contract:** nothing to drop initially (creator stays as data). `user_id` columns
   keep meaning "created_by" (rename is cosmetic — **O-2 resolved 2026-11-16: keep**, document).

Every migration: MySQL 8.4 + MariaDB 11.4 green (CI matrix legs), checksum-safe
(never hand-edit
`_sqlx_migrations`), `sqlx prepare` regen, T52 contract pass if wire shapes change.
If the T011 drill confirms prod is MariaDB 10.6, the collaboration set also gets a
10.6 verification leg or a written acceptance of the delta (see `40-operations.md` §6).

## 4. Concurrency (off last-write-wins)

- **Optimistic concurrency** on cards and notes (the two objects two people edit):
  add `version INT NOT NULL DEFAULT 1` (or use `updated_at` CAS). Requests that mutate
  content send `If-Match`-style `expected_version` (or `updated_at`); mismatch →
  **409** with a dedicated **`conflict` problem+json** carrying the current resource —
  **O-4 resolved 2026-11-16** (extension of problem+json, not an `errors[]` map;
  amended into `20-api-contract.md` via T52). Bump `version` on every content write.
- Drag/move operations (position-only) stay last-write-wins — positions are
  commutative-ish with T010's fractional keys and concurrent same-gap drops are
  acceptable (documented in T010 D3).
- **Frontend:** on 409, refetch + show "updated by someone else" toast with reload —
  no silent overwrite. Wire shape additive (optional `expected_version` → default
  behavior unchanged for single-user until clients opt in).
- Activity log (§5) doubles as the conflict-audit trail.

## 5. Admin panel + activity log (FR43–49)

- **Activity log:** append-only `activity_log` table:

  ```
  activity_log          id, workspace_id NULL, actor_user_id NULL→SET NULL,
                        actor_key_id INT NULL, via ENUM('browser','mcp','cli') NOT NULL DEFAULT 'browser',
                        action VARCHAR(48), subject_type VARCHAR(16), subject_id INT NULL,
                        changes JSON NULL, metadata JSON NULL,
                        request_id VARCHAR(64) NULL,
                        ip_hash CHAR(64) NULL,
                        created_at
  Indexes: (workspace_id, created_at), (subject_type, subject_id, created_at),
           (actor_user_id, created_at)
  FK: actor_key_id → api_keys ON DELETE SET NULL; actor_user_id → users ON DELETE SET NULL
  ```

  The `actor_key_id`/`via` pair is the Wave 3 attribution contract —
  `specs/mcp/60-security.md` §4 and `specs/mcp/91-task-map.md` §4 require it;
  without them an agent's write is attributed to the human and row G cannot
  satisfy the MCP audit trail. `metadata` and `changes` are **native JSON, never TEXT**
  (amendment 2026-11-16, pass 2: `actor_key_id` + `via` added so Wave 3's MCP surface is
  attributable from day one; pass 3: `request_id`, `ip_hash`, `changes` added per audit-log
  best practices — every reference implementation records who/when/what-changed with a
  request correlation id; silent changes are the enemy of dispute resolution):
  - `changes` = `{"<field>": {"before": <scalar>, "after": <scalar>}, …}` for scalar fields
    (title, is_archived, due_date, status-like values…); for free-text (descriptions, note
    bodies, comment bodies) **field names only** in `changes` — values would bloat the table
    and leak content into the audit surface (aligned with MCP's "no free-text values" rule).
  - `request_id` = the `x-request-id` propagated through the stack (never null in HTTP
    mutations; NULL for CLI).
  - `ip_hash` = SHA-256 of the client IP with an app-level salt (never the raw IP; auth
    events may log the raw IP *in addition*, per `specs/security/09-logging-monitoring-ir.md`,
    under the same retention rules).
  - Written **inside the same transaction** as the mutation for the high-value actions
    (create/update/delete/move on boards/columns/cards/notes/**comments**, member changes,
    watch/share changes, auth events). Same rule for `comment.create/update/delete` and
    notification fanout (`32` §6, `33` §2). No background jobs (S4) — synchronous insert.
  - **Append-only at the DB level:** a `BEFORE UPDATE`/`BEFORE DELETE` trigger on
    `activity_log` raises `SIGNAL SQLSTATE '45000'` (works on MySQL 8.4 and MariaDB 11.4);
    the app role gets `SELECT, INSERT` only (operations runbook documents the `GRANT` and
    the separate migration/purge roles). Application discipline alone is not a control.
  - `id` doubles as the **SSE resume cursor** (`id:` frames — `31-realtime-sse.md` §2/§4);
    every mutation row therefore exists before any frame referencing it is published.
  - Retention: no auto-purge initially (org audit needs durability); document a manual
    purge runbook. Notifications (`33`) purge separately and may not touch this table.
- **Read APIs (all authz-scoped):**
  - `GET /api/v1/admin/activity` (platform admin — filters: actor, action, subject,
    date range; paged) — admin panel (row H).
  - `GET /api/v1/boards/{id}/activity` and `GET /api/v1/workspaces/{id}/activity` —
    viewer+ on the resource; powers the "who changed this" board panel (`31-realtime-sse.md` §8).
- **Admin API + UI:** `GET /api/v1/admin/users` (paged), `PATCH …/:id` (block/unblock,
  role), `GET /api/v1/admin/activity` (filters), registration toggle (exists in config —
  expose an endpoint if not present). Platform-admin guard = existing `Role::Admin`
  check (extend `authz` with `require_platform_admin`).
- **Frontend:** `features/admin/` per the original architecture.md structure (admin
  layout, user list, actions, activity log) — route-guarded by role. Board-level
  activity panel renders the "recent activity" feed next to the board view; both consume
  the same read APIs (no duplicate data paths).

## 6. Task decomposition (T031–T042, authored 2026-11-16)

Expected task shape (each = one agent session, TDD, cross-user negative tests).
**Amendment 2026-11-16 (early authoring):** rows are labeled A–L and have landed as
**T031 (A) … T042 (L)**, wired in `status.yaml` with the normative dependency edges
below (T028/T029/T030 already exist: storybook, SQL observability, Sentry/OTel).
The collab set normally would have started at T031 — and did. The relative order and
the dependency edges are normative, not the labels. Rows J–L are new (D-7): realtime
invalidation, comments/mentions/notifications, and their UI. Note: T041/T042 may
overstep the one-session bound and are allowed to land as two sequential PRs under
one task (recorded in the task files).

| # | Content | Depends on |
|---|---------|-----------|
| A | M1+M2 migrations (tables, backfill, personal workspaces) + schema spec amendments (`30-db-schema.md`) | T027 |
| B | `authz` kernel + extractors + list-filter helpers + unit tests | A |
| C | Enforce on boards + columns + cards routes/lists (JOIN filters, cross-user tests) | B |
| D | Enforce on projects + notes + tags/checklists/labels | C |
| E | Membership management API (`POST/DELETE/GET /workspaces/:id/members`, board/project share endpoints) + invite-less direct-add UX | B |
| F | Optimistic concurrency (version columns, 409 flow, frontend conflict UX) | C |
| G | Activity log (table, DB-level append-only trigger, synchronous writes, read APIs incl. board/workspace activity, `request_id` + `ip_hash` + `changes`) — carries `actor_key_id` + `via` per §5 (Wave 3 attribution); must say so in the task ACs. SSE resume cursor (`id`) is wired here | C |
| H | Admin panel (user management, registration toggle endpoint, role guards) + frontend | G |
| I | Frontend sharing UX (member picker, share dialogs, workspace switcher?) + cache-key scoping | E |
| J | Realtime SSE transport (`31`): `GET /events`, in-process broadcast bus, membership-gated fanout + revocation, `Last-Event-ID` resync, heartbeat, presence endpoints (`POST /presence/heartbeat`), metrics (`kanpad_sse_*`), LiteSpeed/proxy runbook entries | B, G |
| K | Comments + mentions + notifications backend (`32`, `33`): two comment tables + mention validation + in-tx notification fanout (mention/comment/shared), inbox read APIs, `unread-count`, watch/unwatch, mention rate limits, `client_token` idempotency; all in one tx with `activity_log` | B, C, D, J |
| L | Frontend for J/K (`32` §7, `33` §7, `31` §6–7): `useWorkspaceEvents` hook + fallback ladder, `['presence', boardId]` avatar stack, comment thread + `@` composer dropdown (Radix), mention chips, inbox badge + All/Mentions panel, deep-link `?card=…&comment=…` scroll+highlight | E, I, J, K |

Sequencing: **A → B → {C → {D, F, G → H}, E → I}, with J after G, K after {C, D, J},
and L after {E, I, J, K}.** A precedes all; B gates C/E/J; C gates D/F/G; G gates H and
J (J publishes the `id` cursor from `activity_log`); D+C gate K (comment stores call the
live authz kernel); K and J gate L; E gates I, and I gates L.

## 7. Open calls

### Resolved at review (2026-11-16, user sign-off)

- **O-1:** workspace `member` default edit vs per-resource explicit grants only.
  **→ RESOLVED: member can edit workspace resources** (personal workspaces remain
  single-user; shared workspaces trust members; `viewer` only via explicit grant).
- **O-2:** rename `user_id` → `created_by` later vs keep. **→ RESOLVED: keep** `user_id`;
  document that it reads as *creator* post-Wave-2.
- **O-3:** notes ownership for top-level notes. **→ RESOLVED: add nullable `workspace_id`**
  to `notes` (backfill to personal workspace).
- **O-4:** 409 conflict payload shape. **→ RESOLVED: problem+json extension** (`conflict`
  problem type carrying the current resource — see `20-api-contract.md` amendment, T52);
  not an `errors[]` map (that shape is for validation).
- **O-5:** workspace switcher UI vs implicit list. **→ RESOLVED: sidebar sections**
  ("My boards" / "Shared with me"), no switcher in v1.
- **O-6:** platform-admin read of all content vs activity-log only. **→ RESOLVED:
  activity-log + user list only** (no silent content reads; moderation goes through
  workspace admins).

### Assigned in the sibling designs (decide at T027 review)

IDs are contiguous across the four files; **no renumbering** — each call is owned where
its design lives.

| ID | File | Question (default) |
|----|------|--------------------|
| O-7 | `32` §3 | Viewer may read comments but not post — **yes** |
| O-8 | `31` §13 | Presence visible to `viewer` board guests — **yes** |
| O-9 | `31` §13 | Stream carries `admin.*` frames — **no** |
| O-10 | `31` §13 | Compression exclusion at route level, not middleware — **route level** |
| O-11 | `31` §13 | Per-user SSE connection cap 6 — **6** |
| O-12 | `32` §11 | Edit own comment anytime (no 15-min window) — **yes** |
| O-13 | `32` §11 | `client_token` idempotency on comment create — **yes** |
| O-14 | `32` §11 | Inbox All/Mentions tabs — **All/Mentions** |
| O-15 | `32` §11 | Comment cap per subject 500 — **yes** |
| O-16 | `32` §11 | Moderation-delete copy — **"deleted by a moderator"** |
| O-17 | `33` §11 | Lazy inbox purge of read rows > 90 days — **yes** |
| O-18 | `33` §11 | Per-type mute for comment notifications — **v2, defer** |
| O-19 | `33` §11 | `shared` notification on membership grant — **yes** |
| O-20 | `33` §11 | Badge capped at `99+` — **yes** |
| O-21 | `33` §11 | Re-mention re-arms a notification — **no (dedupe stands)** |
