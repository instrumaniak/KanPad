# T033: Collab: enforce authz on boards/columns/cards

> **STATUS: pending.** Phase 2. Wave 2 / row C. Closes the **enforcement half of
> S1** for the board surface (join filters on lists + `authz::require` on
> detail/mutation routes). First routes enforced — this task is the pattern
> T034/T041 replicate.

## Goal

Route-by-route enforcement for **boards, columns, cards** (and their lists) per
`30-collaboration.md` §2/§3:

- **Lists** (`GET /boards`, board detail embed, archived, column card lists):
  membership JOIN filters in `WHERE` — board visible iff actor is `created_by`
  ∨ member of the board's workspace ∨ holds an explicit board grant. No
  Rust post-filtering.
- **Detail/mutation** (create/read/update/delete/move/reorder/bulk):
  `authz::require(AuthSession, ResourceRef::Board/…, Perm::Read|Write)`.
- **Single release per route, no config flag** (`30` §3, default) — rollback =
  redeploy of the previous release.
- Personal-workspace behavior identical to today for solo users (no visible
  change).

## Depends On

T032 (authz kernel).

## Phase

2

## Critical

Yes — gates T034 (D), T036 (F), T037 (G).

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §2 (kernel, 404/403 rule,
  list-filter SQL), §3 (enforce step: per-route, cross-user tests, no flag).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1**; global DoD
  (cross-user negative tests as standing ACs; persistence re-read rule;
  both engines).
- `backend-rs/src/features/boards/`, `features/columns/`, `features/cards/`
  (routes + stores), `backend-rs/src/common/authz.rs` (T032),
  `backend-rs/src/common/pagination.rs` (`Page<T>` envelope — nested embeds stay
  unpaginated by contract; top-level lists carry `Page<T>`).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — no shape changes (additive
  only); if a 404/403 needs contract text, it already landed with T032.
- `backend-rs/tests/README.md`; CI matrix legs.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/features/boards/store.rs` + `routes.rs` | list query gains membership filter; detail/mutations call `authz::require` |
| `backend-rs/src/features/columns/store.rs` + `routes.rs` | same (columns scoped through board; embed stays contract-shaped) |
| `backend-rs/src/features/cards/store.rs` + `routes.rs` | same (cards scoped through column→board) |
| `backend-rs/tests/api/collab_board_authz.rs` | **NEW** — cross-user matrix per endpoint family (black-box) |
| Existing `tests/api/boards*`, `columns*`, `cards*` | extend with cross-user cases; no test weakening |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** projects/notes/tags/checklists/labels (T034),
membership API (T035), migrations, wire-shape changes, optimistic concurrency
(T036), activity_log writes (T037 — runs after this task; co-commit wiring for
cards happens there).

## Implementation Steps

1. **Enumerate the route families** from the registry (grep `route(` under the
   three feature dirs); produce the checklist in §Notes — every route gets a
   row: family | route | perm | cross-user test | status.
2. **Lists first:** membership filter in SQL —
   `… WHERE (b.created_by = :a OR w.id IN (…) OR b.id IN (SELECT board_id FROM board_members WHERE user_id = :a))`
   shape (single round-trip; verify index use with `EXPLAIN`; paste evidence).
   Counts (`total`) must use the same filter (no count/list skew).
3. **Detail/mutation routes:** one `authz::require` call right after param
   extraction; `Read` for GET, `Write` for POST/PATCH/PUT/DELETE/move/reorder.
4. **Cross-user negative tests per route** (red first): user B on foreign
   board → 404 everywhere (detail, columns embed, cards list, mutations);
   B with board `viewer` grant → 200 reads, 403 writes (POST/PATCH/DELETE card);
   B with `editor` grant → 200 writes; own personal-workspace board → unchanged.
5. **`/auth/me` and admin routes** already re-lookup — leave untouched.
6. **Regression:** full backend suite + both engines + `cargo sqlx prepare
   --check` (queries changed).
7. Persistence re-read rule: every PATCH/POST/DELETE test re-reads state via a
   second call or sqlx query.
8. Lesson.

## Constraints

- **404 for unauthorized reads / 403 for insufficient writes** — never reverse,
  never 500, never a data leak (existence hiding verified by test on every
  detail route).
- **No Rust post-filtering of lists**; filter in SQL, indexed, `total` consistent.
- No behavior change for single-user/personal-workspace flows (regression suite
  is the guardrail).
- No config flags — per-route enforcement, one release per route.
- Open calls O-7…O-21 defaults stand pending T027 review; kernel semantics
  unchanged (amendment belongs to T032).
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Every boards/columns/cards route has (a) an enforcement call or documented
      `// public by design` justification (expect: only auth/session routes are
      public), and (b) a cross-user negative test — paste the route×test
      checklist as evidence; closes the **S1** board-surface enforcement half.
- [ ] List queries are membership-filtered in SQL; `EXPLAIN` shows index use;
      list vs `total` counts agree.
- [ ] Foreign board → 404 (not 403, not empty 200) on every detail/list/mutation
      route; `viewer` grant → 403 on writes; `editor` grant → writes allowed.
- [ ] Both engines green; no test weakened/deleted; `sqlx prepare --check` clean.
- [ ] Wire shapes unchanged (additive-only) — contract lint/CI drift gate green.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — include the route×test checklist and
the EXPLAIN evidence)

## Lessons

(pending)
