# 91 — Task map (proposal)

> **These are not task files.** Task files are authored into `specs/tasks/`
> **after** the specs in this folder are reviewed — per the instruction that
> the review comes first. This page is the proposal that the review argues
> with: the ordering, the boundaries, the dependencies, and the amount of work
> in each piece.
>
> The labels below are **symbolic** — `3a-1…3b-7` are not task IDs. Task IDs
> are assigned by `taskpad new` at authoring time, after these specs are
> reviewed and the ID space is settled. **Amendment 2026-11-16:** the collab
> wave is rows A–L in `specs/arch-hardening-specs/30-collaboration.md` §6 (rows J–L added by
> D-7, 2026-11-16) and
> starts at the next free taskpad slot (currently ≥ T031 — T028/T029/T030
> already exist: storybook, SQL observability, Sentry/OTel). The MCP track
> remains numbered symbolically so it cannot collide with whatever IDs the
> collab set consumes. **Task IDs do not determine execution order** —
> `status.yaml` orders by `depends` edges.

---

## 1. The two waves, and why they are split

```
Wave 2    T027 ─▶ impl tasks (collab A–L)   collaboration wave (design approved → authoring)
                 │        │
                 │        └──▶ row G (activity log)
                 │
Wave 3a   3a-1 ─▶ 3a-2 ─▶ 3a-3              Wave 3a — agent credentials
                            │               (no dependency on the collab wave)
                            │
Wave 3b   3b-1 ◀────────────┘              needs collab row B (authz kernel)
  │
  └──▶ 3b-2 ─▶ 3b-3 ─▶ 3b-4 ─▶ 3b-5        Wave 3b — the MCP surface
                          │
                          └──▶ 3b-6 ─▶ 3b-7
```

**Why Wave 3a has no dependency on the collaboration wave.** Authentication
("who is this?") and authorization ("what may they do?") are different
concerns. A key resolves to a `users.id` and stays correct regardless of
whether workspaces exist. When they do exist, the key inherits the user's
memberships automatically — the scope check only ever narrows. So the
credential infrastructure can land early and get *better* rather than being
invalidated.

**Why Wave 3b does depend on it.** The MCP tools must call `authz::require`.
Building the MCP surface before the kernel exists means writing a second,
temporary authorization path — the exact failure mode this whole design
exists to prevent.

---

## 2. Wave 3a — agent credentials

> **Migration gating (2026-11-16):** every MCP migration in this map
> (`0004_api_keys`, `0005_card_claims`, `0006_card_links`) is a production
> migration and is gated on **T011** — the backup drill must record
> `SELECT VERSION()` before the first Wave 3 migration lands (cutover
> doctrine; see `specs/arch-hardening-specs/40-operations.md` §3).

### 3a-1 — API key schema and store

**Goal.** A table, a store, and the hashing that makes a key safe to store.

| | |
|---|---|
| **Depends on** | — |
| **Blocks** | 3a-2, 3a-3 |
| **Migration** | `0004_api_keys.sql` (see `20-auth-and-keys.md` §2) — lands only after T011's drill |
| **Backend** | `backend-rs/src/features/api_keys/` — `types.rs`, `store.rs`, `scope.rs`, `token.rs` (generation + HMAC) |
| **Config** | `API_KEY_SECRET` — required in production, validated in `Settings::validate` |
| **Tests** | `tests/api/api_keys_store.rs` — generation, hashing, the unique-index lookup, expiry, revocation, both engines |

**Acceptance**
- [ ] A key round-trips: minted → hashed → looked up by hash → resolves to its user.
- [ ] The plaintext is never stored and never logged.
- [ ] `rand` (or `getrandom`) is a **direct** dependency in `Cargo.toml`
      (transitive reachability is not importability).
- [ ] `cargo deny check` is green after adding `rmcp` (the dependency lands in 3b-1, but the config change lands here).
- [ ] Both engines apply `0004_api_keys.sql` with matching checksums.

---

### 3a-2 — Bearer authentication and scope enforcement

**Goal.** Turn `Authorization: Bearer …` into an `Actor`, and enforce scopes
on every surface.

| | |
|---|---|
| **Depends on** | 3a-1 |
| **Blocks** | 3b-1 |
| **Backend** | `backend-rs/src/features/api_keys/auth.rs` (the middleware), `backend-rs/src/extract.rs` (the `AuthSession` bearer branch) |
| **Middleware** | The key-aware governor extractor (per-key bucket) |
| **Tests** | `tests/api/api_keys_auth.rs` — the full authentication matrix and the scope matrix from `80-testing.md` |

**Acceptance**
- [ ] A browser cookie session is **unaffected** by every change — the existing 40 suites are green with no modification.
- [ ] The `/auth/login`, `/auth/register`, `/auth/logout` paths reject bearer tokens.
- [ ] A bearer key **cannot mint keys** — `POST /api/v1/api-keys` with any `kp_live_` token → 401, regardless of scope (decision O-13).
- [ ] A `read` key cannot write over REST **or** MCP.
- [ ] A revoked key fails on the next request.
- [ ] Foreign ids still return **404**, never 403.
- [ ] `GET /api/v1/api-keys` returns the caller's own keys only.

---

### 3a-3 — Key management: REST, CLI, and the page

**Goal.** Let a user mint, list and revoke their own keys.

| | |
|---|---|
| **Depends on** | 3a-1, 3a-2 |
| **Backend** | `GET/POST/DELETE /api/v1/api-keys`; the `create-api-key` / `list-api-keys` / `revoke-api-key` CLI (mirroring `backend-rs/src/admin.rs`) |
| **Frontend** | `frontend/src/features/api-keys/` + the `/settings/api-keys` route + the sidebar link |
| **Contract** | `backend-rs/openapi.json` regenerated; `frontend/src/lib/api/schema.d.ts` regenerated with `npm run gen:api` |
| **Tests** | `tests/api/api_keys.rs`, `tests/api/api_keys_cli.rs`, and the frontend Vitest suites |

**Acceptance**
- [ ] The plaintext is returned **once** and never again; the list endpoint never contains it.
- [ ] The `POST` response carries `Cache-Control: no-store`.
- [ ] The CLI follows the `create-admin` shape: a core function returning an outcome enum with spec-pinned exit codes.
- [ ] The UI shows the secret once, warns that it cannot be retrieved, and clears it from state on close.
- [ ] A newly minted key's default scope is `read`.

---

## 3. Wave 3b — the MCP surface

### 3b-1 — The MCP server: transport, mount, and the module skeleton

**Goal.** A working MCP endpoint that speaks the protocol and does nothing
else yet.

| | |
|---|---|
| **Depends on** | 3a-2 **and** the collaboration wave's authz kernel (`specs/arch-hardening-specs/30-collaboration.md` §6, row B — assigned a real T0xx ID at authoring) |
| **Dependency** | `rmcp 3.9` — `default-features = false`, features `server`, `macros`, `transport-streamable-http-server` |
| **Backend** | `backend-rs/src/mcp/` — `mod.rs`, `server.rs`, `actor.rs`, `error.rs` |
| **Config** | `MCP_ENABLED` (default `false`), `MCP_PATH`, `MCP_TIMEOUT_SECS`, `MCP_MAX_TOOL_RESULT_BYTES`, `THROTTLE_AGENT_PER_MIN` |
| **Tests** | `tests/api/mcp_wire.rs` — black-box initialize / `tools/list` / `tools/call` |
| **CI** | The `grep -rn "sqlx::query" backend-rs/src/mcp/` gate |

**Acceptance**
- [ ] An OpenCode `type: "remote"` client can `initialize` and `tools/list` against the endpoint — on **both** `protocol: "legacy"` and `protocol: "2026-07-28"`. *(This is the fallback check for **O-9**.)*
- [ ] `MCP_ENABLED=false` → the path 404s through `api_only_fallback`.
- [ ] The endpoint has **no cookie path** — a browser session cannot authenticate an MCP call.
- [ ] The MCP timeout, the MCP rate limit and the MCP compression settings are distinct from the REST ones.
- [ ] The server returns `instructions` (the work loop + the prompt-injection rule).
- [ ] The existing REST suites are green and unmodified.

---

### 3b-2 — Read tools and `get_card_context`

| | |
|---|---|
| **Depends on** | 3b-1 |
| **Tools** | `list_projects`, `list_boards`, `get_board`, `list_columns`, `list_cards`, `get_card`, `get_card_context`, `list_card_notes`, `list_labels` |
| **Store** | New `fetch_card_context` store functions — a bounded query set on **one** connection |
| **Tests** | `tests/api/mcp_tools_read.rs` |

**Acceptance**
- [ ] Every tool calls a `features::*::store` function — **no SQL in `src/mcp/`**.
- [ ] Every tool goes through `authz::require`.
- [ ] `get_card_context` returns the full bundle in `30-tool-surface.md` §3.2 —
      including the `_trust`-fenced description, the `repo` block, and the
      bounded `links` array (O-14).
- [ ] Every bound in `30-tool-surface.md` §4 is enforced, with the mandatory truncation marker.
- [ ] An oversized result is an **error**, not a silently truncated payload.
- [ ] At most 5 queries run on one connection; no parallel fan-out.

---

### 3b-3 — Write tools

| | |
|---|---|
| **Depends on** | 3b-2 |
| **Tools** | `create_card`, `update_card`, `move_card`, `add_card_note`, `create_checklist`, `add_checklist_item`, `set_checklist_item`, `set_card_labels`, `create_label` |
| **Tests** | `tests/api/mcp_tools_write.rs` — with a **DB-persistence assertion per tool** |

**Acceptance**
- [ ] `move_card` delegates to the existing move algorithm — clamping, gap maintenance and deadlock retry are the store's, not the tool's.
- [ ] Every mutating tool writes an activity-log row with `actor_key_id` and `via='mcp'`.
- [ ] Every mutating tool has its persistence assertion.
- [ ] A `read`-scoped key gets a 403-shaped error result for every one of them.
- [ ] `update_card` never moves a card; `move_card` never edits scalars — the same split the REST API uses.

---

### 3b-4 — Claims and leases

| | |
|---|---|
| **Depends on** | 3b-3 |
| **Migration** | `0005_card_claims.sql` — lands only after T011's drill |
| **Tools** | `claim_card`, `release_card`, `heartbeat_claim` |
| **Tests** | `tests/api/mcp_claims.rs` — including the **concurrent-claim** case on both engines |

**Acceptance**
- [ ] Two concurrent claims on one card: exactly one wins, the other gets 409.
- [ ] A live foreign claim blocks a claim and names the holder and the expiry.
- [ ] An expired claim can be taken over.
- [ ] A heartbeat by a non-holder is a 409.
- [ ] The claim statement's behaviour is verified on **MySQL 8.4 and MariaDB 11.4** — the candidate SQL in `40-workflows.md` §3 is a starting point, not a guarantee.

---

### 3b-5 — Artifacts and card-scoped notes

| | |
|---|---|
| **Depends on** | 3b-3 |
| **Migration** | `0006_card_links.sql` (DDL: `30-tool-surface.md` §3.6) — lands only after T011's drill |
| **Backend** | `GET /api/v1/notes?card_id=N` — a new filter on the existing list route; **`GET /api/v1/cards/{id}/links`** — new additive route for the artifact chip (O-14) |
| **Tools** | `link_artifact`, `unlink_artifact` |
| **Contract** | `openapi.json` + `schema.d.ts` regenerated (notes query gains a parameter; a new links route is added) |

**Acceptance**
- [ ] `link_artifact` stores a URL string and **never fetches it** — no outbound HTTP anywhere in the server; the URL scheme is constrained to `http`/`https`.
- [ ] `link_artifact` is idempotent on `(card_id, url)` — a re-link returns the existing link, creates no duplicate row.
- [ ] `GET /notes?card_id=N` returns exactly the card's notes, and composes correctly with the existing `type=card` filter.
- [ ] `GET /cards/{id}/links` returns the card's links newest-first; the existing note/card wire shapes are unchanged (additive only).
- [ ] Every mutating tool here has its persistence assertion (`80-testing.md` §3.6).

---

### 3b-6 — Frontend: notes on the card, claim and artifact chips

| | |
|---|---|
| **Depends on** | 3b-4, 3b-5 |
| **Frontend** | `card-detail-panel.tsx` — the linked-notes section, the claim chip, the artifact chip |
| **Tests** | Vitest for the panel; Playwright for the card-notes and claim-chip specs |

**Acceptance**
- [ ] A note attached to a card is visible in the card detail panel — *the gap that existed before MCP*.
- [ ] A live claim shows the holder and the remaining time; an expired claim shows nothing.
- [ ] The claim chip is a hint, not a lock — a human can still edit a claimed card.
- [ ] The artifact chip renders from `GET /api/v1/cards/{id}/links`; a `pr` chip reads "PR #N"; click opens `target="_blank" rel="noopener noreferrer"`.
- [ ] The Playwright specs use API calls for setup and never call `waitForTimeout()`.

---

### 3b-7 — Documentation, persona, and the end-to-end gate

| | |
|---|---|
| **Depends on** | 3b-6 |
| **Docs** | `docs/ai-agent-integration.md`; the `specs/README.md` index entry |
| **Agent** | `.opencode/agent/kanpad-worker.md` |
| **CI** | The `kp_live_` secret-literal grep gate |
| **Tests** | The end-to-end agent run, recorded as a manual gate in the guide |

**Acceptance**
- [ ] The guide covers: creating a key, wiring OpenCode / Claude Code / Cursor / a generic client, the work loop, scopes, rotation, revocation, and troubleshooting (401 / 403 / 429).
- [ ] The guide states that OpenCode does not read `.env` files.
- [ ] The persona encodes the work loop and the prompt-injection rule.
- [ ] A real agent, given a real `read,write` key, takes a card from "Todo" to "Review" with a report note and a linked PR.
- [ ] The `kp_live_` grep gate is green.

---

## 4. The change this makes to an already-designed task

**The Wave 2 activity-log task (`specs/arch-hardening-specs/30-collaboration.md`
§6, row G — ID assigned at authoring) needs two extra columns** — the collab
design now carries them (§5, amended 2026-11-16: `actor_key_id` `INT NULL` →
`api_keys` `ON DELETE SET NULL`; `via` `ENUM('browser','mcp','cli')` `NOT NULL
DEFAULT 'browser'`), so row G's task file only has to name them in its schema
migration and ACs. This must be verified when the Wave 2 task files are
authored. Without them, an agent's write is attributed to a human, and the
multi-user story has no audit trail. **This is the one cross-wave dependency
the MCP track imposes on the collaboration wave**, and it is recorded in
`specs/arch-hardening-specs/30-collaboration.md` §5/§6 (row G) as well as here.
*(Amendment 2026-11-16: was "T034" / "T028–T034"; the Wave 2 rows are
symbolic A–L until taskpad assigns IDs at authoring — J/K/L added by D-7.)*

## 5. Workload

| Task | Rough size | Dominant cost |
|---|---|---|
| 3a-1 | S | Migration + store + hashing |
| 3a-2 | M | The middleware, the scope matrix, the governor change, and proving the browser path is untouched |
| 3a-3 | M | REST + CLI + a full frontend page |
| 3b-1 | M | rmcp integration, the mount, the middleware stack, the protocol tests |
| 3b-2 | M | 9 tools + the composite context query |
| 3b-3 | M | 9 tools + persistence assertions |
| 3b-4 | S–M | One table, three tools, one hard concurrency test |
| 3b-5 | S | One table, two tools, one query filter |
| 3b-6 | M | Three UI surfaces |
| 3b-7 | S | Docs + persona + a manual e2e |

**3b-1 and 3b-6 are the two that can surprise.** 3b-1 because MCP protocol
details (version negotiation, header handling, the SSE fallback) are easy to
get subtly wrong; 3b-6 because the claim chip requires the card read path to
carry claim state, and that path is hot.

## 6. Explicitly out of scope

- **Autonomous dispatch** (Flow B) — a job queue, a runner, a worktree
  manager, cancellation, timeouts, run status in the UI. See `00-overview.md`
  §5.
- **A browser-based MCP client** — would need `AUTHORIZATION` added to the
  CORS `allow_headers` and a re-audit of T7 in `60-security.md`.
- **OAuth / OIDC for MCP** — a workspace-identity feature, belonging with
  Wave 2.
- **SSE / WebSocket push to agents** — not in this wave. Browser-facing SSE cache
  invalidation exists for collaboration (D-7, `31-realtime-sse.md`); it carries no
  agent-facing tool or subscription surface, and the MCP request/response contract is
  unchanged.
- **Full-text search over cards** — depends on `T023`.
- **Per-resource scopes** — see **O-2**.
- **The agent merging its own PR** — a human merges.
