# T043: Anonymous `/api/auth/me` fire storm on guest routes

> **STATUS: done (2026-11-18), pending user review.** Phase 0. Found live during T016 verification (2026-11-17):
> the anonymous `/login` page fired ~44 `GET /api/auth/me` requests in ~3 s.
> **This exhausted the backend's per-IP rate limiter (T007) → `429 Too Many Requests`
> → anonymous visitors locked out of login; even logged-in users hit
> `429` → "Failed to load board / Retry" until the window resets.** User-facing.
>
> **Fix (2026-11-18):** the exempt (clear-only) branch of the global 401 handler
> now passes the triggering query to `resetSessionCaches(queryClient, { exceptQueryHash })`.
> The flush spares the bootstrap `['auth','me']` query ONLY when it is the 401's
> trigger AND holds no data; everything else (siblings, mutations, epoch bump,
> `cancelQueries()`) is unchanged. Bootstrap `me` with stale data is still
> destroyed — TanStack keeps `data` on refetch error, so a focus-refetch 401
> must not spare a previous user's identity. Verified live: 1 `me` request per
> guest load, 0 additional after 30 s idle, 0 across 20 consecutive `/login`
> loads, register auto-login adds 0, no 429s. Regression tests green (831/831).

## Goal

Stop the unbounded self-sustaining refetch loop on the guest (unauthenticated)
paths. An anonymous visit to any guest page (`/login`, `/register`,
`/forgot-password`) must fire **at most one** `GET /api/auth/me` (two in dev with
StrictMode double-mounting, which is unavoidable and harmless), and must never
induce the backend's rate limiter no matter how long the visitor idles or how many
guest pages they navigate.

## Depends On

(None — but read T014 first; this is its handler's failure mode.)

## Phase:

0

## Critical:

true — anonymous visitors can be locked out of the app by their own browser's request storm.

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R5 (T014's global-401-handler design).
  Note the design itself says the global handler should act on **any 401 _except_ the
  bootstrap `['auth','me']` query** — the shipped implementation exempts only the
  redirect/toast UX for `me`, not the cache flush, which is exactly the loop below.
- `docs/lessons/frontend.md` — T013 (`resetSessionCaches` contract: cancel → clear,
  epoch counter) and T014 (single-flight on the actionable path only; exempt paths run
  concurrently; race test via direct `QueryCache.config.onError` bursts; "exempt the
  UX, never the hygiene" — this task refines that: an exempt 401 that *is* the query
  being refetched must not be allowed to destroy it in a loop).
- Measured evidence: anonymous `/login` fires **44 × `GET /api/auth/me` in ~3 s**
  (Playwright request counter, 2026-11-17; reproduced in three sessions).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/features/auth/auth-provider/index.tsx` | Likely: exempt the bootstrap `me` query from the global 401 **clear** path (not only from redirect/toast), e.g. drop the whole-cache flush for `skipSessionExpiryRedirect`-style `clearOnly` 401s and only touch `['auth','me']` |
| `frontend/src/components/session-expiry-handler.tsx` | Likely: the `clearOnly` branch must not invoke `resetSessionCaches()` in the refetch loop; drop the single `me` query (`queryClient.removeQueries({ queryKey: ['auth','me'] })`) or skip the clear when the only outstanding entry is `me` |
| `frontend/src/lib/query-client-reset.ts` | Possibly: make `resetSessionCaches()` aware of the identity-only case, or add `removeSessionQuery()` sibling |
| `frontend/src/features/auth/auth-provider.test.tsx` | Regression: mount `AuthProvider` with a failing `meApi`; assert exactly 1 (or 2 in dev) fetch invocations — never unbounded |
| `frontend/src/components/session-expiry-handler` tests | Assert the race/`me`-vs-actionable-401 cases still behave; add the me-401-no-loop case |
| `docs/lessons/frontend.md` | Dated entry (tag `frontend,auth`) |

## Implementation Steps

1. **Reproduce deterministically, before touching code.** In a fresh browser context,
   load `/login`, count `GET /api/auth/me` requests (expect ~44/3s in dev). Also
   confirm the loop does not need StrictMode (it reproduces via a plain clear→refetch
   cycle).
2. **Understand the exact loop against the installed `@tanstack/query-core@5.107.1`**:
   - `useQuery(['auth','me'], meApi, { retry: false, staleTime: 5min, meta: { skipSessionExpiryRedirect: true } })` in `AuthProvider` fires a fetch.
   - The 401 lands in the global `QueryCache.onError` (`SessionExpiryHandler`), whose `clearOnly` branch (guest path or `skipSessionExpiryRedirect`) calls `resetSessionCaches(queryClient)` → `sessionEpoch++` + `cancelQueries()` + `queryClient.clear()`.
   - `clear()` destroys the `me` query; the provider's observer re-subscribes on the re-render; with no `me` entry in the cache the query is missing and `useQuery` re-creates + refetches it → 401 → `clear()` → … forever.
   - Verify the exact re-subscribe/refetch semantics in the installed source — do not guess from v5 docs.
3. **Fix, smallest change that breaks the loop without regressing the guarantees:**
   - Preferred: the `clearOnly` path must not destroy the very query that triggered it. Drop only `['auth','me']` (e.g. `removeQueries`) for exempt 401s, or skip the whole-cache flush when the cache holds nothing else — the identity hygiene goal for a guest page is met (there is no user data to leak on a fresh unauthenticated tab).
   - Keep `resetSessionCaches()` semantics (cancel → clear → epoch) unchanged for actionable (non-exempt) 401s and for logout/login/register — the T013/R2 guarantees (no cross-account leakage on shared tabs, in-flight mutation callbacks cannot re-create the destroyed query) must survive intact.
   - Do not reintroduce the H1 starvation: exempt paths must still not consume the single-flight slot that actionable 401s rely on; keep exempt paths concurrent and idempotent.
4. **Regression tests, deterministic:**
   - Mount `AuthProvider` with a failing `meApi` (401); assert the fetch fires **exactly once** per mount (or twice under dev StrictMode) and then stops — no unbounded loop regardless of timers/renders/refetch triggers.
   - Drive `QueryCache.config.onError` directly with `{ status: 401 }` (the T014 test pattern) and assert `['auth','me']` handling does not touch sibling queries in the exempt path.
   - Assert actionable 401 (non-exempt meta) still triggers navigate(`/login`) + toast + one `resetSessionCaches()`, and the me/racing-401 case still does not starve it (existing H1 test stays green).
5. **Live verification:** fresh anonymous `/login` — ≤2 `/api/auth/me` calls; navigate across guest pages repeatedly — backend stays 200, no 429; then a full login → board flow with no storm.
6. Append the lesson to `docs/lessons/frontend.md` (guest-route "clear on exempt 401" → clearer than "redirect exempt only").

## Constraints

- **No behaviour change for logged-in users**: session-expiry UX (redirect + toast on
  actionable 401), logout/login/register cache flushes (T013/R2), mutation rollback
  `sessionEpoch` guard (T014 R2), and the skipped credential-POST 401 UX must all
  remain intact.
- **Do not weaken the anonymous-visitor guarantee** ("no user-scoped data may leak on
  a shared tab") — the fix removes a *query*, it must not start *serving stale
  identity*.
- Do not touch the rate limiter config to "fix" this client-side bug (that is T007's
  domain and masks the real defect); the fix is client-side.
- Keep `staleTime: 5 min` and `retry: false` on the `me` query unless the verified
  v5 semantics demand otherwise (justify in Notes).
- `@tanstack/query-core` is pinned by the lockfile — verify against the installed
  version, not docs or memory.
- No commit without explicit approval.

## Acceptance Criteria

- [x] Anonymous `/login` renders with **≤ 2** `GET /api/auth/me` requests (1 prod, ≤ 2 dev StrictMode); zero afterwards on idle for 30 s — **verified live 2026-11-18: 1 request on load, 0 additional after 30 s idle**
- [x] Repeated guest navigation never lets the backend respond `429` (e.g. 20 consecutive `/login` loads + idles; assert no 429 surfaces in network log; `429` must never be produced by this page itself) — **verified live 2026-11-18: 20/20 loads → exactly 1 `me` each, only 401s, zero 429s**
- [x] `AuthProvider` with failing `meApi` in a test reproduces the exact 401 path and proves fetch count is bounded (regression test in repo, green) — `auth-provider.test.tsx` » `AuthProvider on a guest route with the global 401 handler mounted (T043)` (pre-fix: 839 calls in ~1.5 s; post-fix: exactly 1)
- [x] Actionable (non-exempt) 401 still causes `resetSessionCaches()` + redirect + toast exactly once (existing T014 suite green); the exempt-path race test stays green — full unit suite: 831/831 green
- [x] T013 guarantees hold: logout/login/register still fully flush the cache (existing tests green)
- [x] Logged-out state never renders a previous user's data (`me` = `null`/absent on guest render) — pinned by the new test "destroys a bootstrap me query that still holds data — stale identity never survives an exempt 401" (`session-expiry-handler.test.tsx`)
- [x] Lesson appended to `docs/lessons/frontend.md` (dated, tagged `frontend,auth`) — 2026-11-18, "Anonymous me fire storm on guest routes (T043)"

## Notes

**Evidence (2026-11-17, T016 verification):** Playwright request counter on a fresh
browser context loading `/login`: `44 × GET /api/auth/me` in ~3 s. A fresh anonymous
browser then sees `429 Too Many Requests` from the backend (T007 per-IP limiter) on
subsequent navigations; the login POST itself was 429'd during smoke runs
("Failed to load board / Retry" — app still functioned, but only after the window reset).

**Root-cause hypothesis to verify (step 2):** the `clearOnly` (guest/`me`) branch of the
global 401 handler in `SessionExpiryHandler` calls `resetSessionCaches()` on **every**
401 — including the one produced by the `me` query itself. `clear()` destroys the
`me` entry; the mounted provider re-subscribes and React Query re-creates + refetches
it; 401 → clear → refetch → … — the exempt-from-UX-but-not-from-hygiene path is the
loop. The R5 spec line ("global handler acts on any 401 **except** the bootstrap me
query") supports the exemption; the shipped code implements exemption only for the
redirect/toast side.

**Interaction with T007:** the storm, not a backend bug, trips the per-IP rate limiter;
every anonymous page load pays ~44 requests toward the budget — the reason a
frequently-loaded guest page locks visitors out of login entirely.

**Loop characteristics to be precise about in tests:** the cycle does not require
StrictMode (the clear→refetch pair is self-sustaining once the provider remounts or
re-subscribes), but dev double-mounting roughly doubles the fire rate, which is why
~44 was seen in dev.

**Implementation notes (2026-11-18, resolving the spec's "preferred" ambiguity):**
- The spec offered two shapes ("drop only `['auth','me']`" vs "skip the flush when the
  cache holds nothing else"). A blanket `removeQueries({ queryKey: ['auth','me'] })`
  for exempt 401s does NOT break the loop — the mounted provider re-creates and
  refetches the destroyed query, whose 401 re-enters the handler. What breaks it is
  *not destroying the data-less bootstrap query at all*.
- Sparing is conditioned on the trigger being the bootstrap `['auth','me']` query
  (`queryKey` + `queryHash`) AND `query.state.data === undefined`, evaluated inside
  `resetSessionCaches` against the live cache. An exempt 401 from any other query,
  or a `me` query still holding last-good data (TanStack keeps `data` across a
  failed background refetch), gets the full `clear()` — a destroyed data-less `me`
  refetches once and is spared then, so the path is bounded at ≤ 2 `me` requests
  ever, and zero stale identity can survive on a shared tab.
- Exempt non-bootstrap triggers are NOT spared (unlike an earlier intermediate
  shape): a mounted errored user-scoped query on a guest route would otherwise
  survive the flush on a shared tab — caught by the existing
  `clear-only on a guest route` test, which pins full-flush semantics for
  non-bootstrap exempt triggers.
- Direct `QueryCache.config.onError` drives cannot see the refetch cycle; the
  regression test mounts `AuthProvider` + `SessionExpiryHandler` + failing `meApi`
  and asserts call count after quiescence (pre-fix repro: 839 calls / ~1.5 s).
- Verified against installed `@tanstack/query-core@5.107.1` source:
  `clear()` ⇢ `QueryCache.remove` ⇢ `query.destroy()`; refetch happens via the
  observer's `setOptions → #updateQuery → cache.build()` path
  (`shouldFetchOptionally` when `query !== prevQuery` and `isStale`), not via
  `useSyncExternalStore` re-subscription.
