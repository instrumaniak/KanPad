# T013: Session cache hygiene on logout/login

> **STATUS: pending.** Wave 0 (reliability floor). Closes **R2** (HIGH).

## Goal

User B must never see user A's data after a logout→login on the same tab. Today logout
only does `setQueryData(['auth','me'], null)` and login only sets `['auth','me']` —
every other query (`['columns', …]`, `['boards']`, `['notes']`, …) stays cached for
`staleTime` (5 min), and query keys are not user-scoped.

Fix: one shared helper `resetSessionCaches(queryClient)` that runs on **logout success
and login success** (and register), clearing the whole query cache — ordered so the new
session's `['auth','me']` write survives, and proven not to be repopulated by late
in-flight responses from the previous session.

## Depends On

(None. T014 builds on this helper.)

## Phase

0

## Critical

Yes — cross-account data disclosure on shared devices (R2, HIGH).

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R2 (design: helper, ordering,
  in-flight repopulation, edge cases, test plan).
- `specs/arch-hardening-specs/00-overview.md` — finding R2; global DoD.
- `frontend/src/features/auth/auth-provider/index.tsx:23-35` (the defect),
  `frontend/src/App.tsx:58-69` (QueryClient defaults),
  `frontend/src/lib/api/client.ts` (apiFetch — untouched here).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/features/auth/auth-provider/index.tsx` | Call the helper in logout + login `onSuccess` (and register), with the pinned ordering |
| `frontend/src/lib/query-client-reset.ts` (or co-locate in auth-provider) | **NEW** — `resetSessionCaches(queryClient)`: `cancelQueries()` then `clear()` (verify which is needed — see steps) |
| `frontend/src/features/auth/auth-provider/*.test.tsx` (or new `query-client-reset.test.ts`) | Red-first tests: seed → logout → empty; login-as-B → empty; late in-flight response does not repopulate |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** query-key renames across features (rejected —
whole-cache clear is the minimal correct fix), `staleTime` changes, apiFetch changes.

## Implementation Steps

TDD: write the failing tests first.

1. **Red test 1:** seed `queryClient` with fake `['columns',1]`, `['boards']`,
   `['notes']` data → run logout handler → assert all undefined (and `['auth','me']`
   null). Fails today.
2. **Red test 2:** seed as A → login as B → assert A's data gone, B's `me` present
   (pins the ordering: clear **before** `setQueryData(['auth','me'], B)`).
3. **Red test 3 (the subtle one):** start a query whose response resolves *after*
   clear (deferred promise) → clear → resolve → assert cache stays empty. If
   React Query lets the observer repopulate, the helper must `cancelQueries()` first —
   the test decides the implementation.
4. Implement `resetSessionCaches`; wire into logout/login/register `onSuccess`.
5. Run the auth suite (49 tests) + full vitest; paste results.
6. Lesson: "cache is not user-scoped — session boundaries must flush it".

## Constraints

- No query-key refactor (scope control — whole-cache clear achieves the guarantee).
- Ordering pinned by test (clear vs `me` write).
- Must not break: `GuestRoute`, `AppLayout` redirect, register auto-login, double-logout.
- Global DoD gates; no commit without approval.

## Acceptance Criteria

- [ ] Red-first tests exist and fail before the fix (paste red output), pass after.
- [ ] Logout clears every cached query; login-as-B sees zero of A's data within
      `staleTime` (closes **R2**).
- [ ] Late in-flight response from the previous session cannot repopulate the cache
      (test 3 green — pins `cancelQueries`/ordering decision in §Notes).
- [ ] Auth suite (49) + full frontend vitest green; `tsc --noEmit` clean.
- [ ] Helper has a single owner (one module) — no copy-paste into feature files.
- [ ] Lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
