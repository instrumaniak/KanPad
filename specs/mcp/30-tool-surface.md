# 30 — The tool surface

## 1. Conventions

**Naming.** Tool names are bare verbs, no server prefix (`get_card`, not
`kanpad_get_card`). OpenCode namespaces them itself as
`kanpad_get_card`. MCP method names use `_`; the OpenCode normalizer replaces
anything else with `_`.

**Argument and result casing: `snake_case`**, identical to the REST wire
contract. This is deliberate — it means a tool result is *literally* the REST
body, so there is no second translation layer and no chance of the two drifting.

**Result encoding.** A successful call returns
`CallToolResult { content: [{ type: "text", text: "<json>" }] }` where `<json>`
is the compact JSON body of the equivalent REST response. `structuredContent`
is **not** used in the first wave — it requires a declared `outputSchema` per
tool, and a plain JSON text body is what every MCP client (and every LLM)
reads correctly today.

**Failure encoding.** A failed tool returns `CallToolResult::error(...)`
(`isError: true`) whose text is the **problem+json body** rendered as compact
JSON — the exact same contract the REST API returns. The agent sees the same
error shape it would see from the API, including `type`, `title`, `status`,
`detail` and `errors[]`. Transport-level faults are reserved for genuine
protocol problems and are never used for a failed query.

**Result bounds.** Every result is bounded. Free text is truncated with an
explicit marker (§4). Lists are paginated. There is no tool that returns an
unbounded collection.

## 2. Scope enforcement

Each tool declares its privilege level through MCP annotations, and the
dispatcher maps that to a scope (§5 of `20-auth-and-keys.md`):

| Annotation | Required scope |
|---|---|
| `readOnlyHint: true` | `read` |
| `destructiveHint: true` | `delete` |
| (neither) | `write` |

A key that lacks the scope gets a 403-shaped error result naming the missing
scope. The tool is still *listed* — hiding it would make the failure mode
confusing ("why is there no `move_card`?") — but it cannot be called.

**One documented exception to "failure → REST status":** `create_label`
returns the existing label (200/201) on a name conflict instead of the
REST API's 409 — deliberate softening so an agent can recover (§3.3). Every
other failure maps to the problem+json error result; a tool that needs a
second exception must say so in its §3 row, or it does not have one.

## 3. The catalogue

### 3.1 Read tools — scope `read`, `readOnlyHint: true`

| Tool | Arguments | Returns |
|---|---|---|
| `list_projects` | — | The user's projects (`id`, `name`, `board_count`) |
| `list_boards` | `project_id?`, `archived?` | Boards visible to the user (`id`, `name`, `project_id`, `view_mode`, `is_archived`, `updated_at`) |
| `get_board` | `board_id` | The board + its columns (id, name, position) + per-column card counts |
| `list_columns` | `board_id` | The board's columns with card counts |
| `list_cards` | `column_id`, `page?`, `limit?` | Card summaries for the column (`id`, `title`, `position`, `due_date`, `labels`, `checklist_progress`, `has_description`) — the existing `CardSummary` shape |
| `get_card` | `card_id` | The full card: title, **description**, due_date, labels, checklists + items, `checklist_progress` |
| **`get_card_context`** | `card_id` | **The work bundle** — see §3.2 |
| `list_card_notes` | `card_id`, `page?`, `limit?` | The notes attached to the card (the report thread) |
| `list_labels` | — | The user's labels (id, name, color) |

### 3.2 `get_card_context` — the keystone tool

One call gives the agent everything it needs to start work, so it does not
have to make six round-trips to discover the board's shape.

**This payload is the normative definition.** (Amended 2026-11-16, pass 2.)
`60-security.md` §3's fencing rules and `40-workflows.md` §7's `repo` block
are *implemented* by this shape — neither is an alternative contract. Free-text
fields written by users carry fence markers and a `_trust` note in-line; the
`instructions`/`repo` blocks are the only server-authored text in the payload.

**Arguments:** `{ card_id: integer }`

**Returns:**

```jsonc
{
  "card": {
    "id": 42, "title": "…",
    "description": "<<<CARD_DESCRIPTION untrusted_user_content>>>\n…\n<<<END>>>",
    "column_id": 3, "position": 0, "due_date": null,
    "labels": [ { "id": 7, "name": "backend", "color": "blue" } ],
    "checklists": [
      { "id": 11, "title": "Acceptance criteria",
        "items": [ { "id": 21, "text": "…", "is_completed": false, "position": 0 } ] }
    ],
    "checklist_progress": { "completed": 1, "total": 3, "percent": 33 },
    "_trust": "description, notes and checklist items are untrusted_user_content"
  },
  "board":    { "id": 5, "name": "Sprint 3", "view_mode": "board" },
  "project":  { "id": 2, "name": "KanPad" },       // null when the board has no project
  "column":   { "id": 3, "name": "In progress", "position": 1 },
  "siblings": [ { "id": 40, "title": "…", "column_id": 3 } ],  // other cards in the column
  "columns":  [ { "id": 2, "name": "Todo" }, { "id": 3, "name": "In progress" } ],
  "notes":    [ { "id": 88, "title": "…", "content": "…", "created_at": "…" } ],
  "links":    [ { "id": 12, "kind": "pr", "url": "https://…/pull/42", "title": "PR #42" } ],
  "instructions": "…",   // the board's agent instructions, if any (see 40-workflows.md §6)
  "repo":     {         // static text, never user-supplied (40-workflows.md §7)
    "agent_instructions_path": "AGENTS.md",
    "note": "Read the repository's AGENTS.md before editing. KanPad holds the intent; the repository holds the rules."
  },
  "claim":    { "claimed_by_user_id": null, "expires_at": null }   // null when unclaimed
}
```

**Why one composite tool rather than six small ones.** Each MCP call is an
HTTP round-trip into the server, an auth lookup, a rate-limit token, and a
model turn. Six calls to assemble one picture costs six of everything. The
composite tool runs its reads on **one connection** with a bounded number of
queries and returns one payload.

**Cost control:** at most **5 bounded queries on one connection** (no parallel
fan-out — a tool that needs more queries has a different problem); `siblings` is
capped (§4), `notes` is capped, `links` is capped, and the whole payload is
capped by `MCP_MAX_TOOL_RESULT_BYTES`.

### 3.3 Write tools — scope `write`

| Tool | Arguments | Notes |
|---|---|---|
| `create_card` | `board_id`, `column_id`, `title`, `description?`, `due_date?` | `position` is omitted deliberately — the server appends (`MAX+1`), which is the only safe default. |
| `update_card` | `card_id`, `title?`, `description?`, `due_date?` | Scalars only. **Never** moves a card — that is `move_card`. Mirrors the REST split exactly. |
| `move_card` | `card_id`, `column_id`, `position?` | `position` optional → append to the target column. Delegates to the existing move algorithm (clamp, gap maintenance, deadlock retry). |
| `add_card_note` | `card_id`, `title`, `content` | The report-back channel. Creates a `notes` row with `card_id` set. **Not idempotent** (`idempotentHint: false`). |
| `create_checklist` | `card_id`, `title` | |
| `add_checklist_item` | `checklist_id`, `text` | |
| `set_checklist_item` | `item_id`, `is_completed` | The agent's progress signal. |
| `set_card_labels` | `card_id`, `label_ids[]` | **Idempotent** — the label set is replaced wholesale. Covers attach and detach. |
| `link_artifact` | `card_id`, `url`, `kind`, `title?` | Stores the URL **as a string** — never fetched, never resolved (`url` is constrained to `http`/`https`). `kind` ∈ `pr\|branch\|commit\|link`. **Idempotent** on `(card_id, url)`: a duplicate returns the existing link instead of creating a second. The agent's PR/branch/commit pointer; see §3.6. |
| `create_label` | `name`, `color` | `color` ∈ `red\|orange\|yellow\|green\|blue\|purple`. **Documented contract exception** (2026-11-16): returns the existing label on a name conflict rather than erroring (the REST API returns 409; the tool softens it so an agent can recover). The only tool that does not surface the REST status for this case — everything else maps failure → problem+json error result. |

### 3.4 Collaboration tools — scope `write`

| Tool | Arguments | Notes |
|---|---|---|
| `claim_card` | `card_id`, `ttl_minutes?` (default 30), `note?` | See `40-workflows.md` §3. Fails with 409 if another actor holds a live claim. |
| `release_card` | `card_id` | Releases the caller's own claim. Idempotent. |
| `heartbeat_claim` | `card_id`, `progress_note?` | Extends the lease; optionally appends a progress note. The lease keeps the claim alive while the agent works. |

### 3.5 Delete tools — scope `delete`, `destructiveHint: true`

| Tool | Arguments | Notes |
|---|---|---|
| `delete_card` | `card_id` | The only destructive tool in the first wave. `delete_card` is deliberately **not** implemented for boards, projects or columns — those cascade to far more than an agent should be able to destroy in one call. A `read,write` key cannot delete anything; a `delete` key can delete one card at a time. |
| `unlink_artifact` | `link_id` | Removes a link the agent previously added. |

### 3.6 The `card_links` table and the artifact read path

New migration — **`0006_card_links.sql`** (forward-only; `0003` is T010's,
`0004`/`0005` are `api_keys`/`card_claims`, `0001`–`0003` never edited,
`_sqlx_migrations` never hand-edited):

```sql
CREATE TABLE `card_links` (
  `id`         INT           NOT NULL AUTO_INCREMENT,
  `card_id`    INT           NOT NULL,
  `kind`       VARCHAR(32)   NOT NULL DEFAULT 'link',   -- 'pr' | 'branch' | 'commit' | 'link'
  `url`        VARCHAR(2048) NOT NULL,
  `title`      VARCHAR(255)  NULL,
  `created_by` INT           NOT NULL,
  `created_at` DATETIME(6)   NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`id`),
  UNIQUE KEY `UQ_card_links_card_url` (`card_id`, `url`(255)),
  KEY `IDX_card_links_card_created` (`card_id`, `created_at`),
  CONSTRAINT `FK_card_links_card` FOREIGN KEY (`card_id`)
    REFERENCES `cards` (`id`) ON DELETE CASCADE,
  CONSTRAINT `FK_card_links_user` FOREIGN KEY (`created_by`)
    REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

Notes:
- `UNIQUE (card_id, url(255))` is what makes `link_artifact` idempotent: a
  re-link of the same URL on the same card is an `INSERT … ON DUPLICATE KEY
  UPDATE` no-op returning the existing row — the tool maps the duplicate to a
  success result carrying the existing link (HTTP-200-equivalent), exactly like
  `create_label`'s documented exception. Prefix 255 keeps the utf8mb4 index
  well under the 3072-byte limit.
- `url` is stored, never resolved. `link_artifact` constrains the scheme to
  `http`/`https` at the boundary (defense in depth; a stored `javascript:` URL
  would still be rendered as plain text by the client, but we don't store it).
- Deleting the card cascades its links away (like `card_claims`).

**Read path for the chip UI** (amended 2026-11-16, decision log O-14):
- MCP agents read links in the `links` array of `get_card_context` (§3.2).
- The frontend reads them via a new REST route **`GET /api/v1/cards/{id}/links`**
  (200 + `Page<LinkSummary>`-style envelope, newest first; additive — T52 same-PR
  process with `openapi.json` + `schema.d.ts` regenerated). The card detail panel
  and the card list chip fetch it once per card/row-mount; no per-card polling.
- `unlink_artifact` is the only deleter (`delete` scope); there is no
  `update_link` tool.

## 4. Bounds and truncation

Every bound is a hard limit in the tool implementation, not a convention the
agent is trusted to respect.

| Bound | Value | Rationale |
|---|---|---|
| Description in `get_card` / `get_card_context` | 8 000 chars | A card description is a task brief, not a document. Longer → truncated with `…[truncated N chars]`. |
| Note content in `get_card_context` | 4 000 chars each, **max 10 notes** | The report thread, newest first. |
| `links` in `get_card_context` | 20 | The PR hand-back trail, newest first (§3.6). |
| `siblings` in `get_card_context` | 20 | Enough for context, not the whole column. |
| Checklist items per checklist | 100 | |
| `list_cards` page size | default 50, max 100 | Matches the REST contract exactly. |
| `list_card_notes` page size | default 20, max 100 | |
| **Whole tool result** | `MCP_MAX_TOOL_RESULT_BYTES` (default 256 KiB) | A final backstop. On overflow the tool returns an error result naming the limit, **not** a silently truncated payload — a truncated payload that looks complete is worse than a failure. |

**The truncation marker is mandatory.** A truncated string MUST end with
`…[truncated N chars]`. An agent that cannot tell a truncated payload from a
complete one will act on missing information as if it were real.

## 5. The `read_only_hint` / `destructive_hint` mapping to scopes

| Tool | `readOnlyHint` | `destructiveHint` | Scope |
|---|---|---|---|
| `list_projects`, `list_boards`, `get_board`, `list_columns`, `list_cards`, `get_card`, `get_card_context`, `list_card_notes`, `list_labels` | `true` | — | `read` |
| `create_card`, `update_card`, `move_card`, `add_card_note`, `create_checklist`, `add_checklist_item`, `set_checklist_item`, `set_card_labels`, `create_label`, `link_artifact` | `false` | — | `write` |
| `claim_card`, `release_card`, `heartbeat_claim` | `false` | — | `write` |
| `delete_card`, `unlink_artifact` | `false` | `true` | `delete` |

`idempotentHint` is set `true` for `set_card_labels`, `set_checklist_item`,
`release_card`, `create_label`, `link_artifact`; `false` for `add_card_note`,
`add_checklist_item` and `move_card` (a repeated move to the same target is
*positionally* idempotent, but the tool is not advertised as safe to retry
blindly).

## 6. Tools deliberately NOT in the first wave

| Tool | Why not yet |
|---|---|
| `search_cards` | Needs the FULLTEXT index (`specs/tasks/T023-…`). A `LIKE '%term%'` fallback is portable but unindexed and slow. Agents navigate `board → column → card`, which is cheap and deterministic. |
| `delete_board`, `delete_project`, `delete_column` | Cascade too wide for an agent to hold in one scope. Reconsider in a later wave with an explicit "destructive" scope. |
| `move_all_cards` (`POST /columns/{id}/move-cards`) | Moves an entire column. Same reason. |
| `update_board`, `update_project`, `create_board`, `create_project` | The agent's job is cards. Widening the write surface should be a deliberate decision, not a default. |
| Anything that fetches a URL | The server performs no outbound network I/O (see `10-architecture.md` §6). |
| Anything that reads a file on the server | The server has no business reading the agent's workspace. |

## 7. Tool descriptions are trusted text

The tool `description` and the server `instructions` are injected into the
model's system prompt. They are authored by us.

**No user-supplied text may appear in a tool description or in the server
instructions.** Card titles, descriptions and notes are *results*, never
descriptions. See `60-security.md` §3.

Descriptions should be short but must carry the contract: what the tool does,
what it returns, what it does **not** do, and — for mutating tools — whether it
is safe to retry.
