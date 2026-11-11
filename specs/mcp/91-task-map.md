# 91 — Task map (proposal)

> **These are not task files.** Task files are authored into `specs/tasks/`
> **after** the specs in this folder are reviewed — per the instruction that
> the review comes first. This page is the proposal that the review argues
> with: the ordering, the boundaries, the dependencies, and the amount of work
> in each piece.
>
> The numbering below is a **proposal**. `specs/tasks/status.yaml` currently
> reserves `T028`–`T036` for the collaboration wave (per
> `specs/arch-hardening-specs/30-collaboration.md` §6). The MCP track is
> numbered from `T037` onward. **Task IDs do not determine execution order** —
> `status.yaml` orders by `depends` edges — so the Wave 3a tasks below can
> execute before `T028` while carrying higher IDs.

---

## 1. The two waves, and why they are split

```
T028 ──▶ T029 ──▶ T030 ──▶ T031      collaboration wave (already designed)
                 │        │
                 │        └──▶ T034 (activity log)
                 │
T037 ──▶ T038 ──▶ T039                 Wave 3a — agent credentials
                     │                 (no dependency on the collab wave)
                     │
T040 ◀────────────────┘
  │
  └──▶ T041 ──▶ T042 ──▶ T043 ──▶ T044    Wave 3b — the MCP surface
                                    │
                                    └──▶ T045 ──▶ T046
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

### T037 — API key schema and store

**Goal.** A table, a store, and the hashing that makes a key safe to store.

| | |
|---|---|
| **Depends on** | — |
| **Blocks** | T038, T039 |
| **Migration** | `0003_api_keys.sql` (see `20-auth-and-keys.md` §2) |
| **Backend** | `backend-rs/src/features/api_keys/` — `types.rs`, `store.rs`, `scope.rs`, `token.rs` (generation + HMAC) |
| **Config** | `API_KEY_SECRET` — required in production, validated in `Settings::validate` |
| **Tests** | `tests/api/api_keys_store.rs` — generation, hashing, the unique-index lookup, expiry, revocation, both engines |

**Acceptance**
- [ ] A key round-trips: minted → hashed → looked up by hash → resolves to its user.
- [ ] The plaintext is never stored and never logged.
- [ ] `cargo deny check` is green after adding `rmcp` (the dependency lands in T040, but the config change lands here).
- [ ] Both engines apply `0003_api_keys.sql` with matching checksums.

---

### T038 — Bearer authentication and scope enforcement

**Goal.** Turn `Authorization: Bearer …` into an `Actor`, and enforce scopes
on every surface.

| | |
|---|---|
| **Depends on** | T037 |
| **Blocks** | T040 |
| **Backend** | `backend-rs/src/features/api_keys/auth.rs` (the middleware), `backend-rs/src/extract.rs` (the `AuthSession` bearer branch) |
| **Middleware** | The key-aware governor extractor (per-key bucket) |
| **Tests** | `tests/api/api_keys_auth.rs` — the full authentication matrix and the scope matrix from `80-testing.md` |

**Acceptance**
- [ ] A browser cookie session is **unaffected** by every change — the existing 40 suites are green with no modification.
- [ ] The `/auth/login`, `/auth/register`, `/auth/logout` paths reject bearer tokens.
- [ ] A `read` key cannot write over REST **or** MCP.
- [ ] A revoked key fails on the next request.
- [ ] Foreign ids still return **404**, never 403.
- [ ] `GET /api/v1/api-keys` returns the caller's own keys only.

---

### T039 — Key management: REST, CLI, and the page

**Goal.** Let a user mint, list and revoke their own keys.

| | |
|---|---|
| **Depends on** | T037, T038 |
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

### T040 — The MCP server: transport, mount, and the module skeleton

**Goal.** A working MCP endpoint that speaks the protocol and does nothing
else yet.

| | |
|---|---|
| **Depends on** | T038 **and** the collaboration wave's `T029` (authz kernel) |
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

### T041 — Read tools and `get_card_context`

| | |
|---|---|
| **Depends on** | T040 |
| **Tools** | `list_projects`, `list_boards`, `get_board`, `list_columns`, `list_cards`, `get_card`, `get_card_context`, `list_card_notes`, `list_labels` |
| **Store** | New `fetch_card_context` store functions — a bounded query set on **one** connection |
| **Tests** | `tests/api/mcp_tools_read.rs` |

**Acceptance**
- [ ] Every tool calls a `features::*::store` function — **no SQL in `src/mcp/`**.
- [ ] Every tool goes through `authz::require`.
- [ ] `get_card_context` returns the full bundle in `30-tool-surface.md` §3.2.
- [ ] Every bound in `30-tool-surface.md` §4 is enforced, with the mandatory truncation marker.
- [ ] An oversized result is an **error**, not a silently truncated payload.

---

### T042 — Write tools

| | |
|---|---|
| **Depends on** | T041 |
| **Tools** | `create_card`, `update_card`, `move_card`, `add_card_note`, `create_checklist`, `add_checklist_item`, `set_checklist_item`, `set_card_labels`, `create_label` |
| **Tests** | `tests/api/mcp_tools_write.rs` — with a **DB-persistence assertion per tool** |

**Acceptance**
- [ ] `move_card` delegates to the existing move algorithm — clamping, gap maintenance and deadlock retry are the store's, not the tool's.
- [ ] Every mutating tool writes an activity-log row with `actor_key_id` and `via='mcp'`.
- [ ] Every mutating tool has its persistence assertion.
- [ ] A `read`-scoped key gets a 403-shaped error result for every one of them.
- [ ] `update_card` never moves a card; `move_card` never edits scalars — the same split the REST API uses.

---

### T043 — Claims and leases

| | |
|---|---|
| **Depends on** | T042 |
| **Migration** | `0004_card_claims.sql` |
| **Tools** | `claim_card`, `release_card`, `heartbeat_claim` |
| **Tests** | `tests/api/mcp_claims.rs` — including the **concurrent-claim** case on both engines |

**Acceptance**
- [ ] Two concurrent claims on one card: exactly one wins, the other gets 409.
- [ ] A live foreign claim blocks a claim and names the holder and the expiry.
- [ ] An expired claim can be taken over.
- [ ] A heartbeat by a non-holder is a 409.
- [ ] The claim statement's behaviour is verified on **MySQL 8.4 and MariaDB 10.6** — the candidate SQL in `40-workflows.md` §3 is a starting point, not a guarantee.

---

### T044 — Artifacts and card-scoped notes

| | |
|---|---|
| **Depends on** | T042 |
| **Migration** | `0005_card_links.sql` |
| **Backend** | `GET /api/v1/notes?card_id=N` — a new filter on the existing list route |
| **Tools** | `link_artifact`, `unlink_artifact` |
| **Contract** | `openapi.json` + `schema.d.ts` regenerated (the notes query gains a parameter) |

**Acceptance**
- [ ] `link_artifact` stores a URL string and **never fetches it** — no outbound HTTP anywhere in the server.
- [ ] `GET /notes?card_id=N` returns exactly the card's notes, and composes correctly with the existing `type=card` filter.
- [ ] The existing note wire shapes are unchanged.

---

### T045 — Frontend: notes on the card, claim and artifact chips

| | |
|---|---|
| **Depends on** | T043, T044 |
| **Frontend** | `card-detail-panel.tsx` — the linked-notes section, the claim chip, the artifact chip |
| **Tests** | Vitest for the panel; Playwright for the card-notes and claim-chip specs |

**Acceptance**
- [ ] A note attached to a card is visible in the card detail panel — *the gap that existed before MCP*.
- [ ] A live claim shows the holder and the remaining time; an expired claim shows nothing.
- [ ] The claim chip is a hint, not a lock — a human can still edit a claimed card.
- [ ] The Playwright specs use API calls for setup and never call `waitForTimeout()`.

---

### T046 — Documentation, persona, and the end-to-end gate

| | |
|---|---|
| **Depends on** | T045 |
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

**`T034` (activity log) needs two extra columns**, and this must be recorded
when the Wave 2 task files are authored:

| Column | Type | Purpose |
|---|---|---|
| `actor_key_id` | `INT NULL`, FK → `api_keys` `ON DELETE SET NULL` | Which credential made the change |
| `via` | `ENUM('browser','mcp','cli')` | Which surface made the change |

Without them, an agent's write is attributed to a human, and the multi-user
story has no audit trail. **This is the one cross-wave dependency the MCP
track imposes on the collaboration wave**, and it should be recorded in the
`T034` task file and in `specs/arch-hardening-specs/30-collaboration.md` §5.

## 5. Workload

| Task | Rough size | Dominant cost |
|---|---|---|
| T037 | S | Migration + store + hashing |
| T038 | M | The middleware, the scope matrix, the governor change, and proving the browser path is untouched |
| T039 | M | REST + CLI + a full frontend page |
| T040 | M | rmcp integration, the mount, the middleware stack, the protocol tests |
| T041 | M | 9 tools + the composite context query |
| T042 | M | 9 tools + persistence assertions |
| T043 | S–M | One table, three tools, one hard concurrency test |
| T044 | S | One table, two tools, one query filter |
| T045 | M | Three UI surfaces |
| T046 | S | Docs + persona + a manual e2e |

**T040 and T045 are the two that can surprise.** T040 because MCP protocol
details (version negotiation, header handling, the SSE fallback) are easy to
get subtly wrong; T045 because the claim chip requires the card read path to
carry claim state, and that path is hot.

## 6. Explicitly out of scope

- **Autonomous dispatch** (Flow B) — a job queue, a runner, a worktree
  manager, cancellation, timeouts, run status in the UI. See `00-overview.md`
  §5.
- **A browser-based MCP client** — would need `AUTHORIZATION` added to the
  CORS `allow_headers` and a re-audit of T7 in `60-security.md`.
- **OAuth / OIDC for MCP** — a workspace-identity feature, belonging with
  Wave 2.
- **SSE / WebSocket push** — deferred by the collaboration spec.
- **Full-text search over cards** — depends on `T023`.
- **Per-resource scopes** — see **O-2**.
- **The agent merging its own PR** — a human merges.
