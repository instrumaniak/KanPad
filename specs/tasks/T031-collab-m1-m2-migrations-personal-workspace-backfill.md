# T031: Collab: M1+M2 migrations + personal-workspace backfill

> **STATUS: pending.** Phase 2. Wave 2 / row A. Closes the **schema half of S1**
> (HIGH strategic — no collaboration model). Blocked by T027 (design review gate).
> Everything else in Wave 2 descends from this task.

## Goal

Land the additive collaboration domain schema (M1) and the backfill that makes
single-user behavior unchanged (M2), per `30-collaboration.md` §1.1 and §3
(steps 1–2). **No enforcement, no authz, no behavior change** — lists and detail
routes behave exactly as before until T033/T034 land.

1. **M1 — expand:** `workspaces`, `workspace_members`, `board_members`,
   `project_members` (exact shapes + indexes from `30` §1.1), utf8mb4,
   both-engine DDL (no MySQL-only syntax, no row-alias upserts).
2. **M2 — backfill (split into DDL + data files):** add nullable `workspace_id`
   to `boards`, `projects`, `notes`; create one **personal workspace per existing
   user** (deterministic slug `personal-{user_id}`); backfill `workspace_id` from
   `created_by` → personal workspace. New writes set it from T033 onward.

## Depends On

T027 (design review gate — resolves the implementation-readiness question and
ratifies `30-collaboration.md` §6/§7).

## Phase

2

## Critical

Yes — gates B–L (all Wave 2 tasks). Rows B/J hook off the tables this lands.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §1.1 (core entities, additive;
  `user_id` becomes *creator*, not sole owner — **O-2 resolved: keep the column,
  document it reads as creator**), §3 (expand → backfill → enforce → contract;
  steps 1–2 here, steps 3–4 in T033/T034/T036).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1** (migration/backfill
  half); global DoD (both engines, forward-only, `_sqlx_migrations` never
  hand-edited, backup-gated); D-4 (backup before production migration — T011).
- `specs/api-rewrite-rs-specs/30-db-schema.md` — **amendment target** (T52
  same-PR: add the four tables + `workspace_id` columns to the schema reference).
  No wire-shape change in this task → `openapi.json`/`schema.d.ts` unchanged.
- `backend-rs/migrations/0001_baseline.sql` (ownership-by-`user_id` model today),
  `0003_sort_key_fractional_ordering.sql` (current head). **Migration numbering:**
  head is 0003; MCP reserves 0004–0006 (not landed). If collab ships before MCP,
  M1/M2 take the next free numbers (M2 is split into DDL file + data file so a
  partial failure can resume); comments/notifications (T041) continue the sequence.
  Never reuse or hand-edit an applied version; renumber at authoring time per
  `32-comments-mentions.md` §1/§8.
- `backend-rs/tests/README.md` (harness rules), MySQL 8.4 + MariaDB 11.4 CI legs
  (`.github/workflows/backend-rs.yml`).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/000N_collab_workspaces.sql` (`000N` = next free at authoring time; collab must not collide with MCP's reserved 0004–0006; renumber forward if they have landed — never reuse an applied version) | **NEW** — M1 DDL: `workspaces` (id, name, slug UNIQUE, created_by FK→users, created_at/updated_at), `workspace_members` (UNIQUE(workspace_id,user_id), INDEX(user_id), role ENUM('owner','admin','member')), `board_members` + `project_members` (UNIQUE(resource_id,user_id), INDEX(user_id), role ENUM('viewer','editor')) |
| `backend-rs/migrations/000N_collab_workspace_id.sql` (next free; same renumber rule) | **NEW** — M2 DDL: `ALTER TABLE boards/projects/notes ADD workspace_id INT NULL` + `INDEX IDX_*_workspace (workspace_id)` |
| `backend-rs/migrations/000N_collab_workspace_backfill.sql` (next free; same renumber rule) | **NEW** — M2 data: insert personal workspace per user (idempotent — `NOT EXISTS` guard on slug), then `UPDATE boards/projects/notes JOIN users … SET workspace_id = <personal ws> WHERE workspace_id IS NULL` |
| `specs/api-rewrite-rs-specs/30-db-schema.md` | **AMEND** — four tables + three `workspace_id` columns (T52 same-PR) |
| `backend-rs/migrations/` numbering | If MCP 0004–0006 have landed by execution time, this task renumbers its files forward; never reuse applied versions |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson (migration split DDL/data for non-transactional MySQL DDL) |
| `specs/arch-hardening-specs/00-overview.md` | Annotate S1 schema-half landed (amendment-log entry) |

**Must NOT appear in the diff:** any `authz`/enforcement code (T032–T034), the
`activity_log` table (T037/G), membership API (T035/E), `comments`/`notifications`
tables (T041/K), any route behavior change.

## Implementation Steps

1. **Re-verify S1 against the tree** (fresh grep: `team|share|member|permission` in
   `backend-rs/src`; confirm migrations head = 0003 and MCP 0004–0006 not landed;
   paste evidence into §Notes).
2. **Write M1** with both engines in mind: utf8mb4, `ENGINE=InnoDB`, CHECK-style
   role constraints per house pattern, `ON DELETE` behavior per `30` §1.1
   (members cascade with workspace; users restricted), forward-only.
3. **Write M2 DDL** (`workspace_id` nullable + index on all three tables).
4. **Write M2 backfill** — idempotent (`slug = CONCAT('personal-', id)` with
   `NOT EXISTS` guard; `WHERE workspace_id IS NULL` updates). Split from the DDL
   file: MySQL/MariaDB DDL is non-transactional, so a failed DDL leaves partial
   state that must not re-run cleanly by accident — the split makes each file
   either fully applied or not, and the data file is naturally re-runnable.
5. **Verify backfill deterministically** (direct SQL, paste results):
   `COUNT(workspaces) = COUNT(users)` ∧ zero `boards/projects/notes` rows with
   `workspace_id IS NULL` ∧ every row's `workspace_id` maps to its creator's
   personal workspace (three joins, `HAVING COUNT(*) <> 0` must return 0 rows).
6. **Run both CI legs** locally or rely on the matrix (MySQL 8.4 + MariaDB 11.4);
   `cargo sqlx prepare --check` if any `.sqlx` query landed with tests.
7. **Amend `30-db-schema.md`** in the same PR; annotate the amendment log; lesson.
8. **Backup gate:** M2 runs against a real DB only after T011's drill has recorded
   a restore point (cutover doctrine: restore is the only rollback path).

## Constraints

- **Forward-only, additive-only.** No drop/rename of `user_id` (O-2 resolved:
  keep; it reads as *creator* post-Wave-2). No column dropped anywhere.
- **Backup-gated** (T011). `_sqlx_migrations` never hand-edited; migration files
  renumbered at authoring time if the tree moved.
- Both engines green — CHECK/ENUM/FK syntax validated on 8.4 and 11.4 legs.
- Personal workspaces must be deterministic: slug `personal-{user_id}`,
  `created_by = user_id`, role `owner` in `workspace_members`.
- **No enforcement in this task** — existing behavior (single-user) must be
  byte-identical; the only observable change is new tables/columns existing.
- Open calls O-1…O-6 ratified at T027 (user sign-off 2026-11-16); if the T027
  review overrides any (incl. the §1.1 entity shapes), amend this task before
  implementing.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] M1 DDL creates the four tables exactly as `30` §1.1 (columns, UNIQUE/INDEX,
      FKs, ENUM roles) — evidence: `SHOW CREATE TABLE` output pasted for each.
- [ ] M2 split (DDL + backfill) applied; verification queries from Step 5 all
      return zero mismatches, pasted as evidence — closes the **S1** migration
      half with traceability to the findings register.
- [ ] Idempotency proven: backfill file re-run is a no-op (second run changes 0
      rows; demonstrated).
- [ ] No behavior change: existing test suites (backend + frontend) pass
      unmodified; `migrate info` shows all applied, pending = 0.
- [ ] `30-db-schema.md` amended in same PR; migration checksums consistent;
      amendment log in `00-overview.md` updated.
- [ ] Both engines green (CI matrix legs); `_sqlx_migrations` untouched by hand.
- [ ] Backup recorded pre-migration (T011 drill log reference).
- [ ] Cross-user negative tests: N/A by design (no endpoints yet) — noted here so
      it is not "silently adopted"; they become standing ACs in T033.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — include the S1 re-verification grep and
the migration numbers actually used, incl. any renumber vs. MCP 0004–0006)

## Lessons

(pending)
