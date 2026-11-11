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
specs/arch-hardening-specs/        Wave 0/1 hardening + Wave 2 collaboration design (T027 → T028–T036)
specs/mcp/                         ← THIS FOLDER — Wave 3: the agent surface
specs/tasks/                       taskpad task files (T001–T027 exist; T028+ not yet authored)
```

The MCP work is **Wave 3** and it is layered on top of Wave 2
(`specs/arch-hardening-specs/30-collaboration.md`): the MCP tools call the same
`authz::require` chokepoint that the REST routes will call, so an agent can
never see more than the human who owns the key.

## House rules this folder inherits

- **TDD, coverage checked.** Every task that lands here follows `AGENTS.md`.
- **DB persistence assertions.** Every mutating tool gets a second read that
  proves the row actually changed — not just the API response.
- **MySQL 8.4 *and* MariaDB 10.6.** Every migration and every query in this
  folder must be green on both engines (`specs/tasks/T006-…`).
- **Forward-only migrations.** New tables are new migration files. `0001` and
  `0002` are never edited; `_sqlx_migrations` is never hand-edited.
- **One authorization implementation.** The MCP surface calls the same kernel
  as the REST surface. See [`10-architecture.md`](10-architecture.md) §6.
