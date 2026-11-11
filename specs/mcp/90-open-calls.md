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
answer.

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

`0003_api_keys.sql` and `0004_card_claims.sql` are proposed. The collaboration
wave (`T028`) also wants a migration, and it has not been written yet.

**Recommendation:** claim `0003` and `0004` for the MCP/authn tables, and
renumber the collaboration wave's migration to `0005`. The collaboration task
files have not been authored, so the renumber is free **now** and expensive
later.

**If the collaboration wave is executed first**, the MCP migrations become
`0004`/`0005` instead. The order does not matter functionally — sqlx applies
migrations in version order — but the numbering should be settled before the
task files are written.

---

## Decision log

Record the outcome of each call here, with the rationale, once reviewed.

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
