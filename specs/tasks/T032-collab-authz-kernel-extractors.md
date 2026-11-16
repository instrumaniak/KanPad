# T032: Collab: authz kernel + extractors

> **STATUS: pending.** Phase 2. Wave 2 / row B. Closes the **enforcement half of
> S1** (the chokepoint every collaboration route calls). Design-only otherwise:
> nothing is wired to routes in this task (that is T033/T034).

## Goal

Implement the single authorization chokepoint from `30-collaboration.md` §2:
**one shared helper used by every store — never per-route ad-hoc checks.**

```rust
backend-rs/src/common/authz.rs (NEW)
  pub enum Perm { Read, Write, Admin }
  pub enum ResourceRef { Workspace(i32), Board(i32), Project(i32), Note(i32), ... } // extend per subject type
  pub async fn require(conn, actor: &AuthSession, resource: ResourceRef, perm: Perm)
      -> Result<(), AppError>
  // ONE indexed query per check; batch variant for lists:
  pub async fn visible_workspace_ids(conn, actor) -> Result<Vec<i32>, AppError>
```

- **404 vs 403:** unauthorized *read* of a workspace resource → **404** (existence
  hiding, matches `AppError::NotFound`); authenticated-but-insufficient *write* →
  **403** (`AppError::Forbidden`). **Carve-out — governance routes**
  (`/admin/*`, `require_platform_admin`): authenticated-but-non-admin → **403**
  by design (the admin surface is not hidden; matches the existing `/auth/me` +
  admin re-lookup behavior in `extract.rs`). Document both rules in
  `20-api-contract.md` amendments (T52).
- **List endpoints** filter by membership in SQL (`JOIN workspace_members…` /
  `WHERE … IN (SELECT …)`), indexed by `workspace_members.user_id` /
  `board_members.user_id` — **no post-filtering in Rust.**
- **Extractor:** extend the existing `AuthSession` (`backend-rs/src/extract.rs`)
  with an `authorize(conn, resource, perm)` method so route handlers stay
  one-liners; errors reuse the problem+json renderer.
- **`require_platform_admin`:** extends the kernel for governance routes
  (role = `admin` in `users`; block applied in T038). Needed by T038.
- Applies to Wave 2 surfaces too (D-7): `Note(id)`, `Card(id)` ResourceRefs for
  comment/notification stores (T041), SSE connect handshake (T040), presence
  heartbeat (T040), mention validation (T041) — all call this kernel.

## Depends On

T031 (M1/M2 tables must exist).

## Phase

2

## Critical

Yes — gates T033, T034, T035, T036, T037, T040, T041.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §2 (authorization kernel —
  the chokepoint), §1.2 (access rule: `created_by` ∨ workspace member (any role)
  ∨ explicit `*_members` grant on the resource or a parent — board grant covers
  columns/cards/notes; write requires `editor` or workspace `member`+ on
  workspace-owned resources — **O-1 resolved 2026-11-16**; `viewer` via explicit
  grant only), §2 list-filter rule, 404-vs-403 decision.
- `specs/arch-hardening-specs/31-realtime-sse.md` §5 (SSE connects resolve the
  visible workspace set with the same indexed membership query), §7 (presence
  heartbeat re-uses board-read authz).
- `specs/arch-hardening-specs/32-comments-mentions.md` §3 (permission matrix —
  comment stores call `authz::require`, never re-implement).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1**; global DoD
  (cross-user negative tests as standing ACs; both engines; cargo gates).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — **amendment target** (T52):
  the 404-read / 403-write rule, additive; no route shape changes in this task.
- `backend-rs/src/extract.rs` (`AuthSession`, `FromRequestParts` — the payload is
  trusted without a per-request DB lookup; `me` + admin re-lookup is the
  exception), `backend-rs/src/common/ids.rs` (`Role`, `UserId`),
  `backend-rs/src/error.rs` (problem+json renderer), `backend-rs/tests/README.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/common/authz.rs` | **NEW** — `Perm`, `ResourceRef`, `require`, `visible_workspace_ids` (batch), `require_platform_admin`, unit tests inline |
| `backend-rs/src/common/mod.rs` | export `authz` |
| `backend-rs/src/extract.rs` | add `AuthSession::authorize(...)` convenience (one-line handlers stay) |
| `backend-rs/src/common/` list-filter helpers (or `authz.rs`) | SQL fragments/builders for membership JOINs (lists filter in WHERE, never post-filter) |
| `specs/api-rewrite-rs-specs/20-api-contract.md` | **AMEND** — 404-read / 403-write rule (T52 same-PR) |
| `backend-rs/tests/api/authz_kernel.rs` | **NEW** — black-box cross-user matrix (user B → 404/403/allowed per role/grant) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** route-side enforcement (T033/T034), wire changes
(no new endpoints), migrations.

## Implementation Steps

1. **Re-verify** the access rule against the tree: no existing share/member
   checks (expect zero hits — S1 re-verified at T031), `AuthSession` shape as in
   `extract.rs:279`.
2. **Design the query shapes first** (one indexed query per check):
   - read/write a board: `workspaces` ⋈ `workspace_members` (member?) ∨
     `board_members` grant ∨ `boards.created_by = actor`; resolve
     `workspace_id` + a derived Perm verdict in one round-trip (JOIN with
     aggregate or two shaped queries — pick the single-round-trip form; record
     the choice in §Notes).
   - `visible_workspace_ids`: `SELECT workspace_id FROM workspace_members
     WHERE user_id = ?` (+ union of granted-resource workspace) — one query.
3. **TDD red:** unit tests for `require` — member read OK; member write OK (O-1);
   non-member read → `NotFound`; non-member write → `Forbidden`; explicit
   board `viewer` grant → read OK / write `Forbidden`; explicit `editor` grant →
   read+write OK; `created_by` of foreign workspace's shared board (grant to
   outsider) → OK per grant; platform admin → read-only governance path allowed,
   content write not implied; `visible_workspace_ids` returns exactly the
   memberships; batch variant used for list filters.
4. **Implement** `authz.rs`; keep `AuthSession` untouched except the
   `authorize()` method; no per-call DB hit beyond the one indexed query.
5. **Cross-user negative tests** (black-box, `wire::bind_blackbox`, ephemeral
   port): seed two users + workspace + board with a grant; assert the 404/403
   matrix. These tests are the enforcement proof template T033/T034 replicate.
6. **`require_platform_admin`**: `role = 'admin'` + (T038 extends with block
   check); governance read only — never implies content write (O-6).
7. **Amend `20-api-contract.md`** (404/403 rule) — T52 same-PR; no other
   contract text changes.
8. `cargo fmt`, `clippy -D warnings`, workspace tests, both engines green for
   any SQL touched; lesson.

## Constraints

- **Workspace-resource reads → 404 (existence hiding); insufficient-write →
  403. Governance routes (`/admin/*`, `require_platform_admin`) → 403 for
  authenticated non-admins** — never the reverse for resources, never a 500
  (`AppError::NotFound` / `AppError::Forbidden` via the existing problem+json
  renderer; no new problem types).
- List filtering happens **in SQL**, indexed — Rust post-filtering is a defect.
- One indexed query per check (batch variant for lists); no N+1 membership
  probes per row.
- Platform admin gets governance **read only** — never silent content write
  (O-6 resolved).
- `AuthSession` construction is unchanged; only an `authorize()` method is added.
- Open calls O-7…O-21 are recorded defaults pending T027 review; if the review
  overrides any that touch the kernel (e.g. O-7 viewer-read model), amend this
  task before implementing.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] `authz::require` + `visible_workspace_ids` + `require_platform_admin`
      implemented; unit + black-box cross-user matrix tests green — closes the
      **S1** chokepoint half (with T031 covering the schema half).
- [ ] 404/403 split documented in `20-api-contract.md` (T52 same-PR).
- [ ] Every permission path has a test: member/non-member/viewer-grant/
      editor-grant/creator/platform-admin × read/write — no silent path.
- [ ] List-filter helpers demonstrated with `EXPLAIN` showing index use on
      `workspace_members.user_id` (paste evidence).
- [ ] Cross-user negative test suite exists and is parameterized for reuse by
      T033/T034/T041 (named, documented pattern).
- [ ] Both engines green; no route behavior changes anywhere (routes are not
      wired — that is T033/T034); existing suites pass unmodified.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — record the chosen query shapes, the
single-round-trip resolution, and any deviation from `30` §2 torn up at review)

## Lessons

(pending)
