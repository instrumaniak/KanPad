# 40 — Agent workflows

## 1. The work loop

This is the protocol the agent follows. It is published in three places, all
authored by us and all identical in substance:

1. The MCP server `instructions` (injected into the model's system prompt by
   the client — OpenCode does this automatically).
2. `docs/ai-agent-integration.md` (the human-facing runbook).
3. A repository-local agent persona (`.opencode/agent/kanpad-worker.md`) for
   OpenCode specifically.

```
  ┌─ 1. CONTEXT ────────────────────────────────────────────────┐
  │  get_card_context(card_id)                                  │
  │    → card, description, checklists, labels, notes,           │
  │      board, project, column, siblings, board instructions    │
  │  Read the repository's AGENTS.md before touching code.       │
  └──────────────────────────────┬───────────────────────────────┘
                                 ▼
  ┌─ 2. CLAIM ──────────────────────────────────────────────────┐
  │  claim_card(card_id, ttl_minutes=30)                        │
  │    → 409 if another actor holds a live claim.                │
  │      Do not proceed on 409; report and stop.                 │
  └──────────────────────────────┬───────────────────────────────┘
                                 ▼
  ┌─ 3. WORK ────────────────────────────────────────────────────┐
  │  In the repository, not in KanPad.                           │
  │  heartbeat_claim(card_id) every ~5 minutes while working.    │
  │  set_checklist_item(item_id, true) as items are finished.    │
  └──────────────────────────────┬───────────────────────────────┘
                                 ▼
  ┌─ 4. REPORT ─────────────────────────────────────────────────┐
  │  add_card_note(card_id, title, content)                     │
  │    → what changed, what was tested, the PR link,            │
  │      and anything the reviewer must know.                    │
  │  link_artifact(card_id, url, title, kind)                    │
  │    → the pull request / branch / commit.                     │
  └──────────────────────────────┬───────────────────────────────┘
                                 ▼
  ┌─ 5. HAND BACK ───────────────────────────────────────────────┐
  │  move_card(card_id, <review column id>)                     │
  │  release_card(card_id)                                      │
  └──────────────────────────────────────────────────────────────┘
```

**Why the loop matters.** It is the industry-standard protocol (KanbanMCP's
published loop: claim → heartbeat → complete / request_review / block) and it
solves the two problems that appear the moment a second agent joins a board:

- **Two agents, one card.** `claim_card` is exclusive.
- **An agent dies mid-task.** The claim has a TTL; without a heartbeat it
  lapses and the card becomes claimable again.

## 2. Who is "the agent"?

The agent acts **as the key's owner**. There is no separate agent identity,
no `users.role = 'agent'`, and no agent user account.

This is the single most important simplification in the design:

- The key resolves to a `users.id`.
- Every existing ownership predicate applies.
- Every existing permission the human has applies to the agent, minus the
  key's scopes.
- The audit trail says "user 7, via MCP, key #3" — which is exactly the truth.

**The consequence for the collaboration wave.** When `T028`–`T031` let a user
be a member of another user's board, the agent of that user automatically
reaches that board. No per-board key, no sharing dance, no separate
grant table. **A key is a credential for a person, not a robot.**

## 3. Claims and leases

A new table — `0004_card_claims.sql`:

```sql
CREATE TABLE `card_claims` (
  `card_id`       INT         NOT NULL,
  `actor_user_id` INT         NOT NULL,
  `actor_key_id`  INT         NULL,
  `claimed_at`    DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `expires_at`    DATETIME(6) NOT NULL,
  `note`          VARCHAR(255) NULL,
  PRIMARY KEY (`card_id`),
  KEY `IDX_card_claims_actor` (`actor_user_id`),
  KEY `IDX_card_claims_expiry` (`expires_at`),
  CONSTRAINT `FK_card_claims_card` FOREIGN KEY (`card_id`)
    REFERENCES `cards` (`id`) ON DELETE CASCADE,
  CONSTRAINT `FK_card_claims_user` FOREIGN KEY (`actor_user_id`)
    REFERENCES `users` (`id`) ON DELETE CASCADE,
  CONSTRAINT `FK_card_claims_key` FOREIGN KEY (`actor_key_id`)
    REFERENCES `api_keys` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

**One claim per card** (`PRIMARY KEY (card_id)`) — a card has at most one
holder. This is the exclusivity guarantee.

### Claim semantics

| Operation | Behaviour |
|---|---|
| `claim_card` on an unclaimed card | Insert the claim. `expires_at = NOW() + ttl`. |
| `claim_card` on a card claimed by **the same actor** | **Re-claim**: extend `expires_at`. Not an error. |
| `claim_card` on a card claimed by **another actor**, claim not expired | **409 conflict.** The error body names the holder and the expiry — *so the holder is never stolen from and the contender is never left guessing.* |
| `claim_card` on a card whose claim has **expired** | Take over. The expired row is replaced. |
| `release_card` | Delete the caller's own claim. A release of a card claimed by someone else is a **no-op**, not an error. |
| `heartbeat_claim` | Extend `expires_at` by the TTL, optionally appending a progress note. A heartbeat on a card not held by the caller is a **409**. |

### Atomicity

The claim MUST be race-free: two agents claiming the same card concurrently
must not both succeed.

**Approach:** a single `INSERT … SELECT` guarded by the primary key, inside a
transaction, with the "is it already claimed and live?" check performed in the
same statement rather than as a separate read:

```sql
INSERT INTO `card_claims` (card_id, actor_user_id, actor_key_id, expires_at)
SELECT ?, ?, ?, NOW(6) + INTERVAL ? MINUTE
 WHERE NOT EXISTS (
       SELECT 1 FROM `card_claims`
        WHERE card_id = ? AND expires_at > NOW(6) AND actor_user_id <> ? )
ON DUPLICATE KEY UPDATE
   actor_user_id = VALUES(actor_user_id),
   actor_key_id  = VALUES(actor_key_id),
   claimed_at    = NOW(6),
   expires_at    = VALUES(expires_at),
   note          = VALUES(note);
```

The `ON DUPLICATE KEY UPDATE` makes re-claim by the same actor an update, and
the `NOT EXISTS` guard means a live foreign claim blocks the insert. Rows
affected = 0 → the card is claimed by someone else → 409.

**This is a candidate, not gospel.** The implementing task MUST verify the
behaviour under real concurrency on **both** MySQL 8.4 and MariaDB 10.6 — the
`ON DUPLICATE KEY UPDATE` + `NOT EXISTS` combination is exactly the kind of
statement where the two engines differ (MariaDB's `VALUES()` deprecation, gap
and next-key locks under `REPEATABLE READ`). If it proves fragile, the
fallback is `SELECT … FOR UPDATE` on the card row followed by an upsert, which
is slower but obviously correct.

### Expiry and the sweeper

Expired claims are treated as absent at read time (`expires_at > NOW()`), so
**no sweeper is required for correctness**. A sweeper may delete stale rows
later; it is a housekeeping task, not a correctness one. The `T011` backup
runbook and the `IDX_card_claims_expiry` index make a periodic cleanup cheap.

## 4. Multi-agent coordination on a shared board

Two users' agents on the same board, with the claims table in place:

| Situation | Behaviour |
|---|---|
| Agent A claims card 42 | Agent B's `claim_card(42)` → **409**, body says "claimed by user 5 until 14:32". B moves on. |
| Agent A's process dies | The claim expires. B can claim it after `expires_at`. |
| Agent A's heartbeat continues | The lease slides forward. B keeps getting 409. |
| A human edits card 42 while A works | T033's optimistic concurrency gives A a **409** on write. A re-reads and re-applies. The agent MUST treat a 409 on `update_card` as "re-read, then retry", never as a fatal error. |
| Agent A writes a note while a human is typing | Notes are append-only; no conflict. |

**The 409-on-write contract is important for agents.** An agent that treats a
409 as fatal will abandon work it could have completed. The server's
problem+json body for a conflict carries the current resource state (per the
Wave 2 decision), so the agent can re-read and re-apply in one round-trip.

## 5. Reporting back

The report lands as a **note attached to the card** (`notes.card_id`).

**Why notes and not a new comments table** — this is open call **O-1** in
`90-open-calls.md`. The recommendation is to reuse `notes` because:

- the table, the FK and the markdown rendering already exist;
- the frontend already has a full note editor, a link selector that can target
  a card, and a markdown + mermaid renderer;
- an agent report is **long-form** (what changed, how it was tested, the PR, the
  risks) — a tweet-sized comment would be a worse fit.

The cost is that a card-linked note is currently **invisible on the card**
itself: `card-detail-panel.tsx` contains no note references, and
`GET /notes?board_id=N` does not return card-linked notes (their `board_id` is
NULL). Two small additions close this:

1. `GET /api/v1/notes?card_id=N` — a new filter on the existing list route.
2. A **Linked notes** section in the card detail panel.

Both are in scope for `50-frontend.md` and are worth doing regardless of MCP —
they fix a pre-existing gap in which a user can attach a note to a card and
then never see it there.

## 6. Board-level instructions

An agent needs to know the conventions of a specific board ("always run the
integration suite before moving to Review", "never edit the migration folder").
Today there is no place to put that.

**Reuse `notes` again**: a note with `board_id` set and a reserved title acts
as the board's agent instructions. `get_card_context` returns it in the
`instructions` field.

**The reserved-title convention** (open call **O-6**):
the note whose title is exactly `AGENT.md` is the instruction note. It is
distinguished by title, not by a new column, so no schema change is needed.

**Alternative** — a dedicated `board_instructions` column on `boards`. Cleaner
semantically, one more migration, one more DTO field, and it duplicates what
`notes` already does. The recommendation is the reserved title.

## 7. The agent's own repository context

The MCP server knows nothing about the repository. The **repository** holds
that context, and the agent reads it from disk:

- `AGENTS.md` at the repository root — the house rules, the conventions, the
  test commands. The agent MUST read it before editing. This file already
  exists in this project.
- `specs/tasks/T0NN-*.md` — the task specification, when the card refers to
  one.
- The project's `CONVENTIONS.md` / `README.md`.

**The card and the repository are two different systems and must not be
conflated.** KanPad holds the *intent*; the repository holds the *rules*. The
server `instructions` say so explicitly, and `get_card_context` returns a
`repo` hint block:

```jsonc
"repo": {
  "agent_instructions_path": "AGENTS.md",
  "note": "Read the repository's AGENTS.md before editing. KanPad holds the intent; the repository holds the rules."
}
```

This block is **static text**, never user-supplied.

## 8. The full flow, end to end

```
Human                KanPad                       Agent (in its own repo)
  │                    │                                │
  │ "work on card 42"  │                                │
  │────────────────────┼───────────────────────────────▶│
  │                    │◀── get_card_context(42) ───────│
  │                    │    card + description +        │
  │                    │    checklists + board + notes   │
  │                    │──▶│                            │
  │                    │◀── claim_card(42, 30) ────────│
  │                    │    201 (claim held)            │
  │                    │                                │
  │                    │◀── heartbeat_claim(42) ───────│  every ~5 min
  │                    │                                │
  │                    │    ┌─ agent edits code, runs tests, opens a PR ─┐
  │                    │                                │
  │                    │◀── add_card_note(42, …) ───────│
  │                    │◀── link_artifact(42, pr_url) ─│
  │                    │◀── set_checklist_item(21,true)│
  │                    │◀── move_card(42, review_col) ─│
  │                    │◀── release_card(42) ──────────│
  │◀── the card moved, the note is on it, the PR is linked
```

Every arrow is one MCP tool call, one authorization check, one activity-log
row (for the writes).
