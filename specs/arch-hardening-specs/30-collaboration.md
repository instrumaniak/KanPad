# 30-collaboration — Wave 2 design (S1): workspaces, sharing, authorization

> **Design-only.** Implementation tasks (T028+) are written after this design is
> reviewed and approved (T027). The system today is architecturally single-user
> (`user_id` ownership on every table; no team/share/member/permission concept; admin =
> one CLI-minted superadmin, no admin panel, no activity log; last-write-wins). This
> file is the target model so implementation can be delegated task-by-task without
> re-litigating decisions. Constitution: `00-overview.md`.

## Goals (from the product plan)

Org-wide (100+ users) cross-functional collaboration: employees and teams share boards,
projects and notes for project management, product development and brainstorming —
with governance (admin panel, activity log) and safe concurrent editing.

## Non-goals (this design)

Real-time/WebSocket (deferred — polling adequate; see `00-overview.md` out-of-scope),
file attachments, per-resource granular roles beyond the three below, orgs-below-workspaces
hierarchy, external/guest accounts, SSO (record as future work — self-hosted orgs often
want OIDC later; keep the session model pluggable).

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
  top-level (top-level notes belong to a workspace — `workspace_id` nullable column or
  membership via `created_by`'s workspace; decide in T027 review: default =
  add `workspace_id` nullable to `notes`, personal-workspace backfill).
- **Cards/checklists/labels/tags** stay scoped through their parent (board/column) — no
  per-card ACL (YAGNI at this scale; card visibility = board visibility).
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
> workspace-owned resources — **open call O-1**: default = workspace `member` can edit
> everything in a *shared* workspace; `viewer` only via explicit grant).

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
   behind `REGISTRATION`-style config flag? No flag — per-route, each task ships its own
   tests** (simpler; rollback = restore backup per cutover doctrine).
4. **Contract:** nothing to drop initially (creator stays as data). `user_id` columns
   keep meaning "created_by" (rename is cosmetic — **open call O-2**: rename to
   `created_by` in a later cleanup migration vs keep; default = keep, document).

Every migration: MySQL 8.4 + MariaDB 10.6 green, checksum-safe (never hand-edit
`_sqlx_migrations`), `sqlx prepare` regen, T52 contract pass if wire shapes change.

## 4. Concurrency (off last-write-wins)

- **Optimistic concurrency** on cards and notes (the two objects two people edit):
  add `version INT NOT NULL DEFAULT 1` (or use `updated_at` CAS). Requests that mutate
  content send `If-Match`-style `expected_version` (or `updated_at`); mismatch →
  **409** with the current resource in the problem+json `errors` (or a dedicated
  `conflict` problem type — T52 decision). Bump `version` on every content write.
- Drag/move operations (position-only) stay last-write-wins — positions are
  commutative-ish with T010's fractional keys and concurrent same-gap drops are
  acceptable (documented in T010 D3).
- **Frontend:** on 409, refetch + show "updated by someone else" toast with reload —
  no silent overwrite. Wire shape additive (optional `expected_version` → default
  behavior unchanged for single-user until clients opt in).
- Activity log (§5) doubles as the conflict-audit trail.

## 5. Admin panel + activity log (FR43–49)

- **Activity log:** append-only `activity_log` table (`id, workspace_id NULL,
  actor_user_id, action VARCHAR, subject_type, subject_id, metadata JSON/BLOAT-free
  TEXT, created_at`, indexes on `(workspace_id, created_at)`, `(subject_type,
  subject_id)`). Written **inside the same transaction** as the mutation for the
  high-value actions (create/update/delete/move on boards/columns/cards/notes,
  member changes, auth events). No background jobs (S4) — synchronous insert.
  Retention: no auto-purge initially (org audit needs durability); document a manual
  purge runbook.
- **Admin API + UI:** `GET /api/v1/admin/users` (paged), `PATCH …/:id` (block/unblock,
  role), `GET /api/v1/admin/activity` (filters), registration toggle (exists in config —
  expose an endpoint if not present). Platform-admin guard = existing `Role::Admin`
  check (extend `authz` with `require_platform_admin`).
- **Frontend:** `features/admin/` per the original architecture.md structure (admin
  layout, user list, actions, activity log) — route-guarded by role.

## 6. Task decomposition (T028+ — to be written after T027 review)

Expected task shape (each = one agent session, TDD, cross-user negative tests):

| Task | Content | Depends |
|------|---------|---------|
| T028 | M1+M2 migrations (tables, backfill, personal workspaces) + schema spec amendments (`30-db-schema.md`) | T027 |
| T029 | `authz` kernel + extractors + list-filter helpers + unit tests | T028 |
| T030 | Enforce on boards + columns + cards routes/lists (JOIN filters, cross-user tests) | T029 |
| T031 | Enforce on projects + notes + tags/checklists/labels | T030 |
| T032 | Membership management API (`POST/DELETE/GET /workspaces/:id/members`, board/project share endpoints) + invite-less direct-add UX | T029 |
| T033 | Optimistic concurrency (version columns, 409 flow, frontend conflict UX) | T030 |
| T034 | Activity log (table, synchronous writes, read API, admin UI panel) | T030 |
| T035 | Admin panel (user management, registration toggle endpoint, role guards) + frontend | T034 |
| T036 | Frontend sharing UX (member picker, share dialogs, workspace switcher?) + cache-key scoping | T032 |

(Numbering provisional — actual IDs assigned when written; they continue from T027.)

## 7. Open calls (decide at T027 review)

- **O-1:** workspace `member` default edit vs per-resource explicit grants only.
  *Default: member can edit workspace resources.*
- **O-2:** rename `user_id` → `created_by` later vs keep. *Default: keep.*
- **O-3:** notes ownership for top-level notes (workspace_id vs created_by-only).
  *Default: add nullable `workspace_id`.*
- **O-4:** 409 conflict payload shape (problem+json extension vs `errors` map) — T52.
- **O-5:** workspace switcher UI vs implicit "my workspace + shared with me" list.
  *Default: sidebar sections, no switcher in v1.*
- **O-6:** platform-admin read of all content (needed for moderation?) vs activity-log
  only. *Default: activity-log + user list only.*
