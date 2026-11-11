# 00 — Overview

## 1. Goal

> **A KanPad user can connect their own AI agent to their board and have it do
> real work on their cards — with the server enforcing exactly what that agent
> is allowed to do.**

Concretely, three outcomes:

1. **A user mints their own agent credential** from the frontend, scoped to what
   they want the agent to do, revocable at any moment.
2. **The agent connects to `backend-rs` over MCP** and can read the board and
   write cards, checklists, labels and notes — through the *same* store
   functions and the *same* authorization kernel the browser uses.
3. **Everything the agent did is attributable** — to the user, to the key, and
   to the MCP channel — and visible in the UI.

## 2. The two flows

### Flow A — Assisted (the human drives)

```
user                KanPad API                       AI agent
 │   "work on card #42 in board Sprint 3"
 │──────────────────────▶│                              │
 │                       │                              │ GET /mcp  (initialize)
 │                       │◀─────────────────────────────│
 │                       │  MCP tools: list_boards,    │
 │                       │  get_card_context,          │
 │                       │  claim_card, move_card,     │
 │                       │  add_card_note, ...         │
 │                       │─────────────────────────────▶│
 │                       │  every call: Bearer <key>   │
 │                       │  every call: authz::require │
 │                       │  every write: activity_log  │
 │  sees the card move, the note appear, the claim chip
```

The human names a card; the agent pulls context, works in the repo, and writes
back. **No daemon, no queue, no scheduler.** This is the flow that ships first
and the one that matters.

### Flow B — Autonomous (the board drives) — *deferred, not designed here*

A card entering an "AI" column triggers a runner that spawns an agent, works
the card, and reports back. Deferred deliberately: it needs a job queue, a
runner process, cancellation, leases, timeouts and a second process to
operate — none of which are needed for Flow A. See §5.

## 3. Non-goals

| Non-goal | Why |
|---|---|
| A **separate MCP service** (sidecar / proxy) | A proxy re-implements the board logic (position clamping, gap maintenance, ownership) or pays a network hop to avoid it. Both are worse. See §4. |
| **OAuth / OIDC** for MCP clients | Bearer API keys cover agent credentials today; OAuth is a workspace-identity feature and belongs with Wave 2. |
| **Autonomous dispatch** (Flow B) | Deferred — see §2. |
| **A public tool marketplace** | The tool surface is KanPad's own data. |
| **The agent fetching URLs** | The MCP server never performs outbound HTTP. `link_artifact` stores a string; it does not resolve it. |
| **Replacing the REST API** | The REST API stays the primary contract. MCP is an additional surface over the same core. |

## 4. Design evaluation

The brief was *efficient, secure, reliable, fast*. Four candidate architectures
were scored.

| | **A. In-process Rust MCP** *(chosen)* | B. Sidecar proxy over REST | C. Local stdio shim | D. MCP calls own REST API over loopback |
|---|---|---|---|---|
| **Security** | One authorization kernel; scope enforced at the edge; key never leaves the server | Two auth paths → drift; proxy must re-implement ownership | Proxy holds a real **password**; scope is advisory, not enforced | Two auth paths; a bearer key would need a REST auth surface anyway |
| **Efficiency** | In-process; shares the pool; no extra hop | +1 network hop per tool call; +1 serialization | +1 hop per tool call; +1 process | +1 loopback hop per tool call |
| **Reliability** | Stateless MCP; survives restart; single deploy unit | 2 deploy units; version skew between proxy and API | 2 deploy units; shim is Node, API is Rust | Self-HTTP: risk of pool starvation + timeout layering |
| **Speed** | Tool call → SQL, nothing else | Tool call → HTTP → SQL | Tool call → HTTP → SQL | Tool call → HTTP → SQL |
| **Implementation cost** | Medium — one Rust module | Medium — plus a second codebase | Low — but throwaway | Medium |
| **Verdict** | ✅ | rejected: duplicate auth + logic | rejected: weak credential model | rejected: pointless hop |

**The chosen design is A**, with one refinement that makes it clearly better
than B and C: **the MCP tools call the same store functions the REST handlers
call.** There is exactly one implementation of "move a card", "check ownership",
and "write the activity log" in the system.

### Why the MCP server lives in `backend-rs`

| Property | In-process | Why it matters here |
|---|---|---|
| Authorization | One `authz::require` chokepoint | `specs/arch-hardening-specs/30-collaboration.md` §2 already mandates a single kernel; a second surface would be a second kernel |
| Board logic | One `features::cards::store::move_card` | The move algorithm (clamp, gap open/close, deadlock retry, `READ COMMITTED`) is subtle and tested; duplicating it is how drift starts |
| Ownership | One SQL predicate | Ownership is already baked into the store (`WHERE … AND user_id = ?`); foreign ids → 404, never 403 |
| Latency | ~0 extra | No serialization of the actor, no second pool, no extra TLS |
| Deploy | One artifact | The live deployment already has one container and one proxy |

### Scoring of the chosen design

| Criterion | How the design achieves it | Residual risk |
|---|---|---|
| **Efficient** | Composite tools (`get_card_context`) collapse 5 REST round-trips into 1 MCP call; store functions run on one connection; result payloads are bounded and truncated | Composite queries must stay portable across MySQL 8.4 and MariaDB 10.6 — no `JSON_ARRAYAGG` reliance |
| **Secure** | Server-side scope enforcement; per-key identity; revocation is a single indexed `UPDATE`; audit row per write; `read`-only keys physically cannot call write tools | Prompt injection via card content — see [`60-security.md`](60-security.md), the dominant risk |
| **Reliable** | Stateless MCP (no session store, no affinity requirement); idempotent tool semantics where it matters; per-key rate limit; explicit `isError` results rather than transport failures | Agent-side retries can double-apply non-idempotent writes (`add_card_note`) — mitigated by `idempotentHint: false` + tool guidance |
| **Fast** | In-process; one indexed `SELECT` for auth; no per-request user lookup (the key *is* the identity) | `last_used_at` write must be amortized, not per-request |

## 5. Why autonomous dispatch is deferred

Flow B needs: a durable job queue, a runner process that owns a git worktree, a
lease/heartbeat protocol with crash recovery, cancellation, per-run timeouts,
and a place to surface run status in the UI. That is a second subsystem with
its own failure modes — and it is *not* required for a human to say "work on
card #42".

The good news: Flow A's primitives (key, scope, claim, report, move) are
exactly what Flow B would need. Deferring the dispatcher does not defer the
foundation.

## 6. Sequencing against the existing roadmap

The MCP work is **Wave 3**, but it does **not** all land at once.

```
Wave 1  T011–T027   hardening, T010 fractional sort keys, T027 collaboration design
        ─────────────────────────────────────────────────────────────────────
Wave 2  T028–T036   workspaces + sharing + authz kernel + activity log + admin
        ─────────────────────────────────────────────────────────────────────
Wave 3a  A1–A3     api_keys + bearer auth + key management UI     ← can land
        ─────────────────────────────────────────────────────────────────────              BEFORE Wave 2
Wave 3b  C1–C4     rmcp server + tool surface + card-notes UI     ← needs Wave 2's
                                                                   authz kernel
```

**Wave 3a is deliberately early.** Authentication and authorization are two
different concerns:

- **Authentication** — *who is this?* — is `key → users.id`. It does not change
  when workspaces land.
- **Authorization** — *what may they do?* — is `user rights ∩ key scopes`. When
  Wave 2 gives users workspace memberships, the **same key automatically
  inherits them**. No re-issuance, no migration, no code change in the
  middleware.

So the key infrastructure ships early and gets *better* when the collaboration
wave lands, instead of being invalidated by it.

Wave 3b waits for Wave 2 on purpose: the MCP tools must call `authz::require`,
and that kernel does not exist until T029. Landing MCP first would mean writing
a second, temporary authorization path — precisely the failure mode this design
exists to avoid.

## 7. What "review" needs to settle

The open decisions are collected in [`90-open-calls.md`](90-open-calls.md). The
four that change the shape of the implementation:

| Open call | Recommendation | Impact if changed |
|---|---|---|
| **O-1** — card discussion: reuse `notes`, or add a `comments` table? | Reuse `notes` | Adds a table, an API family and a UI component |
| **O-2** — scope granularity: method-mapped, or per-resource? | Method-mapped (`read`/`write`/`delete`/`admin`) | Per-resource needs a per-route scope declaration layer |
| **O-3** — claim semantics: lease table, or advisory labels? | Lease table (`card_claims`) | Advisory labels cannot prevent two agents working the same card |
| **O-4** — artifact links: structured `card_links`, or URLs inside notes? | Structured `card_links` | A structured table enables PR chips in the UI |

## 8. Document conventions

These specs follow the house style of `specs/arch-hardening-specs/` and
`specs/api-rewrite-rs-specs/`:

- File names are `NN-topic.md`.
- Normative language: **MUST** / **MUST NOT** / **SHOULD** / **MAY**.
- Every table and query that appears here is a **candidate**; the task that
  implements it owns the final text.
- Anything marked *(open call)* is **not** decided. See `90-open-calls.md`.
