# T014: Global 401 handling + query retry policy + request timeouts

> **STATUS: review fixes applied (2026-11-17).** Review verdict: "request
> changes" → H1 + M1–M3 + L1–L5 fixed, red-proofed, re-verified (777/777,
> `tsc --noEmit` clean, eslint clean). Wave 0. Closes **R5** (MEDIUM).
> **Depends on T013** (helper reuse + auth-layer file ownership — do not
> parallelize).
>
> **2026-11-17 review verdict (pre-implementation):** "not ready as written —
> one design-level defect (login-POST 401 fires the global handler), one
> unpinned seam (how the handler reaches toast/navigate), carried item #2's
> suggested mitigation doesn't cover non-401 failures." All three are resolved
> in this amendment. Decisions taken (industry standard, see §Notes):
> Q1 credential-POST exemption · Q2 in-tree `<SessionExpiryHandler>`
> (soft nav) · Q3 `sessionEpoch` guard at the concrete-value rollback sites ·
> Q4 `getErrorMessage`: handler + new call sites only, server message wins,
> existing ~35 raw-`err.message` sites deferred to a follow-up.

## Goal

Three reliability gaps in the frontend data layer, one owner each:

1. **401 anywhere → session-expiry UX.** `lib/error-messages.ts` implements the
   taxonomy (401 → "Session Expired" + redirect) but is **dead code** (only its own
   test calls it). Wire a single global handler: any 401 **except the credential
   POSTs (login/register)** and the redirect/toast for the bootstrap `['auth','me']`
   query → `resetSessionCaches()` (T013 helper) → redirect `/login` → one toast.
   The cache clear itself applies to **every** 401, including `['auth','me']` — the
   exemption covers redirect/toast only (avoids double-redirect via AppLayout), it
   does **not** leak the cache. Idempotent under concurrent 401s (single-flight
   flag). The handler never fires on the login/register page (guest routes) and
   never double-fires.
2. **Retry policy:** queries currently `retry: false` (one blip = user-visible
   failure). New: retry **idempotent GET queries once** on network/5xx/timeout
   errors with a short delay; never retry 4xx; never retry `CancelledError`;
   mutations never retry (explicit `retry: false`, keeps current behavior).
3. **Request timeouts:** `apiFetch` has no AbortController. Add a default timeout
   (15 s) → typed `TimeoutError` with a human message ("Network timeout — check
   your connection").

## Depends On

T013 (helper reuse + auth-layer file ownership — do not parallelize). Done 2026-11-17.

## Phase

0

## Critical

No — user-visible but not data-corrupting; R5 is MEDIUM.

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R5 (design: handler placement,
  ordering, edge cases, test plan — **superseded where conflicting** by this
  amendment, see §Notes D1–D6).
- `specs/arch-hardening-specs/00-overview.md` — finding R5; global DoD.
- `frontend/src/lib/error-messages.ts` (dead taxonomy — un-dead it),
  `frontend/src/lib/api/client.ts` (apiFetch — add abort/timeout),
  `frontend/src/App.tsx:58-69` (QueryClient defaults), `frontend/src/App.tsx:76-100`
  (routes/boundary context for the redirect).
- AGENTS.md: single owner per transport (ONE error-message owner).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/lib/api/client.ts` | `DEFAULT_TIMEOUT_MS = 15_000` const; `TimeoutError extends Error` (name `TimeoutError`); AbortController threading incl. caller-`signal` wiring; `TimeoutError` on **our** abort, caller abort propagates unchanged; network `TypeError` propagates unchanged |
| `frontend/src/lib/api/client.test.ts` | **NEW** — timeout abort at 15 s (fake timers + deferred response), caller-signal abort ≠ `TimeoutError`, `TimeoutError` shape |
| `frontend/src/lib/error-messages.ts` | Become the single owner called by the handler; **server message wins** rule (Error#message preferred over taxonomy text; taxonomy keeps title/retry/redirect); detect `TimeoutError` (and legacy `AbortError`) → timeout entry. Exported as before |
| `frontend/src/lib/error-messages.test.ts` | Updated expectations (message preference) + new cases; no scenario deleted |
| `frontend/src/components/session-expiry-handler.tsx` | **NEW** — `<SessionExpiryHandler />`: mounted under `BrowserRouter` + `ToastProvider` in `App.tsx`; `useEffect` assigns `queryClient.getQueryCache().config.onError` and `getMutationCache().config.onError` (TkDodo-endorsed single-assignment seam); `useNavigate` + `useToast`; single-flight flag; cleanup restores prior handlers on unmount |
| `frontend/src/components/session-expiry-handler.test.tsx` | **NEW** — handler matrix (see Acceptance Criteria) |
| `frontend/src/App.tsx` | QueryClient `retry` predicate (once for network/5xx/`TimeoutError`, never 4xx, never `CancelledError`), `retryDelay: 1000`, explicit `mutations: { retry: false }`; mount `<SessionExpiryHandler />` inside `ToastProvider` |
| `frontend/src/lib/query-client-reset.ts` | Export `currentSessionEpoch()`; `resetSessionCaches()` **bumps the epoch first thing** (before `cancelQueries()`) |
| `frontend/src/lib/query-client-reset.test.ts` | Epoch bump + `currentSessionEpoch()` tests |
| `frontend/src/features/auth/auth-provider/index.tsx` | `meta: { skipSessionExpiry: true }` on login/register mutations **and** on the bootstrap `me` query; **onError** on login/register mutations → silent `resetSessionCaches(queryClient)` (carried item #2, belt-and-braces, no-op on empty cache) |
| Concrete-value mutation `onError` rollbacks (sessionEpoch guard): `use-cards.ts` (3 sites: `:118`, `:170`, `:204`), `use-checklists.ts` (`:127`), `use-projects.ts` (`:54`) | Capture `sessionEpoch: currentSessionEpoch()` in `onMutate` context; in `onError`, skip the rollback write when the epoch changed (a session boundary was crossed while the mutation was in flight). `invalidateQueries` untouched |
| Tests: `use-cards.test.tsx` / `use-checklists.test.tsx` / `use-projects` tests (existing files) | One post-boundary-rollback test per file (or one shared helper test — implementer's choice) |
| `docs/lessons/frontend.md` | Dated, tagged lesson |
| `specs/tasks/T014-*.md` | This file — §Notes filled in during implementation |

**Must NOT appear in the diff:** `staleTime` changes; feature-level ad-hoc
401 call sites (feature code must not sniff 401s — all through the one
handler); offline queue (`navigator.onLine` — out of scope); conversion of the
~35 existing raw `err.message` call sites (follow-up task — see §Notes D4);
user-scoped query keys (rejected — whole-cache clear is the chosen fix);
hard-navigation redirect (chose soft nav, see §Notes D2).

## Implementation Steps

TDD: red-first for each of the three behaviors.

1. **Timeout:** fake timers + deferred response → assert `TimeoutError` fires at
   15 s with the friendly message (red today: no abort exists). Also: caller
   abort → **not** `TimeoutError`; predicate sees `TimeoutError` as retryable.
2. **Retry matrix:** mock 500 → query refetches once then fails; mock 400 →
   fails immediately; mutation 500 → no retry; CancelledError → no retry
   (four tests).
3. **401 handler:** non-auth query returns 401 → cache cleared (spy T013
   helper) + navigate `/login` + exactly one toast; second concurrent 401 →
   still one toast; `['auth','me']` 401 → **cache cleared** (T013 helper
   called) but **no** navigate, **no** toast (AppLayout owns the redirect);
   login/register mutation 401 → zero handler actions; already on
   `/login` → clear only, no toast, no (soft) navigate.
4. **sessionEpoch guard:** rollback after `resetSessionCaches()` leaves the
   cache empty (query not recreated); rollback before the boundary (same
   epoch) works as today.
5. **error-messages:** new rule pinned by tests (server message wins;
   taxonomy title/retry/redirect retained; `TimeoutError` detected).
   Existing expectations updated to the new behavior (documented behavior
   change, no scenario deleted).
6. **Implement; consolidate error-message ownership** (one greppable
   implementation; the handler routes through `getErrorMessage`; no new raw
   `err.message` call sites).
7. Full vitest + `tsc --noEmit`; paste results.
8. Lesson: "dead error taxonomy is worse than none — wire it or delete it" +
   "a session boundary is not just cache hygiene: fence in-flight mutation
   rollbacks (epoch) or every `onError` snapshot write re-leaks A's data".

## Constraints

- Mutations keep `retry: false` (non-idempotent — POST/PATCH must not auto-replay).
- 401 handler must not loop (login page stays) and must not double-toast
  (single-flight flag; skip redirect/toast when already on a guest route).
- 401 handler must not fire for credential POSTs — **form-level error is the
  UX** for bad credentials (backend returns 401 on mismatch — verified in
  `backend-rs/src/features/auth/routes.rs`; `AppError::Unauthorized` → 401).
- Timeout must not break slow-but-legit ops (15 s ≫ API NFR <500 ms;
  justify in Notes).
- T013's tests stay green; global DoD; no commit without approval.
- The global handler clears the cache on **every** 401, including
  `['auth','me']`; `me` is exempt only from redirect/toast.

## Acceptance Criteria

- [x] Timeout: abort at 15 s → typed `TimeoutError` with friendly message
      (red-proof in §Notes — red run `4 failed | 6 passed (10)` before the
      implementation; 12/12 green after).
- [x] Retry: 500 query retries once, 400 does not, mutation does not,
      `CancelledError` does not (7 tests in `app-query-client.test.ts`).
- [x] 401 on any non-bootstrap query → cache cleared + `/login` + single
      toast; concurrent 401s idempotent; bootstrap `me` → cache cleared but
      **no** redirect, **no** toast (closes **R5**). Race case (exempt 401
      first, actionable 401 same burst) red-proofed: fails with the single-
      flight at the top, passes with exempt paths bypassing the flag.
- [x] Credential POST 401 (bad login) → handler does nothing: no cache
      clear, no redirect, no session-expired toast; form-level error only.
- [x] sessionEpoch: post-boundary rollback does not recreate
      `['cards', …]` / `['card', …]` / `['projects']` — move, reorder,
      delete, checklist-item, project-rename sites, each with a
      post-boundary test + same-epoch positive rollback test; all three
      post-boundary site tests red-proofed (neutralized → fail).
- [x] One error-message owner; `getErrorMessage` prefers server message
      (and browser-jargon network failures route to the `network` taxonomy
      on all three engines); no new raw `err.message` call sites; dead path
      removed or un-dead — greppable single implementation.
- [x] Full frontend suite green (**777/777**, 82 files — was 734 at T013);
      `tsc --noEmit` clean; eslint clean on all changed files; lesson
      appended.

## Spec review amendments (2026-11-17)

Pre-implementation review found three defects; all resolved as follows.

- **D1 — Login-POST 401 misfire (defect, fixed).** The original design
  exempted only `['auth','me']`; bad credentials are a 401 and would have
  produced a session-expired toast, a cache reset and a `/login` redirect
  (soft → form state survives, but wrong UX + double toast; with hard nav →
  form wipe). Industry standard (axios-auth-refresh lineage, Stirling-PDF
  production frontend, JWT-wiring guides): credential endpoints and guest
  routes are exempt from the global handler; auth-page 401 toasts are
  suppressed — the form shows its own error. Amended: handler no-ops for
  login/register mutations (`meta.skipSessionExpiry`) and on guest routes.
- **D2 — Toast/navigate seam unpinned (defect, fixed).** Toasts are
  React context; the QueryClient in `App.tsx` is created above both
  `BrowserRouter` and `ToastProvider`. Chosen: **in-tree handler**
  `<SessionExpiryHandler />` (option B, TkDodo's endorsed pattern for
  handlers that need React context — he explicitly notes one global handler
  per cache, so `useEffect` assignment of `config.onError` is safe). Soft
  navigation, existing toast system, no new singleton module. Hard
  navigation (option A) remains the fallback — it would also dissolve the
  D3 hazard entirely — but was rejected: codebase standard (T013/T014) is
  soft-nav cache hygiene, and the session-expiry UX should not blow away
  in-progress UI beyond the cache itself.
- **D3 — Carried item #2 mitigation mismatch (defect, fixed).** "Login
  succeeds server-side (cookie set) but the client treats it as an error"
  is mostly **not** a 401 (500-with-cookie, parse failure, abort) — the
  suggested global-401 coverage would not have caught it. Fixed on two
  rails: (a) the `me` exemption now covers redirect/toast **only** — every
  401 clears the cache; (b) login/register mutations get a silent
  `onError → resetSessionCaches()` (belt-and-braces; no-op on empty cache).
- **D4 — getErrorMessage scope inconsistent (fixed).** R5 says "route all
  user-facing error text through the mapper"; the old AC said only "no new
  raw call sites"; step 4 implied converting all ~35 sites (~20 files —
  violates the D-6 ≤ ~10-files task sizing). Resolved: **handler + new call
  sites**; existing sites deferred to a follow-up (not created yet);
  `getErrorMessage` gains a **server-message-wins** rule (it currently
  *never* returns `error.message`, so converting sites as-is would have
  regressed UX).
- **D5 — Timeout error type doesn't typecheck (fixed).** Spec said
  `ApiError('timeout')`; the ctor takes `status: number`. Resolved: new
  `TimeoutError extends Error` in `client.ts` (name `TimeoutError`).
  `getErrorMessage` maps `TimeoutError`/`AbortError` → timeout taxonomy;
  retry predicate treats it as network-class (once);
  `CancelledError` is explicitly **not** retried (T013's `cancelQueries`
  rejection).
- **D6 — Carried item #1 precision (fixed).** The named call sites were
  "unconditional setQueryData" as a class, but against the installed
  `@tanstack/query-core@5.107.1` (`setQueryData`: updater returning
  `undefined` is a **no-op**; a defined result **builds/recreates** the
  query — `queryClient.js:192-204`), the live post-boundary writers are the
  **concrete-value `onError` rollbacks**: `use-cards.ts:118`, `:170`,
  `:204`; `use-checklists.ts:127`; `use-projects.ts:54`. (`use-projects.ts:41`
  `setQueriesData` only targets existing queries; the `onMutate` updaters
  self-protect via `old?.filter` → `undefined`; `drag-drop-context.tsx:83`
  returns `old` unchanged when missing.) Amended: `sessionEpoch` guard at
  exactly those five sites; epoch bumped inside `resetSessionCaches()`.
- Nits absorbed: "existing 49 auth tests" → **56** post-T013;
  `client.test.ts` is **new**, not an extension; R5's "timeout error must
  not be retried as if server error" is satisfied by treating it as
  network-class with `failureCount < 1`.

## Carried from T013 review (2026-11-17)

1. **In-flight mutation rollback repopulation — RESOLVED as designed
   (sessionEpoch guard).** No accept-and-document; no global rewrite.
   `currentSessionEpoch()` exported from `query-client-reset.ts`;
   `resetSessionCaches()` bumps it before clearing; the five concrete-value
   rollback sites skip their write when the epoch changed during the
   mutation. Review those five sites by name in the diff.
2. **Success-only session reset — RESOLVED (see D3).** `me`-401 now clears
   the cache (redirect/toast still AppLayout-owned); login/register
   mutations additionally reset silently on error. Document the chain in
   §Notes: after T013 logout-clear + T014 401-clears + D3(a/b), every path
   that lands the user on the guest surface holds an empty cache, so
   B-cannot-inherit-A holds even when B's login response is treated as a
   failure. Remove this carried item's risk statement.

(Both documented in `specs/tasks/T013-*.md` §Notes items 1–5 and
`docs/lessons/frontend.md` "Session cache hygiene (2026-11-17)".)

## Notes

(filled in during implementation — 2026-11-17)

**Implementation evidence (2026-11-17):**

- **Timeout (red-first).** `client.test.ts` red run before implementation:
  `Tests 4 failed | 6 passed (10)` (`TimeoutError` missing; caller signal
  passed straight through instead of being chained through the timer
  controller). After `client.ts` changes: 10/10. `apiFetch` now wraps every
  request in an internal `AbortController` + `setTimeout(DEFAULT_TIMEOUT_MS)`;
  caller signals are chained in, and only the internal timer abort is
  re-labelled as `TimeoutError` (caller aborts propagate unchanged; network
  `TypeError` propagates unchanged).
- **Retry matrix (red-first).** Behavior pinned through `createAppQueryClient()`
  (`lib/app-query-client.ts`, single owner of the App QueryClient defaults):
  500 → exactly 2 attempts (initial + 1 retry), TypeError network → 2,
  `TimeoutError` → 2, 400 → 1, 401 → 1, `CancelledError` → 1, mutation 500 → 1
  call to `mutationFn`. `retryDelay`: flat 1_000 ms.
- **401 handler.** `<SessionExpiryHandler />` (`components/session-expiry-handler.tsx`)
  mounted once in `App.tsx` under `ToastProvider`. Assigns `config.onError` on
  the query and mutation caches in an effect (TkDodo-endorsed seam for handlers
  that need React context); restores the previous callbacks on unmount; reads
  through a latest-ref so closures never go stale; `useRef` single-flight
  guard held until the cache reset settles; skips redirect/toast when on
  guest routes (`/login`, `/register`, `/forgot-password`); `me` exempt from
  redirect/toast but NOT from the cache clear; credential-POST mutations
  (`meta.skipSessionExpiry`) are completely untouched; non-exempt mutation
  401s and query 401s go through the same clear → soft `navigate('/login')`
  → one `getErrorMessage`-routed toast. api import in the handler uses
  duck-typing (`status === 401`) so it never depends on `ApiError` class
  identity across module boundaries.
- **MutationCache signature trap (caught by tests).**
  `MutationCache.config.onError` is `(error, variables, onMutateResult,
  mutation, context)` — the `mutation` is the FOURTH argument (verified
  against the installed `@tanstack/query-core@5.107.1` types); the first
  smoke version read `mutation.meta` from the variables slot and threw
  "Cannot read properties of undefined (reading 'meta')" on every mutation
  401. Fixed to `(error, _variables, _onMutateResult, mutation)`.
- **sessionEpoch guard.** `query-client-reset.ts` now exports
  `currentSessionEpoch()` and bumps it **before** `cancelQueries()` in
  `resetSessionCaches()`. Five concrete-value rollback sites capture the
  epoch in `onMutate` context and bail in `onError` when it changed:
  `use-cards.ts` move/reorder/delete, `use-checklists.ts` item update,
  `use-projects.ts` rename. One post-boundary rollback test per file.
  **Red-proof (2026-11-17):** all three guard tests were verified to FAIL
  when the `if (context?.sessionEpoch !== currentSessionEpoch()) return;`
  checks were temporarily neutralized to `if (false)` (`Tests 3 failed |
  21 passed`), and pass again after restoring (`Tests 24 passed (24)`).
- **error-messages.ts** rewritten per D4/D5: server message wins
  (`message` prefers the error's own `message`; taxonomy owns
  `title`/`retry`/`redirect`); `TimeoutError`/`AbortError` → timeout
  taxonomy; browser jargon (`failed to fetch|load failed|...`) never
  surfaced. Existing Response/plain-object cases unchanged.
- **Full gates (2026-11-17, after implementation):** `vitest run --project
  unit` → **768/768 across 82 files** (734 at T013 + 34 new); `npx tsc
  --noEmit` → clean; `npx eslint` on all changed files → clean (one
  `react-hooks/refs` error fixed — `latestRef` now synced in its own
  effect before the registration effect).
- **Scope held:** no `staleTime` changes; `~35` existing raw `err.message`
  call sites untouched (follow-up); no offline queue; no user-scoped keys;
  no hard-navigation redirect; `drag-drop-context.tsx:83` and the `onMutate`
  updaters deliberately not guarded (verified no-ops/pre-boundary — see D6).
- **Follow-up (not created):** convert the ~35 raw `err.message` call sites
  to `getErrorMessage`; mechanical after D4's message-preference rule.
- **Test-file notes:** `session-expiry-handler.test.tsx` renders a real
  `<Routes>` + `ToastProvider`; assertions use cache state (`getQueryData`)
  rather than `isError` where the `clear()` destroys the query (the observer
  re-attaches to a fresh instance — T013 render-hop rule); `me`-query and
  guest-route tests verify cache-clear-only + zero toast + path unchanged.

**Decisions (2026-11-17, pre-implementation amendments):**

- **D1–D6** from §Spec review amendments are the governing decisions; where
  R5's text conflicts ("any 401 except me", "route all error text",
  "ApiError('timeout')"), this amendment wins.
- **15 s timeout justification:** API p95 NFR is <500 ms (20-performance.md);
  15 s is ~30× the NFR and ~10× any legitimate cold path (DB migration-held
  locks, VPS I/O); still bounded enough that a hung connection surfaces as an
  error rather than a forever-spinner. Configurable via `DEFAULT_TIMEOUT_MS`.
- **retry semantics:** predicate returns `false` for 4xx and
  `CancelledError`, `true` only while `failureCount < 1` otherwise
  (network `TypeError`, `TimeoutError`, 5xx `ApiError`, unknowns);
  `retryDelay: 1_000` flat. Queries only; mutations explicitly pinned
  `retry: false`.
- **single-flight:** `useRef(false)` in `SessionExpiryHandler`, set for the
  duration of `resetSessionCaches()` + navigate, released in `finally`.
- **me exemption direction:** exemption = no redirect, no toast; **never**
  a cache exemption (D3). AppLayout stays the single owner of the
  redirect for the bootstrap case; non-me 401s get the handler's redirect.
- **guest routes** (/login, /register, /forgot-password): clear-only, no
  toast, no navigate (already there).
- **follow-up (not in this task):** convert the ~35 raw `err.message`
  call sites to `getErrorMessage` after D4's message-preference rule lands;
  "server message wins" makes that conversion mechanical.
- **timeout vs retry:** `TimeoutError` is network-class for the predicate
  (retry once), satisfying R5's "don't retry a timeout as a server error".
- **double-write risk:** `<SessionExpiryHandler>` assigns
  `config.onError` in an effect on mount; StrictMode double-invoke is
  idempotent (assignment, not add/remove). Cleanup restores previous
  handlers on unmount.
- **Review subagent (2026-11-17), verdict "request changes" → all fixed:**
  - **H1** — single-flight held *before* the exempt decision, so a `me`/guest
    401 arriving first starved a racing actionable 401 (no redirect, no
    toast). Fix: exempt paths take NO slot; only the actionable path
    single-flights. Red-proofed deterministically (direct `config.onError`
    burst; fails only with the old drop-early line; `Tests 1 failed | 7
    passed` → `8 passed (8)`).
  - **M1** — network taxonomy now matches Firefox (`NetworkError …`) and
    Safari (`Load failed`) via `BROWSER_JARGON`, not just Chrome's exact
    string.
  - **M2** — `TimeoutError` explicitly checked in the retry predicate
    (was fall-through from a comment).
  - **M3** — post-boundary tests added for `useMoveCard` and
    `useReorderCard` (previously only `useDeleteCard`), plus same-epoch
    POSITIVE rollback tests for checklists and projects (guard must not
    over-fence).
  - **L1** — handler chains `.catch()` (clear is best-effort; user is
    routed out even if it fails) — no unhandled rejection path.
  - **L2** — timer-vs-caller aborts discriminated by a `timedOut` flag set
    inside the timer callback; caller abort always wins.
  - **L3** — guest-route match is `pathname === p || startsWith(p + '/')`.
  - **L4** — taxonomy objects spread on return (no shared-reference leaks).
  - **L5** — tests for pre-aborted caller signal and timer cleanup on the
    caller-abort path.
  - **L6** — informational (mount order, ref sync — correct as written).
  - **L7** — this file finalized (STATUS, ACs, gates); the unrelated
    standing rule added to `AGENTS.md` on 2026-11-17 ("Whenever in doubt
    follow industry standards and community best practices") is the
    owner's own edit, not this task's diff — cited here, not reverted.
  - **Observation (not a defect):** login/register `onError →
    resetSessionCaches()` fires on EVERY failed login including typos
    (flush + epoch bump, no-op on empty cache). Documented as fail-safe
    behavior; see lesson note.
- **Gates after fixes (2026-11-17):** `vitest run --project unit` →
  **777/777 across 82 files**; `npx tsc --noEmit` → clean; `npx eslint` on
  all 19 changed/new files → clean (exit 0).

**Open call (resolved):** un-dead `error-messages.ts` — it already has the
taxonomy + tests; D4/D5 amend its rules.

## Lessons

See `docs/lessons/frontend.md` — "Global 401 handling + retry policy +
timeouts (2026-11-17, T014)": single owner for error text with a
server-message-wins rule; global 401 handler = `QueryCache`/`MutationCache`
`onError` + a mount-once `<SessionExpiryHandler />` (TkDodo seam); credential
POSTs and guest routes are exempt; `me` exempt only from redirect/toast,
never from the cache clear; in-flight rollback writes need a `sessionEpoch`
fence; and the review H1 — **a single-flight flag must never arbitrate
eligibility decisions** (exempt paths must bypass it; only the side-effect-
bearing path single-flights).
