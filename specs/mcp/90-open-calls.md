# 90 — Open calls

> **This is the review agenda.** Each open call has a recommendation. Nothing
> here is decided until the review says so, and each decision here changes the
> shape of the work in `91-task-map.md`.
>
> Format matches `specs/arch-hardening-specs/30-collaboration.md` §7.

---

## O-1 — Card discussion: reuse `notes`, or add a `comments` table?

**The problem.** The agent needs a channel to report back on a card. There is
no comment system today.

**Option A — reuse `notes` (recommended).**
`notes` already has a `card_id` FK, markdown content, tags, and a full
frontend (editor, link selector, markdown + mermaid renderer). An agent report
is long-form, which suits a note better than a comment.

*Cost:* a note requires a `title` (the agent must invent one); notes are
"documents" conceptually, so the UI needs a "Linked notes" section on the card;
`GET /notes` needs a `card_id` filter.

**Option B — a dedicated `comments` table.**
`comments(id, card_id, author_user_id, body, kind, created_at)` — semantically
correct, append-only, no title needed.

*Cost:* a new table, a new REST family, a new frontend component, a new
activity-log subject type, and the OpenAPI + generated-types churn.

**Recommendation: Option A.** The concept of a comment thread is not what an
agent produces — it produces a *report*. And Option A fixes a pre-existing gap
(users can already attach a note to a card and never see it there).

**If B is chosen:** add `card_comments` to the tool surface and a comment
section to the card detail panel; `notes` stays for notes.

---

## O-2 — Scope granularity: method-mapped, or per-resource?

**The problem.** `write` covers "create a card" and "rename a board" alike.

**Option A — method-mapped (recommended).** `GET`→`read`, `POST`/`PATCH`/`PUT`→`write`,
`DELETE`→`delete`, plus `/admin/*`→`admin`.

*Cost:* a `write` key can create and rename boards. It cannot delete them.

**Option B — per-resource scopes.** `cards:read`, `cards:write`, `boards:write`, …

*Cost:* every route must declare its scope. With 26 routes today and a
`route_layer` per scope group, this is roughly a day more work plus a
maintenance burden — every new route must remember to declare a scope, and a
route that forgets it silently gets the default.

**Recommendation: Option A now.** The coarse mapping already captures the
property that matters (a `read` key cannot write; a `write` key cannot
delete). Option B is the right answer if the user model ever grows
per-resource grants beyond Wave 2's `viewer`/`editor`.

**Middle path:** the *tool surface* is already precise — `delete_board` and
`delete_project` simply are not exposed (`30-tool-surface.md` §6). The
coarseness only shows up over raw REST, which is a scripting surface, not the
agent surface.

---

## O-3 — Claim semantics: a lease table, or advisory labels?

**The problem.** Two agents on one board can pick the same card.

**Option A — `card_claims` lease table (recommended).** Expiring rows, exclusive
`PRIMARY KEY (card_id)`, atomic claim, heartbeat. Visible in the UI as a chip.

*Cost:* one table, one migration, three tools, a claim-state join on the card
read path.

**Option B — advisory labels.** Attach a reserved label (`agent:working`) to
the card.

*Cost:* nothing! But it is **not exclusive** — two agents can attach the same
label, and there is no expiry, so a dead agent's card stays marked forever.

**Recommendation: Option A.** Option B is a reasonable *deferral* if the
claim is not wanted in the first wave — the tool surface can ship without
`claim_card` and the workflow simply skips step 2. But "each user runs their
own agent on the same board" is the stated multi-user goal, and that goal needs
exclusivity.

**If deferred:** the MCP tool surface ships without the claim tools, and
`40-workflows.md` §1 is marked as "step 2 pending".

---

## O-4 — Artifact links: a `card_links` table, or URLs inside notes?

**Option A — `card_links` (recommended).** `card_links(id, card_id, kind, url,
title)`. The agent links the pull request; the card shows a "PR #42" chip.

*Cost:* one table, one migration, two tools, a UI chip.

**Option B — URLs inside the note body.** The agent writes the PR URL into the
report note's markdown.

*Cost:* nothing. But the URL is buried in prose, is not clickable as a chip,
cannot be tracked, and is lost if the note is edited.

**Recommendation: Option A.** The link is structured data that the UI can act
on, and "which PR belongs to this card" is a question the board will want to
answer. The recommended design is now specified end-to-end (amendment
2026-11-16, decision O-14): DDL in `30-tool-surface.md` §3.6 (`0006_card_links.sql`),
`link_artifact`/`unlink_artifact` in the §3/§5 catalogue, the `links` array in
`get_card_context`, and **`GET /api/v1/cards/{id}/links`** as the frontend
read path (additive — T52 same-PR).

---

## O-5 — Where the MCP server lives: in-process, or a sidecar?

**Option A — in-process in `backend-rs` (recommended).** One authorization
kernel, one implementation of board logic, no extra hop, one deploy unit.

**Option B — a separate service.** Isolated, independently deployable, but it
duplicates the authorization and board logic or pays a network hop.

**Recommendation: Option A.** The reasoning is laid out in `00-overview.md` §4.

**The counter-argument, stated fairly:** a sidecar can be written in a
different language, deployed without rebuilding the Rust service, and cannot
break the main API. For a solo operator with one live deployment, "cannot break
the main API" is worth something. The mitigations for Option A are: the MCP
mount is behind `MCP_ENABLED` (default `false`), it is stateless, and its
failure modes are contained in one module.

---

## O-6 — Board instructions: a reserved note title, or a column?

**Option A — a note whose title is exactly `AGENT.md` (recommended).** No
schema change; `get_card_context` returns it in the `instructions` field.

*Cost:* the convention is implicit. A note titled `AGENT.md` is found by exact
title match — a note titled `AGENTS.md` would be missed, silently.

**Option B — a `board_instructions` column on `boards`.** Explicit, typed, no
ambiguity.

*Cost:* one more migration, one more DTO field, one more OpenAPI field — and it
duplicates what a note already is (markdown text attached to a board).

**Recommendation: Option A**, with the exact-title convention documented in
both the integration guide and the card-detail UI. If the ambiguity proves
annoying in practice, Option B is a small follow-up.

---

## O-7 — Should the key be able to reach the REST API?

Today the plan says **yes**: the same bearer key authenticates `/api/v1/*`
and `/mcp`, with the same scope enforcement. That is convenient for scripts
and CI, and it is one implementation of the scope rules.

**The alternative** is to restrict keys to `/mcp` only, forcing every script to
use MCP. That is a worse ergonomics trade for a real benefit: a `read` key
used in a shell script is harmless, and the scope system already bounds it.

**Recommendation: yes.** The bearer key is a general scoped credential for the
account; MCP is one surface that uses it.

---

## O-8 — Should a key's scopes be immutable?

**Option A — immutable (recommended).** A key's scope set is fixed at
creation. Widening it means creating a new key and revoking the old one.

*Why:* a key's permissions should never widen silently. Rotation is explicit,
visible in the activity log, and leaves a trail.

*Cost:* the owner mints a new key to add a scope. That is a one-minute
operation and the UI can offer it directly ("create a replacement with
`write`").

**Option B — mutable.** The owner edits the scopes in place.

*Why not:* a key whose permissions changed under it may be in the middle of a
long autonomous run whose behaviour suddenly widens. Immutability keeps the
blast radius of a key constant for its whole life.

**Recommendation: Option A.**

---

## O-9 — MCP session mode: stateless, or stateful?

**Option A — stateless (recommended).** `legacy_session_mode(false)` +
`json_response(true)`. No in-memory session store; a restart does not
invalidate clients; no replica affinity needed.

*Risk:* rmcp's legacy-compatibility path must actually work with the OpenCode
client's default `protocol: "legacy"`. **This must be verified in task C1**
before the rest of the MCP work depends on it.

**Option B — stateful.** `LocalSessionManager` holds sessions in memory.
Works with every client; a restart drops the sessions; a second replica needs
affinity.

**Recommendation: Option A, with B as the documented fallback.** The fallback
costs one config flag, not a redesign.

---

## O-10 — Should `MCP_ENABLED` default to `true` in production?

**Option A — `false` everywhere (recommended).** The endpoint does not exist
until it is deliberately enabled.

**Option B — `true` in production, `false` in development.** One less
deployment step for the operator.

**Recommendation: Option A.** A new public endpoint that performs reads and
writes on a user's data should be opt-in, even for a solo deployment. It costs
one environment variable.

---

## O-11 — Should the agent be able to delete anything at all in the first wave?

**Option A — yes, `delete_card` behind the `delete` scope (recommended).**
An agent that cannot remove a card it created leaves debris.

**Option B — no delete tools at all in the first wave.**

**Recommendation: Option A.** `delete_card` is the narrowest possible
destructive capability, and it sits behind a scope the owner must explicitly
grant. `delete_board`, `delete_project` and `move_all_cards` stay out
regardless.

---

## O-12 — Where do the new migrations land?

> **Amendment 2026-11-16 — current state of the tree:** `0001_baseline.sql`,
> `0002_convert_to_utf8mb4.sql`, and **`0003_sort_key_fractional_ordering.sql`**
> (T010) exist. `0003` is **taken**; the earlier draft's proposal to "claim
> `0003` and `0004` for the MCP/authn tables" is void — reusing `0003` would
> collide with its `_sqlx_migrations` checksum.

**Recommendation (amended):** claim **`0004_api_keys.sql`** and
**`0005_card_claims.sql`** for the MCP/authn tables, and
**`0006_card_links.sql`** for the artifacts table; the Wave 2 collaboration
wave's migration then takes the next free slot (currently `0007`, or later if
something else lands first). sqlx applies migrations in version order, so the
split is functionally irrelevant — the numbering just has to stop colliding.

**If the collaboration wave is executed first**, its migration takes `0004` and
the MCP set shifts to `0005`/`0006`/`0007`. The order does not matter
functionally — sqlx applies migrations in version order — but the numbers
should be settled before task files are written, and the rule is: **never
reuse or hand-edit an applied version; always take the next free number.**

---

## O-13 — Can a bearer key mint keys? **Decided 2026-11-16 (spec review)**

**The problem.** Under the bare method map (`POST → write`), any `write` key
could call `POST /api/v1/api-keys` with `scopes: ["delete", "admin"]` and
mint a strictly wider key — a one-call privilege escalation.

**Decision: minting is cookie-session only.** The bearer middleware rejects
`POST /api/v1/api-keys` with **401** before the handler runs, for every
scope. Key minting is an interactive browser act (the UI page); it is not an
agent act.

**Alternatives rejected:** (a) cap minted scopes at the caller's own (weaker —
a leaked `write` key still mints unlimited lifetime); (b) require `admin`
scope for minting `delete`/`admin` keys (couples the user page to a Wave 2
scope that gates nothing yet). `GET /api-keys` (list/read own) and `DELETE`
(revoke own) remain bearer-reachable — they are the owner's recovery levers.

Recorded in `20-auth-and-keys.md` §5/§6.1; verified by the SEC-028 audit
(`specs/security/11-mcp-api-key-surface.md`).

## O-14 — Where does the card-detail artifact chip read its data? **Decided 2026-11-16 (spec review)**

**The problem.** The card list/detail shows a PR chip per artifact link
(`50-frontend.md` §3), but no read path existed — the notes list does not
carry links, and `get_card_context` is agent-facing, not frontend-facing.

**Decision:** agents read links via the `links` array in `get_card_context`
(bounded 20, §3.2 of `30-tool-surface.md`); the frontend reads them via a new
additive REST route **`GET /api/v1/cards/{id}/links`** (newest first, T52
same-PR process with `openapi.json` + `schema.d.ts` regenerated). No new tool,
no wire-shape change to card responses.

## Decision log

Record the outcome of each call here, with the rationale, once reviewed.
Calls O-1 through O-12 remain **undecided** — they are the review agenda.
O-13/O-14 were settled by the 2026-11-16 spec review and are recorded for the
audit trail.

| Call | Decision | Rationale | Date |
|---|---|---|---|
| O-1 | | | |
| O-2 | | | |
| O-3 | | | |
| O-4 | | | |
| O-5 | | | |
| O-6 | | | |
| O-7 | | | |
| O-8 | | | |
| O-9 | | | |
| O-10 | | | |
| O-11 | | | |
| O-12 | | | |
| O-13 | **Decided:** minting (`POST /api/v1/api-keys`) is cookie-session only; bearer → 401 | One-call privilege escalation under the method map; cookie-only is the minimal invariant that closes it. See §O-13 above. | 2026-11-16 |
| O-14 | **Decided:** `GET /api/v1/cards/{id}/links` (frontend) + `links` array in `get_card_context` (agents) | Additive wire change, no new tool, no reshaping of card responses; both consumers share the `card_links` table. | 2026-11-16 |
