# T013: Session cache hygiene on logout/login

> **STATUS: review fixes applied (2026-11-17).** Wave 0 (reliability floor). Closes **R2** (HIGH). Review subagent verdict: "ship with nits" — M1 + T1/T2 fixed, no CRITICAL/HIGH.

Pinned 2026-11-17 (plan-mode review + user decisions):

- Helper lives in `frontend/src/lib/query-client-reset.ts` (single owner; T014's global-401 reuses it) — not co-located in auth-provider.
- Register ordering mirrors login: `resetSessionCaches()` → `setQueryData(['auth','me'], registerResponse)`. The current `invalidateQueries` becomes a no-op after `clear()` (empty cache) — replaced, not kept.
- Red test 4 added: mutation deferred through logout → resolves → cache stays empty (closes the gap with R2's design test plan in `specs/arch-hardening-specs/10-reliability.md`).
- `@tanstack/query-core@5.107.1` source verified (2026-11-16, unchanged through implementation): `clear()` → `QueryCache.clear()` → `remove()` → `query.destroy()` → `cancel({ silent: true })`; retryer rejects with `CancelledError` so a late in-flight response never reaches `setData`. `MutationCache.clear()` does NOT cancel in-flight mutations. Test 3 may pass on first write (characterization test); `cancelQueries()` kept as belt-and-braces.

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
| `frontend/src/features/auth/auth-provider/index.tsx` | Call the helper in logout + login `onSuccess` (and register), with the pinned ordering; register: replace `invalidateQueries(['auth','me'])` with `setQueryData(['auth','me'], registerResponse)` after the clear |
| `frontend/src/lib/query-client-reset.ts` | **NEW** — `resetSessionCaches(queryClient)`: `await queryClient.cancelQueries()` then `queryClient.clear()` |
| `frontend/src/lib/query-client-reset.test.ts` | Helper unit tests (ordering, idempotency, late in-flight response) |
| `frontend/src/features/auth/auth-provider.test.tsx` | Red-first integration tests: seed → logout → empty; login-as-B → empty; late in-flight response does not repopulate; mutation in flight through logout; register auto-login |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** query-key renames across features (rejected —
whole-cache clear is the minimal correct fix), `staleTime` changes, apiFetch changes.

## Implementation Steps

TDD: write the failing tests first.

1. **Red test 1:** seed `queryClient` with fake `['columns',1]`, `['boards']`,
   `['notes']` data → run logout handler → assert all undefined (and `['auth','me']`
   null). Fails today.
2. **Red test 2:** seed as A → login as B → assert A's data gone, B's `me` present
   (pins the ordering: clear **before** `setQueryData(['auth','me'], B)`). Fails today.
3. **Test 3 (subtle one):** start a query whose response resolves *after*
   clear (deferred promise) → clear → resolve → assert cache stays empty. Verified
   against `@tanstack/query-core@5.107.1` source (see pins above): `clear()` alone
   suffices; test may pass on first write. Kept as a regression pin; helper still
   calls `cancelQueries()` first as belt-and-braces.
4. **Red test 4:** a mutation whose `mutationFn` resolves *after* logout →
   logout → resolve mutation → assert no cache repopulation / no throw (pins the
   documented caveat: `MutationCache.clear()` does NOT cancel in-flight mutations;
   their `onSuccess`/`onSettled` still run and must tolerate it).
5. **Register:** replace `invalidateQueries(['auth','me'])` with
   `resetSessionCaches(queryClient)` then `setQueryData(['auth','me'], registerResponse)`
   (pinned: deterministic; `invalidateQueries` after `clear()` is a no-op). Add/adjust
   the register auto-login assertion so `user` ends up set to the registered user.
6. Implement `resetSessionCaches`; wire into logout/login/register `onSuccess`.
7. Run the auth suite (49 tests) + full vitest; paste results.
8. Lesson: "cache is not user-scoped — session boundaries must flush it".

## Constraints

- No query-key refactor (scope control — whole-cache clear achieves the guarantee).
- Ordering pinned by test (clear vs `me` write).
- Must not break: `GuestRoute`, `AppLayout` redirect, register auto-login, double-logout.
- Global DoD gates; no commit without approval.

## Acceptance Criteria

- [x] Red-first tests exist and fail before the fix (paste red output), pass after.
- [x] Logout clears every cached query; login-as-B sees zero of A's data within
      `staleTime` (closes **R2**).
- [x] Late in-flight response from the previous session cannot repopulate the cache
      (test 3 green — pins `cancelQueries`/ordering decision in §Notes; may pass on
      first write, see pins).
- [x] Register auto-login still works without `invalidateQueries` (user ends up set
      to the registered user; covered by a pinned test).
- [x] Mutation in flight through logout does not resurrect cache (test 4 green).
- [x] Auth suite (49 → **56** with new tests) + full frontend vitest green (**734**);
      `tsc --noEmit` clean; eslint clean on changed files.
- [x] Helper has a single owner (`frontend/src/lib/query-client-reset.ts`) — no
      copy-paste into feature files.
- [x] Lesson appended.

## Notes

Implementation began 2026-11-17. Red run (before fix), both files:

```
 Test Files  2 failed (2)
      Tests  5 failed | 10 passed (15)

 × logout clears every cached query and nulls auth me
 × login as B wipes A cached data and sets me to B
 × late in-flight query response after logout does not repopulate the cache
 × mutation settling after logout does not resurrect the cache
 × register auto-login sets me from the register response without a refetch
 FAIL  |unit| src/lib/query-client-reset.test.ts [ import failed — module did not exist ]
```

After implementing the helper + wiring: all green; auth suite 49 → **54**; full
`npm run test` (unit) **732/732** across 78 files (one mid-iteration full run showed
2 failures that were the pre-fix register/me-write assertions plus a flake — see
below — and has since been 4/4 clean); `npx tsc --noEmit` clean; eslint clean on the
four changed/new files.

Decisions / discoveries during implementation:

1. **Register ordering pinned to `setQueryData(['auth','me'], registerResponse)`.**
   `invalidateQueries` after `clear()` finds an empty cache and is a no-op; relying
   on the mounted observer's optimistic-result/`build()`/`setOptions` re-creation path
   would have made auto-login work only by accident. The register response IS the
   user payload (`201 {data:{id,email,role}}` + session cookie), so the write is
   equivalent and deterministic — and `meApi` stays at 1 call (bootstrap only),
   pinned in the test.
2. **One extra render hop after login/register.** `clear()` destroys the me query;
   the mounted `useQuery` observer still points at the destroyed instance until the
   effect (`setOptions` → `build`) re-attaches it. The provider therefore shows the
   new `me` one — sometimes two — flushes after the mutation's `onSuccess` runs
   (mutation `success` dispatch happens *after* the async `onSuccess` completes, and
   the observer re-attach is yet another hop). Tests must `waitFor(() =>
   expect(result.current.user)...)`, not assert synchronously — an early version of
   the tests flaked between 1- and 2-flush renders. This transient exists in the
   product too (one render hop of logged-out context after login); it settles before
   interactive navigation, mirrors the logout belt-and-braces null window, and matches
   the pinned design. Watch it in T014 if a global-401 redirect ever lands inside it.
3. **Test 3 could not be observed red in isolation** — the helper module failed at
   import in the red run, taking its 4 tests with it. After implementing, the
   deferred-query-resolves-after-clear assertion passes (as predicted by the verified
   5.107.1 source: destroy → CancelledError → `setData` never reached). It stays as a
   regression pin. `cancelQueries()` is kept as belt-and-braces regardless.
4. **Test 4 pins the documented caveat, not a new guarantee.** `MutationCache.clear()`
   does NOT cancel in-flight mutations; their callbacks still run. The test shows a
   mutation that invalidates `['boards']` in `onSettled` leaves the cache empty after
   logout (no observer → no refetch → nothing resurrects). An `onSuccess` that does
   unconditional `setQueryData` COULD still repopulate post-logout — accepted React
   Query semantics; rewriting feature callbacks is out of scope for T013 (flagged for
   T014 if it ever bites).
5. **Logout error path unchanged:** if `logoutApi` rejects, `onSuccess` never runs,
   so the cache is NOT cleared — deliberate (user stays logged in; matches the
   spec's "on logout success" wording).
6. `GuestRoute` / `AppLayout` / register auto-login / double-logout: covered by the
   existing 49 auth tests (all green); double-logout additionally covered by the
   helper's idempotency test (empty-cache no-op).
7. **Review subagent (2026-11-17), verdict "ship with nits"** — no CRITICAL/HIGH.
   Fixes applied: **M1** — lesson bullet now names `setQueryData` (not just
   `invalidateQueries`) as the residual R2 vector with the live call sites; T014 gained
   a "Carried from T013 review" section (session-epoch guard vs accept-and-document
   before the 401 wiring ships; plus the success-only reset caveat as a tracked
   requirement). **T1** — new test: in-flight `['auth','me']` bootstrap across logout
   never repopulates (late `me` response stays discarded; context stays logged-out).
   **T2** — characterization pin: logout failure retains the cache (deliberate).
   After these, auth provider file is 17/17 (auth suite 56), full unit suite green.
   Accepted without code change: success-only reset on login/register error paths
   (carried to T014), the mutation-caveat boundary (out of scope, documented).

## Lessons

See `docs/lessons/frontend.md` — "Session cache hygiene (2026-11-17)": keys are not
user-scoped → flush the cache on session boundaries; `cancelQueries` belt-and-braces;
`clear()` destroys queries but never cancels in-flight mutations; `waitFor` the
context value after a clear (render-hop re-attach).
