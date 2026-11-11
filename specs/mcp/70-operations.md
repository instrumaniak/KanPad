# 70 — Operations

## 1. Configuration

New `Settings` fields (`backend-rs/src/config.rs`), each with
`#[serde(default = ...)]`, added to the `Default` impl and — where secret
bearing — to the **redacting `Debug` impl**:

| Variable | Type | Default | Notes |
|---|---|---|---|
| `API_KEY_SECRET` | `SecretString` | dev: falls back to `SESSION_SECRET` with a `warn`; **production: required, ≥ 32 bytes** | Boot-validated like `SESSION_SECRET`. Rotating it invalidates every key at once. |
| `MCP_ENABLED` | `bool` | `false` | The kill switch. When `false`, `/mcp` is not mounted at all → the path 404s through `api_only_fallback`. |
| `MCP_PATH` | `String` | `/mcp` | The mount path. |
| `MCP_TIMEOUT_SECS` | `u64` | `120` | The MCP timeout layer (the REST 30 s is separate and shorter). |
| `MCP_MAX_TOOL_RESULT_BYTES` | `usize` | `262144` (256 KiB) | The result-size backstop. |
| `THROTTLE_AGENT_PER_MIN` | `u32` | `300` | The per-key MCP rate limit. |

**`MCP_ENABLED` defaults to `false`** — the endpoint does not exist until it is
deliberately turned on. A new public endpoint should never appear by default
on a live deployment.

**The production boot screen** (`Settings::validate`) MUST print, in the same
style as the existing configuration summary:

```
MCP: enabled  path=/mcp  timeout=120s  max_result=256KiB
MCP: agent rate limit 300/min per key
API keys: secret configured (sha256 fingerprint ab12…)
```

The fingerprint — a truncated hash, never the secret — lets an operator
confirm that two replicas share a key secret without revealing it.

## 2. Deployment

The MCP endpoint lives in the **existing** `backend-rs` container. There is no
new service, no new port, and no new proxy route beyond the API origin
itself.

```
https://api-kanpad-rs.raziur.com/mcp
```

### Proxy requirements

The existing reverse proxy already fronts `/api/v1/*`; `/mcp` needs the same
treatment plus:

| Requirement | Why |
|---|---|
| **Forward `Host` unchanged** | The `Origin`/`Host` fallback and rmcp's origin check both depend on it (standing rule from `T007` §Notes 4). |
| **Do not buffer the response** | Streamable HTTP may answer with `text/event-stream`. With `with_json_response(true)` a simple tool call is a single JSON body, but a handler that emits a notification first falls back to SSE. A buffering proxy would hang it. |
| **No request-body rewriting** | The `Authorization` header must pass through untouched. |
| **No path rewriting** | `/mcp` must reach the service as `/mcp`. |
| **Body size** | The existing 2 MiB limit is inherited and is ample. |
| **Read timeout** | Must exceed `MCP_TIMEOUT_SECS` or the proxy will cut a long tool call before the server does. |

### Post-deploy smoke list

```sh
# 1. The endpoint exists and is not open.
curl -sS -o /dev/null -w '%{http_code}\n' \
  -X POST https://api-kanpad-rs.raziur.com/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2026-07-28",
        "capabilities":{},
        "clientInfo":{"name":"smoke","version":"0"}}}'
# expect: 401 (no Authorization header)

# 2. With a read-only key, initialize + list tools.
curl -sS -X POST https://api-kanpad-rs.raziur.com/mcp \
  -H "Authorization: Bearer $KANPAD_API_KEY" \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2026-07-28","capabilities":{},
        "clientInfo":{"name":"smoke","version":"0"}}}'
# expect: 200 + a JSON-RPC result carrying the server instructions

# 3. A write tool with a read-only key.
curl -sS -X POST https://api-kanpad-rs.raziur.com/mcp \
  -H "Authorization: Bearer $KANPAD_API_KEY" \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
        "name":"create_card","arguments":{"board_id":1,"column_id":1,"title":"x"}}}'
# expect: an isError result naming the missing scope (403), NOT a created card
```

Also re-run the existing smoke list from `docs/deployment.md` — the new layers
must not have disturbed `/health`, `/ready`, `/api-docs/openapi.json` or the
SPA behaviour.

## 3. Metrics

New Prometheus series on the existing `/metrics` listener (which stays
unauthenticated and on its own port):

| Series | Type | Labels |
|---|---|---|
| `kanpad_mcp_requests_total` | counter | `tool`, `outcome` (`ok` / `error` / `forbidden` / `unauthorized`) |
| `kanpad_mcp_tool_duration_seconds` | histogram | `tool` |
| `kanpad_mcp_tool_result_bytes` | histogram | `tool` |
| `kanpad_mcp_tool_truncated_total` | counter | `tool` — how often a bound was hit |
| `kanpad_api_key_auth_total` | counter | `outcome` (`ok` / `unknown` / `revoked` / `expired` / `scope_denied`) |
| `kanpad_api_key_active` | gauge | — (count of non-revoked, non-expired keys) |
| `kanpad_agent_rate_limited_total` | counter | — |

**The `scope_denied` counter is the interesting one.** A steady stream of it
means either an agent is misconfigured or somebody is probing the key.

## 4. Logging

| Event | Level | Fields |
|---|---|---|
| MCP request | `info` | `request_id`, `tool`, `outcome`, `duration_ms`, `api_key_id`, `user_id` — **never the token** |
| Key created | `info` | `api_key_id`, `user_id`, `scopes`, `expires_at` — **never the plaintext** |
| Key revoked | `info` | `api_key_id`, `user_id` |
| Scope denied | `warn` | `api_key_id`, `tool`, `required_scope`, `granted_scopes` |
| Repeated auth failure | `warn` | the failure class, **not** the presented token |

The `TraceLayer` span already carries method + path + status + latency and
`x-request-id`; the MCP layer adds the tool name and the actor id to the same
span so the two log families correlate.

## 5. The kill switch

`MCP_ENABLED=false` unmounts `/mcp`. The path then falls through to
`api_only_fallback` → `404 application/problem+json`, which is exactly what an
unmounted path looks like everywhere else in the API.

**This is a configuration change and a restart.** For a faster stop, the
per-key revocation path is the lever: revoke the offending key and it is dead
on the next request, with no restart.

**Both levers are documented in the runbook** with the question to ask:

| Symptom | Lever |
|---|---|
| One agent misbehaving | Revoke its key (immediate, no restart) |
| The whole MCP surface suspect | `MCP_ENABLED=false` + restart |
| A key leaked in public | Revoke it, then rotate `API_KEY_SECRET` to kill any sibling |
| The key secret leaked | Rotate `API_KEY_SECRET` → every key dies at once |

## 6. Runbook excerpts

### Rotating a key

```sh
backend-rs create-api-key --email you@example.com --name "opencode (new)" --scopes read,write
# → prints the plaintext ONCE
export KANPAD_API_KEY='kp_live_…'      # update the agent's environment
opencode mcp list                        # verify: ✓ kanpad connected
backend-rs list-api-keys --email you@example.com
backend-rs revoke-api-key --id 3         # revoke the old one
```

### Rotating `API_KEY_SECRET`

1. `revoke-api-key` everything you can, or accept that they all die.
2. Set the new `API_KEY_SECRET` (≥ 32 bytes, from a CSPRNG).
3. Restart the service.
4. Re-mint the keys you still need.
5. Update every agent's environment.

**Every key stops working at the moment of the restart.** This is the
"invalidate everything" lever; use it only deliberately.

### Diagnosing a 401

| Cause | How to tell |
|---|---|
| Wrong key | The value in the agent's env differs from the minted one |
| Revoked | `GET /api/v1/api-keys` shows `revoked_at` |
| Expired | `GET /api/v1/api-keys` shows `expires_at` in the past |
| Rotated secret | The service was restarted with a new `API_KEY_SECRET` |
| Header not sent | `opencode mcp list` shows the server as failed to connect |

### Diagnosing a 403

The key is valid and lacks the scope. The error names the required scope.
The owner grants it by creating a new key with the wider scope set — scopes
are immutable on an existing key, by design (a key's permissions should never
be widened silently).

## 7. Capacity

The MCP surface is not free, but it is bounded:

| Load | Cost |
|---|---|
| One agent, a few tool calls per minute | Negligible |
| One agent in a tight loop | `THROTTLE_AGENT_PER_MIN` (300) caps it, per key |
| Many agents, many users | Each key has its own bucket; the pool is the shared constraint |

The pool is the real ceiling. `DB_MAX_CONNECTIONS` bounds concurrency, and
each MCP request takes **at most one** connection for the duration of the tool
call (composite tools use one connection for their whole read set). A tool
call therefore consumes no more pool capacity than the equivalent REST call.

**Watch for the composite-tool temptation.** `get_card_context` doing five
sequential queries on one connection holds that connection for five round-trips.
Under concurrency that is expensive. The mitigation is a **bounded** query set
(≤ 5) with indexes — not a parallel fan-out, which would multiply pool
consumption. If profiling shows the composite tool is hot, the next step is
fewer queries per call, never more connections per call.
