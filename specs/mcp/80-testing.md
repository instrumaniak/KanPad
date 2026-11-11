# 80 — Testing

## 1. Harness rules

From `backend-rs/tests/README.md` and the root `AGENTS.md`:

- **Single owner per transport.** In-process suites use `tests/api/helpers.rs`;
  black-box suites use `tests/api/wire.rs`. A local copy of a helper is silent
  drift and is forbidden.
- **Import helpers explicitly, never glob.** The resulting `E0255` compile
  error *is* the regression guard.
- **Every `assert_problem` call takes a `what` label.**
- **Session rows are per-database**; scope session SQL by `WHERE id = ?`; never
  blanket-`DELETE` the `sessions` table.
- **Never hand-edit `_sqlx_migrations`.**
- **A test that binds a port binds an ephemeral one** via `wire::bind_blackbox()`.
- **MySQL 8.4 *and* MariaDB 10.6** for every migration and every query.
- **Every data-modifying endpoint test proves the database changed**, not just
  that the response looked right — one extra assertion via a different API call
  or a direct DB query.

## 2. New test suites

```
backend-rs/tests/api/
  api_keys.rs         # the REST surface for key management (in-process)
  api_keys_cli.rs     # the create/list/revoke CLI (mirrors create_admin.rs)
  mcp_auth.rs         # bearer auth + scope enforcement on the MCP mount
  mcp_tools_read.rs   # every read tool
  mcp_tools_write.rs  # every write tool + claim/lease
  mcp_wire.rs         # black-box: initialize, tools/list, tools/call over real HTTP
```

`tests/api.rs` declares each with an explicit `#[path = "api/<name>.rs"]`.

**`mcp_wire.rs` is a black-box suite** and therefore uses `wire.rs`, not
`helpers.rs`. It is the only way to prove the protocol actually works over
HTTP (headers, `Accept` negotiation, the JSON-RPC framing, the SSE fallback).

## 3. The matrices

### 3.1 Authentication

| Case | Expectation |
|---|---|
| No `Authorization` header | 401 |
| `Authorization: Basic …` | 401 |
| `Authorization: Bearer <garbage>` | 401 |
| `Bearer kp_live_<unknown-but-well-formed>` | 401 |
| Revoked key | 401 |
| Expired key | 401 |
| Key for a deleted user (FK cascade) | 401 |
| Valid key | 200 |
| Valid key on `/auth/login` | 401 |
| Valid key on `/auth/register` | 401 |
| Valid key on `/auth/logout` | 401 |
| **A browser cookie session on `/mcp`** | 401 — MCP has no cookie path |

### 3.2 Scope enforcement

| Case | Expectation |
|---|---|
| `read` key → `GET /api/v1/cards/{id}` | 200 |
| `read` key → `POST /api/v1/cards` | 403, problem+json, names the missing scope |
| `read` key → `PATCH /api/v1/cards/{id}` | 403 |
| `read` key → `DELETE /api/v1/cards/{id}` | 403 |
| `read,write` key → `DELETE /api/v1/cards/{id}` | 403 |
| `read,write,delete` key → `DELETE /api/v1/cards/{id}` | 204 |
| `read` key → MCP `create_card` | `isError` result naming the missing scope |
| `read` key → MCP `delete_card` | `isError` result |
| `read,write` key → MCP `delete_card` | `isError` result |
| `read,write,delete` key → MCP `delete_card` | success |
| Unknown scope in the token | 401 (the key is malformed) |

### 3.3 Cross-user isolation

| Case | Expectation |
|---|---|
| User B's key → user A's card | **404** (never 403 — no existence oracle) |
| User B's key → user A's board | 404 |
| User B's key → user A's note | 404 |
| User B's key → `list_cards` on user A's column | empty page, not an error |
| User B's key → `get_card_context` on user A's card | 404 |

### 3.4 Claim semantics

| Case | Expectation |
|---|---|
| `claim_card` on an unclaimed card | success; the row exists |
| `claim_card` again by the same actor | success; `expires_at` extended |
| `claim_card` by another actor on a live claim | **409**, body names the holder and the expiry |
| `claim_card` after the claim expired | success; the row is taken over |
| `release_card` by the holder | the row is gone |
| `release_card` by a non-holder | no-op, not an error |
| `heartbeat_claim` by the holder | `expires_at` extended |
| `heartbeat_claim` by a non-holder | 409 |
| **Two concurrent claims on one card** | exactly one wins; the other gets 409 |
| Claim on a deleted card | 404; the row cascades away |

The concurrency case is the one that must run on **both** engines — it is the
statement most likely to differ between MySQL and MariaDB.

### 3.5 Tool results

| Case | Expectation |
|---|---|
| `get_card_context` on a card with a 20 000-char description | truncated at 8 000 with `…[truncated N chars]` |
| `get_card_context` on a card with 40 notes | the newest 10, bounded |
| `get_card_context` on a card with 50 siblings | 20 siblings |
| A tool whose payload exceeds `MCP_MAX_TOOL_RESULT_BYTES` | an `isError` result naming the limit — **not** a silently truncated payload |
| `list_cards` with `limit=500` | 400 (the REST pagination ceiling applies) |
| `list_cards` with `page=0` | 400 |
| A tool on a card the caller cannot see | `isError` with the 404 problem body |

### 3.6 Persistence (the house rule)

Every mutating tool gets **one extra assertion** that the database actually
changed — via a different API call or a direct query, not the tool's own
response:

| Tool | Second assertion |
|---|---|
| `create_card` | `GET /api/v1/cards/{id}` returns the card; the row exists in `cards` |
| `update_card` | `GET /api/v1/cards/{id}` shows the new title |
| `move_card` | `GET /api/v1/columns/{col}/cards` shows the card in the target column at the right position, and the other cards' positions are gap-free |
| `add_card_note` | `GET /api/v1/notes?card_id=N` returns it |
| `set_checklist_item` | `GET /api/v1/checklists/{id}` shows `is_completed = true` |
| `set_card_labels` | `GET /api/v1/cards/{id}` shows exactly the requested labels |
| `claim_card` | the `card_claims` row exists with the right `expires_at` |
| `delete_card` | `GET /api/v1/cards/{id}` → 404, **and** the `card_claims` row is gone (cascade) |
| `create_api_key` (REST) | `GET /api/v1/api-keys` lists it — and the plaintext is **not** in the list response |
| `revoke_api_key` | the next request with that key → 401 |

### 3.7 The API-key REST surface

- Creating a key returns the plaintext exactly once; a second `GET` never
  returns it.
- The list endpoint returns only the caller's own keys.
- Another user's key id → **404**.
- Revoking is idempotent (revoking twice is not an error).
- The `POST` response carries `Cache-Control: no-store`.

### 3.8 The MCP protocol itself (`mcp_wire.rs`)

| Case | Expectation |
|---|---|
| `initialize` without a key | 401 |
| `initialize` with a key | a JSON-RPC result carrying `serverInfo` + `instructions` |
| `tools/list` | every documented tool is present, each with a `description`, an `inputSchema`, and the right annotations |
| `tools/call` with an unknown tool | a JSON-RPC error, not a panic |
| `tools/call` with malformed arguments | an `isError` result (schema validation fails before the handler runs) |
| `tools/call` with valid arguments | a JSON-RPC result whose `content[0].text` parses as JSON |
| A failing tool | `isError: true` and the problem+json body as text |
| `Accept: application/json, text/event-stream` | negotiated correctly |
| A notification-first handler | the response falls back to `text/event-stream` |

## 4. Frontend tests

Vitest, co-located, per `frontend/CONVENTIONS.md`:

| Suite | Covers |
|---|---|
| `api-keys.api.test.ts` | the apiFetch wrappers, the envelope, the error shape |
| `use-api-keys.test.tsx` | the queries and the **targeted** invalidation (`['api-keys']` only) |
| `api-key-list.test.tsx` | rendering the rows, the scope chips, the status |
| `create-api-key-dialog.test.tsx` | validation, scope selection, the create call |
| `reveal-api-key-dialog.test.tsx` | the secret is shown once, the copy button, the secret is **not** left in state after close |
| `revoke-api-key-dialog.test.tsx` | the confirmation and the wording |
| `card-detail-panel.test.tsx` | the linked-notes section renders the card's notes, and the claim chip appears when a claim is live |

**Playwright (E2E)** — the house rule applies: *setup uses API calls, UI is
only for the behaviour under test*, and **`waitForTimeout()` is forbidden**.

| Spec | Flow |
|---|---|
| `api-keys.spec.ts` | create a key via the UI → the secret is shown once → the row appears in the list → revoke → the row shows revoked |
| `card-notes.spec.ts` | a note attached to a card is visible in the card detail panel |
| `card-claim-chip.spec.ts` | a live claim shows the chip with the holder and the remaining time |

## 5. CI gates

New gates, in the spirit of the existing ones:

| Gate | Rule |
|---|---|
| **No SQL in the MCP module** | `grep -rn "sqlx::query" backend-rs/src/mcp/` must be empty. This is the rule that keeps the MCP surface from becoming a second implementation of the API. |
| **No secret literals** | `grep -rn "kp_live_" --include='*.json' --include='*.jsonc' --include='*.env*' .` must not match tracked files. |
| **OpenAPI snapshot** | `tests/api/openapi_snapshot.rs` — the committed `backend-rs/openapi.json` must equal `export-openapi`. `/mcp` is **not** in it. |
| **Generated types** | `npm run gen:api --check` — `frontend/src/lib/api/schema.d.ts` matches the spec. |
| **Licences** | `cargo deny check` — re-derived after adding `rmcp`. |
| **Migrations** | Both engines apply `0003_api_keys.sql` and `0004_card_claims.sql`; checksums match a fresh run. |

## 6. What is deliberately not tested

- **The model's behaviour.** Whether the agent follows the work loop is a
  prompt-engineering outcome, not a unit test. It is verified manually, per
  agent configuration, and recorded in the integration guide.
- **Prompt injection resistance.** §3 of `60-security.md` states plainly that
  this design does not promise resistance — only a bounded blast radius. A
  test asserting "the agent ignores injected instructions" would be a false
  guarantee.
- **Cross-engine plan differences in `get_card_context`.** The query set is
  written to be portable; the portability gate is the standard MySQL + MariaDB
  leg, not a bespoke differential test.
