# 11 — MCP & API-Key Surface (Wave 3)

**Objective:** Audit the agent credential surface designed in `specs/mcp/` —
the bearer pipeline, key issuance/revocation, scope enforcement on REST and
MCP, the `/mcp` mount, and `API_KEY_SECRET` handling — before it reaches
production.

**Status (2026-11-16):** Authored from the spec review. **Not executed yet.**
The Wave 3a implementation (credentials) is un-landed; this phase's active
testing runs against the local stack / test database as the implementation
lands, and its passive checks run against prod after `MCP_ENABLED=true` is
deployed. The design owner is `specs/mcp/`; this folder audits it like any
other auth surface. Finding: `findings-register.md` **F-07** (SEC-028).

**Scope:** `features/api_keys/` (types/store/auth), `src/mcp/` (the module a
Wave 3b task will add), the `/api/v1/api-keys` REST family, `config.rs`
(`API_KEY_SECRET`, `MCP_*`, `THROTTLE_AGENT_PER_MIN`), the
`kp_live_` secret-literal gate, and the `specs/mcp/` documents themselves
(reviewed 2026-11-16 — this phase exists because of that review).

---

## 1. Test plan

### 1.1 Key lifecycle (REST, used by the frontend)
- `POST /api/v1/api-keys` with a **browser session** → 201; plaintext in the
  response exactly once; `Cache-Control: no-store` present; list endpoint
  never contains the secret (re-read via `GET /api/v1/api-keys`).
- **CSRF on minting:** the mint route is cookie-authenticated, so the standard
  CSRF layer applies — cross-site mint attempt → 403.
- **Bearer keys cannot mint keys** (amendment 2026-11-16, specs/mcp/20 §5):
  any `kp_live_…` token on `POST /api/v1/api-keys` → 401, regardless of scope.
  This is the anti-escalation control; without it a `write` key widens itself
  to `delete`/`admin`.
- Scopes validated against the fixed set (`read|write|delete|admin`) — a
  forged scope string → 400, not silent storage.
- `expires_in_days` accepted range capped (≤ 365); `0`/negative/`abc` → 400.
- Idempotent revoke (`DELETE …/{id}` twice → 204 then 404 or 204 no-op);
  foreign key id → 404.

### 1.2 Bearer pipeline (both surfaces)
- No `Authorization` / wrong scheme (`Basic`) / garbage token / well-formed
  `kp_live_*` unknown / revoked / expired / deleted user (FK cascade) →
  **401 with one uniform message** (no "was valid once" oracle).
- Key on `/auth/login`, `/auth/register`, `/auth/logout` → **401** (a key must
  never mint/destroy a session; `session.flush()` would destroy the browser
  session it didn't own).
- Key on `/mcp` → authenticates; a **browser cookie on `/mcp`** → 401 (no
  cookie path).
- Cookie session on REST unaffected by any MCP change (existing suite green,
  unmodified).
- **Timing:** the HMAC + unique-index lookup leaks no oracle for
  well-formed-vs-malformed tokens (both reject at the same pipeline step,
  malformed failing before the DB hit — acceptable and documented).

### 1.3 Scope enforcement (the blast-radius matrix)
For each scope ∈ {`read`, `read,write`, `read,write,delete`}:
- REST: `GET/HEAD` → read; `POST/PATCH/PUT` → write; `DELETE` → delete;
  `POST /api-keys` → **always rejected for bearer** (1.1).
- MCP tools: `readOnlyHint` → bound to `read`; `destructiveHint` → bound to
  `delete`; else → `write`. Scope-denied call → `isError` result naming the
  missing scope (not a transport failure), tool still listed.
- 404-or-empty across users (never 403-with-existence-hint): a key owned by
  user A cannot read/touch user B's boards/cards/notes/columns.
- `write` cannot delete anything; `read` cannot mutate anything; `admin`
  gates nothing until a Wave 2 admin route exists (and the create UI hides
  the `admin` checkbox until then — verify at the frontend layer).

### 1.4 MCP endpoint
- `MCP_ENABLED=false` → `/mcp` is **not mounted** — path 404s through the
  normal fallback. Nothing about the app changes when the flag is absent.
- With the flag on: `initialize` / `tools/list` / `tools/call` behave; the
  kill switch and the per-key revocation path both stop requests on the very
  next call (no cache, no grace).
- `API_KEY_SECRET`: required ≥ 32 bytes in prod at boot; dev falls back to
  `SESSION_SECRET` **with a warn log** (verify the warn fires); rotating it
  invalidates every key at once (revoke-then-remint runbook path).
- The `POST /api-keys` body never appears in a log line, span, or activity
  log; the key appears in no URL (header only); the plaintext is rendered to
  the browser once and cleared from component state on dialog close.
- `SetSensitiveRequestHeadersLayer` covers `AUTHORIZATION` (span/log hygiene)
  — re-verify for any new MCP handler.
- `grep -rn "kp_live_"` over tracked files (CI gate) finds nothing.
- The MCP module contains no SQL (`grep -n "sqlx::query" src/mcp/`) — the
  single-kernel rule, enforced by CI.
- `rmcp` added to `deny.toml`'s allow-list derives from a fresh `cargo deny
  check`, not by hand-editing the list.

### 1.5 Rate limiting
- Per-key bucket: `THROTTLE_AGENT_PER_MIN` (default 300) is independent of
  the browser's per-IP bucket; an agent burst cannot starve the owner's own
  browser; 429 carries `Retry-After` and the shared problem+json shape.
- The bucket key derives from a **domain-separated** HMAC of the token (e.g.
  `HMAC(secret, "mcp-bucket:" || token)`) — not the stored token hash.

## 2. Live findings so far
- None — the surface does not exist in production yet (by design;
  `MCP_ENABLED` defaults to `false`). This phase exists to catch design and
  implementation drift before that changes.

## 3. Remediation guidance
- If a bearer key can mint a key: remove the bearer path on
  `POST /api/v1/api-keys` (cookie-only minting) or cap minted scopes at the
  caller's scopes — enforce in the extractor/middleware, not in the handler.
- If the `isError`-vs-transport distinction is violated: tool failures must
  remain tool results; only protocol faults raise `McpError`.
- If the activity log row for an MCP write omits `api_key_id` or `via='mcp'`:
  fix the write path; attribution is the Wave 3 audit contract
  (`specs/mcp/60-security.md` §4).
- If `deny.toml` was edited instead of re-derived: regenerate from a fresh
  `cargo deny check`.

## 4. Exit criteria
- All 1.1–1.5 checks pass (local stack for implementation; live spot-checks
  only after `MCP_ENABLED=true` is deployed, single spaced `curl` per the
  provider constraints in `01 §0`).
- F-07 re-marked `Verified` in `findings-register.md` with evidence.
- Regression rows added to CI (`SEC-024`'s job): bearer-mint rejection,
  scope matrix, revoked-next-request.

## 5. Task mapping
- `SEC-028` MCP/API-key surface audit — this file.
- Design owner: `specs/mcp/` (20-auth-and-keys, 30-tool-surface, 60-security).
- Implementation: the Wave 3a/3b tasks (specs/mcp/91-task-map.md, IDs by
  taskpad at authoring time) — **gated on `T011`** (backup drill) before any
  Wave 3 migration lands.
