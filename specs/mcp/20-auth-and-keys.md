# 20 — Authentication, API keys, and scopes

## 1. The credential

```
kp_live_<43-char base64url(32 random bytes)>
```

- `kp_live_` — a fixed, non-secret prefix. It makes the credential
  self-identifying in logs and secret scanners, and it leaves room for
  `kp_test_` in a future sandbox environment.
- 43 base64url characters = 256 bits of entropy from a CSPRNG.

**Generation:** 32 bytes from `rand::rngs::OsRng` (the `rand` crate arrives
transitively via rmcp; if it is not a direct dependency, `getrandom` is
already reachable through `uuid`). Base64url-encoded **without padding**.

**The plaintext is shown exactly once**, at creation, and is never stored.

## 2. Schema

New migration — `0003_api_keys.sql` (forward-only; `0001`/`0002` are never
edited; `_sqlx_migrations` is never hand-edited).

```sql
CREATE TABLE `api_keys` (
  `id`           INT          NOT NULL AUTO_INCREMENT,
  `user_id`      INT          NOT NULL,
  `name`         VARCHAR(100) NOT NULL,
  `token_prefix` VARCHAR(24)  NOT NULL,
  `token_hash`   CHAR(64)     NOT NULL,
  `scopes`       VARCHAR(255) NOT NULL DEFAULT 'read',
  `last_used_at` DATETIME(6)  NULL,
  `expires_at`   DATETIME(6)  NULL,
  `revoked_at`   DATETIME(6)  NULL,
  `created_at`   DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at`   DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
                               ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`id`),
  UNIQUE KEY `UQ_api_keys_token_hash` (`token_hash`),
  KEY `IDX_api_keys_user_id` (`user_id`),
  CONSTRAINT `FK_api_keys_user` FOREIGN KEY (`user_id`)
    REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

Column notes:

- `token_prefix` — the **non-secret** display fragment, e.g. `kp_live_a1b2c3`.
  Lets the UI show `kp_live_a1b2c3…` without revealing the secret. 6 secret
  characters ≈ 36 bits of a 256-bit value; non-recoverable.
- `token_hash` — `CHAR(64)` hex = HMAC-SHA-256 output (§4). **Uniquely
  indexed** → the lookup is an index seek.
- `scopes` — a comma-separated list. Chosen over a join table because the set
  is tiny, fixed and never queried *across* users; a join table would add a
  table for no read pattern that exists.
- `revoked_at` — soft revoke. The row survives so the audit trail can show
  "key #3 was created, used until 12:04, revoked by the owner".
- `expires_at` — optional. `NULL` = never expires.

**MySQL + MariaDB.** `ENGINE=InnoDB`, `utf8mb4` + `utf8mb4_unicode_ci`,
`DATETIME(6)`, `INT SIGNED` — identical to the conventions of
`0001_baseline.sql`. No `IF NOT EXISTS` on app tables (fail fast), no
`AUTO_INCREMENT=N` (deterministic checksums).

## 3. Actor model

A key resolves to an **actor**:

```rust
pub struct Actor {
    pub user_id:   UserId,
    pub email:     String,
    pub role:      Role,        // 'user' | 'admin'
    pub scopes:    ScopeSet,    // what the KEY allows
    pub api_key_id: Option<ApiKeyId>,
    pub via:       Channel,     // Browser | Mcp
}
```

**The invariant that makes this safe:**

> effective permission = **the user's own rights** ∩ **the key's scopes**

The scope check can only ever **remove** access. It never grants anything the
user could not already do.

**Consequence for the collaboration wave.** When `T028`–`T031` give users
workspace memberships, the **same key automatically inherits them** — the
middleware resolves `key → user_id` and the Wave 2 kernel resolves
`user → memberships`. No re-issuance, no migration, no change to the
middleware. The scope set only ever narrows.

**`via: Channel`** is recorded on every activity-log row written by an MCP
tool, so the UI and the audit trail can distinguish "the human did this in the
browser" from "the agent did this over MCP, using key #3".

## 4. Hashing

```
token_hash = HMAC-SHA-256(key = API_KEY_SECRET, msg = "<full token string>")
             rendered as lowercase hex (64 chars)
```

**Why HMAC and not a bare SHA-256.** A bare SHA-256 of a 256-bit random secret
is not brute-forceable, so the *cryptographic* argument is weak — but an HMAC
adds a second factor the attacker does not have: the server-side secret. A
database dump alone then yields **no usable key**, even if a future
implementation weakens the token's entropy.

**`API_KEY_SECRET`** — a new setting:

- **required in production** (boot error if missing or shorter than 32 bytes),
  validated exactly like `SESSION_SECRET`;
- in development it MAY fall back to `SESSION_SECRET` with a `warn` log.

**Why not reuse `SESSION_SECRET` outright.** Rotating `SESSION_SECRET` logs
every browser out. If the key hash also depended on it, the same rotation
would silently invalidate every agent key — a surprising coupling. A separate
secret keeps the two rotations independent. (Both are still documented
together in the key-rotation runbook.)

**Why an indexed equality lookup is not a timing oracle.** The presented token
is hashed and looked up with `WHERE token_hash = ?` on a unique index. The
comparison happens inside the database's index seek, and the attacker cannot
observe index-lookup timing from outside to recover the hash. A constant-time
comparison (`subtle`) would only be needed if the secret were compared
byte-by-byte against a stored value — which it never is. `subtle` is therefore
**not** a dependency.

## 5. Scopes

| Scope | Grants | Rationale |
|---|---|---|
| `read` | `GET` / `HEAD` on the key owner's accessible resources | Default for every new key |
| `write` | `POST` / `PATCH` / `PUT` — create/update/move cards, add notes, tick checklist items, attach labels, claim/release | The agent's working scope |
| `delete` | `DELETE` — cards, boards, columns, notes, labels, tags, projects | **Separate on purpose.** `DELETE /projects/{id}` cascades to boards → columns → cards. An agent that can update cards must not be able to wipe a project. |
| `admin` | Platform-admin routes (Wave 2, `T035`) | Nothing to gate today; reserved so the vocabulary does not have to change later |

**Scope → operation mapping (REST).** The mapping is by HTTP method, with
explicit exceptions:

| Request | Required scope |
|---|---|
| `GET`, `HEAD` | `read` |
| `POST`, `PATCH`, `PUT` | `write` |
| `DELETE` | `delete` |
| `/admin/*` (future) | `admin` |
| `/auth/me` | `read` |
| `/auth/login`, `/auth/register`, `/auth/logout` | **bearer forbidden → 401** |

**The `/auth/*` exclusion is load-bearing.** `POST /auth/logout` calls
`session.flush()`, which **destroys the session row**. A bearer-authenticated
request reaching it would flush a *browser* session. A key MUST NOT be able to
mint a session, create an account, or destroy one. The middleware rejects
bearer tokens on `/auth/login`, `/auth/register` and `/auth/logout` before the
handler runs.

**Scope → operation mapping (MCP).** The MCP surface derives the requirement
from the **tool's own MCP annotations**, not from the HTTP method:

| Tool annotation | Required scope |
|---|---|
| `readOnlyHint: true` | `read` |
| `destructiveHint: true` | `delete` |
| otherwise | `write` |

A tool therefore declares its own privilege level once, and both the MCP
dispatcher and the UI agree about it.

## 6. Issuance

### 6.1 REST (used by the frontend)

| Route | Method | Notes |
|---|---|---|
| `/api/v1/api-keys` | `GET` | The caller's own keys. **Never** another user's. Never returns a secret. |
| `/api/v1/api-keys` | `POST` | Body: `{ name, scopes[], expires_in_days? }`. Returns `201` with the **plaintext token once** + `Location`. |
| `/api/v1/api-keys/{id}` | `DELETE` | Soft revoke (`revoked_at = NOW()`). `204`. |
| `/api/v1/api-keys/{id}` | `GET` | Metadata only (prefix, scopes, timestamps). Optional — the list may be enough. |

The `POST` response is the **only** time the secret exists outside the user's
browser. It MUST NOT be logged, MUST NOT be cached, and MUST NOT appear in the
activity log.

### 6.2 CLI (mirrors the existing `create-admin`, T12)

```sh
backend-rs create-api-key --email you@example.com \
                          --name "opencode-agent" \
                          --scopes read,write [--expires-in 30d]

backend-rs list-api-keys [--email you@example.com]
backend-rs revoke-api-key --id 3
```

Why a CLI as well as the UI: the `create-admin` precedent exists precisely
because there is no admin panel yet. The CLI needs no frontend work, and the
operator already has database access in order to run migrations. The CLI
implementation follows `backend-rs/src/admin.rs` exactly — a core function
that returns an outcome enum with spec-pinned exit codes (`0` ok, `1`
validation, `2` not-found/already-revoked, `3` DB/config), and `main.rs` owning
the prompting and the exit.

```
API key created — shown once, it cannot be retrieved again:
  kp_live_xK3mQ7pR9vT2wY5zA8bC1dE4fG6hJ0kL3nO7qS9tU
  id=3  user=you@example.com  scopes=read,write  expires=never
```

### 6.3 Defaults for a newly minted key

- `scopes` = `read` — **a new key is read-only until the user opts in.**
- `expires_at` = `NULL` (never expires) — but the UI *suggests* a 90-day expiry
  and the runbook documents rotation.
- `name` is required and is shown in the UI so the user can tell "opencode
  (laptop)" from "cursor (work)".

## 7. Authentication pipeline

Per MCP request:

```
1. Authorization header absent                → 401 (MCP has no cookie path)
2. Not "Bearer <token>"                       → 401
3. Token does not start with "kp_live_"        → 401   (fail fast, no DB hit)
4. HMAC-SHA-256(token) → hex
5. SELECT id, user_id, scopes, expires_at, revoked_at
     FROM api_keys WHERE token_hash = ?       -- unique index seek
6. no row / revoked_at IS NOT NULL / expired  → 401 (ONE uniform message)
7. required scope ∉ key.scopes                → 403 problem+json
8. INSERT actor into request extensions        → next.run(request)
```

**Step 6 uses one message for all three failure modes** — "no such key",
"revoked", "expired" — so the response is not an oracle for "this token once
existed". The *specific* reason is available to the **owner** through
`GET /api/v1/api-keys` (which shows `revoked_at` / `expires_at`), which is the
only party entitled to know it.

**Step 7 returns 403, not 401** — the key is valid; it simply lacks the
scope. The problem+json body names the missing scope, which is actionable
information for a legitimate key holder and meaningless to an attacker who
already holds a valid key.

### `last_used_at`

Written **at most once per 5 minutes per key**, with a conditional update:

```sql
UPDATE api_keys SET last_used_at = NOW(6)
 WHERE id = ? AND (last_used_at IS NULL OR last_used_at < NOW(6) - INTERVAL 5 MINUTE)
```

A per-request write would turn every read into a write and pollute the
binlog; the amortized write gives the UI a meaningful "last used" without the
cost. The key lookup itself is **never** cached in memory — revocation must
take effect on the very next request.

## 8. Rate limiting

The REST governor keys on client IP. Every agent behind one NAT egress shares
that bucket, so an agent burst would starve the human's own browser.

The MCP limiter keys on the **API key**:

```
bucket key = "key:" + HMAC-SHA-256(API_KEY_SECRET, token)[..16]
```

Deriving the bucket from a keyed hash of the *presented* token means:

- no database lookup inside the limiter (it runs before the handler);
- two keys never share a bucket, and an agent never touches the browser's
  bucket;
- the bucket is stable for the life of the key and meaningless to anyone who
  sees it.

New setting: `THROTTLE_AGENT_PER_MIN` (default `300`).

**Why not also raise the REST limit.** The REST limit is 120/min/IP, which is
correct for a browser. The agent has its own bucket and its own limit; the
browser's limit stays as it is.

## 9. Revocation

| Action | Effect |
|---|---|
| `DELETE /api/v1/api-keys/{id}` | Sets `revoked_at = NOW()`. The next request with that key → 401. **Effective immediately** — no cache, no grace period. |
| `POST /api/v1/auth/logout` | **Does not** revoke keys. A user logging out of the browser must not silently kill their agent. |
| Rotating `API_KEY_SECRET` | Invalidates every key at once (their hashes no longer match). The emergency "invalidate everything" lever. |

**What revocation does NOT do:** it does not undo changes the agent already
made. It stops the next call. This is stated in the UI next to the revoke
button.

## 10. What a key can reach

A key reaches exactly what its owner reaches, less its scopes. Concretely,
**before** the collaboration wave:

- `projects`, `boards`, `columns`, `cards`, `labels`, `tags`, `notes`,
  `checklists` rows where `user_id` matches (cards via
  `cards → columns → boards.user_id`).
- Foreign ids → **404**, never 403 — the existing store predicates handle it
  and MCP inherits that for free.

**After** the collaboration wave, the reachable set is whatever the
`authz::require` kernel grants the owning user across their workspaces.

## 11. Rotation & lifecycle runbook

| Event | Action |
|---|---|
| Routine (90 days) | Create a new key, deploy it to the agent, revoke the old one. |
| Suspected leak | `revoke-api-key --id N` → effective on the next request. |
| Total compromise | Rotate `API_KEY_SECRET` → every key dies at once; re-mint. |
| User leaves (workspace membership removed) | Their key loses access automatically — the key resolves to a user who is no longer a member. |
| Key unused for 60 days | The UI greys it out and suggests revoking. |
