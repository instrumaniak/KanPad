# T038: Collab: admin panel (users + activity)

> **STATUS: pending.** Phase 2. Wave 2 / row H. The governance surface for
> platform operators: user management + activity-log review. **No silent content
> reads** (O-6 resolved 2026-11-16: activity + user list only; moderation goes
> through workspace admins).

## Goal

Platform-admin API + minimal admin UI per `30-collaboration.md` §5 and the
`features/admin/` structure from the original architecture spec:

**Backend (`/api/v1/admin/*`, all behind `require_platform_admin` — T032):**
- `GET /api/v1/admin/users` — paged user list (`Page<T>`).
- `PATCH /api/v1/admin/users/{id}` — **block/unblock** and **role** change.
  Block = additive `users.blocked_at DATETIME(6) NULL` migration; blocked user
  is rejected at login + on the re-lookup paths (`/auth/me`, admin routes);
  platform-admin cannot block themselves; cannot block another admin via this
  endpoint (demote first, or block via CLI — decided; recorded here).
- `GET /api/v1/admin/activity` is delivered by T037 — this task **consumes** it
  (no duplicate read path).
- **Registration toggle**: exists in config (`registration disabled` knob) —
  expose `GET/PATCH /api/v1/admin/registration` if not already an endpoint;
  same platform-admin guard; otherwise wire the existing config through an
  endpoint (small).

**Frontend (`features/admin/`):** admin layout (role-guarded route), user list
(search/page, block/unblock action with confirm dialog), activity-log view
(filters: actor, action, subject_type, from, to; paged; actor email or
"Deleted user"), registration toggle switch. **Board-level activity panel is
NOT part of this task** — it belongs to T042 (L) next to the board view.

Blocking enforcement v1 (documented residual window): block takes effect
immediately on the re-lookup paths (`/auth/me`, admin) and at next login;
long-lived sessions on non-re-lookup routes remain valid until expiry —
documented in the runbook; if the session store format allows a cheap reliable
purge (`DELETE FROM sessions …`), add it as best-effort with a test, else rely
on the documented window. **Decision to ratify at T027** (see Constraints).

## Depends On

T037 (G) — the activity read API (`/admin/activity`) this panel consumes.

## Phase

2

## Critical

No.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §5 (admin API + UI,
  platform-admin guard = existing `Role::Admin` → T032 `require_platform_admin`;
  **O-6: activity log + user list only, no silent content reads**; FR43–49).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1**; global DoD
  (cross-user negative tests; both engines; persistence re-read).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — **amendment target** (T52):
  admin endpoints, `blocked_at` user field, problem slugs (`self-block`/
  `admin-block` if new; reuse `forbidden` otherwise).
  `30-db-schema.md` — **amendment target**: `users.blocked_at`.
- `backend-rs/src/extract.rs` (`/auth/me` + admin re-lookup is where block is
  enforced today), `backend-rs/src/common/ids.rs` (`Role`),
  `backend-rs/src/features/admin/` (exists? check — likely absent),
  `specs/planning-artifacts/architecture.md` (original `features/admin/` sketch,
  FR43–49), `specs/security/09-logging-monitoring-ir.md` (auth-event audit —
  T037 covers).
- `frontend/CONVENTIONS.md` (Radix `Dialog` for confirm; no hand-rolled modals).
- `backend-rs/tests/README.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/000N_collab_users_blocked_at.sql` | **NEW** — `users.blocked_at DATETIME(6) NULL` (additive; both engines) |
| `backend-rs/src/features/admin/` | **NEW** module — `users` list/patch, `registration` get/patch; `require_platform_admin` guard |
| `backend-rs/src/features/auth/` (login + `/auth/me`) | blocked check → `403 account-blocked`-style problem (new slug, T52) |
| `backend-rs/src/lib.rs` | register routes + OpenAPI paths |
| `frontend/src/features/admin/` | **NEW** — admin layout, user list + block/unblock (Radix Dialog), activity view (reuses T037 read API), registration toggle; route-guarded (`/admin`) |
| `frontend/src/App.tsx` | `/admin/*` route with role guard (non-admin → 404-style redirect; hide nav entry) |
| `specs/api-rewrite-rs-specs/{20-api-contract.md,30-db-schema.md}` | **AMEND** (T52 same-PR) |
| `backend-rs/tests/api/admin_panel.rs` | **NEW** — cross-user matrix: non-admin → 403; admin blocked-user login → rejected; self-block → 400; activity view consumes T037 (no duplicate) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** silent access to note/card content (O-6 — no
impersonation, no content preview), activity write calls (T037 owns writes),
board-level activity panel (T042), workspace-admin moderation UI (workspace
admins moderate via workspace roles — not platform UI in v1).

## Implementation Steps

1. **Re-lookup audit**: confirm `/auth/me` + admin routes perform the user-row
   re-lookup (they do); the block check rides on the same query (no new hot path).
2. **Migration** (additive `blocked_at`); registration-toggle endpoint = config
   read + flip via existing config path if present, else minimal endpoint;
   document either way.
3. **Users list**: `Page<T>`, `q` search (email prefix/substring), role filter;
   never returns `password_hash`; returns `{id, email, role, blocked_at, created_at}`.
4. **Block/unblock PATCH**: body `{blocked: bool}`; role PATCH `{role: 'admin'|'user'}`;
   rejects self-block (400), rejects blocking/demoting other platform admins (403);
   every PATCH emits `activity_log` (`admin.user_blocked|unblocked|role_changed`,
   actor = platform admin) via the T037 helper — **this task wires those call
   sites** (against T037's helper) — and on block, best-effort session purge if
   the store format allows (verify; else document residual window).
5. **Frontend**: `features/admin/` layout mirroring the boards layout pattern;
   nav entry visible only to `role === 'admin'`; activity view = typed fetch of
   the T037 endpoint (reuse `apiFetch` + `Page<T>` helpers); empty/error/skeleton
   states per house rules; Radix primitives only; optimistic toggle with
   rollback.
6. **Route guard test (frontend)**: non-admin user navigating `/admin` →
   redirected (no flash of admin UI); unit tests with mocked queries.
7. T52 regen; both engines; persistence re-read (block → second login rejected);
   lesson.

## Constraints

- **O-6 is binding:** no content reads. The activity view shows metadata
  (who/what/when), never free-text values (fields are names-only by design).
- **Platform admin ≠ workspace admin** — keep the roles and guards distinct;
  workspace-level moderation is workspace admins' job (out of this panel).
- `require_platform_admin` everywhere under `/admin/*`; 403 (not 404) for
  authenticated non-admins per house convention (admin surface is not hidden,
  it is forbidden).
- No new auth providers; session model unchanged (block = rejection, not
  re-login flow redesign).
- Open calls O-7…O-21 defaults stand pending T027; **the block-enforcement
  residual window is a T027 ratification point** (document; if overridden,
  amend before implementation).
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] T037's `/admin/activity` is consumed (grep proves no duplicate read
      path); filters actor/action/subject_type/from/to all work (black-box).
- [ ] Block flow proven end-to-end: admin blocks user → user's next
      `/auth/me` → 401/403; user's next login → rejected; unblock → login OK
      (persistence re-read at each step). Closes part of **S1**'s governance gap.
- [ ] Self-block → 400; blocking another platform admin → 403 (both tested).
- [ ] Registration toggle reflects config; flips via admin API; test proves
      new-registration behavior change.
- [ ] Frontend admin area: role-guarded (test: non-admin sees no nav entry,
      `/admin` redirects), Radix-only primitives, skeleton/empty/error states,
      optimistic toggle rolls back on error.
- [ ] Every `/admin/*` mutation emits `activity_log` rows (test via T037 read
      API: action present, actor = platform admin).
- [ ] T52 same-PR: contract + `30-db-schema.md` + `schema.d.ts` regen.
- [ ] Both engines green; no test weakened; lesson appended to
      `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — record the block-enforcement decision
ratified at T027 and whether the session purge was implemented or documented
as residual window)

## Lessons

(pending)
