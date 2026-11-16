# 31-realtime-sse — Wave 2 design: real-time updates via SSE + cache invalidation

> **Design-only** (no implementation in T027; implementation tasks are authored by taskpad
> after design review — see `30-collaboration.md` §6 rows J/K/L). Transport decision **D-7**
> (2026-11-16): **Server-Sent Events carry invalidation metadata only; clients respond by
> invalidating React Query caches.** This file is the single owner of the realtime design.
> Constitution: `00-overview.md`. Sibling designs: `30-collaboration.md` (domain model, authz
> kernel, activity log), `32-comments-mentions.md`, `33-notifications.md`.
>
> *(Amendment 2026-11-16: new file. Promotes realtime from the D-2 "deferred" clause to
> designed scope; D-2's 100+-user performance urgency is unchanged.)*

## Goals (from the product plan)

"Real-time updates" for the organization-wide (100+ users) cross-functional collaboration
use case: a collaborator's edit on a shared board is visible to other viewers in seconds,
without a manual refresh, and the notification badge updates live — on a self-hosted,
single-box, MySQL-only deployment with **zero new infrastructure**.

## Non-goals

WebSockets (bidirectional, no use case — every write is an ordinary POST/PATCH),
CRDT/OT collaborative document editing, presence for MCP agents, typing indicators
*as a build* (documented stretch only), cross-replica fanout (scale-out readiness is
an S2 note; a Redis backplane is explicitly deferred), client→server messaging over
the event stream.

## 1. Decision & alternatives considered

| Option | Verdict | Why |
|--------|---------|-----|
| **SSE (`text/event-stream`) for cache invalidation** | **Chosen (D-7)** | Server→client only; built-in reconnect + `Last-Event-ID`; plain HTTP through every proxy that doesn't *buffer* (verified requirement, §9); cookie auth just works (same-site subdomains → `SameSite=Lax` cookies are sent); React Query already owns server state, so the client reaction is one `invalidateQueries` call per event; zero new services (in-process `tokio::sync::broadcast`) |
| WebSocket | Rejected | Bidirectional overhead with no product need; upgrade handshake through LiteSpeed/proxies is fragile; manual heartbeat/reconnect protocol; identical fanout mechanism in-process anyway |
| Long-polling as primary | Rejected | SSE is strictly better latency/conn-cost; keep polling as the automatic *fallback* only |
| React Query `refetchInterval` alone | Rejected as primary | 15–30 s staleness defeats multi-user editing; kept as fallback (§6) |
| MySQL `LISTEN/NOTIFY` / triggers | Rejected | MySQL 8.4 and MariaDB 11.4 have no LISTEN/NOTIFY; trigger-based fanout couples writes to notification and gives no delivery guarantees. The **activity_log row is the event cursor** — the app publishes post-commit, in-process |

Research basis: SSE is the community-standard answer for "server→client invalidation feed"
(automatic resume via `Last-Event-ID`, HTTP/2 multiplexing, proxy-friendly if unbuffered);
the "database is the source of truth, realtime is a notification channel" pattern is
followed throughout — events **carry no resource bodies**, only metadata.

## 2. Architecture

```
HTTP mutation (route handler, existing features/*/routes.rs)
  └─ in ONE sqlx transaction: mutate + INSERT activity_log row (id = cursor)
  └─ post-commit (on Ok): EventBus::publish(Event { ... })   ← in-process, tokio::sync::broadcast
       └─ per-connection subscriber task (spawned per SSE connection)
            └─ Event -> SSE frame: id: <activity_log.id>  data: {json}
```

- **Event envelope** (JSON, `snake_case`, bounded — no bodies, no PII, no email):
  `{ "id": int, "type": "board.updated|column.*|card.*|note.*|comment.*|notification.*|presence.*|member.*",
     "workspace_id": int|null, "subject_type": "board|column|card|checklist|note|comment|notification",
     "subject_id": int, "via": "browser|mcp|cli", "actor_key_id": int|null, "occurred_at": iso8601 }`
- **`id`:** frames emitted from an `activity_log` mutation carry `id: <activity_log.id>`
  (the resume cursor, §4). Presence frames carry **no `id:` line** — they must never advance
  the browser's `Last-Event-ID` (it would corrupt the audit cursor), and they are never
  replayable; presence state is always hydrated fresh from the heartbeat response
  (§7) or from `presence.update` deltas — never replayed.
- **Bus:** one `tokio::sync::broadcast::Sender<EventEnvelope>` in `AppState` (capacity
  **1024**). Publish happens **after commit** — a rolled-back transaction must never emit
  an event. A `Lagged(n)` receiver error maps to a `resync` frame (client broad-invalidates),
  never to a panic or silent drift.
- **`activity_log` insert stays synchronous** (per `30-collaboration.md` §5 / MCP constraint
  #4). The bus is a fan-out of *notifications*, not a source of truth; if the bus drops a
  packet, nothing persists incorrectly — the client refetches.

## 3. Transport

**`GET /api/v1/events`** — one stream per browser tab.

- `Content-Type: text/event-stream`, `Cache-Control: no-store`,
  **`X-Accel-Buffering: no`** (tells downstream proxies not to buffer), `Connection: keep-alive`.
- Compression: the route is **excluded from gzip/br** (compression middleware must skip
  `text/event-stream`, else buffers defeat streaming — verified against `docs/deployment.md`
  two-origin findings; the exclusion is a spec'd requirement, §9).
- **Auth:** the session cookie (`kanpad.sid` / `__Host-kanpad.sid`) + `AuthSession`
  extractor. Unauthenticated → `401` problem+json (never an empty stream). CSRF: safe
  method, no state change — immune; must remain side-effect-free forever.
- **Membership gate:** at connect, the handler resolves the user's visible workspace set
  (`SELECT workspace_id FROM workspace_members WHERE user_id = ?` — indexed). Connections
  are unauthorized for everything else. See §5 for revocation.
- **Heartbeat:** SSE comment line (`: hb\n\n`) every **25 s** — defeats proxy idle timeouts
  (LiteSpeed/nginx default 60 s) and detects half-open connections.
- **`Last-Event-ID`** honoured on reconnect (§4).

**Topology notes (both supported deployment paths):**

1. **Docker appliance (D-1):** same-origin vs two-origin irrelevant to buffering — the
   container emits unbuffered flush per frame; verify the shipped compose file sets no
   `proxy_buffering` on the API route (T012 follow-up, spec'd here).
2. **cPanel / LiteSpeed two-origin (D-1):** the SPA host (`.htaccess`) does **not** proxy
   SSE; the API origin does. Requirements: LiteSpeed must not buffer SSE responses
   (`X-Accel-Buffering: no` honoured or server config adjusted — documented in the
   operations runbook), the Node/app proxy (`scripts/app-proxy.js`) must flush per
   frame and must forward `Host` unchanged (T007 rule), `Refer`/`Origin` rules unchanged.
   The two origins are **same-site** (subdomains under one registrable domain) so
   `SameSite=Lax` cookies are sent on `/events`. If an org deploys the SPA and API on
   **different registrable domains**, `Lax` blocks the cookie — documented as unsupported
   for SSE; the client falls back to polling (§6), which already works cross-site today.

## 4. Resume & resync (`Last-Event-ID`)

- On reconnect the browser sends `Last-Event-ID: <activity_log.id>`. The handler:
  1. Re-authenticates + re-reads the visible workspace set (cheap, indexed).
  2. `SELECT MAX(id) FROM activity_log WHERE workspace_id IN (<visible_set>) OR workspace_id
     IS NULL` — if `id <= last_seen`, send nothing; client is current (no replay needed —
     it has already invalidated). (Filtered, not global MAX: a gap that contains no visible
     events must not force a resync.)
  3. If there is a gap, **do not replay individual rows** (a burst would re-send events for
     workspaces the user may have been removed from). Instead send one frame:
     `event: resync  data: {"from_id": <last_seen>}`. The client performs a **broad
     invalidation** (all affected queries: boards, columns, cards, notes, comments,
     notifications). Resync is idempotent and scope-safe. Presence is not part of resync —
     the `['presence', boardId]` query is invalidated like any other and re-hydrates from
     the next heartbeat.
- Presence frames are never replayed; on `resync`, clients also discard the presence map
  and accept the next heartbeat-hydrated roster (§7).

## 5. Fan-out & authorization at scale

- **Per-connection state:** `HashMap<workspace_id, Vec<ConnectionId>>` + `HashMap<ConnectionId, UserContext>` in process memory (guarded by a `tokio::sync::RwLock`). ~100 users × 2 tabs ≈ 200 connections — trivial.
- **Filtering:** a published event is delivered only to connections subscribed to its
  `workspace_id` (resource-scoped `board_members`/`project_members` grants are resolved at
  connect time into the visible set — same rule as lists; see `30-collaboration.md` §2).
- **Revocation:** membership mutations publish `member.revoked` / `member.added` carrying
  `{workspace_id, user_ids[]}`. Subscribers for the affected users re-read membership: on
  `revoked` they drop the workspace from their filter (send a `member.revoked` frame first);
  on `added` they extend it (no reconnect needed). Stale cookies cannot leak events: **each
  frame batch is authorization-filtered after the membership re-check**.
- **Connection cap:** **6 per user** (configurable; mirrors typical tab + tool count).
  Exceeding → oldest tab's stream is closed with a problem+json `429` (no `Retry-After`
  needed; this is protection, not throttling) — client-side hook closes politely and shows
  a non-blocking "live updates paused" pill; polling fallback continues to work.
- **Metrics (feeds T024):** `kanpad_sse_connections`, `kanpad_sse_events_published_total{type}`, `kanpad_sse_events_dropped_total{reason=lagged|filter|cap}`, `kanpad_sse_resyncs_total`, `kanpad_sse_heartbeat_failures_total`.

## 6. Client integration

- New hook `useWorkspaceEvents()` in `frontend/src/hooks/` (shared, not feature-co-located):
  - Opens one `EventSource('/api/v1/events', { withCredentials: true })` per tab.
  - Maps event `type` → `queryClient.invalidateQueries` targets:
    `card.*|column.*|board.*` → `['columns', boardId]` (board view renders from the columns
    tree), `['boards']`; `note.*` → `['notes']`, `['note', id]`; `comment.*` →
    `['comments', subjectType, subjectId]`; `notification.*` → `['notifications'],
    ['notifications-unread-count']`; `presence.*` → `['presence', boardId]`;
    `member.revoked` → route to `/login` if the 401 path triggers, else broad invalidate.
  - `resync` → `queryClient.invalidateQueries()` (broad, but bounded by the client's
    mounted queries; called once per resync).
- **Fallback ladder:** `EventSource.onerror` → close → exponential backoff reconnect
  (1, 2, 4, … max 30 s, ±20 % jitter) → after 3 consecutive failures, switch the board
  view to React Query `refetchInterval: 10_000` (only while `document.visibilityState ===
  'visible'`) and show the "live updates paused" pill → manual reload re-enables SSE.
  On `visibilitychange` → re-open the stream. All paths converge on *the same* invalidation
  targets — no duplicate code paths.
- React Query `staleTime` stays as today (60 s+); invalidation now usually beats it.
- The pill is non-blocking (Radix toast/inline badge, not a dialog).

## 7. Presence (in-scope per review, 2026-11-16)

- **Ephemeral, never persisted, never audited.** Server memory only.
- **Heartbeat:** client POSTs `POST /api/v1/presence/heartbeat {board_id}` every **20 s**
  (no-op if the user is not viewing a board; `navigator.sendBeacon` on `unload`). The
  endpoint is write-only, body-bounded, rate-limited (global per-IP bucket), and enforces
  the same board-read authz — you cannot announce presence on a board you cannot read.
- **State:** `HashMap<board_id, HashMap<user_id, Instant>>`, TTL **45 s**; lazy sweep on
  read + on each broadcast (no background task — S4 preserved).
- **Distribution:** on heartbeat that changes membership of a board's viewer set (join /
  leave / TTL expiry), publish `presence.update` with `{board_id, user_ids[]}` to that
  board's subscribers only ("who is viewing", never names-of-activity). Debounced: max one
  `presence.update` per board per 2 s.
- **Response:** `200 {data: {board_id, user_ids[]}}` — the heartbeat response carries the
  current viewer set so `['presence', boardId]` is always hydrated; SSE deltas keep it live.
  Presence rows never enter `activity_log` or `notifications`.
- **UI:** avatar-circle stack in the board header (max 5 + "+N"), hover = email; source
  `['presence', boardId]` hydrated by heartbeat responses (initial roster) + SSE deltas.
- **Scale-out:** per-process only — documented; cross-replica presence needs a shared
  store (deferred, S2).
- **Stretch (not committed):** comment-box "user is typing…" reuses the same heartbeat
  channel with a `typing: true` field; distinct UI; still ephemeral.

## 8. Activity read models (feed the "who changed what" UX)

- **`GET /api/v1/boards/{id}/activity?page&limit`** (viewer+; paged per contract):
  `activity_log` filtered to `workspace_id` (or subject chain) with `subject_id` in the
  board's columns/cards or the board itself; ordered `created_at DESC`, limit ≤ 100.
  Joins `users` for actor email (LEFT JOIN — actor may be deleted → "Deleted user").
- **`GET /api/v1/workspaces/{id}/activity`** (admin/owner+; filters `actor`, `action`,
  `subject_type`, `from`, `to`) — backs the admin panel (Wave 2 row H) and the
  "recent activity" tab.
- Both are authz-scoped (404-no-oracle); both read only `activity_log` — no new tables.
- The realtime layer and these feeds are deliberately complementary: SSE says *"something
  changed"*, the feed says *"what, who, when"*.

## 9. Security requirements (standing)

1. SSE is cookie-authenticated, `HttpOnly`, `SameSite=Lax`; no tokens in URLs, headers the
   browser can't send, or `EventSource` query params.
2. Events carry **no bodies, titles, descriptions, emails, or PII** — a recipient who gains
   board access legitimately *caused* the event; the payload itself reveals only
   `(type, subject_type, subject_id, occurred_at)`; subject resolution always goes back
   through authorized GETs.
3. Member-removal path (§5) is mandatory, not best-effort — stale viewers must stop
   receiving the stream within one frame.
4. `text/event-stream` responses: `Cache-Control: no-store`; nothing is cached, ever.
5. `GET /events` is excluded from request-body limits and CSRF mutation checks by virtue
   of being safe; it must carry **zero handlers** that mutate.
6. Presence heartbeat validates authz (board read); it cannot be used as a membership
   oracle (unknown/foreign board → 404, per house convention).
7. Mention/notification fanout is independent of this stream — see `32`/`33`.

## 10. Performance & capacity (100+ users)

| Path | Cost | Notes |
|------|------|-------|
| SSE connect | 1 query (membership); no writes | O(workspaces of user) |
| Frame publish | 1 INSERT inside mutation tx (existing §5 cost) + in-process fanout | No extra DB round-trip |
| Frame delivery | O(subscribers of workspace) sends | ~200 conns max; each frame < 1 KB |
| Heartbeat | 1 tiny UPDATE-free payload every 25 s/conn | ~8 req/s at 200 conns — negligible |
| Resync gap check | 2 queries | Only on reconnect |
| Presence heartbeat | 1 authz read + O(1) map update / 20 s/user | 5 req/s at 100 users |
| Fallback poll | 1 board fetch / 10 s / visible tab | Bounded; only in degraded mode |

Buffers: `broadcast` capacity 1024; a user lagging >1024 events gets one `resync`, not 1024
frames. Connection cap 6/user backstops runaway tabs. All counters exported (§5).

## 11. Contract & amendments (T52 same-PR)

- New endpoints: `GET /api/v1/events`, `POST /api/v1/presence/heartbeat`,
  `GET /api/v1/boards/{id}/activity`, `GET /api/v1/workspaces/{id}/activity` (nav/admin).
  OpenAPI + `schema.d.ts` regenerated in the same PR; `/events` is a **non-JSON** response
  (`text/event-stream`) — documented in `20-api-contract.md` as such (never problem+json).
- Amends: `api-rewrite-rs-specs/20-api-contract.md` (SSE section),
  `api-rewrite-rs-specs/30-db-schema.md` (`activity_log` additions carried by `30`, presence
  has no table), `frontend/CONVENTIONS.md` (hook placement), `docs/deployment.md`
  (§9 requirements for both paths).
- **MCP note:** Wave 3's "realtime deferred" statements (`mcp/91-task-map.md` §6,
  `50-frontend.md` §6) remain accurate — this design is invalidation-only; the agent/tool
  surface is unchanged. No MCP files are amended.

## 12. Test plan (TDD — failing tests first)

- **Wire (black-box, `tests/api/wire.rs` style):** user A and B on shared board; A creates
  card → B's stream receives `card.create` with the new `subject_id` within 2 s;
  user C on a different workspace receives nothing in 2 s.
- **Authz:** remove B from workspace mid-stream → B's next frame batch is closed/filtered;
  B sees no further events (and receives `member.revoked`).
- **Resume:** disconnect B for 30 s; A mutates; B reconnects with `Last-Event-ID` →
  B receives exactly one `resync` frame; broad invalidation (client unit test).
- **Auth:** no session → 401; stale/foreign board presence heartbeat → 404.
- **Fallback:** React unit tests for the onerror ladder (backoff, pill render, poll mode
  activated after 3 failures, reactivation on visibilitychange); E2E: kill `/events`
  (route abort) → board still updates within 15 s.
- **Persistence rule:** presence/notifications assert no `activity_log` rows written for
  heartbeats; comment/mutation tests assert `activity_log` rows co-committed (rollback on
  failure).
- **Load sanity (T026 input):** 200 concurrent SSE connections + mutation storm; assert
  p99 frame latency < 500 ms and zero 500s; absence of heartbeat timeouts.

## 13. Open calls (decide at T027 review)

- **O-8:** presence visible to `viewer`-role board guests (recommended yes — read-only
  social signal).
- **O-9:** `events` stream includes `admin.*` frames (recommended no — admin panel polls;
  keeps the stream minimal).
- **O-10:** compression exclusion implemented as middleware rule (`Accept: text/event-stream`
  bypass) vs route-level (recommended route-level, explicit > clever).
- **O-11:** per-user connection cap 6 (recommended) vs 4.
