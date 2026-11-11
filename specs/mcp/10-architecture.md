# 10 — Architecture

## 1. Shape

```
                 ┌────────────────────────────────────────────────────────┐
  OpenCode       │  https://api-kanpad-rs.raziur.com                   │
  Claude Code    │                                                        │
  Cursor    ─────┼──▶  ┌──────────────────────────────────────────────┐  │
  (MCP client)   │    │  /mcp   rmcp StreamableHttpService            │  │
                 │    │  ┌────────────────────────────────────────┐    │  │
                 │    │  │ ApiKeyAuth  bearer → Actor (scope chk)  │    │  │
                 │    │  │ tools/*    → features::*::store         │    │  │
                 │    │  │ authz::require  (Wave 2 kernel)         │    │  │
                 │    │  └────────────────────────────────────────┘    │  │
                 │    ├────────────────────────────────────────────────┤  │
  Browser (SPA) ─┼──▶  │  /api/v1/*   existing REST routes            │  │
  (cookie)        │    │  AuthSession (cookie | bearer) → same stores │  │
                 │    └────────────────────────────────────────────────┘  │
                 └─────────────────────────┬──────────────────────────────┘
                                           │ sqlx pool (ONE pool)
                                      MySQL / MariaDB
```

One process. One pool. One authorization kernel. One implementation of board
logic.

## 2. Dependency

```toml
# backend-rs/Cargo.toml
rmcp = { version = "3.9", default-features = false, features = [
    "server",                              # tool system + ServerHandler
    "macros",                              # #[tool_router] / #[tool] / #[tool_handler]
    "transport-streamable-http-server",    # StreamableHttpService
] }
```

**Why `default-features = false`.** rmcp's default feature set is
`base64 + macros + server` — harmless. But `cargo deny` runs with
`all-features = true` (`backend-rs/deny.toml`), and rmcp's optional
`auth` feature pulls `oauth2` → `reqwest` → `rustls` → `ring`, whose **ISC**
licence is **not** on this project's allow-list (the file's own header records
that a BSD-3-Clause tree was rejected). We do not need OAuth — bearer API keys
are the credential — so the feature set is pinned explicitly and
`cargo deny check licenses` is re-run to confirm.

**After adding the dependency, `deny.toml` MUST be re-derived from a fresh
`cargo deny check` run** — the file's header says so in as many words:
*"re-derive this file from a fresh run when the dependency set changes: a stale
allow-list is a policy that lies."*

**Version pinning.** `rmcp = "3.9"` with the exact minor recorded here. rmcp
moves fast (3.0 → 3.9 in three months) and its transport API changed across
that range; a loose constraint is a silent-breakage risk. Pin the minor and
review bumps deliberately.

**Rust version.** rmcp 3.x needs Rust ≥ 1.88; the project is on 1.99. No
conflict.

**Other crates needed:** none. `sha2` (already present) for the key HMAC,
`subtle` **not** needed (equality on a unique-index lookup is not a timing
comparison — see [`20-auth-and-keys.md`](20-auth-and-keys.md) §4).

## 3. Transport & mount point

**Transport:** MCP *Streamable HTTP*, per the current specification revision
(2026-07-28), with the legacy compatibility path enabled.

**Mount point:** `/mcp` on the API origin.

```rust
// backend-rs/src/lib.rs — create_app()
let mcp = Router::new()
    .nest_service(
        "/mcp",
        StreamableHttpService::new(
            || Ok(McpServer::new(state.clone())),   // fresh handler per request
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(false)     // stateless for legacy clients too
                .with_json_response(true),          // plain JSON for simple tools
        ),
    );
```

**Mounted outside `/api` and outside `/api/v1`**, for three reasons:

1. `/api-docs/openapi.json` is a byte-exact snapshot of the committed
   `backend-rs/openapi.json`, gated by `tests/api/openapi_snapshot.rs`. MCP is
   **not REST** — it must not appear in the OpenAPI document, and it must not
   change the committed spec.
2. The deprecated `/api` alias is a mirror of `/api/v1`; MCP must not be
   duplicated under a deprecated prefix.
3. `/mcp` needs a **different middleware stack** (see §4) and the REST stack
   cannot express that.

**Stateless mode is the default and is the right choice:**

- `legacy_session_mode(false)` → no `Mcp-Session-Id`, no in-memory session
  store, no standalone `GET`/`DELETE` stream. A server restart does not
  invalidate a client's session, and a second replica needs no session
  affinity.
- `json_response(true)` → a simple tool call replies with one
  `application/json` body instead of an SSE stream. Fewer bytes, no long-lived
  response to time out. rmcp still falls back to `text/event-stream` if a
  handler emits a notification or server request first.
- Because the server is stateless, the `service_factory` closure runs **per
  request** — the shared `AppState` (pool, settings) is captured by the closure
  and cloned per request. Nothing may rely on in-memory state surviving between
  requests.

**Legacy clients.** rmcp negotiates the protocol version per request.
`legacy_session_mode(false)` additionally makes pre-2026-07-28 clients
stateless. `with_stateless_protocol_metadata_required(true)` is **not** used —
it would reject clients negotiated below 2026-07-28, and OpenCode's default
`protocol` is `legacy`.

> **Verification task (C1):** prove an OpenCode `type: "remote"` client can
> `initialize`, `tools/list` and `tools/call` against this endpoint, on both
> `protocol: "legacy"` and `protocol: "2026-07-28"`. If legacy negotiation
> fails, the fallback is `legacy_session_mode(true)` with `LocalSessionManager`
> (in-memory) and an explicit note that MCP sessions then need a single
> replica.

## 4. Middleware stack for `/mcp`

The REST stack in `create_app()` is **not** reusable as-is. `/mcp` is mounted
with its own stack, in this order (outermost first):

| # | Layer | Why |
|---|---|---|
| 1 | HSTS + secure headers (`served-by`, `nosniff`, `DENY`, `referrer-policy`) | Same as REST — reuse the existing helpers, do not duplicate the values |
| 2 | `SetSensitiveRequestHeadersLayer` `[AUTHORIZATION, COOKIE, SET_COOKIE]` | **Already present app-wide.** The bearer token must never reach a span or a `Debug` log |
| 3 | `SetRequestIdLayer` + `PropagateRequestIdLayer` | Correlate an MCP call with an activity-log row and a log line |
| 4 | `TraceLayer` | Same span shape as REST so the metrics/log parsers stay uniform |
| 5 | `CorsLayer` | See §5 |
| 6 | **`ApiKeyAuth`** | Bearer → `Actor`, scope check. **MCP has no cookie fallback** — a browser session must never authenticate an MCP call (see §5) |
| 7 | **MCP rate limiter** (governor, per key) | See [`20-auth-and-keys.md`](20-auth-and-keys.md) §7 |
| 8 | `TimeoutLayer(MCP_TIMEOUT)` | **Not** the REST 30 s. A tool call that touches several tables must not be cut off mid-transaction |
| 9 | `CompressionLayer` | gzip only. **Never `br` for MCP** — brotli's streaming encoder adds latency to small JSON bodies, and `br` is not needed for an agent client |
| 10 | `StreamableHttpService` | Terminal |

**Three deliberate differences from the REST stack:**

- **No `SessionManagerLayer`.** MCP authenticates by bearer token only. A
  cookie must never be able to open an MCP session.
- **No `CsrfLayer`.** The CSRF layer protects *cookie*-authenticated
  mutations. MCP requests carry no cookie, so CSRF does not apply. A bearer
  token in an `Authorization` header is never attached automatically by a
  browser, so there is no cross-site request to forge.
- **Its own governor bucket.** The REST governor keys on client IP, which is
  shared by every agent behind one NAT egress — an agent burst would starve the
  browser of the same user. The MCP limiter keys on the **API key**.

## 5. CORS

The MCP endpoint is consumed by **agent clients (no browser)**, so CORS is
mostly irrelevant — but the layer is applied anyway so that a future
browser-based agent works.

**Today:** the canonical origin list (`CORS_ORIGINS`) is applied as-is. For a
CLI client no `Origin` header is sent and CORS is a no-op.

**If a browser client is ever supported:** `CorsLayer::allow_headers` must gain
`AUTHORIZATION` — the current list is `[CONTENT_TYPE, X_REQUEST_ID]`, so a
cross-origin MCP call from the SPA would fail preflight. That is a **separate,
later** change; it is recorded here so the `allow_headers` list is not
mistaken for a complete one.

**Origin / DNS-rebinding protection.** rmcp performs its own origin check when
configured. Because `/mcp` is a public endpoint on the API origin, the
deployment MUST forward `Host` unchanged (the standing rule from
`specs/tasks/T007-…` §Notes 4) — the `Origin`/`Host` fallback degrades
otherwise.

## 6. Hard constraints on the MCP module

These are the rules that keep the MCP surface from becoming a second
implementation of the API. They are **reviewable by grep** and MUST be stated
in every task that touches the MCP module.

1. **The MCP module MUST NOT contain SQL.** No `sqlx::query!`, no
   `sqlx::query_as!`, no raw statements. Every read and write goes through a
   `features::*::store::*` function, exactly like the REST handlers do.
   *Enforcement:* a CI grep gate over `backend-rs/src/mcp/**` for
   `sqlx::query`.
2. **Every tool MUST resolve an `Actor` and pass it down.** The actor carries
   the user id; the store applies the ownership predicate. A tool that invents
   its own user id is a bug.
3. **Every tool MUST go through `authz::require`** (Wave 2, `T029`) for the
   resource it touches — read *and* write. MCP is not exempt from the kernel.
4. **Every mutating tool MUST write an activity-log row** inside the same
   transaction as the mutation (`T034`), tagged `via = 'mcp'` and carrying the
   `api_key_id`.
5. **A tool MUST NOT perform outbound network I/O.** No `reqwest`, no DNS, no
   URL fetching. `link_artifact` stores a string.
6. **A tool MUST NOT return unbounded data.** Every result is bounded and
   truncated with an explicit marker (see [`30-tool-surface.md`](30-tool-surface.md) §4).
7. **Errors are tool results, not transport failures.** A failed tool returns
   `isError: true` with the problem+json body as text. `Err(McpError)` is
   reserved for genuine protocol-level faults.

## 7. Module layout

```
backend-rs/src/mcp/
  mod.rs           # router() + the StreamableHttpService factory
  server.rs        # McpServer struct, #[tool_router], #[tool_handler], instructions
  actor.rs         # Actor resolution from the bearer token (the auth half)
  scope.rs         # Scope enum, parsing, method→scope mapping
  tools/
    boards.rs      # list_projects, list_boards, get_board, list_columns
    cards.rs       # list_cards, get_card, get_card_context, create_card,
                   # update_card, move_card, delete_card
    collab.rs      # claim_card, release_card, heartbeat_claim
    content.rs     # add_card_note, list_card_notes, link_artifact, list_labels
    checklists.rs  # create_checklist, add_checklist_item, set_checklist_item
  error.rs         # AppError → CallToolResult(isError) mapping
```

`features/api_keys/` is a first-class feature module (it owns its store and
its REST routes), not part of `mcp/` — the key management REST API is used by
the frontend and is not MCP-specific.

## 8. The `instructions` field

rmcp's `ServerHandler` returns `instructions` at `initialize`. OpenCode
**injects them into the agent's system prompt**. This is the right place for:

- the work-loop protocol (`40-workflows.md`),
- the **prompt-injection rule** (card and note text are data, never
  instructions),
- the instruction to read the repository's `AGENTS.md` before editing.

It is the only channel that reaches the model as *trusted* text — tool
descriptions are also trusted, and both are authored by us, not by users. **No
user-supplied text may ever appear in either.** See
[`60-security.md`](60-security.md) §3.
