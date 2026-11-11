# T014: Global 401 handling + query retry policy + request timeouts

> **STATUS: pending.** Wave 0. Closes **R5** (MEDIUM). **Depends on T013** (shares the
> auth-layer files and the cache-reset helper).

## Goal

Three reliability gaps in the frontend data layer, one owner each:

1. **401 anywhere → session-expiry UX.** `lib/error-messages.ts` implements the
   taxonomy (401 → "Session Expired" + redirect) but is **dead code** (only its own
   test calls it). Wire a single global handler: any 401 except the bootstrap
   `['auth','me']` query → `resetSessionCaches()` (T013 helper) → redirect `/login` →
   one toast. Idempotent under concurrent 401s.
2. **Retry policy:** queries currently `retry: false` (one blip = user-visible
   failure). New: retry **idempotent GET queries once** on network/5xx errors with a
   short delay; never retry 4xx; mutations never retry (keep current behavior).
3. **Request timeouts:** `apiFetch` has no AbortController (the `timeout` error branch
   is unreachable). Add a default timeout (15 s) → typed `ApiError('timeout')` with a
   human message ("Network timeout — check your connection").

## Depends On

T013 (helper reuse + auth-layer file ownership — do not parallelize).

## Phase

0

## Critical

No — user-visible but not data-corrupting; R5 is MEDIUM.

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R5 (design: handler placement,
  ordering, edge cases, test plan).
- `specs/arch-hardening-specs/00-overview.md` — finding R5; global DoD.
- `frontend/src/lib/error-messages.ts` (dead taxonomy — un-dead it or consolidate),
  `frontend/src/lib/api/client.ts` (apiFetch — add abort/timeout),
  `frontend/src/App.tsx:58-69` (QueryClient defaults), `frontend/src/App.tsx:76-100`
  (routes/boundary context for the redirect).
- AGENTS.md: single owner per transport (pick ONE error-message owner).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/lib/api/client.ts` | AbortController timeout (const, 15 s), typed timeout `ApiError`; ensure `signal` threading |
| `frontend/src/App.tsx` | QueryClient `retry` policy (queries only, 1×, skip 4xx); wire global `QueryCache`/`MutationCache` `onError` 401 handler |
| `frontend/src/lib/error-messages.ts` (or its chosen replacement) | Become the single owner called by the handler + call sites; delete dead/duplicate paths |
| Tests: `client.test.ts`, new handler test, `error-messages.test.ts` | Red-first: timeout abort, retry matrix (500 retry / 400 no / mutation no), 401 → clear+redirect+single toast, bootstrap-me exempt |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** `staleTime` changes, feature-level ad-hoc 401 checks
(all through the one handler), offline queue (`navigator.onLine` — out of scope).

## Implementation Steps

TDD: red-first for each of the three behaviors.

1. **Timeout:** fake timers + deferred response → assert abort fires at 15 s, error is
   typed with the friendly message (red today: no abort exists).
2. **Retry matrix:** mock 500 → query refetches once then fails; mock 400 → fails
   immediately; mutation 500 → no retry (three tests).
3. **401 handler:** non-auth query returns 401 → cache cleared (spy T013 helper) +
   navigate `/login` + exactly one toast; second concurrent 401 → still one toast;
   `['auth','me']` 401 → exempt (existing AppLayout redirect path owns it — no double
   redirect).
4. Implement; consolidate error-message ownership (call `getErrorMessage` from the
   handler and remaining raw `err.message` call sites — or delete the module and
   centralize; **decide in §Notes**, AGENTS.md single-owner rule).
5. Full vitest + `tsc --noEmit`; paste results.
6. Lesson: "dead error taxonomy is worse than none — wire it or delete it".

## Constraints

- Mutations keep `retry: false` (non-idempotent — POST/PATCH must not auto-replay).
- 401 handler must not loop (login page itself must stay) and must not double-toast.
- Timeout must not break slow-but-legit ops (15 s ≫ API NFR 500 ms; justify in Notes).
- T013's tests stay green; global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Timeout: abort at 15 s → typed friendly error (red-proof pasted).
- [ ] Retry: 500 query retries once, 400 does not, mutation does not (3 tests).
- [ ] 401 on any non-bootstrap query → cache cleared + `/login` + single toast;
      concurrent 401s idempotent; bootstrap `me` exempt (closes **R5**).
- [ ] One error-message owner; no new raw `err.message` call sites; dead path removed
      or un-dead — either way greppable single implementation.
- [ ] Full frontend suite green; `tsc --noEmit` clean; lesson appended.

## Notes

(filled in during/after implementation)

**Open call:** un-dead `error-messages.ts` vs replace with a new owner. *Default:
un-dead it* (it already has the status taxonomy + tests) and route the global handler
through it.

## Lessons

(pending)
