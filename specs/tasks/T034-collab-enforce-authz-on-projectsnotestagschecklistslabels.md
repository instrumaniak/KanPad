# T034: Collab: enforce authz on projects/notes/tags/checklists/labels

> **STATUS: pending.** Phase 2. Wave 2 / row D. Closes the **enforcement half of
> S1** for the remaining feature areas — same pattern, second surface.

## Goal

Route-by-route enforcement for **projects, notes, tags, checklists, labels** per
`30-collaboration.md` §2/§3, replicating T033's pattern:

- **Lists** (projects, notes, archived states): membership JOIN filters in SQL
  (`WHERE` via workspace membership + explicit grants). Notes carry a
  `workspace_id` (backfilled by T031 M2); top-level notes are workspace-visible
  (**O-3 resolved: nullable `workspace_id`, backfill to personal workspace**).
- **Detail/mutation routes:** `authz::require` with `ResourceRef::Project/Note/…`.
- **Tags / checklists / labels** stay scoped through their parent (tag →
  workspace/board scope as designed; checklist → card → board; label →
  workspace/board) — **no per-card ACL** (YAGNI; card visibility = board
  visibility, `30` §1.1). Enforce via the parent's ResourceRef.
- **Commentable note handling** is NOT part of this task (`comments.subject_type='note'`
  arrives with T041); here we only enforce the note routes themselves.
- Notes access follows **the note itself**, not its container — per
  `32-comments-mentions.md` §1 note-commentability rule and `30` §1.1; list
  filters must not hide a note the actor can read via an explicit grant just
  because its board anchor changed.

## Depends On

T033 (C) — board surface enforces first; the pattern, test suite shape and
helper conventions are reused.

## Phase

2

## Critical

No — but required before T041 (K) (comment stores call the live kernel for
notes; `32` permission matrix).

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §1.1 (notes: top-level notes
  belong to a workspace — **O-3 resolved**; tags/checklists/labels scoped via
  parent, no per-card ACL), §2 (kernel usage, 404/403), §3 (per-route enforce,
  no flags).
- `specs/arch-hardening-specs/32-comments-mentions.md` §1/§8 — note access is
  the note's own ACL (commentability context for T041).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1**; global DoD
  (cross-user negative tests; persistence re-read; both engines).
- `backend-rs/src/features/projects/`, `features/notes/`, `features/tags/`,
  `features/checklists/`, `features/labels/` (routes + stores),
  `backend-rs/src/common/authz.rs` (T032).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — no shape changes.
- `backend-rs/tests/README.md`; CI matrix legs.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/features/projects/{store.rs,routes.rs}` | membership filter on lists; `authz::require` on detail/mutations/share-adjacent reads |
| `backend-rs/src/features/notes/{store.rs,routes.rs}` | same; backfilled `workspace_id` used for list filter; note-own ACL |
| `backend-rs/src/features/tags/`, `features/checklists/`, `features/labels/` | parent-scoped enforcement (ResourceRef of the parent board/card/workspace) |
| `backend-rs/tests/api/collab_project_note_authz.rs` | **NEW** — cross-user matrix (reuse T032/T033 test shape) |
| `backend-rs/tests/api/collab_tags_checklists_labels_authz.rs` | **NEW** — parent-scoped cross-user tests |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** projects/notes FTS (T023), comments (T041),
migrations (T031 only), contract changes, board-surface routes (T033).

## Implementation Steps

1. Enumerate route families (grep `route(` under the five feature dirs); build
   the route×test checklist in §Notes.
2. **Notes list filter** uses `workspace_id` (indexed after T031); notes with
   NULL `workspace_id` must not exist post-T031 verification — assert in test
   setup (fail loudly if they do).
3. **Projects list** joins workspace membership; archived variants filtered the
   same way.
4. **Parent-scoped families:** checklist items → resolve card → board → enforce;
   labels/tags → workspace (or board) scope; one `authz::require` per handler,
   reuse the T033 route×test checklist format.
5. **Cross-user negative tests** (red first) per route: B foreign → 404;
   B with explicit grant on projects/notes → 200/403 per role; `viewer` grant
   → 403 on writes; parent-scoped families inherit the parent's grant semantics.
6. Regression: full suite, both engines, `sqlx prepare --check`.
7. **Activity-log retrofit seam:** T037 (G) owns the `activity_log` table and
   helper; the second of {T034, T037} to land wires `projects/notes` mutation
   handlers to co-insert activity rows (field names only, no values). Record
   which was the case in §Notes; if T037 has already landed, do the wiring here
   in the same PR series.
8. Lesson.

## Constraints

- 404/403 split identical to T033 (no new conventions); no Rust post-filtering.
- Notes lists must not leak container-inherited visibility mistakes: the note's
  own grant wins; document any intentional container-inherited behavior in §Notes.
- No migrations here; no new endpoints; additive-only.
- Open calls O-7…O-21 defaults stand pending T027 review.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Route×test checklist complete (pasted): every projects/notes/tags/
      checklists/labels route enforced + cross-user negative test — closes the
      remaining **S1** enforcement surface.
- [ ] Notes list/detail use the note's own ACL (workspace membership ∨ explicit
      grant), verified by test (grant-to-outsider note accessible without
      board access).
- [ ] Parent-scoped families (checklist/label/tag) enforce via the parent with
      a cross-user test that fails if enforcement is removed (red-proof).
- [ ] Both engines green; no test weakened; `sqlx prepare --check` clean.
- [ ] If T037 landed first: projects/notes mutations co-insert `activity_log`
      rows (test asserts rows) — or the follow-up is explicitly recorded in
      T037/T034 §Notes with the second-landed rule applied.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — paste the route×test checklist and
the projects/notes activity-log retrofit outcome)

## Lessons

(pending)
