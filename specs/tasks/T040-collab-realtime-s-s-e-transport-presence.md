# T040: Collab: realtime SSE transport + presence

> **STATUS: pending.** Phase 2. Wave 2 / row J. **Closes S6** (HIGH — no realtime
> invalidation: collaborators' edits invisible until staleTime/manual refresh)
> via D-7 (**SSE + React Query invalidation; metadata-only frames; in-process;
> zero new infrastructure**). Server→client only — no WebSockets, no CRDT.

## Goal

Implement the transport half of `31-realtime-sse.md` — **backend only** (the
client hook lands in T042):

1. **Bus:** one `tokio::sync::broadcast::Sender<EventEnvelope>` (capacity
   **1024**) in `AppState`; published **after commit** only; `Lagged(n)` →
   `resync` frame (never panic, never silent drift).
2. **`GET /api/v1/events`** — one `text/event-stream` per browser tab:
   `Cache-Control: no-store`, **`X-Accel-Buffering: no`**, `Connection:
   keep-alive`; **excluded from compression** (route-level, **O-10 default**);
   cookie auth via `AuthSession`; unauthenticated → 401 (never an empty
   stream); side-effect-free forever.
3. **Membership gate at connect:** visible workspace set via the T032 indexed
   membership query; resource-grant resolutions included (same rule as lists).
4. **Fan-out:** per-connection `HashMap<workspace_id, Vec<ConnectionId>>` +
   `HashMap<ConnectionId, UserContext>` in process memory (`tokio::sync::RwLock`);
   frames delivered only to subscribers of the event's workspace.
   **Revocation (mandatory, not best-effort):** membership mutations publish
   `member.revoked`/`member.added` carrying `{workspace_id, user_ids[]}`;
   affected connections re-read membership — on revoked, the workspace is
   dropped from the filter (after sending a `member.revoked` frame); on added,
   extended — no reconnect needed. **Each frame batch is authorization-filtered
   after the re-check.**
5. **Resume (`Last-Event-ID`):** re-auth + re-read memberships; filtered
   `SELECT MAX(id) FROM activity_log WHERE workspace_id IN (set) OR
   workspace_id IS NULL` — if `<= last_seen`, nothing sent; else **one**
   `event: resync; data: {"from_id": <last_seen>}` frame — **never per-row
   replay**. Presence frames carry **no `id:` line** (they must not advance the
   browser's cursor).
6. **Heartbeat:** SSE comment line (`: hb`) every **25 s**; half-open detection
   feeds `kanpad_sse_heartbeat_failures_total`.
7. **Connection cap: 6 per user** (**O-11 default**); exceeding → oldest stream
   closed with problem+json `429` (no `Retry-After`; protection, not
   throttling — client handles politely in T042).
8. **Presence (ephemeral, server-memory only; never `activity_log`):**
   `POST /api/v1/presence/heartbeat {board_id}` — board-read authz (foreign
   board → 404), rate-limited (global per-IP bucket), body-bounded, write-only;
   `HashMap<board_id, HashMap<user_id, Instant>>`, TTL **45 s**, lazy sweep on
   read + on broadcast (no background task — S4); on viewer-set changes publish
   `presence.update {board_id, user_ids[]}` to that board's subscribers,
   **debounced ≤ 1 frame per board per 2 s**; response
   `200 {data: {board_id, user_ids[]}}` (initial roster hydration);
   `navigator.sendBeacon` support is the client's concern (T042).
9. **Metrics (feeds T024's conventions):** `kanpad_sse_connections`,
   `kanpad_sse_events_published_total{type}`,
   `kanpad_sse_events_dropped_total{reason=lagged|filter|cap}`,
   `kanpad_sse_resyncs_total`, `kanpad_sse_heartbeat_failures_total`.
10. **Publishing contract (T037 ordering rule):** the mutation path inserts the
    `activity_log` row **inside** the tx (T037), publishes to the bus
    **post-commit**; presence/notifications never publish with an `id:` line.
11. **Docs:** `docs/deployment.md` (and/or the 40-operations runbook reference)
    gains the **LiteSpeed/proxy requirements** — SSE unbuffered, proxy flush per
    frame, `Host` unchanged (T007 rule), SPA↔API **same-site** registrable
    domain (SameSite=Lax cookies; cross-reg-domain deployments unsupported for
    SSE → client polling fallback).

## Depends On

T032 (kernel — the handshake re-uses the indexed membership query) and
**T037** (the bus publishes `activity_log.id` as the resume cursor; member
change events ride the same post-commit path). **Coupling note:** member/share
mutation coupling to the activity log + bus is governed by the second of
{T035, T037, T040} to land (see T035/T037 §Notes) — this task records its side
of the rule here rather than assuming T037 covers every hook.

## Phase

2

## Critical

Yes — gates T041 (K, publishes through the same bus/post-commit path) and T042
(L, the client ladder consumes this contract).

## Spec References

- `specs/arch-hardening-specs/31-realtime-sse.md` — **the full design**:
  §1 decision D-7, §2 architecture (bus, envelope, post-commit publish), §3
  transport (headers, auth, membership gate, heartbeat, topology notes —
  including Docker appliance + LiteSpeed requirements), §4 resume/resync
  (filtered MAX, single `resync` frame), §5 fan-out & revocation, connection
  cap, §7 presence (heartbeat, TTL, debounce, response shape), §9 security
  standing requirements, §10 capacity targets, §11 contract amendments, §12
  test plan, §13 open calls (**O-8 presence visible to viewer guests — yes;
  O-9 `admin.*` frames on the stream — no; O-10 route-level compression
  exclusion; O-11 connection cap 6**).
- `specs/arch-hardening-specs/30-collaboration.md` §2 (SSE handshake resolves the
  visible workspace set with the same membership query), §5 (cursor row exists
  before frames reference it).
- `specs/arch-hardening-specs/33-notifications.md` §6 (`notification.new` rides
  the bus — wired by T041), `32-comments-mentions.md` §2 (no SSE endpoint shape
  in the comment contract).
- `specs/arch-hardening-specs/00-overview.md` — findings **S6** (and S1
  governance); global DoD; **D-7** (2026-11-16 decision).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — **amendment target** (T52):
  `GET /api/v1/events` (**non-JSON** response documented as such; never
  problem+json), `POST /api/v1/presence/heartbeat`, the two activity read APIs
  ship via T037.
- `specs/mcp/91-task-map.md` §6 — Wave 3 "realtime deferred" stays accurate
  (invalidation-only stream; no agent push channel). No MCP files amended.
- `backend-rs/src/lib.rs` (request-id layers; the `MatchedPath` cardinality
  guard lives at `lib.rs:462-479`), `backend-rs/src/main.rs` (sweeper task
  lifecycle to mirror for the heartbeat task — `main.rs:140-214`),
  `backend-rs/src/state.rs` (`AppState`),
  `backend-rs/src/metrics.rs` (registry/render), `backend-rs/tests/README.md`
  (ephemeral ports, wire harness).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/common/event_bus.rs` | **NEW** — `EventEnvelope` (serde snake_case, bounded), `EventBus` wrapper over `broadcast::Sender<1024>`, `Lagged → resync` mapping, unit tests |
| `backend-rs/src/features/events/` | **NEW** — `routes.rs` (`GET /events`), per-connection subscriber task, membership filter, revocation re-check, heartbeat task, connection-cap bookkeeping (6/user), `Last-Event-ID` handler (filtered MAX + `resync`), route-level gzip/br exclusion, metrics |
| `backend-rs/src/features/presence/` | **NEW** — `POST /presence/heartbeat`, in-memory map + TTL sweep, debounce (2 s/board), `presence.update` publish, response `{board_id, user_ids[]}` |
| `backend-rs/src/common/authz.rs` (T032) | extend with `visible_workspace_ids` call reuse; presence re-uses board-read authz |
| `backend-rs/src/state.rs` / `lib.rs` | `EventBus` in `AppState`; sweeper-style abort-on-shutdown for any long-running tasks |
| `backend-rs/src/metrics.rs` | `kanpad_sse_*` series + cardinality guard reuse |
| `backend-rs/tests/api/sse_wire.rs` | **NEW** — black-box wire suite (ephemeral ports): A/B shared board delivery < 2 s; foreign-workspace silence; mid-stream revocation closes/filters; `Last-Event-ID` → single `resync`; unauthenticated 401; heartbeat frames present; cap 6 closes oldest |
| `docs/deployment.md` (+ `40-operations.md` pointer if needed) | LiteSpeed/proxy requirements: unbuffered SSE, per-frame flush, `Host` unchanged, same-site registrable domain; T012 `X-Accel-Buffering` verification note |
| `specs/api-rewrite-rs-specs/20-api-contract.md` | **AMEND** (T52 same-PR) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** client hook/UI (T042), notification emission
(T041 — `notification.new` publishes through this bus but is wired there),
cross-replica fanout (S2 — deferred, document only), WebSockets, changing the
`activity_log` schema (T037 owns it).

## Implementation Steps

1. **Envelope + bus first** with unit tests: publish post-commit (construct a
   rollback case → no publish), capacity semantics, `Lagged` → single `resync`.
2. **`GET /events` handler**: cookie auth (401 pre-stream), membership set at
   connect (indexed query, paste `EXPLAIN`), SSE framing (`id:` only for
   cursor-carrying events; `: hb` every 25 s; `X-Accel-Buffering: no`;
   `Cache-Control: no-store`; Brotli/gzip bypass at the route).
3. **Subscriber registry** + **filtering**: per-connection workspace set; frame
   delivered iff event workspace ∈ set; metrics counters.
4. **Revocation path (red-first)**: `member.revoked` handler drops the
   workspace from the affected connections' filters (+ sends a `member.revoked`
   frame first); a foreign-workspace subscriber receives nothing in 2 s in the
   black-box test; revocation observed mid-stream stops further frames.
5. **`Last-Event-ID`**: re-auth + re-read memberships; filtered `MAX(id)` vs
   client `last_seen`; single `resync` (idempotent; absence of visible gap →
   nothing); presence excluded from resync; `kanpad_sse_resyncs_total`.
6. **Connection cap**: 6/user → oldest closed with 429 problem+json (assert in
   test); counters exported.
7. **Presence**: heartbeat endpoint (authz: foreign board → 404; per-IP bucket;
   body-bounded); TTL map + lazy sweep; debounced `presence.update`; response
   roster hydration shape; assert **no** `activity_log` rows written by
   heartbeats (test).
8. **Compression exclusion**: verify with an actual gzip-negotiating client that
   `/events` is served unbuffered (integration check in the wire suite or a
   curl-with-`Accept-Encoding` smoke); document the rule.
9. **Docs** (deployment runbook entries for both paths — appliance + LiteSpeed);
   `deployment.md` SSE section; T012 `X-Accel-Buffering` cross-check.
10. **Load sanity (T026 input, not a gate):** 200 concurrent connections +
    mutation storm — record p99 frame latency + zero 500s in §Notes (optional
    here; T026 runs the formal capacity pass).
11. T52 regen (openapi.json includes the two endpoints; `/events` documented as
    non-JSON); both engines; lesson.

## Constraints

- **Metadata-only frames** (`30` §5/`31` §9): no bodies, titles, emails, PII —
  ever. A frame carries `{id, type, workspace_id, subject_type, subject_id,
  via, actor_key_id, occurred_at}` only.
- **Post-commit publish only**; rollback ⇒ silence.
- **No background fanout tasks** for presence (S4) — lazy sweep only.
- **No persistence** of presence; nothing in `activity_log`; nothing in
  `notifications`.
- Revocation is a **hard requirement**, verified by black-box test — not a
  best-effort nicety.
- SSE is not cacheable, not gzipped, never empty-stream-on-401.
- **O-8/O-9/O-10/O-11** defaults stand pending T027 ratification (viewer-
  presence yes; `admin.*` frames no; route-level compression exclusion; cap 6)
  — amend if overridden at review.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Black-box wire suite green (Step 2–5): A→B delivery < 2 s on a shared
      board; foreign user receives nothing; mid-stream revocation stops events;
      `Last-Event-ID` yields exactly one `resync` (never replay; never a
      workspace the user left). Closes **S6** (with T042 for the client ladder).
- [ ] `GET /events` unauthenticated → 401; foreign board presence heartbeat →
      404; no session → no stream.
- [ ] Heartbeat `: hb` every 25 s observed in a streaming client (test with a
      timeout < 30 s).
- [ ] Presence: heartbeat updates roster; TTL expiry removes viewer; debounce
      ≤ 1 `presence.update`/board/2 s; zero `activity_log` rows for heartbeats
      (test).
- [ ] Cap 6 enforced; oldest stream closed with 429 (test).
- [ ] `kanpad_sse_*` series exported and scrape-safe (no `{id}` label
      cardinality blowup — `MatchedPath` guard).
- [ ] Contract: `20-api-contract.md` amends (SSE non-JSON, presence endpoint);
      OpenAPI + `schema.d.ts` regen same-PR; MCP "realtime deferred" statements
      untouched.
- [ ] Both engines green (no SQL in the hot path beyond T032's membership query;
      it is checked); no test weakened; deployment docs updated for both paths.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — paste the revocation test evidence,
the `EXPLAIN` for the membership connect query, and the gzip-bypass verification)

## Lessons

(pending)
