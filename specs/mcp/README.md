# MCP specs — agent integration for KanPad

> **Status: design only.** Nothing in this folder is implemented. Task files are
> authored **later**, into `specs/tasks/`, after these specs are reviewed.
> Every task that descends from this folder MUST cite the spec section it
> implements (the house rule from `specs/tasks/T027-collaboration-design-spec.md`).

## What this is

KanPad exposes an **MCP (Model Context Protocol) server inside `backend-rs`**, so
that an AI coding agent (OpenCode, Claude Code, Cursor, …) can read and write
board data on behalf of a KanPad user. Each user mints their own **scoped API
key** from the frontend; the key is the agent's identity, and the **server**
enforces what that key may do.

The server runs **in the same Rust process** as the REST API, over the same
database pool, behind the same authorization kernel. It is not a sidecar, not a
proxy, and it does not re-implement board logic.

## Document map

| File | Covers |
|---|---|
| [`00-overview.md`](00-overview.md) | Goals, non-goals, the two flows, and the evaluation of the chosen design against *efficient / secure / reliable / fast*. |
| [`10-architecture.md`](10-architecture.md) | Process + transport + mount point, the middleware stack, the dependency graph, the `rmcp` version and feature set, and the hard constraints on the MCP module. |
| [`20-auth-and-keys.md`](20-auth-and-keys.md) | `api_keys` schema, token format, hashing, scopes, the actor model, issuance via REST + CLI, revocation, and per-key rate limiting. |
| [`30-tool-surface.md`](30-tool-surface.md) | The tool catalogue: every tool, its input schema, its result shape, its MCP annotations, and the scope it requires. |
| [`40-workflows.md`](40-workflows.md) | The agent work loop — context, claim, work, report, hand back — plus multi-agent coordination on a shared board and board-level instructions. |
| [`50-frontend.md`](50-frontend.md) | The API-key management page, the card-linked notes surface, and the agent-presence UI. |
| [`60-security.md`](60-security.md) | Threat model, prompt injection (the dominant risk), blast-radius analysis, and the audit trail. |
| [`70-operations.md`](70-operations.md) | Configuration, deployment, metrics, the kill switch, and the runbook. |
| [`80-testing.md`](80-testing.md) | Test strategy, harness rules, and the matrices that must be green. |
| [`90-open-calls.md`](90-open-calls.md) | The decisions still open, each with a recommendation. **These are the review agenda.** |
| [`91-task-map.md`](91-task-map.md) | A *proposal* for the task breakdown. **Not** task files — those are authored after review. |

## Relationship to the rest of the spec tree

```
specs/planning-artifacts/          product requirements
specs/api-rewrite-rs-specs/        the Rust backend contract (20-api-contract, 30-db-schema)
specs/arch-hardening-specs/        Wave 0/1 hardening + Wave 2 collaboration design (T027 review gate; collab implementation landed as T031–T042 — next free ≥ T043)
specs/mcp/                         ← THIS FOLDER — Wave 3: the agent surface
specs/tasks/                       taskpad task files (T001–T042 exist; next free ≥ T043)
```

The MCP work is **Wave 3** and it is layered on top of Wave 2
(`specs/arch-hardening-specs/30-collaboration.md`): the MCP tools call the same
`authz::require` chokepoint that the REST routes will call, so an agent can
never see more than the human who owns the key.

## House rules this folder inherits

- **TDD, coverage checked.** Every task that lands here follows `AGENTS.md`.
- **DB persistence assertions.** Every mutating tool gets a second read that
  proves the row actually changed — not just the API response.
- **MySQL 8.4 *and* MariaDB 11.4** (the CI matrix legs — `.github/workflows/backend-rs.yml`).
  Prod flavor (10.6 vs 11.4) is unverified; the T011 restore drill records
  `SELECT VERSION()` and `specs/arch-hardening-specs/40-operations.md` §6 owns the
  correction. Every migration and every query in this
  folder must be green on both engines (`specs/tasks/T006-…`).

- **Amendments** (2026-11-16, post-review, passes 1 + 2): migration numbering re-based after
  `0003_sort_key_fractional_ordering.sql` (T010) landed — this folder's migrations
  are now `0004_api_keys` / `0005_card_claims` / `0006_card_links` (see `90-open-calls.md`
  O-12); task IDs are no longer pre-reserved — the collab wave's taskpad range
  landed 2026-11-16 ahead of its review as **T031–T042** (next free ≥ T043) and
  the MCP tasks are labeled symbolically (`3a-1…3b-7`)
  until authored (see `91-task-map.md`). Pass 2 (spec review 2026-11-16):
  **key minting is cookie-session only** (O-13 — a bearer key cannot `POST
  /api/v1/api-keys`; closes a one-call scope-escalation); **`link_artifact` is
  now fully specified** (DDL `30-tool-surface.md` §3.6, `links` array in
  `get_card_context`, read path `GET /cards/{id}/links` — O-14);
  `get_card_context` is the normative payload (`_trust` fences, `repo`,
  `links`); claim atomicity defaults to `SELECT … FOR UPDATE` (the
  `INSERT … SELECT` candidate dropped `note`); `rand`/`getrandom` must be a
  **direct** dependency; the collab `activity_log` now carries
  `actor_key_id`/`via` (see `specs/arch-hardening-specs/30-collaboration.md`
  §5); Wave 3 migrations are gated on T011; MCP/API-key audit coverage
  added to the security folder (phase 11, SEC-028).
- **Contract discipline.** The MCP surface itself is not REST, but two things this
  folder does produce REST contract changes: the `/api/v1/api-keys` family and
  `GET /notes?card_id=`. Both MUST go through the T52 same-PR process (spec +
  utoipa + `openapi.json` + `schema.d.ts` together), and the frontend regenerates
  via `npm run gen:api`.
- **Forward-only migrations.** New tables are new migration files. `0001`–`0003` are never edited; `_sqlx_migrations` is never hand-edited.
- **One authorization implementation.** The MCP surface calls the same kernel
  as the REST surface. See [`10-architecture.md`](10-architecture.md) §6.
