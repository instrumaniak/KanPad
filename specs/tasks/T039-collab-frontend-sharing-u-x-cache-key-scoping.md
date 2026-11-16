# T039: Collab: frontend sharing UX + cache-key scoping

> **STATUS: pending.** Phase 2. Wave 2 / row I. The user-facing half of sharing:
> member pickers, share dialogs, sidebar separation of own vs shared resources.
> No new endpoints — consumes T035's membership API.

## Goal

Frontend for the sharing model per `30-collaboration.md` §1.2/§5 and **O-5 resolved
2026-11-16: sidebar sections ("My boards" / "Shared with me"), no workspace
switcher in v1**:

- **Sidebar sections** split boards (and projects/notes listings where they
  exist) into "My boards" (`created_by === me.id`) vs "Shared with me"
  (everything else the server returns — access-filtered already). Pure client
  partition of the existing `GET /boards` envelope; no contract change.
- **Share dialog** (Radix `Dialog`) on board/project detail header: current
  members/grants list (from `GET /workspaces/{id}/members` + grant lists),
  **add-by-member** (search input fed by `GET /workspaces/{id}/members?search=`,
  keyboard-navigable results), role select (`viewer`/`editor` for resources;
  `admin`/`member` for workspace), remove grant (confirm inline, optimistic).
  Advanced path: direct `user_id` grant for a non-member org user (input
  visible behind the search box; same POST endpoint).
- **Workspace members management** page/modal: list members, add (owner/admin),
  remove (owner/admin, self = leave), role display. Owner removal is rejected
  by the API (`owner-not-removable`) — surface the problem+json detail
  gracefully.
- **Cache-key scoping (defense-in-depth):** React Query keys for workspace-
  contextual data gain the workspace id where a single current workspace exists
  in view; global lists (`['boards']`, `['notes']`…) remain global **relying on
  the server-side membership filter** (introduced by T033/T034) — no stale
  foreign-data window; `T013`'s `clear()` on logout/login remains the hard
  guarantee. Do **not** user-scope every key (over-broad refactor, rejected in
  R2's design).
- **"Watch" toggle** is NOT part of this task (T042 brings watch UI with the
  comments/notification surfaces).

## Depends On

T035 (E) — the membership API being wired up.

## Phase

2

## Critical

No.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §1.2 (access rule — UI gates
  render/actions on the same rule, server remains the enforcement), §5
  (member/share UX follows admin panel location conventions), **O-5 resolved:
  sidebar sections, no switcher v1**.
- `specs/arch-hardening-specs/31-realtime-sse.md` §6 — invalidation targets stay
  as today (`['columns', boardId]`, `['boards']`…); sharing UX must not fork
  the cache model that T040/T042 build on.
- `specs/arch-hardening-specs/32-comments-mentions.md` §2 — `GET
  /workspaces/{id}/members?search=` is the autocomplete source (limit 50).
- `frontend/CONVENTIONS.md` (Radix-only menus/dialogs; no hand-rolled popovers;
  `apiFetch` + `Page<T>` helpers; React Compiler enabled).
- `frontend/src/layouts/sidebar.tsx` (the actual sidebar lives here — not under
  `features/`), `frontend/src/layouts/app-layout.tsx`,
  `features/boards/` (share dialog host), `features/auth/` (`me` query).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1** (sharing UX half);
  global DoD (frontend gates: tsc, vitest, gen:api check).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/features/workspaces/` (NEW) | members list page/modal, add-member search (debounced, ≥2 chars), remove/leave with confirm (Radix `Dialog`) |
| `frontend/src/features/boards/` + `features/projects/` | share dialog component (list grants, add grant, remove grant) |
| `frontend/src/layouts/sidebar.tsx` | "My boards" / "Shared with me" section split (client partition of the boards envelope) |
| `frontend/src/lib/api/` | typed calls for the T035 endpoints (`workspaces.ts`), no raw fetch |
| `frontend/src/App.tsx` / router | workspace members route |
| `frontend/src/features/*/__tests__/` | component tests: sections render correctly; dialog flows mock API; optimistic remove rolls back on error; leave-workspace moves item out of "My boards" |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** new endpoints (T035 owns the contract), global
user-directory search (does not exist by design), workspace switcher (O-5),
watch UI / presence / comments / inbox (T042), admin panel (T038).

## Implementation Steps

1. **Tree check:** locate sidebar, board list partition point, existing dialog
   patterns (reuse), and the `me` query shape.
2. **API layer:** `workspaces.ts` — typed member list/search/add/remove/leave +
   board/project grant add/remove; reuse `apiFetch`; add keys
   (`['workspace-members', workspaceId]`, `['board-grants', boardId]`).
3. **Sidebar split:** partition board list by `created_by === me.id`; empty
   states for both sections; stable counts.
4. **Share dialog:** fetch grants on open; search-as-you-type (debounce 200 ms,
   min 2 chars, `LIMIT 50`); add grant → optimistic row + rollback on error;
   remove → confirm dialog, optimistic; error surfaces via existing
   `getErrorMessage`-style owner (pick the R5 owner — T014's); focus management
   via Radix (no `document.querySelector`).
5. **Workspace members modal:** list; add (owner/admin only — hidden for
   members); remove (owner/admin + self-leave); surface `owner-not-removable`
   problem detail as a readable message; after leave/remove, invalidate
   `['boards']`, `['columns']`, `['notes']` (closes the "stale shared board
   after revoke" gap client-side; server is the enforcement).
6. **Cache scoping:** workspace-scoped keys only where a single workspace is in
   view; no key-shape changes for global lists; logout/login clear untouched.
7. Tests: vitest component tests for dialog flows (mock `apiFetch`); no
   `waitForTimeout`; E2E deferred to T042's suite (optional smoke: share board
   → B's sidebar shows it under "Shared with me").
8. Frontend gates: `tsc --noEmit`, `vitest`, `npm run gen:api -- --check`
   (should be a no-op — additive consumer of T035 contract). Lesson.

## Constraints

- **Server is the enforcement** — the UI hides what the server forbids; never
  rely on client hiding for security (server returns 404/403 regardless).
- Radix primitives only; no hand-rolled modals/popovers/dropdowns.
- No global user directory — search is workspace-scoped; direct `user_id`
  entry is the only cross-workspace path and it is explicit/advanced.
- O-5 holds: **no workspace switcher** in v1; sections only.
- Reuse the R5 error-message owner; no new toast/error copy subsystems.
- Open calls O-7…O-21 defaults stand pending T027 (esp. grant-issuer rule,
  `owner-not-removable` copy) — amend if overridden.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Sidebar renders "My boards" / "Shared with me" from the existing envelope
      with correct partition + empty states (component tests) — closes part of
      **S1**'s sharing-UX gap with traceability to the findings register.
- [ ] Share dialog: list/add/remove board (and project) grants against the
      mocked T035 API; optimistic add/remove with rollback on error; revoke →
      item disappears from B's "Shared with me" after invalidation (E2E smoke or
      component-level simulation).
- [ ] Workspace members modal: add/remove/leave flows; `owner-not-removable`
      surfaced readably; permissions hide add/remove for plain members.
- [ ] Search autocomplete hits `?search=` with ≥2 chars, limit respected; no
      global user listing possible from the UI (grep proves no other endpoint).
- [ ] Logout/login cache hygiene (T013) untouched; global list keys unchanged.
- [ ] Frontend gates green (`tsc`, `vitest`, gen:api check); no test weakened.
- [ ] Lesson appended to `docs/lessons/frontend.md`.

## Notes

(filled in during/after implementation — record any UX copy decisions ratified
at T027, e.g. remove/leave confirmations and sidebar labels)

## Lessons

(pending)
