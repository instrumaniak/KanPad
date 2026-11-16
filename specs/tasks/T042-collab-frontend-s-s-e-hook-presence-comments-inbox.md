# T042: Collab: frontend (SSE hook, presence, comments, inbox)

> **STATUS: pending.** Phase 2. Wave 2 / row L. **Closes S6** client-side (the
> realtime loop) and the **frontend half of S5**. The biggest Wave 2 task — may
> land as **two PRs** (PR 1: SSE hook + presence avatar stack + live-paused
> ladder; PR 2: comment thread + inbox + watch toggles) under one task file.

## Goal

Frontend for realtime, presence, comments and the inbox, per
`31-realtime-sse.md` §6–7, `32-comments-mentions.md` §7, `33-notifications.md`
§7 — composing with (never bypassing) T013's cache hygiene and the existing
`apiFetch`/`useInfiniteQuery` conventions.

1. **`useWorkspaceEvents()`** (`frontend/src/hooks/` — shared, not
   feature-co-located): one `EventSource('/api/v1/events', {withCredentials:
   true})` per tab; event-type → `queryClient.invalidateQueries` map
   (`card.*|column.*|board.*` → `['columns', boardId]` + `['boards']`;
   `note.*` → `['notes']`, `['note', id]`; `comment.*` → `['comments',
   subjectType, subjectId]`; `notification.*` → `['notifications']`,
   `['notifications-unread-count']`; `presence.*` → `['presence', boardId]`;
   `member.revoked` → route per 401 path, else broad invalidate); `resync` →
   one broad (bounded) `invalidateQueries()`.
2. **Fallback ladder:** `onerror` → close → backoff reconnect (1, 2, 4… max
   30 s, ±20 % jitter) → after **3 consecutive failures**, board view switches
   to `refetchInterval: 10_000` (visible tabs only) + a non-blocking
   **"live updates paused"** pill (Radix toast/inline badge, never a dialog) →
   manual reload re-enables SSE; `visibilitychange` → re-open stream. All
   paths converge on the same invalidation targets — no duplicate code paths.
3. **Presence UI:** avatar-circle stack in the board header (max 5 + "+N";
   hover = email); `['presence', boardId]` hydrated from the heartbeat response;
   live via `presence.update` deltas; `POST /presence/heartbeat` every **20 s**
   (no-op when not viewing a board; `navigator.sendBeacon` on unload);
   `useWorkspaceEvents` keeps the roster fresh.
4. **Comment thread** — in the card detail panel (`features/cards/`) and a
   comments tab in the notes view: auto-growing textarea + 10k counter (soft
   warn at 9k), Radix primitives only, **`@` dropdown** (keyboard nav, fed by
   `GET /workspaces/{id}/members?search=`, inserts the atomic `@[user:id]`
   token), submit on `Ctrl/Cmd+Enter`, **optimistic post with `client_token`**
   + temp-id reconciliation (replace with server row; rollback on error),
   edited badge (`edited_at`), **mention chips** rendered as React nodes after
   sanitize (never HTML-built), delete-confirm (Radix `Dialog`),
   **"Show more" infinite scroll** (`useInfiniteQuery`, page>1 dedupes keys),
   empty state ("Be the first to comment").
5. **Sanitization:** markdown subset pipeline — `DOMPurify.sanitize()` **and**
   `rehype-sanitize` (allow `a,p,strong,em,code,pre,ul,ol,li,br`, `class` on
   `code` only); no headings/images/mermaid/raw HTML; `rehype-highlight`
   reused; link protocols restricted to http(s)/mailto.
6. **Inbox:** header bell (Radix `Popover`) with badge (**`99+` cap** — O-20
   default); inbox panel with **All / Mentions tabs** (each with its own
   unread count — O-14 default); rows = actor avatar/email, one-line summary
   ("X mentioned you in card Y"), relative time, unread dot; row click →
   mark read + **deep-link** with scroll + 2 s highlight (`?card=&comment=` /
   `?comment=` on notes); per-row hover actions (read/unread, dismiss);
   footer **"Mark all as read"**; skeleton on first paint; `unread-count`
   failure → badge hidden, never blocks the app; v1 ships **no per-type
   mutes** (O-18 — v2).
7. **Watch toggle:** card/board "watch" affordance (`PUT/DELETE
   /watches/…`), showing current state (hydrate from… watch has no list
   endpoint — track by optimistic toggle + local state; document the chosen
   approach in §Notes).
8. **Board activity panel** — "who changed this" feed next to the board view,
   consuming T037's `GET /boards/{id}/activity` (paged, authz-scoped); renders
   actor + action + relative time; collapsed by default.
9. **E2E:** A comments on a card B watches → B's badge increments ≤ 5 s (SSE
   path); B opens inbox → deep link scrolls + highlights; **kill `/events`**
   (route abort) → board still updates ≤ 15 s (polling ladder); viewer role →
   composer hidden/disabled (O-7).

## Depends On

T035 (E — members/search endpoints), T039 (I — sharing UX patterns/sidebar),
T040 (J — the event contract this client consumes), T041 (K — comments/inbox
APIs). **Practical note:** the E2E portion (§9) runs against the Playwright
harness retargeted by T017 — if T017 has not landed when this task starts, the
E2E scenarios are written but cannot run in CI until it is; do not silently drop
them. (Taskpad depends edges are per the normative `30` §6 sequencing and do not
include T017 — this note is informative, not a new edge.)

## Phase

2

## Critical

No.

## Spec References

- `specs/arch-hardening-specs/31-realtime-sse.md` §6 (client integration —
  hook, event→invalidation map, fallback ladder, `staleTime` stays), §7
  (presence UI), §12 (fallback E2E).
- `specs/arch-hardening-specs/32-comments-mentions.md` §5 (rendering/
  sanitization), §7 (frontend expectations — thread in card detail panel +
  notes tab, composer rules, deep links).
- `specs/arch-hardening-specs/33-notifications.md` §5 (read-time resolution —
  actor email via `GET /users?ids=`; lost-access rows render
  *"An item you no longer have access to"*), §6 (badge via `notification.new`),
  §7 (inbox UI — All/Mentions tabs, `99+` cap, no v1 mutes).
- `frontend/CONVENTIONS.md` (Radix-only; `apiFetch`; React Compiler; no
  `document.querySelector` — refs/callback props), `frontend/src/hooks/`
  (hook placement).
- `specs/arch-hardening-specs/00-overview.md` — findings **S5**, **S6**; global
  DoD (frontend gates; no test weakening; traceability).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — types flow from
  `schema.d.ts` regen (T041); no parallel DTO copies.
- `frontend/playwright.config.ts` + `e2e/` (API-driven fixtures; never
  `waitForTimeout`; console-error monitor must stay quiet).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/src/hooks/use-workspace-events.ts` | **NEW** — the SSE hook + fallback ladder + visibilitychange re-open |
| `frontend/src/hooks/__tests__/use-workspace-events.test.tsx` | **NEW** — fake `EventSource`: map correctness, backoff sequence, 3-failure → poll mode, pill render, recovery on visibilitychange |
| `frontend/src/features/presence/` | **NEW** — avatar stack component (max 5 + N), heartbeat hook |
| `frontend/src/features/comments/` | **NEW** — thread, composer, mention dropdown (Radix), chips, edit/delete, infinite scroll, deep-link scroll+highlight |
| `frontend/src/features/notifications/` | **NEW** — bell + badge (`99+`), All/Mentions panel, row actions, deep-link click, `unread-count` query (`['notifications-unread-count']`) |
| `frontend/src/features/cards/card-detail-panel.tsx` | embed comments tab/section |
| `frontend/src/features/notes/` | notes comments tab |
| `frontend/src/features/boards/` | `useWorkspaceEvents` wired at board view root; "recent activity" panel (collapsed); presence stack in header |
| `frontend/src/features/watches/` or in feature dirs | watch toggle on card/board (optimistic) |
| `frontend/e2e/` | new spec: share+comment → badge ≤5 s; deep-link highlight; `/events` abort → poll fallback ≤15 s; viewer composer hidden |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** server changes (T040/T041 own them), presence
for MCP agents, typing indicators (documented stretch only), per-type mutes
(O-18 — v2), CRDT/collab editing, email delivery surfaces.

## Implementation Steps

1. **`useWorkspaceEvents` + tests first** (red): fake EventSource drives
   connect/error/message; assert the invalidation map for each `type`; force 3
   consecutive errors → `refetchInterval` active + pill rendered; succeed
   visibilitychange → stream re-opens; `resync` → broad invalidate once.
2. **Wire at board view** (and app root for notifications badge): one hook
   instance per tab; guard duplicate mounting (React StrictMode double-mount
   — verify the hook tolerates mount/unmount/mount cleanly).
3. **Presence**: heartbeat effect (20 s; beacon on unload); avatar stack from
   `['presence', boardId]`; `presence.update` merges deltas.
4. **Comments**: TDD component tests — optimistic post (temp id → reconciled),
   rollback on error with toast, mention insert via dropdown (keyboard:
   ↑↓/Enter/Esc), sanitizer test (`<script>`, `javascript:` links stripped;
   garbage token `@[user:abc]` renders as plain text), edit sets badge, delete
   confirm flows, infinite scroll dedupes keys.
5. **Inbox**: component tests — badge cap 99+; All/Mentions partition by type;
   per-tab unread counts; row click → read + navigate with `?comment=` +
   highlight; hover actions; "Mark all as read"; error → badge hidden.
6. **Watch toggle**: optimistic flip; rollback; invalidates related comment
   lists (author/watcher sees new comments without refresh).
7. **Board activity panel**: fetch page 1 on expand only (collapsed default);
   render rows; no PII beyond actor email (already-sanitized API).
8. **E2E** (API-driven setup; never `waitForTimeout`): the four scenarios in
   Step "Goal §9" — all green locally; console-error monitor quiet.
9. Gates: `tsc --noEmit`, `vitest`, `npm run gen:api -- --check`, coverage
   floor; lesson.

## Constraints

- **Server is the authority** — UI hides/forbids nothing the server permits or
  vice versa; viewer role: composer not rendered (O-7).
- Radix primitives only; no raw DOM mutation; `document.querySelector`
  forbidden (refs/callback props).
- No new state libraries; React Query + component state only; React Compiler
  must stay effective (no prop-identity churn in the drag path — T021's
  invariants hold).
- `client_token` = UUID per composed comment; regenerated on retry after
  failure; stable across re-submit.
- Notification deep links land through the router (invalidate `['notifications']`
  after click-mark).
- Open calls O-7/O-14/O-18/O-20 defaults stand pending T027 ratification —
  amend if overridden.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] `useWorkspaceEvents` ladder test matrix green (event map, backoff, 3-fail
      polling mode + pill, recovery) — closes **S6** (client half) with
      traceability to the findings register.
- [ ] Realtime E2E green: B's board reflects A's edit ≤ 5 s (SSE); with
      `/events` aborted, ≤ 15 s via polling (fallback ladder proven end-to-end).
- [ ] Presence: opening the board in two sessions shows both avatars in the
      stack; closing/TTL removes within TTL+sweep window; foreign board never
      shows presence (404 path).
- [ ] Comments: user story "mention → inbox → deep-link → reply" works E2E;
      optimistic insert reconciles; sanitizer strips XSS vectors (test);
      moderation delete renders the placeholder, never the old body.
- [ ] Inbox: tabs partition correctly; badge caps at 99+; click-mark-read +
      deep link; failure hides badge without blocking the app. Closes the
      **S5** frontend half.
- [ ] Watch toggle: subscribe → comment from another user arrives in B's
      inbox (E2E with API-driven setup).
- [ ] Viewer role cannot post (composer hidden) — verified cross-user.
- [ ] No `waitForTimeout` anywhere in the new E2E/specs; console-error monitor
      quiet.
- [ ] Frontend gates green (`tsc`, `vitest`, gen:api check); no test weakened;
      `frontend/CONVENTIONS.md` "hook placement" verified for
      `src/hooks/`.
- [ ] Lesson appended to `docs/lessons/frontend.md`.

## Notes

(filled in during/after implementation — record the pill copy, the
`['presence']` merge strategy, the watch-toggle hydration approach, and any
T027-ratified UI decisions)

## Lessons

(pending)
