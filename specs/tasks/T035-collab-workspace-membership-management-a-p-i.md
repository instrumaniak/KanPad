# T035: Collab: workspace membership management API

> **STATUS: pending.** Phase 2. Wave 2 / row E. The management surface for
> workspaces and explicit resource grants — no invitations (direct-add by design;
> email invites are a documented future seam). The frontend for this API lands in
> T039 (I).

## Goal

Membership management API per `30-collaboration.md` §2 and `32-comments-mentions.md`
§2, with the **invite-less direct-add** model (a grant itself confers access —
`30` §1.2):

- `POST /api/v1/workspaces/{id}/members` `{user_id, role: 'admin'|'member'}` —
  workspace owner/admin only. Adding the `owner` role is not offered (ownership
  transfer is out of scope v1).
- `DELETE /api/v1/workspaces/{id}/members/{user_id}` — workspace owner/admin (or the
  member themselves = "leave workspace"). Removing the workspace **owner** is
  rejected (transfer is out of scope v1) → `409`/`400` problem slug documented.
- `GET /api/v1/workspaces/{id}/members` — member+ (read); `?search=` matches email
  prefix/substring, limit 50; returns `{id, email, role}` only. **This is the
  mention/share autocomplete endpoint** consumed by T041 (K) and T039 (I).
  **Caller gate (per `32-comments-mentions.md` §2):** the caller must be a member,
  OR hold read access to at least one shared (granted) resource in the workspace —
  never a global directory. (An earlier draft of this task required full
  membership; aligned to `32` §2 on 2026-11-16 — grant-only grantees may
  autocomplete in the context they can read.)
- `POST /api/v1/boards/{id}/members` `{user_id, role: 'viewer'|'editor'}` and
  `DELETE` counterpart; same for `/api/v1/projects/{id}/members` — the grant API
  (`board_members`/`project_members` from T031).
- **Who may grant:** workspace owner/admin, or the resource's `created_by`
  (design decision flagged for T027 ratification; default recorded here).
- All mutations run in one transaction; **leave a clean post-commit seam** (or a
  small, documented hook point) so T037 (G) can co-insert `activity_log` rows
  (member.added/removed, share grant/revoke) and T040 (J) can publish the
  `member.added`/`member.revoked` bus events — **this task does not depend on
  either; the second of {T035, T037, T040} to land wires the coupling**
  (deterministic rule; both tasks record in §Notes).

## Depends On

T032 (authz kernel). Does **not** depend on T037/T040 by design (spec edge B → E).

## Phase

2

## Critical

No — but T039 (I) and T041 (K)'s mention autocomplete consume it; keep it tight.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §1.1 (roles: workspace
  owner/admin/member; resource viewer/editor grants; personal workspaces are
  single-user), §1.2 (access rule — a grant alone confers access), §2 (404/403;
  list filtering in SQL), §6 (row E edges).
- `specs/arch-hardening-specs/32-comments-mentions.md` §2 — `GET
  /api/v1/workspaces/{id}/members?search=` is the mention autocomplete
  (read-access holders; wire contract owned there).
- `specs/arch-hardening-specs/33-notifications.md` §2 — `shared`-type fanout on
  grant is wired by the **second of {T035, T041} to land** (the same
  second-landed rule as the activity_log coupling — if T035 lands second, the
  wiring happens there).
- `specs/arch-hardening-specs/31-realtime-sse.md` §5 — `member.added`/
  `member.revoked` bus events + live revocation are wired by T040.
- `specs/arch-hardening-specs/00-overview.md` — finding **S1**; global DoD.
- `specs/api-rewrite-rs-specs/20-api-contract.md` — **amendment target** (T52):
  new endpoints, problem slugs (`membership-exists` for duplicate POST — default;
  `owner-not-removable` for the owner-removal rejection), additive only.
- `backend-rs/src/extract.rs` (`AuthSession`), `backend-rs/tests/README.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/features/workspaces/` (NEW module or `features/members/`) | routes + store for workspace members + board/project grant endpoints; garde DTOs (snake_case) |
| `backend-rs/src/lib.rs` | register routes + OpenAPI paths |
| DTOs + `schema.d.ts`/`openapi.json` regen (`npm run gen:api`) — T52 same-PR | additive |
| `backend-rs/tests/api/workspace_members.rs` | **NEW** — black-box cross-user matrix: grant→read, revoke→404, self-leave, owner-not-removable, duplicate-grant conflict, search autocomplete authz |
| `specs/api-rewrite-rs-specs/20-api-contract.md` | **AMEND** — section for membership endpoints + problem slugs (T52) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** email invites/tokens (out of scope v1),
`activity_log` inserts (T037 — this task leaves the seam), bus publishes
(T040), `shared` notification fanout (T041), frontend (T039).

## Implementation Steps

1. **Re-read `30` §1.1/§2**; confirm `workspaces`/`workspace_members`/
   `board_members`/`project_members` exist (T031).
2. **DTOs + routes** (garde-validated, snake_case): POST workspace member,
   DELETE member, GET members (+`search` prefix/substring, `LIMIT 50`,
   `ORDER BY email`), POST/DELETE board grant, POST/DELETE project grant.
3. **Authorization matrix** (tests first, red):
   - POST member: workspace owner/admin → 200/201; plain member → 403;
     non-member → 404; adding `owner` role → 400 (`errors[]`).
   - DELETE member: owner/admin → 204; self (non-owner) → 204; target is owner →
     `409` `owner-not-removable`; non-member → 404.
   - Board/project grant: workspace owner/admin or resource `created_by` →
     200/201/204; others → 403 (member) / 404 (non-member); grantee gets read
     immediately (verify via the T033/T034-enforced routes), loses it on revoke
     (verify 404 after).
   - Duplicate POST same role → `409` `membership-exists` (default; ratify at
     T027); role change = DELETE + POST.
4. **`search` autocomplete** (per `32` §2): returns only **workspace members**
   (never a global directory); caller must be a workspace member **or hold read
   access to at least one shared/granted resource in the workspace** — otherwise
   404/403 per matrix. Grant-only grantee can search in the context they can
   read; a total non-member cannot enumerate anything.
5. **Single transaction per mutation**; leave a post-commit seam (e.g. a
   `fn after_grant_change(...)` hook point or a documented call site) for
   T037/T040/T041 to attach without restructuring.
6. **T52**: regen OpenAPI + `schema.d.ts` in the same PR; contract doc amended;
   problem slugs added to the problem-types table.
7. Both engines green; `sqlx prepare --check`; persistence re-read on every
   mutation test (AGENTS.md rule).
8. Lesson; annotate `00-overview.md` amendment-log entry (share endpoints landed).

## Constraints

- **Direct-add only** — no invitation tokens, no email sending (email stays out,
  D-7/O scope). The grant is immediate; an org user must exist to be added.
- **404-no-oracle** on every path (non-member → 404, never 403 with existence).
- Self-removal allowed (leave); **owner removal rejected**; no owner transfer v1.
- No global user directory — `search` is workspace-scoped; `GET /users?ids=`
  is T041's sanitized endpoint, do not duplicate it here.
- Cross-user negative tests are the standing AC (every endpoint in the matrix
  above).
- Open calls O-7…O-21 defaults stand pending T027; grant-issuer rule
  (owner/admin or resource `created_by`) and the duplicate-grant 409 slug are
  defaults to ratify at T027 — amend this task if overridden.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Full authorization matrix above passes as black-box tests — closes the
      **S1** management-surface half (traceability to the findings register).
- [ ] Grant → immediate access (second API call proves it through the enforced
      routes); revoke → next request 404 (T033/T034 enforcement proof).
- [ ] `search` autocomplete is scoped: workspace member / granted read-holder →
      results; total non-member → 404/403; no global directory leaks
      (tests: neither path enumerates users).
- [ ] `GET /workspaces/{id}/members?search=` contract matches `32` §2
      (limit 50, `{id,email,role}`).
- [ ] Member mutations are transactional; post-commit seam documented in §Notes
      for T037/T040/T041 (second-landed wiring rule recorded).
- [ ] OpenAPI + `schema.d.ts` regenerated same-PR (T52); problem slugs documented.
- [ ] Both engines green; contract drift gate green; no existing test weakened.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — record the ratified grant-issuer rule,
duplicate-grant behavior, and which of T035/T037/T040 landed first for the
activity/bus coupling)

## Lessons

(pending)
