# T009: BE-RS: MariaDB CI leg — replace prepare --check with online describe gate

> **STATUS (2026-11-11): implementation COMPLETE, reviewed, landed on `dev`, and CI-verified green**
> (run `43010525748`; merged from `dev-local` @ `ec83f489`). This file doubles as the **review
> handoff**: give a reviewer agent this file plus `git diff` and it needs nothing else. See
> §Review checklist.

## Goal

The `backend-mariadb` CI job (T006's portability leg) is **red** and has never been green since it
was added (`d8b0a033`, 2026-11-03). Red run: `instrumaniak/KanPad` run
`42864757750`, job `130000003381`, failing step `sqlx prepare --check`, commit `ec83f489` on `dev`.
Symptom: 10 compile errors in the `lib` target (10 more in `lib test`) — `E0308`/`E0277` at exactly 6
source sites — then `cargo check` exit 101.

**Root cause (mechanism, not guesswork):**

1. `cargo sqlx prepare --check` (sqlx-cli) **forces `SQLX_OFFLINE=false`** on its inner build
   (`sqlx-cli/src/prepare.rs`, `run_prepare_step` → `.env("SQLX_OFFLINE", "false")` — verified in the
   local 0.8.6 source; empirically confirmed on the CI-pinned 0.9.0, because the step's
   `SQLX_OFFLINE: "true"` demonstrably did not stop the macros from going online). So the macros
   re-describe **every** query against the job's live MariaDB 10.6.
2. MariaDB 10.6 reports **different result-set metadata** than the MySQL 8.4 that generated the
   committed `.sqlx/` cache, for 4 queries:
   - `CASE ... END AS "has_description: i64"` → MySQL `nullable=false`, MariaDB nullable → macro
     yields `Option<i64>` where the struct field is `i64`;
   - `SELECT EXISTS(...)` → MySQL `LongLong` (→ `i64`), MariaDB 32-bit `Long` (→ `i32`);
   - `COALESCE(MAX(...), -1) AS "max_pos: i64"` ×4 → nullable on MariaDB → `Option<i64>` where the
     code does `i32::try_from(row.max_pos)`.
3. A `: i64` type override **does not pin nullability** — sqlx still wraps in `Option` whenever the
   *server* says nullable (`sqlx-macros-core-0.9.0/src/query/output.rs:107-117`).
4. **Even a perfect compile fix cannot make `--check` pass here**: `prepare_check` byte-compares the
   regenerated JSON against `.sqlx/` (`if prepare_json != cache_json → bail`), and that JSON stores
   the **raw server describe** (`QueryData::from_describe`, `sqlx-macros-core-0.9.0/src/query/data.rs`
   — no override adjustment). The two servers legitimately differ, so the comparison is
   environment-bound, not repo-bound. **The step can never be green on MariaDB.**

**End state:** the MariaDB leg keeps a *real, passable* portability gate — every `query!` prepares on
MariaDB 10.6 and the code type-checks against MariaDB-derived row types — while the canonical
`.sqlx` cache gate stays where the cache is generated (the MySQL `backend` job). Row types become
server-independent, which is what "the backend must run on MySQL **and** MariaDB (prod = MariaDB)"
actually requires.

## Depends On

- **T006** (hard): this fixes the CI job T006 created; its AC list is amended here.
- T66 (sqlx 0.9 driver + pinned CLI) — the override syntax and CLI behaviour above are 0.9's.

## Phase

0

## Critical

No — production is unaffected (runtime SQL is unchanged; only column *aliases* carry the type pins,
and no query text outside the aliases moved). It blocks CI signal on `dev`, so it should land soon.

## Spec References

- `specs/tasks/T006-b-e-r-s-my-s-q-l-maria-d-b-s-q-l-portability.md` — the job's origin, its AC
  list, and the **false assumption this task corrects** ("set `SQLX_OFFLINE=true` on the
  `prepare --check` step so the cache is validated against the repo, not the job's DB name" — sqlx-cli
  overrides that env, so the step did the exact opposite of its stated intent).
- `.github/workflows/backend-rs.yml:381-390` — the MySQL job's canonical `prepare --check`
  (**untouched** by this change); `:550-573` — the replaced MariaDB step.
- `specs/api-rewrite-rs-specs/30-db-schema.md` — supported servers (MySQL 8.0.19+ / MariaDB 10.6+)
  and the portable-SQL rules.
- `docs/lessons/ci-quality.md` — the lesson appended by this task.
- sqlx internals (verified in `~/.cargo/registry/src/*/sqlx-{cli,macros-core,mysql}-*/src/`):
  `output.rs` (override parsing + `column_to_rust`), `data.rs` (raw-describe JSON),
  `prepare.rs` (`SQLX_OFFLINE=false`, `setup_minimal_project_recompile`, JSON byte-compare),
  `types/int.rs` (`int_compatible` / `int_decode`).

## Files to Create/Modify

**Source — alias pins + comments only (5 files, 6 sites → 5 unique queries):**

| File | Change |
|---|---|
| `backend-rs/src/common/card_summary.rs:139` | `AS "has_description: i64"` → `AS "has_description!: i64"` |
| `backend-rs/src/common/session_store.rs:94` | `SELECT EXISTS(...)` → `... AS "exists!: i64"` (alias required: the raw expression contains `?`, which defeats `ColumnDecl::parse`) |
| `backend-rs/src/features/cards/store.rs:131` | `max_pos: i64` → `max_pos!: i64` |
| `backend-rs/src/features/checklists/store.rs:309` | same |
| `backend-rs/src/features/columns/store.rs:120` and `:316` | same (2×) |

**Workflow (1 file):** `.github/workflows/backend-rs.yml` — the `backend-mariadb` job's
`sqlx prepare --check` step (old `:550-554`) is replaced by `online check against MariaDB (every
query prepares)` (`:550-573`): `SQLX_OFFLINE=false` + `cargo clean -p backend-rs` +
`cargo check --locked --workspace --all-targets`, with a WHY-NOT comment. **The MySQL job's step is
not in the diff.**

**Cache (regenerated, not hand-edited):** `backend-rs/.sqlx/` — 5 tracked files deleted, 5 new files
untracked (new SQL text → new hash → new filename). 78 other query cache files unchanged (plus
`.gitkeep`).

**Docs:** `docs/lessons/ci-quality.md` (one appended lesson, dated 2026-11-10);
`specs/tasks/T006-...md` (Step-4 bullet struck through as a false assumption, files-to-change bullet
and the AC line at ~346 corrected to per-leg wording).

**Must NOT appear in the diff:** `.env`, `DATABASE_URL` anywhere, `_sqlx_migrations`, the Dockerfile,
`frontend/`, `openapi.json`, or any change to the MySQL job.

## Implementation Steps

1. **Step 0 — Red proof (TDD, and it doubles as the gate's own sanity check).** Started
   `kanpad-rs-mariadb` (`:3338`, db `kanpad`, migrations applied), then
   `cargo clean -p backend-rs && SQLX_OFFLINE=false cargo check --locked --workspace --all-targets`
   → **failed with the 6 diagnosed sites**. This also proves the forced rebuild works: without
   `cargo clean`, `SQLX_OFFLINE` is not part of cargo's fingerprint, so a prior offline build would
   make the check pass *vacuously* (sqlx-cli solves the same problem via
   `setup_minimal_project_recompile`, which touches sources).
2. **Step 1 — Pin the 6 sites** with `!` nullability (and the `EXISTS` alias+pin), plus a comment at
   each site explaining *why* the pin exists. No logic changed anywhere.
3. **Step 2 — Regenerate `.sqlx/` on MySQL 8.4 with a database named `kanpad`** (the cache's
   `origin.table` is DB-qualified — `"kanpad.cards"` — so any other DB name fails CI's `--check`).
   Verified the diff is only `query`/`name`/`hash` per file: **zero origin/collation/describe drift**.
   Re-ran the exact CI command → green.
4. **Step 3 — Swap the workflow step** (see Files table) keeping its position: after `migrate run`,
   before the three test suites.
5. **Step 4 — Verify both legs locally** (evidence in §Notes 3).
6. **Step 5 — Docs**: lesson + T006 spec corrections (same change set).

## Constraints

- **No commit/push without explicit user approval** (AGENTS.md). When committing, the 5 new `.sqlx`
  files are untracked — `git add backend-rs/.sqlx` explicitly; **never `git commit -a`** (it would
  delete the old cache files without staging the new ones).
- The `.sqlx/` cache must be regenerated **on MySQL only, never MariaDB** — regenerating on MariaDB
  just moves the red to the MySQL job.
- The regeneration DB must be named `kanpad` (DB-qualified cache names).
- Keep the MySQL job's `prepare --check` (`:386-390`) byte-identical — it is the canonical gate.
- Any new query with a MySQL/MariaDB describe divergence must get the same `!`/`: type` pin; a bare
  `: type` override is not enough.
- Test conventions: `SQLX_OFFLINE=true` for the test suites (mirrors CI); `DATABASE_URL` exported per
  leg; never set `SQLX_OFFLINE` in a committed `.env`.

## Acceptance Criteria

- [x] Red proof reproduced locally at exactly the 6 sites (gate is not vacuous).
- [x] MariaDB online gate green after the pins (`SQLX_OFFLINE=false cargo check --locked
      --workspace --all-targets`, forced `cargo clean -p backend-rs`).
- [x] `cargo fmt --check` 0 · `clippy --locked --workspace --all-targets -- -D warnings -A
      clippy::pedantic` 0.
- [x] Full suite on MySQL `--workspace --all-targets`: **242 passed / 0 failed**.
- [x] The three never-in-CI MariaDB suites green: `common::session_store` 10, `api sql_portability`
      6, `api auth_session` 4 → **20 passed / 0 failed**.
- [x] `SQLX_OFFLINE=true cargo sqlx prepare --check --workspace --all -- --locked --all-targets`
      green on MySQL (re-run after the 2026-11-11 host restart).
- [x] `export-openapi` output byte-identical to the committed `openapi.json`.
- [x] `.sqlx/` diff limited to the 5 affected queries; no unrelated drift.
- [x] Lesson appended (`docs/lessons/ci-quality.md`, dated) + T006 spec's false assumption corrected.
- [x] **CI green on `dev`** — run `43010525748` (2026-11-11): `backend`, `backend-mariadb`, `web`, and
      the docker/vitest job all green.

## Notes

### 1. Why each fix shape was chosen (and the two rejected ones)

- **Rejected: delete the step from the MariaDB job.** True and minimal, but it throws away a real
  signal the user explicitly wants ("my backend must run properly on MySQL **and** MariaDB, prod is
  MariaDB"): *does every query in the codebase prepare on MariaDB 10.6?* The runtime suites only
  exercise a subset of statements.
- **Rejected: SQL-level `CAST`/`IFNULL` gymnastics** to force both servers to report identical
  metadata. Fragile (depends on server internals) and still loses: `type_info`-level differences
  (flags/max_size/origin) do not change the Rust type, so they would not surface as compile errors
  yet would still break the JSON byte-compare.
- **Chosen: pin the row types + an online describe gate.** The pins make the Rust types a property of
  the *code*, not of whichever server prepared the query; the gate then compiles the whole workspace
  (all targets, all queries) against MariaDB's live describe. Byte-equality of two servers' describe
  output was never a meaningful invariant.

### 2. Safety of the pins (why nothing can break at runtime)

- `has_description`: `CASE` branches are the constants `1`/`0` → never NULL.
- `max_pos`: `COALESCE(MAX(...), -1)` → never NULL (`-1` fallback).
- `exists`: `EXISTS` is never NULL, and forcing `i64` when MariaDB sends a 32-bit `Long` is safe —
  `int_compatible` (`sqlx-mysql-0.9.0/src/types/int.rs:10-19`) accepts non-unsigned `Long` for `i64`,
  and `int_decode` (`:93-115`) widens up to 8 bytes. Confirmed empirically: the session-store suite
  (which is the only runtime consumer of `id_exists`) passes on MariaDB.
- `src/features/cards/store.rs:716` (`CardPageRow.has_description`) is a runtime
  `QueryBuilder`/`FromRow` decode, **not** a macro — deliberately left alone (its value can never be
  NULL, and nullability flags do not affect runtime decoding).

### 3. Verification evidence (all run locally, 2026-11-10/11)

| Leg | Command | Result |
|---|---|---|
| MariaDB `:3338` | online `cargo check` (pre-fix, forced clean) | **RED** — 10 errors (lib) + 10 (lib test) at the 6 sites |
| MariaDB `:3338` | online `cargo check` (post-fix, forced clean) | **GREEN** (exit 0) |
| MariaDB `:3338` | `--lib common::session_store` / `--test api sql_portability` / `--test api auth_session` | **10 + 6 + 4 = 20 passed / 0 failed** |
| MySQL `:3337` | `cargo fmt --check` | 0 diffs |
| MySQL `:3337` | `clippy --locked --all-targets -D warnings -A pedantic` | 0 warnings |
| MySQL `:3337` | `cargo test --locked --workspace --all-targets` | **242 passed / 0 failed** |
| MySQL `:3337` | `cargo sqlx prepare --check …` (exact CI form) | exit 0 (run twice, incl. post-restart) |
| MySQL `:3337` | `export-openapi` + `diff openapi.json` | byte-identical |

No additional divergent sites were found: the post-fix `--all-targets` check compiles clean, so CI's
"compile died at the `lib` target" had hidden no test-target sites, and the three never-run suites
needed no portability fixes.

**Full-suite MariaDB run (out of CI scope, 2026-11-11):** `cargo test --test api` on MariaDB 10.6
gives 126 passed / 5 failed — none related to the pins. The `migrate_cli`/`serve_boot` password-leak
assertions fail because the local MariaDB root password is empty (`err.contains("")` is always
true), and the two `cards_move` fault-injection tests get 500 because MariaDB's
`SIGNAL ... SET MYSQL_ERRNO` error-number semantics differ from MySQL's, so the 1213/1205 retry
matcher misses. These are the unmet T006 "full suite green on `mariadb:10.6`" AC, not T009
regressions (the pins do not change runtime SQL). The MariaDB CI job intentionally runs only the
three suites above.

Local DBs: `docker compose -f backend-rs/docker-compose.test.yml` — `test_db` (`mysql:8.4`,
`:3337`, root pw `dev_root_pw`) and `mariadb` (`mariadb:10.6`, `:3338`, empty root pw). Both use
`tmpfs` for `/var/lib/mysql`, so **a container/host restart wipes them** (a restart on 2026-11-11 made
all three MariaDB suites fail with connection panics in `sqlx-core/src/testing/mod.rs:227` — that was
dead containers, not a code regression; restart + `cargo sqlx database create && cargo sqlx migrate run`
made them green again). Note the pre-existing container `kanpad-rs-testdb` name-conflicts with the
compose service — `docker start kanpad-rs-mariadb kanpad-rs-testdb` is the workaround.

### 4. Review checklist (for the reviewer agent)

Run from the repo root; the whole change must be explainable by this file.

```bash
git status --short                     # 9 modified + 5 D + 6 ?? (5 .sqlx + this spec)
git diff --stat                        # workflow + 5 src + 1 lesson + 2 specs + status.yaml
git diff -- .github/workflows/backend-rs.yml   # expect ONE hunk: @@ -547,11 +547,30 @@
git diff -- backend-rs/src             # alias text + comments ONLY — no logic, no reformatting
git status --short -- backend-rs/.sqlx # exactly 5 D + 5 ??, zero M
```

Invariants to hold the diff to:

1. **Workflow** — the hunk touches only the `backend-mariadb` job's step; the MySQL job's
   `sqlx prepare --check --workspace --all -- --locked --all-targets` (lines 386-390) is unchanged;
   the new step keeps `cargo clean -p backend-rs` (without it the gate can pass vacuously) and sits
   after `migrate run`, before the three test suites.
2. **Source** — only the SQL string literals changed (plus explanatory comments). Every pin is `!`
   (never `?` — these expressions are never NULL). The `EXISTS` query must keep its alias (an
   unaliased expression containing `?` cannot carry an override).
3. **`.sqlx`** — the 5 new files must each show `db_name: "MySQL"`, a `!` pin in `query`, and
   `origin.table` values under `kanpad.` where the query touches a table:
   ```bash
   python3 -c "import json,glob;[print(json.load(open(f))['db_name'], '!' in json.load(open(f))['query'], f.split('/')[-1]) for f in glob.glob('backend-rs/.sqlx/query-*.json')]" | sort | uniq -c | head
   ```
   Exactly 5 files changed, all regenerated on MySQL 8.4 with DB `kanpad`.
4. **Docs** — the lesson is *appended* (append-only, dated, tagged); the T006 edits correct the
   false `SQLX_OFFLINE=true` instruction rather than deleting unrelated spec content.
5. **Re-verification (optional, ~5 min):** the commands in §Notes 3, or push to `dev` and watch run
   `backend-mariadb` — it has never reached its test suites in CI, so the suites are the real prize.

### 5. How to land it

Branch is `dev-local` @ `ec83f489`; CI runs on `dev`. Suggested message
(lowercase conventional, 40 chars): `fix(ci): replace mariadb sqlx prepare gate`. Stage with
`git add .github/workflows/backend-rs.yml backend-rs/src docs/lessons/ci-quality.md specs/tasks/T006-*.md specs/tasks/T009-*.md specs/tasks/status.yaml backend-rs/.sqlx`
(`backend-rs/.sqlx` is required — the new cache files are untracked; the T009 spec and `status.yaml`
are part of the task record. `AGENTS.md` is unrelated and committed separately).

**Log**

- [2026-11-11 16:36] Implementation complete + locally verified on both legs (MySQL 8.4: 242 tests, prepare --check green; MariaDB 10.6: 20 tests + online describe gate green). All changes UNCOMMITTED on dev-local @ ec83f489 — awaiting review of the uncommitted diff against this file.
- [2026-11-11 17:14] Review applied: lesson error count corrected (20, not 18), cache-count and
  checklist wording fixed, the T009 spec + `status.yaml` added to the landing set, and the
  out-of-scope full-suite MariaDB result recorded in §Notes 3. Landed on `dev`.
- [2026-11-11 17:40] CI green on `dev` at `78a82826` — run `43010525748`: all four jobs succeeded,
  including `backend-mariadb` (the first green run of the leg).

## Lessons (2026-11-11)

1. **`cargo sqlx prepare --check` is server-flavour-bound, not repo-bound.** sqlx-cli forces
   `SQLX_OFFLINE=false` and byte-compares describe JSON, so a second-database CI leg can *never*
   pass it against a cache generated on the first. A second leg gets an online
   `SQLX_OFFLINE=false cargo check` gate instead; the cache gate belongs to the server that
   generated the cache.
2. **A sqlx `: Type` override does not pin nullability** — sqlx still wraps in `Option` whenever the
   server says nullable. Pin both with `AS "name!: Type"` (or `?`). The Rust row type must be a
   property of the code, not of whichever server prepared the query.
3. **`SQLX_OFFLINE` is not part of cargo's fingerprint.** Switching it between two cargo runs in the
   same `target/` may not re-expand the proc macros, so an online gate can pass *vacuously*. Force a
   rebuild (`cargo clean -p backend-rs`, or sqlx-cli's source-touch trick) — and prove the gate can
   go red before trusting it green.
4. **A column override cannot ride on an expression containing `?`**: `ColumnDecl::parse` splits the
   column name at the first `:`/`!`/`?`, so `SELECT EXISTS(... WHERE id = ?)` silently falls back to
   the inferred type. Alias the expression (`AS "exists!: i64"`) to carry the override.
5. **`.sqlx` cache filenames and `origin.table` are DB-qualified** — regenerate on the canonical
   server *and* the canonical DB name (`kanpad`), or the other leg's `--check` fails for a reason
   that has nothing to do with the change.
6. **A CI job that has never been green hides its next failure behind the current one** (this job
   went `migrate run` → sqlx-cli install → `prepare --check` across four runs). Flush the remaining
   steps locally before pushing, and treat a dead dev DB (tmpfs + container restart) as the first
   suspect when suites suddenly fail with connection panics.
