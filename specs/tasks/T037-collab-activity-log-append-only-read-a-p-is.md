# T037: Collab: activity log (append-only + read APIs)

> **STATUS: pending.** Phase 2. Wave 2 / row G. The audit trail for Wave 2 —
> `id` doubles as the **SSE resume cursor** (T040 relies on this: no frame may
> reference an event whose row does not exist). Carries the Wave 3 attribution
> contract (`actor_key_id` + `via`) per `30` §5 / `specs/mcp/60-security.md` §4.

## Goal

Land `activity_log` end-to-end per `30-collaboration.md` §5:

1. **Table (own migration; numbering per tree at authoring time — never reuse or
   hand-edit an applied version; never collide with MCP's reserved 0004–0006)** —
   ```
   activity_log  id, workspace_id NULL, actor_user_id NULL→SET NULL,
                 actor_key_id INT NULL (FK → api_keys ON DELETE SET NULL only if
                                      MCP's 0004 has landed; otherwise plain INT NULL now,
                                      FK added by the following migration once `api_keys` exists),
                 via ENUM('browser','mcp','cli') NOT NULL DEFAULT 'browser',
                 action VARCHAR(48), subject_type VARCHAR(16), subject_id INT NULL,
                 changes JSON NULL, metadata JSON NULL,
                 request_id VARCHAR(64) NULL, ip_hash CHAR(64) NULL, created_at
   Indexes: (workspace_id, created_at), (subject_type, subject_id, created_at),
            (actor_user_id, created_at)
   ```
   `metadata`/`changes` are **native JSON, never TEXT**. `actor_key_id` lands as
   plain `INT NULL` if MCP's 0004 has not landed when this migration is authored;
   the FK is added by a subsequent migration — never hand-edit an applied version.
2. **Append-only at the DB level:** `BEFORE UPDATE`/`BEFORE DELETE` trigger →
   `SIGNAL SQLSTATE '45000'` (MySQL 8.4 + MariaDB 11.4 compatible); documented
   `GRANT SELECT, INSERT` for the app role with separate migration/purge roles —
   documented in `docs/operations-backup.md` (T011's deliverable; the single owner of
   migration-adjacent ops docs — never a second copy elsewhere). This task extends or
   creates that file's DB-role section with the GRANT text; if T011 has not landed yet,
   create the file with this section only — T011 appends the backup procedure/drill
   record when it runs.
3. **Write helper** (`common::activity::log(conn, …)`) — synchronous, **inside
   the mutation transaction**, for the high-value actions: create/update/delete/
   move on boards/columns/cards/notes, member/share/watch changes, comment
   create/update/delete (call sites added by T041; see retrofit rule below),
   auth events (login success/failure, logout — `specs/security/09-logging-
   monitoring-ir.md` alignment, raw-IP allowance there only).
   - `changes` = `{"<field>": {"before": <scalar>, "after": <scalar>}}` for
     scalars; **field names only** for free-text (bodies/descriptions) — never
     content. `metadata` carries extras (e.g. `mentioned: <n>`).
   - `request_id` = `x-request-id` (already propagated stack-wide via tower-http
     layers — read it from the request extensions; **never null in HTTP
     mutations**; NULL for CLI).
   - `ip_hash` = SHA-256(client IP + app-level salt). New config
     `ACTIVITY_IP_HASH_SALT` in `.env.example` (generate with
     `openssl rand -hex 32`; rotate = re-baseline hashing, documented).
4. **Read APIs (all authz-scoped, paged per `Page<T>`):**
   - `GET /api/v1/admin/activity` (platform admin; filters: actor, action,
     subject_type, from, to) — feeds T038.
   - `GET /api/v1/boards/{id}/activity` (viewer+; filters activity to the
     board's subject chain; ordered `created_at DESC`) — feeds the board
     panel (T042).
   - `GET /api/v1/workspaces/{id}/activity` (admin/owner+; filters actor,
     action, subject_type, from, to) — feeds T038 / workspace governance.
5. **Cursor contract:** every mutation inserts the row **before** any SSE frame
   referencing its `id` is published (T040 publishes post-commit); presence and
   notifications never touch this table (overlap rule: audit ≠ delivery).
6. **Retrofit rule:** this task wires the helper into boards/columns/cards
   mutations (T033's enforced surface) + auth events + membership/share/watch
   mutations (seam left by T035). **Projects/notes mutations are wired by the
   second of {T034, T037} to land** — record which was the case in §Notes; if
   T034 landed first, do the projects/notes retrofit in this task; otherwise
   T034 does it.

## Depends On

T033 (C) — the board surface is enforced first; the retrofit edits those
handlers. (`30` §6 edge: G after C.)

## Phase

2

## Critical

Yes — gates T040 (J) (the bus's `activity_log.id` cursor) and T038 (H) (admin
activity view).

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §5 (activity log — full
  contract: schema, append-only trigger, same-transaction writes, read APIs,
  admin panel, `request_id`/`ip_hash`/`changes`, `actor_key_id`/`via`, retention
  = no auto-purge v1 + manual purge runbook note, **O-6: platform admin reads
  activity/user list only**).
- `specs/arch-hardening-specs/31-realtime-sse.md` §2/§4 (`activity_log.id` is
  the resume cursor; `Last-Event-ID`; no per-row replay — `resync` frame).
- `specs/arch-hardening-specs/32-comments-mentions.md` §6 (comment/notification
  co-commit rule — same tx) and `33-notifications.md` §1 (notifications are
  never referenced by `activity_log`).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1**; global DoD.
- `specs/api-rewrite-rs-specs/20-api-contract.md` — **amendment target** (T52):
  three new endpoints, `activity_log` row shape, additive only.
  `30-db-schema.md` — **amendment target**: table + trigger.
- `specs/mcp/60-security.md` §4 + `specs/mcp/91-task-map.md` §4 (attribution
  contract this table must satisfy for Wave 3); `specs/mcp/90-open-calls.md`.
- `backend-rs/src/lib.rs` (request-id layers: `SetRequestIdLayer` +
  `PropagateRequestIdLayer` already echo `x-request-id`),
  `backend-rs/src/error.rs` (instance = `{METHOD} {path}#{x-request-id}`),
  `backend-rs/src/extract.rs`, `backend-rs/tests/README.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/000N_collab_activity_log.sql` | **NEW** — table + indexes + `BEFORE UPDATE`/`BEFORE DELETE` triggers (`SIGNAL SQLSTATE '45000'`), both engines |
| `backend-rs/src/common/activity.rs` | **NEW** — `log()` helper, `IpHash` (SHA-256 + salt), `changes` builder (scalar before→after; names-only for free-text) |
| `backend-rs/src/common/mod.rs` | export |
| `backend-rs/src/extract.rs` | read `x-request-id` from extensions for the helper (request-id already echoed by T-layers) |
| `backend-rs/src/features/boards|columns|cards/{routes.rs,store.rs}` | retrofit: co-insert activity rows inside the mutation tx |
| `backend-rs/src/features/auth/` (login/logout) | auth events (login_success/failure, logout) — ip_hash + request_id |
| `backend-rs/src/features/members/` (T035 seam) | member/share/watch change events (2nd-landed rule) |
| `backend-rs/src/features/activity/` (NEW) or `features/admin/activity` | the three read endpoints |
| `.env.example` | `ACTIVITY_IP_HASH_SALT` (documented; never a real value) |
| `docs/operations-backup.md` | **EXTEND or CREATE** (T011's file — single owner): the DB-role section with `GRANT SELECT, INSERT` + migration/purge role split |
| `specs/api-rewrite-rs-specs/{20-api-contract.md,30-db-schema.md}` | **AMEND** (T52 same-PR) |
| `backend-rs/tests/api/activity_log.rs` | **NEW** — black-box: mutation→row co-commit (rollback on failure), append-only trigger rejects UPDATE/DELETE, read-API authz matrix, ip_hash/request_id present, no content leakage in `changes` |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** comments wiring (T041), SSE publish/bus (T040 —
this task records the ordering contract, not the publisher), notifications
table (T041), retention purge job (manual runbook only), admin UI (T038).

## Implementation Steps

1. **Migration first** (additive): table + indexes + both triggers; verify
   trigger fires on both engines (black-box test issues UPDATE/DELETE as the
   app role → expect SQLSTATE 45000 error surfaced as 500 problem+json *in
   production path never reachable via API* — assert at the SQL level in the
   test, not through a route).
2. **Helper**: `common::activity::log(conn, workspace_id, actor, action,
   subject_type, subject_id, changes, metadata, request_id, ip_hash)` — takes a
   `&mut sqlx::Transaction` (or the executor) so it **cannot** run outside the
   caller's tx; actor may be `None` (system/CLI) with `NULL` actor; builds
   `changes` only from explicitly-passed field pairs (compile-time resistance to
   stuffing free-text values in).
3. **Retrofit boards/columns/cards** (T033 surface, post-enforcement): every
   mutating handler calls the helper inside its existing transaction; scalar
   diffs only (title, is_archived, due_date, sort positions: before/after as
   sort_key strings? — sort_key is derived, log `moved` actions with
   before/after column ids instead, no free positions).
4. **Auth events**: login success/failure, logout — `action: 'auth.login'` etc.,
   `ip_hash` mandatory; never the password/email in `changes`; tenant-agnostic
   (`workspace_id = NULL`).
5. **Membership/share/watch retrofit** (T035 seam; 2nd-landed rule for
   projects/notes) — same helper.
6. **Read APIs** per §5 list above with the T032 authz kernel; paged, filtered,
   404-no-oracle; order `created_at DESC`; `ip_hash`/`request_id` returned only
   to platform admin (governance need) and board/workspace admins — not to
   viewers (avoid leaking infra detail); document this in the contract.
7. **Contract** (T52): three endpoints + `activity_log` row shape + problem
   slug if any new one (none expected); OpenAPI + `schema.d.ts` regen same-PR.
8. Both engines; `sqlx prepare --check`; persistence re-read rule on all
   mutation tests; lesson.

## Constraints

- **Same-transaction or nothing** — no background jobs (S4); a rolled-back
  mutation must leave **zero** activity rows (rollback-injection test mandatory).
- **Append-only enforced in the DB**, not by convention (trigger test is red-first).
- **No content in `changes`** — scalars only; free-text → field names. Never the
  comment/note body (MCP rule).
- Notifications are `activity_log`-free (overlap rule, `33` §1).
- Visibility of `ip_hash`/`request_id` restricted to admin read paths.
- `actor_key_id`/`via` written for every row (`browser` today; T041/Wave 3 fill
  `mcp`/`cli`) — `actor_key_id` is a plain `INT NULL` unless MCP's 0004 has
  landed; the `api_keys` FK is added by the following migration — never hand-edit
  an applied version.
- Open calls O-7…O-21 defaults stand pending T027; nothing here overrides them.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] `activity_log` created + both triggers reject UPDATE/DELETE on both
      engines (test evidence pasted) — closes the **S1** audit-trail half with
      traceability to the findings register.
- [ ] Every retrofitted mutation route has a co-commit test: on success, an
      `activity_log` row exists with matching action/subject (re-read via the
      read API or sqlx); on injected failure, **no row exists**.
- [ ] `changes` contains only scalars/field-names (assert on a card move + a
      note body edit; no body text present anywhere).
- [ ] `request_id` non-null on HTTP mutations; `ip_hash` is salted SHA-256
      (not the raw IP — assert length/hexshape and inequality across salts);
      `actor_key_id`/`via` present (`browser`).
- [ ] Read APIs: platform-admin (403 for non-admin — house convention),
      board viewer+ (404 for foreign board), workspace admin/owner+ (403/404
      for plain member/outsider) — cross-user negative tests green.
- [ ] 2nd-landed projects/notes retrofit resolved and recorded in §Notes
      (T034 ⇄ T037 rule applied).
- [ ] T52 same-PR: contract + `30-db-schema.md` + `schema.d.ts` regen.
- [ ] Cursor contract documented in §Notes for T040 (no frame before commit;
      `activity_log.id` order = publish order).
- [ ] Both engines green; `_sqlx_migrations` untouched; lesson appended to
      `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — include trigger evidence, the
2nd-landed retrofit outcome, and the cursor-ordering note for T040)

## Lessons

(pending)
