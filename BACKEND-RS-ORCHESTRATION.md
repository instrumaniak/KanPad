# Backend-rs Orchestration — pause/resume state

> Manager: orchestrator. Executors: subagents in git worktrees.
> Source of truth for behavior: `specs/api-rewrite-rs-specs/50-tasks.md` (one task = one PR).
> This file is the pause/resume pointer. Update it on every task complete/merge.

## Base
- Base branch: `dev-rs-local` (T01 merged `be37720`)
- Worktree root: `/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/KanPad-wt/` (PERSISTENT volume — /tmp/opencode was wiped by 2026-10-11 server restart, losing uncommitted T03 skeleton + T02 progress; never use /tmp for worktrees)
- Shared cargo target cache (deps prebuilt): `/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-target-kanpad` — every Rust command MUST export `CARGO_TARGET_DIR=<that>` (+ `CARGO_HOME=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-home` for installs; root disk / is 98% full)
- Shared test DB (Must Use a tmpfs (RAM disk) for MySQL in Docker for testing see: `specs/api-rewrite-rs-specs/53-test-optimization.md`): `kanpad-rs-testdb` on host :3337 (db `kanpad`); every Rust command MUST also export `DATABASE_URL='mysql://root:dev_root_pw@127.0.0.1:3337/kanpad?charset=utf8mb4'` (compile-time `query!` + `#[sqlx::test]` runtime; ensure migrated before trusting red/green).
- Merge target: `dev-rs-local`. Resume: `git worktree list` + this table + `git log --oneline -20`.

## Cleanup 2026-10-12 (done-worktree purge)
- Removed 17 merged worktrees + branches: wt-t02/t03/t04/t05/t06/t07/t08/t10/t11/t12/t13/t20/t21a/t21b/t22/t23a/wt-fixsess (`rs-*` branches deleted; `rs-t05-config` had no unique work — content already in via `28db999` + cherry-pick `f7f9ae7`).
- Kept only active WIP: `wt-t23b` (`rs-t23b-sort`), `wt-t23c` (`rs-t23c-move`), `wt-t24a` (`rs-t24a-cards`). `KanPad-wt/` 437M → 18M.
- Removed T02 scratch datadir `../.mysql-t02` (198M, mysql-uid files cleared via `docker run mysql:8.4 rm`, then `rmdir`) + exited scratch containers `kanpad-t07/t10/t11-mysql`.
- Kept shared `../.cargo-target-kanpad` (6.4G debug cache) — still needed for active builds; trim only when Rust work is done (`cargo sweep`).
- Note: root `/` 95% vs `/mnt/mint` 71% (72G avail) are different filesystems; worktrees live on `/mnt/mint`.
- Standing rule: delete each task worktree+branch right after its merge; never let done worktrees accumulate again.

## Restart 2026-10-11 (server restart, /tmp wiped)
- Lost: uncommitted T03 skeleton (clippy was green, tests pending) + any T02 progress. Branches reset from `dev-rs-local`; worktrees recreated on persistent volume.
- T01 SAFE (merged `be37720`: compose.yaml + CONTRACT-CHANGES.md + vite.config.ts).

## Wave 1 retry (re-dispatched)
| Task | Branch | Worktree | Depends | Status | Subagent session |
|------|--------|----------|---------|--------|------------------|
| T01 contract+compose | (merged, branch deleted) | — | none | DONE be37720 | ses_ed6b464b8ffe0k6NoBgUExmLOh |
| T03 crate skeleton | `rs-t03-skeleton` | `KanPad-wt/wt-t03` | none | DONE merged e0bfc23 (commit 7fc7462, 57 files; fmt/clippy/test/prepare-check/export-openapi/migrate-info all green) | ses_ed6710404ffe96EwnFmEsCVC19 |
| T02 baseline DB | `rs-t02-baseline` | `KanPad-wt/wt-t02` | T01 DONE | DONE merged 01d3c81 (commit 5b4774e; migrate run+info pending=0, dump diff = documented deviations only, smoke green) | ses_ed6710402ffeyqCiQZYBKUjLAT |

## T52 amendment debt (from T02 — spec conflicts found, NOT fixed per scope)
- D1: `30-db-schema` + T02 text describe store table as `id VARCHAR(255), data
  BLOB NULL, expiry_date BIGINT + INDEX(expiry_date)`; actual
  `tower-sessions-sqlx-store 0.15.0` `migrate()` creates `char(22) / blob NOT
  NULL / timestamp(6)` with NO index. Baseline follows "store DDL is source of
  truth" (verbatim table) + separate `IDX_session_expiry_date` statement.
  Fix via T52: correct §30 line 10 + T02 text + T11 expiry test (past
  **timestamp**, not epoch-ms).
- D2: baseline requires server-fresh `tower_sessions` absence (fail-fast, no IF
  NOT EXISTS on schema). Noted for T50/T51 runbooks.
- D3 (from T07): `CREATE SCHEMA tower_sessions` without `IF NOT EXISTS` races
  under parallel `#[sqlx::test]` setups and fails repeat setups (1007).
  Workaround in-tree: one setup per process + teardown `DROP SCHEMA`. Proposal:
  T52 amends to `CREATE SCHEMA IF NOT EXISTS` (app tables keep fail-fast);
  until then suites migrate once (shared-DB + tx rollback already mandated).
- Fixture-path lesson (T07): `#[sqlx::test(fixtures(..))]` paths resolve
  relative to the TEST FILE dir → suite fixtures must be `../fixtures/<n>.sql`.
- Scratch containers removed 2026-10-12 (`kanpad-t02-scratch`, `kanpad-t02-mysql` already gone; `kanpad-t07/t10/t11-mysql` removed); T02
  datadir `../.mysql-t02` REMOVED 2026-10-12 (see Cleanup section).

## Wave 2 (T04+T06+T07 parallel; T05+T08 after T04 merges)
| Task | Branch | Worktree | Depends | Status | Subagent session |
|------|--------|----------|---------|--------|------------------|
| T04 shared kernel | `rs-t04-kernel` | `KanPad-wt/wt-t04` | T03 DONE | DONE merged c0e3ef1 (commit adeea93; 14/14 errors + full suite green) | ses_ed649d952ffecL08IPMMU1UcCL |
| T06 CI + .sqlx + docker | `rs-t06-ci` | `KanPad-wt/wt-t06` | T03 DONE | DONE merged e1119c3 (commit 1e1a6e8; order-asserted yaml, grep gates live-tested, docker build substituted w/ review — disk infeasible) | ses_ed649d77bffedDan6nSONLwTKq |
| T07 fixtures + seed | `rs-t07-fixtures` | `KanPad-wt/wt-t07` | T02+T03 DONE | DONE merged acac0d5 (commit 90c35d8; harness+12 fixtures+label_seed; suite green twice) | ses_ed649d777ffeSB3nv6SKKWepOj |
| T05 config/obs/versioning | `rs-t05-config` | `KanPad-wt/wt-t05` | T03+T04 | DONE merged 28db999 + compat f7f9ae7 (cherry-picked aabb125; full suite 36 lib + 47 api green w/ live DB) | ses_ed638a92fffe9SlwO4j05L5Bb4 (session died in restart, work finished by orchestrator) |
| T08 validation core | `rs-t08-validation` | `KanPad-wt/wt-t08` | T04 | DONE merged e694f6a (commit 61aa3de; garde catalogue + Valid* extractors) | ses_ed638a92bffeLexI6j2gRF26X9 |

## Stale sessions (closed, no action)
- `ses_ed6b1914...` (pre-restart T02) reported post-merge: verified merge, changed
  nothing. Cross-checks from its report: store version in T03 `Cargo.lock` is
  exactly `0.15.0` (matches T02's DDL reference — no checksum risk); Nest-tip
  dump confirms `utf8mb4_unicode_ci` + all §30 index names present.
- Dispatch hygiene (from its duplicate-dispatch note): one live executor per
  task; re-dispatch only after prior session is confirmed dead + worktree state
  inspected. Applied to wave 2.

## Lessons (orchestrator fixes applied during T03 finish)
- Post-merge compile debt is real: T05's rebase compiled pre-T08 but the merge
  broke `tests/api/helpers.rs` (Settings literal vs expanded struct). Fixed via
  cherry-pick f7f9ae7 (aabb125). ALWAYS re-verify `--all-targets` after merges.
- Shared `CARGO_TARGET_DIR` across worktrees can print a bogus sub-second
  "Finished" (stale fingerprints) — after any merge/rebase, `touch` a source
  file first or treat instant passes as suspect. T10's worktree predates
  f7f9ae7; its merge rebase will bring the fix.
- Reused target dir moved across paths breaks utoipa-swagger-ui build (stale
  OUT_DIR in embed.rs `#[folder]`): fix with `cargo clean -p utoipa-swagger-ui`.
- `tests/api.rs` suite mods need `#[path = "api/<name>.rs"]` (bare `mod x;`
  resolves to `tests/x.rs`). Noted in-file; future suite tasks must follow it.

## Phase 1 — Auth (T10 unblocked: T02+T04+T07+T08 all merged)
| Task | Branch | Worktree | Depends | Status | Subagent session |
|------|--------|----------|---------|--------|------------------|
| T10 register | `rs-t10-register` | `KanPad-wt/wt-t10` | T02+T04+T07+T08 | DONE merged 5fe3cd3 (register 201+cookie+6 labels+Argon2id; 48/48 api green x2) | ses_ed625f90bffesbq8x70oyxEP6p |
| T11 login/logout/me+sessions | `rs-t11-auth` | `KanPad-wt/wt-t11` | T02+T04+T07+T08+T10 | DONE merged a36f4a0 (49/49 x2; session-key SHA-512 fix; no-wipe store rule; teardown scope exception accepted) | ses_ed543735affe4Vzolre9V1OxC6 |

## Early-T52 applied (during T10 merge, 2026-10-11)
- T02-D3 RESOLVED in code: `CREATE SCHEMA IF NOT EXISTS` + sweeper index
  INLINE in CREATE TABLE (commit d9c4c39). Parallel `#[sqlx::test]` setups
  raced 1007 (schema) then 1061 (standalone CREATE INDEX on the shared
  table); full suite flaked until both fixed, now 48/48 x2. App tables keep
  fail-fast. T52 still owes the §30 TEXT correction (store shape
  char(22)/blob/timestamp(6) + inline-index rule).
- Merge-verify discipline confirmed again: shared test DB (:3337 `kanpad`)
  needed `migrate run` first or `query!` compile checks fail (1146); always
  ensure the compile-time DB is migrated before trusting red/green.
- Phase-2 test rule (from T11): NO suite may wipe/drop the shared
  `tower_sessions` store (single coordinated wipe at auth_session startup;
  per-suite jars + set-difference assertions). T52 runbook material.
- Flake watch: one `helpers_seeded_fixtures_suite` failure in a full parallel
  run (message never captured; green alone + 4 consecutive full greens
  after). Suspect overloaded-MySQL transient on :3337. Reopen if it recurs
  with a captured message.

## Phase 2 kickoff (T12+T13+T20 parallel; all disjoint except tests/api.rs mod line)
| Task | Branch | Worktree | Depends | Status | Subagent session |
|------|--------|----------|---------|--------|------------------|
| T12 create-admin CLI | `rs-t12-admin` | `KanPad-wt/wt-t12` | T02+T10 | DONE merged c5dd11b (exit 0/1/2/3, --force reset, single-admin guard) | ses_ed52073f0ffeqscI5AvOgfMTz6 |
| T13 users record | `rs-t13-users` | `KanPad-wt/wt-t13` | T02+T10 | DONE merged 516300c (no-route absence proof, re-export owner) | ses_ed52073caffeQqbz9QQhyjiCSC |
| T20 projects CRUD | `rs-t20-projects` | `KanPad-wt/wt-t20` | T02+T04+T07+T08+T11 | DONE merged 962b84d (201+Location, 2-query board_count proof, explicit in-tx cascade; 53/53) | ses_ed52073c8ffel22pkjLyG539re |
| T21a boards create+detail | `rs-t21a-boards` | `KanPad-wt/wt-t21a` | T02+T04+T07+T08+T11+T20 | DONE merged 97209fc (one-tx 3 cols, nested ASC detail; 54/54) | ses_ed502dd37ffePnTP9juvZfsLwy |
| T21b boards list+filters | `rs-t21b-list` | `KanPad-wt/wt-t21b` | T21a | DONE merged d4116d8 (Page, shared WHERE builder, /archived 400; 55/55; no .sqlx change) | ses_ed4e60f3affeTfFDok3KNC2fiP |
| T22 boards update+state+delete | `rs-t22-state` | `KanPad-wt/wt-t22` | T21a+T21b | DONE merged 5fd6dc7 (PATCH null-detach, 409 active-delete, verb 404s; 56/56) | ses_ed4d48460ffe1ZrAH18PJDFUzF |
| FIX session-wipe race | `rs-fix-session-isolation` | `KanPad-wt/wt-fixsess` | T22 | DONE merged ccd0f11 (wipe retired, expiry row-scoped; 56/56 x5 total) | ses_ed4b8a97bffeZR0z4rOlJAUuqP |
| T23a columns CRUD+gap-close | `rs-t23a-columns` | `KanPad-wt/wt-t23a` | T02+T04+T07+T08+T11+T21a | DONE merged a855e7b (nested routes, shared card_summary; 57/57) | ses_ed4b14f39ffeW4yLfdElPtcZZn |
| T23b columns sort | `rs-t23b-sort` | `KanPad-wt/wt-t23b` | T23a | DONE merged 3adfa97 (sort suite + 2 .sqlx; single CASE rewrite) | ses_ed154e7f1ffeV8iVSQLQD726D4 |
| T23c columns move-cards | `rs-t23c-move` | `KanPad-wt/wt-t23c` | T23a | DONE merged 1917445 (move suite + 2 .sqlx; manual resolve w/ T23b) | ses_ed154e7ebffec7azSTzlRhF6jB |
| T24a cards CRUD+list | `rs-t24a-cards` | `KanPad-wt/wt-t24a` | T02+T04+T07+T08+T11+T23a | DONE merged 25fb548 (929-line suite + 11 .sqlx; auto-merge clean; full 60/60) | ses_ed154e7e8ffeppWsIDydg0wrxn |

## Lessons from T23a (for AGENTS.md/T52)
- Generic `E: MySqlExecutor` values move: split multi-statement fns one-query-per-fn (T20 style), caller orchestrates. No `+ Copy` (blocks `&mut Tx`).
- `COUNT(CASE…)` not `SUM()` for int aggregates (`SUM` exact-value → DECIMAL, no i64 decoder).
- Fresh-DB register seeds 6 labels for user 1 — test links must reuse seeded rows (UQ 1062).
- Timestamp convention for summaries: `YYYY-MM-DDTHH:MM:SS.ffffffZ`; `due_date: null` when unset; `checklist_progress` absent (not null) without checklists.

## Standing test rule (hardened by session fix — fail-closed)
- `clear_sessions()` DELETED (compile error on reintroduction). No global DELETE/UPDATE-all/DROP on the store's `sessions` table, ever. Store assertions via set difference only, or scope session SQL by `WHERE id = ?`.

## Findings from T22 (for T52/AGENTS.md)
- sqlx `QueryBuilder::separated()` inserts separator between EVERY push: a `` `col` = `` prefix push must use `push_bind_unseparated`. In code comment.
- Wipe race is now the top flake: 3rd occurrence (this time `boards_state` itself 401 at create). Fix dispatched before T23a.

## Findings from T21a (for T52/AGENTS.md)
- `query!` overrides compose with inferred nullability: `AS "project_id: Option<i32>"` on nullable col yields `Option<Option<i32>>`; correct is inner type (`AS "project_id: i32"` → `Option<i32>`). `tinyint` decodes as `i8`, never `bool` (`!= 0` in Rust). In `boards/store.rs` docs.
- Session-wipe race upgraded from watch to pattern: 2 full-run flakes in 4 runs (both `projects` 401 mid-flow, `boards_create` alone always green, final 54/54). Single coordinated wipe at auth-suite startup still races parallel suites' jars. Needs a real fix (per-suite session isolation or serialized wipe) — candidate for T22 or dedicated hardening task, not just T52 text.

## Findings from T20 (for T52/AGENTS.md)
- Spec conflict: 20-contract+T20 AC mandate project-delete cascades boards, but baseline FK `FK_boards_project_id` is `ON DELETE SET NULL` (matches Nest entity). Implemented as explicit app-level in-tx cascade (contract wins). T52: schema-text note.
- Portability: lockfile `time` 0.3.55 renamed `PrimitiveDateTime`→`PlainDateTime`; sqlx unifies `time` support via session store so `query!` maps DATETIME→`time` not `chrono`. Future DATETIME selects need `AS "col: chrono::NaiveDateTime"` overrides. T52/AGENTS.md note.
- T04 kernel (after T03) → T05 config/obs (after T03) + T06 CI (after T03) + T07 fixtures (after T02+T03) + T08 validation (after T04)
- T10 register → T11 auth sessions (blocks all Phase 2) → T12/T13
- Phase 2 chain: T20 → T21a → T21b → T22 → T23a → {T23b,T23c,T24a} → T24b/T25/T26a → T24c/T26b/T27a → T27b
- T30/T31, T40a→T40b→T41a-e, T50a-c→T51, T02b→T51, T52 on demand

## Resume 2026-10-12 (cleanup + T23b/T23c/T24a merges)
- Shared test DB restarted: `kanpad-rs-testdb` (mysql:8.4, host :3337, db `kanpad`, pw `dev_root_pw`), baseline migrated (pending=0). No documented startup existed — fresh container + `sqlx migrate run`. Re-create the same way after any docker/host restart.
- T23b/T23c/T24a finished by fresh subagents (prior `ses_ed49da…` sessions dead) and merged T23b→T23c→T24a: `3adfa97` (clean) → `1917445` (routes/store/api.rs conflicts resolved: union imports, both route regs kept, sort fns + move fns concatenated, mod lines alphabetical) → `25fb548` (auto-merge clean). Each merge re-verified `--all-targets`: final fmt/clippy/prepare-check green + full suite **60/60**.
- Merge-verify gotcha (new): `cargo fmt --check | tail` masks the exit code — capture `$?` without a pipe (`> file; echo $?`).
- Merged worktrees+branches removed immediately per standing rule (wt-t23b/t23c/t24a).

## Resume 2026-10-12 Wave A (T24b+T25+T26a+T27a merged)
- Base verified before dispatch: fmt 0, clippy 0, full suite **60/60** (`86s`).
- Dispatched 4 parallel executors (fresh sessions, worktrees on persistent `KanPad-wt/`):
  T24b `ses_ed11a2334ffed9mEhM1B6mHx8A` → `c4db15a` (cards move: clamp-then-write, 500 sequential exact-order + concurrent invariant fuzz; 4 `.sqlx/`);
  T25 `ses_ed11a2317ffeSuVBbE2XSQYe5X` → `da43487` (labels Page+lazy-seed+trim+409 incl. case-variant; 5 `.sqlx/`);
  T26a `ses_ed11a2312ffePMVZwmRIJAuOWg` → `9d6a868` (checklists 5 handlers, deep 4-table ownership; 7 `.sqlx/`);
  T27a `ses_ed11a2300ffewnYIUhjxyxtYy2` → `c2a751b` (notes two-step IDs-then-enrich, COUNT(DISTINCT), LIKE-escape; 6 `.sqlx/`).
- Merged T24b (ff) → T25 (`3351532`, auto-clean) → T26a (`cfe8570`, lib.rs+api.rs union resolved) → T27a (`dbfddc1`, lib.rs+api.rs union resolved).
- Merge-verify: fmt 0, clippy 0, full suite **64/64** (was 60, +cards_move/labels/checklists/notes), prepare-check exit 0. Verified 2026-10-12.
- Merged worktrees+branches removed immediately per standing rule (wt-t24b/t25/t26a/t27a).
- LOAD-BEARING (from T25, out-of-scope workaround): `error.rs` 1062→409 arm checks `code()=="1062"` but sqlx 0.8 reports dup as `code==Some("23000")` (SQLSTATE; 1062 only in unexposed `number`, `kind: UniqueViolation`). T04 unit stub never catches it; labels worked around locally via `map_write_error` (match `ErrorKind::UniqueViolation`). Needs kernel-owner fix (match UniqueViolation) — candidate for T30 wave or dedicated hardening, + T52 note. T24c reuses labels `fetch_label` 404-for-foreign.
- Shared-target-dir cross-contamination confirmed again (T27a: phantom 404s on real routes under concurrent worktree builds): serialize heavy cargo ops at merge time (`cargo clean -p backend-rs` + rebuild), `touch` source before trusting instant passes.
## Resume 2026-10-12 Wave B (T24c+T26b+T27b+T30 merged, verify pending)
- Pre-restart Wave-B executors died uncommitted; their file changes survived in
  persistent worktrees. Post-restart take-over agents continued (no rework):
  T24c `ses_ed0da67f4ffe5xm7423r7NMYVv` → `e2ac210` (prior ~95% + pub-path fix,
  3 `.sqlx/`); T26b `ses_ed0da67f1ffeEVfKFzA7NePO2F` → `9e756c7` (prior complete,
  4 `.sqlx/` + stale-binary `touch`-before-test lesson); T27b
  `ses_ed0da67efffexQ4SqRm3WJQbYf` → `8a7dbac` (prior ~95%, 5 `.sqlx/`);
  T30 `ses_ed0da67dbffenc5w7PnKphZqh3` → `8350406` (governor route_layer /api/*
  only, 6/6 throttle green, lib 51/51).
- Merged T24c (`5df06fe`, clean) → T26b (`595a3ab`, clean) → T27b (`f1e85bd`,
  clean) → T30 (`04d2d01`, api.rs union resolved; lib.rs auto-merged).
- Merge-verify: fmt 0, clippy 0, full suite **73/73**, prepare-check exit 0.
  Verified 2026-10-12.
- Merged worktrees+branches removed per standing rule.
- Next: T31 (golden fixtures, needs all resources — NOW unblocked: T20–T27
  all merged) + T40a (openapi export) + T02b (charset) + kernel 1062 fix.

## Resume 2026-10-12 Wave C (T31+T40a+fix1062+T02b merged, verified)
- Base verified before dispatch: fmt 0, clippy 0, full suite **73/73** (Wave B baseline).
- Dispatched 4 parallel verifications (fresh sessions, worktrees on persistent `KanPad-wt/`):
  T31 `wt-t31` → `a912bdd` (25 golden fixtures + gate test, 27 files, 642 ins; 74/74 green);
  T40a `wt-t40a` → `0be8918` (openapi.json regen + snapshot tests, 3.9k ins; 76/76 green);
  fix1062 `wt-fix1062` → `cac74b6` (UniqueViolation→409 via ErrorKind match in kernel + labels/tags stores + test; 74/74 green);
  T02b `wt-t02b` → `2d0c1c4` (0002_convert_to_utf8mb4.sql 12 tables DYNAMIC + unicode_emoji_gate_roundtrip; 74/74 green).
- Merged T31 (`6f5c869`, clean) → T40a (`933670b`, clean) → fix1062 (`45acfe3`, clean) → T02b (`60838d2`, clean).
- Merge-verify: fmt 0, clippy 0, full suite **79/79** (73 base + golden + 2 openapi + 1 error-kind + unicode), prepare-check exit 0. Verified 2026-10-12.
- Merged worktrees+branches removed per standing rule (wt-t31/wt-t40a/wt-fix1062/wt-t02b).
- LOAD-BEARING: fix1062 resolves the T25 1062→409 gap at kernel level (ErrorKind::UniqueViolation), eliminating per-store workarounds. T02b migration is live-safe (CONVERT TO, no FK changes, ROW_FORMAT=DYNAMIC for 1020-byte UNIQUE).
- Next unblocked: T30 already merged → T40b (Swagger UI serve) + T41a-e (spec hardening) + T50a-c (runbooks) + T51 (cutover) + T52 (amendments).

## Resume 2026-10-13 Wave D (T40b+T50a+T50b+T50c dispatched)
- Base verified before dispatch: fmt 0 (`cargo fmt --check` in `backend-rs/`), shared DB `kanpad-rs-testdb` (:3337) re-migrated (`migrate run` applied baseline + 0002, `migrate info` pending=0 — container had restarted, DB was pending). HEAD `89fb22d` (2 commits ahead of Wave-C note: `30a8c56` test-opt plan + `89fb22d` slow-tests investigation, incl. `backend-rs/docker-compose.test.yml` + trimmed `53-test-optimization.md`).
- Dispatched 4 parallel executors (fresh sessions, worktrees on persistent `KanPad-wt/`, branches off `89fb22d`):
  T40b `ses_ecba65380ffe6SgH4KiHdGzOan` → branch `rs-t40b-client` in `wt-t40b` (frontend client + gen:api CI, frontend-only);
  T50a `ses_ecba65370ffe97i4MtTeKUjl3o` → branch `rs-t50a-e2e-auth` in `wt-t50a` (e2e auth/projects/boards + helpers env-switch);
  T50b `ses_ecba65355ffegf1gwFPcICqlB4` → branch `rs-t50b-e2e-cards` in `wt-t50b` (e2e columns/cards + 500-move fuzz);
  T50c `ses_ecba65353ffeNa9P0PSLC0BcLQ` → branch `rs-t50c-e2e-rest` in `wt-t50c` (e2e labels/checklists/notes/tags).
- Expected merge order: T40b (frontend, no Rust conflicts) + T50a → T50b → T50c (each adds one `tests/api.rs` mod line — union-resolve, keep all). T50b/T50c must NOT touch `helpers.rs` (T50a owns env-switch).
- Next unblocked after Wave D: T41a-e (need T40b merged) → T51 (needs T41a-e + T50a-c) + T52 amendments on demand.

## Resume 2026-10-13 Wave D merges (T40b merged; T50a/b/c in flight)
- T40b MERGED `d32c336` (branch `b93e850` deleted, worktree pruned per standing rule). Verify: `npm run gen:api -- --check` exit 0 (up-to-date) + `tsc --noEmit` exit 0 in `wt-t40b` (main-worktree `node_modules` lacks the new dep — verify only where installed). Accepted: `package-lock.json` side-effect + `40-typegen.md` version line (`openapi-typescript 7.13.0` exact); `tsconfig.app.json` untouched (no change needed). `getErrorMessage` needs no edit (already reads `.status`).
- T50b executor session died mid-verify ("waiting on clippy"); uncommitted work survived in `wt-t50b` (`M tests/api.rs` + `?? e2e_columns_cards.rs`). Take-over re-dispatched (no rework).
- T41a-e NOW UNBLOCKED (T40b merged): dispatch after T50 wave merges to avoid `package.json`/`schema.d.ts` contention — or parallel-safe now (frontend-only T41* vs Rust-only T50* touch disjoint files; `schema.d.ts` regen only if `openapi.json` changes, which neither does).

## Resume 2026-10-13 Wave D merges II (T50b+T50c merged, T50a in flight)
- T50b MERGED `dac2a65` (take-over commit `2c236ef`, auto-clean; branch deleted, worktree pruned). Executor evidence: fmt 0, clippy 0, `e2e_columns_cards` 1 passed/188s genuine run, prepare-check 0, no `.sqlx/` change. Merge-verify: fmt 0 + prepare-check 0.
- T50c MERGED `686448d` (commit `f3cf3c7`; `api.rs` mod-line union resolved: kept both `e2e_columns_cards` + `e2e_rest`; branch deleted, worktree pruned). Executor evidence: fmt 0, clippy 0, `e2e_rest` 1 passed x2 (77s cold/15.6s warm), prepare-check 0, no `.sqlx/` change. Merge-verify: fmt 0 + prepare-check 0.
- LOAD-BEARING review note (T50c design, accepted): `e2e_rest.rs` binds real `axum::serve` on `127.0.0.1:3001` + raw `TcpStream` HTTP instead of the `TestServer` harness pattern — fails fast with clear message if `:3001` occupied; self-contained client (helpers.rs untouched). Works (green x2) but diverges from house style; T50a's env-switch may subsume the overlap. Revisit in T52 if flaky under parallel runs.
- Remaining Wave D: T50a (`wt-t50a`, in flight). After it lands: full-suite re-verify (now 81 tests: 79 + 2 e2e) + dispatch T41a-e.

## Resume 2026-10-13 Wave D complete (T50a merged — full 82/82 green)
- T50a MERGED `67c83d2` (commit `73a6be9`; `api.rs` 3-way union resolved: kept all three e2e mod lines; `helpers.rs` env-switch `e2e_base_url`/`e2e_target_is_nest` auto-merged; branch deleted, worktree pruned). Executor evidence: fmt clean, clippy 0, e2e 1 passed x2, prepare-check 0, no `.sqlx/` change (runtime `query_*` only). Red proof: `E2E_TARGET=nest` run FAILED 101 at `assert_nest_error` (one assertion set cannot pass on both targets).
- Wave-D merge-verify on `67c83d2`: fmt 0, clippy 0, prepare-check 0, `cargo test --test api -- --skip e2e_` **79/79**, then serial e2e (port rule — all three bind `:3001`, never run parallel): `e2e_auth_projects_boards` ok/4.26s, `e2e_columns_cards` ok/11.22s, `e2e_rest` ok/2.02s. **Total 82/82.**
- PORT RULE (load-bearing, for T52/runbooks) — **RESOLVED 2026-10-19**: the three T50* e2e suites each bind `:3001` via in-test `axum::serve`. Full-suite parallel `cargo test --test api` would clash — always verify as `--skip e2e_` + three serial filters. Same rule applies to any future in-test server bind. *Resolution: the two suites that actually bind a listener (`e2e_rest`, `e2e_auth_projects_boards` — `e2e_columns_cards` is in-process) now bind an OS-assigned EPHEMERAL port via `wire::bind_blackbox()` and drive `listener.local_addr()`. Parallel full-suite runs are green; `E2E_BASE_URL` still pins a fixed port when an operator needs one.*
- Wave D total: T40b + T50a/b/c merged. Progress ~35/42 ≈ **83%**.

## Resume 2026-10-13 Wave E dispatched (T41a-e frontend migration, parallel)
- Base `67c83d2` (82/82 green). 5 executors, worktrees `wt-t41a..e`, branches `rs-t41a-auth/t41b-projects/t41c-boards/t41d-colcards/t41e-rest`.
- ISOLATION (load-bearing): each executor runs its OWN tmpfs mysql (`kanpad-t41{a..e}-mysql`, host ports 3341-3345) + backend on its own port (3101-3105) + `VITE_API_URL` pointed there. Never use shared `:3001`/`:3337` (siblings + orchestrator verify would clash). `stop+rm` scratch containers when done. Shared `CARGO_TARGET_DIR` still contends builds — serialize heavy cargo ops, `touch`-before-trust, `cargo clean -p utoipa-swagger-ui` on stale OUT_DIR.
- Expected merge order: any (disjoint `features/*` dirs); `package.json`/`schema.d.ts` shared — keep T40b's exact `7.13.0`, regen `schema.d.ts` if `openapi.json` drifted.

## Subagent contract (copy into every dispatch)
1. Read `specs/api-rewrite-rs-specs/README.md`, `00-overview.md`, your task in `50-tasks.md` + linked `10/20/21/30` sections. `50-tasks.md` Touch-only is hard scope.
2. Work ONLY in your worktree/branch. Never commit to `dev-rs-local` directly, never touch other tasks' files.
3. TDD: failing test first. No test deletion/weakening. Every POST/PATCH/DELETE re-reads state via second GET or sqlx SELECT.
4. Evidence in final report: exact `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test …`, `cargo sqlx prepare --check --workspace --all -- --all-targets` outputs + red-test output + `.sqlx/` decision.
5. Stop at task AC. Report: files changed, test log excerpt, merge conflicts expected, resume notes.
6. If blocked (missing DB, spec conflict), report blocker + partial diff, do NOT expand scope.

## Pause procedure
1. `git worktree list` → note dirty worktrees.
2. In each worktree: `git status --short; git log --oneline -3`.
3. Update Status column above. Commit WIP with `git commit -am "WIP <task> <note>"` on task branch (never on base).
4. Resume: re-read this file + `git log dev-rs-local.. <branch>` per worktree, re-dispatch subagent with `sessionID` or fresh prompt pointing at same worktree/branch.

## Resume 2026-10-13 Wave E complete (T41a-e merged; T51 merged)

- Wave E work verified before commit:
  - T41a auth: tsc 0, gen:api --check pass, vitest 49/49
  - T41b projects: tsc 0, gen:api --check pass, vitest 36/36
  - T41c boards: tsc 0, gen:api --check pass, vitest 174/174
  - T41d columns+cards: fixed test mock for `sanitizeCardListParams`; tsc 0, gen:api --check pass, vitest 150/150
  - T41e labels/checklists/notes/tags: tsc 0, gen:api --check pass, vitest 199/199
- All merged into `dev-rs-local` (`43e19465` chain), then clean combined frontend run: 77 files / 723 tests passing; tsc 0; gen:api --check pass.
- Worktrees removed per standing cleanup rule.
- T51 cutover: defaults flipped to Rust :3001 (Vite proxy + compose), `docs/cutover.md` added; rollback remains one env var (`VITE_API_URL=http://localhost:3000`).
- Verification after merge: backend fmt 0, clippy 0, prepare-check 0, non-e2e 79/79, 3 e2e 1/1 each = **82/82**.
- Progress: 41/42 tasks merged — **≈98%** by task count. Remaining: T52 contract-amend process/doc amendments only (on-demand), no blocking feature work.

## Resume 2026-10-19 (session store moved into the app database + test-harness dedup)

- **Session store follows the pool's database.** `AppState::with_pool(settings, pool)` binds
  `MySqlStore` to `pool.connect_options().get_database()` with `.with_table_name("sessions")`;
  the separate `tower_sessions` schema is gone. The baseline creates a fail-fast
  `CREATE TABLE sessions` in the app database (deviation 11 in `30-db-schema.md`).
- **Consequence: sessions are per-database, not server-global.** Under `#[sqlx::test]` each
  test gets its own database, so no suite can reach another's sessions and there is nothing
  to wipe. The old "the store table is server-global, so no wipe here" rationale across the
  suite module docs is stale — those notes were rewritten (this also retired the last
  "T11 owns the one coordinated wipe" comments).
- **Boot-panic fix:** `MySqlStore::with_schema_name` rejects identifiers stricter than
  MySQL (leading digit, `-`, `.`, space). `Settings::validate` now screens the DB name via
  `db::is_valid_db_name`, so a bad `DB_NAME` fails as a clean `ConfigError` instead of
  panicking when the store is constructed.
- **Test-harness dedup (Phases 0-3):** `tests/api/helpers.rs` is now the single owner of
  the HTTP harness (`app_on_pool`, `send`, `json_of`, `authed_request`, `cookie_jar`,
  `assert_problem` with a `what` label, `assert_errors_nonempty`, `assert_no_legacy_keys`,
  `register`, `create_project`, `create_board`, `create_board_json`). All suites import
  EXPLICITLY (`use super::helpers::{…}`) so a leftover local copy is a compile error
  (E0255) rather than a silently-shadowed glob import. Dead `JarClient`/`login_as`/
  `authed` scaffolding deleted. `tests/api` went 19,508 → ~18,200 LOC.
- **Config-test fix:** `Settings::load_from_env()` (no dotenv) added; the config unit tests
  use it so a developer's local `backend-rs/.env` cannot re-supply a key a test deliberately
  unset (that made 5 `config::tests` fail whenever a local `.env` existed).
- Verification: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -D warnings`
  clean, `cargo test -p backend-rs --test api` **82/82** (serial; the parallel run has one
  pre-existing `:3001` port conflict between the two black-box e2e suites).
- **`:3001` port conflict RESOLVED (same day).** The two black-box suites that bind a
  listener (`e2e_rest`, `e2e_auth_projects_boards`; `e2e_columns_cards` is in-process)
  now bind an OS-assigned EPHEMERAL port via the new `wire::bind_blackbox()` and drive
  `listener.local_addr()` — so cargo's parallel scheduler can no longer collide, and a
  dev server already holding `:3001` cannot break the suite either. `E2E_BASE_URL` still
  pins a fixed port when an operator needs one (the Nest-target workflow).
  `helpers::e2e_base_url()` now returns `Option<String>` (`None` = "no pin").
  Parallel full-suite runs are green: **82/82** x3 consecutive runs.

## Resume 2026-10-19 (code review of the session-store + harness change — 5 findings fixed)

A review of the uncommitted change above found 5 issues. All are fixed; the tree is
still UNCOMMITTED (the user is preparing the commits by hand).

- **F1 — the `helpers.rs` ownership header was NORMATIVE but the code violated it.**
  The header declared a "complete forbidden-local set" (`send`, `json_body`, `json_of`,
  `assert_errors_nonempty`, …) that FOUR suites still defined locally — and the E0255
  guard never fired for them, because a suite can shadow a shared name forever simply by
  NOT importing it (`auth_register.rs` aliased `json_of as parse_json`; `throttle.rs`
  imported nothing from `super::helpers` and kept its own `send` + a duplicate
  `PROBLEMS` const). Fixes:
  - `helpers.rs` gained the shared variants the suites were re-implementing:
    `send_ref` (borrowed router — `throttle.rs` drives ONE app across many flood
    probes), `send_json` (pre-parsed body — `auth_register.rs`), and `cookie_jar_from`
    (the cookie-jar core, now shared with the wire adapter). `send` delegates to
    `send_ref`.
  - `auth_register.rs`: local `send` deleted (imports `send_json`), local `json_body`
    renamed `register_body` — no more name collision, and the `json_of as parse_json`
    alias hack is gone.
  - `throttle.rs`: local `send` deleted (imports `send_ref`), duplicate `PROBLEMS` const
    deleted (imports `helpers::PROBLEM_BASE`). It is no longer a self-contained island.
  - `e2e_rest.rs`: local `json_of` deleted (uses `WireResponse::json(what)`, the wire
    counterpart), local `register` renamed `register_user` (it collided with
    `helpers::register`; it stays per-suite because it is bound to that suite's
    blocking client).
  - `e2e_auth_projects_boards.rs`: its target-aware `assert_errors_nonempty` adapter is
    legitimate — it is now documented as a SANCTIONED EXCEPTION, and the header grew a
    "Sanctioned exceptions" section + a precise scope (IN-PROCESS suites).
  - A second, broader audit (this time against the COMPLETE forbidden set, including
    `test_settings` / `test_app` / `PROBLEM_BASE`) found THREE more suites that were
    shadowing shared names: `health.rs` (local `test_settings` + `test_app`, byte-identical
    to the shared ones) and `errors.rs` (local `test_settings`) — plus a duplicated
    `PROBLEMS` const in `health.rs`, `errors.rs` and `validation.rs`, and inline
    `"https://kanpad.dev/problems/…"` literals in `boards_list.rs`. All deleted; the suites
    now import `test_settings` / `test_app` / `PROBLEM_BASE` from the shared harness.
    The first review had MISSED these because its grep pattern omitted those three names.
- **F2 — the in-place baseline edit is a sqlx checksum break.** `0001_baseline.sql` was
  amended in place, and sqlx stores a SHA-384 of each migration's bytes in
  `_sqlx_migrations` — a DB migrated with the pre-2026-10-19 baseline now fails with
  `VersionMismatch` (app boot AND `migrate run`). The baseline is a squash (the cutover
  always builds a FRESH DB), so an empty target is unaffected; but any pre-existing DB
  must be dropped and re-migrated. Documented in
  `docs/data-migration-live.md` §"Baseline amendment (2026-10-19)" and in
  `30-db-schema.md` deviation 11. Never hand-edit `_sqlx_migrations`.
- **F3 — `wire::header_first` now lowercases its name argument**, matching
  `WireResponse::header` (all 6 call sites already passed lowercase, so no behavior
  change — only the footgun is gone).
- **F4 — the two `cookie_jar` implementations now share one core.**
  `helpers::cookie_jar_from` is the core; `helpers::cookie_jar` (axum `HeaderMap`) and
  `wire::cookie_jar` (raw wire headers) are per-transport adapters over it — the same
  pattern as `assert_problem_fields`.
- **F5 — `AppState::with_pool`'s panic is now contractual.** The `pub` constructor
  panics if the pool's database name is not a valid identifier; the doc now has a
  `# Panics` section explaining the precondition, why it is unreachable in production
  (`Settings::validate` screens it) and why a silent fallback would be worse. The
  `session_store` panic message now names the rule and the reachability.
- AGENTS.md gained the matching rules (forbidden-set names + sanctioned exceptions,
  "pick the shared variant", the cookie-jar core) and a lesson: **a normative header
  that the code violates is worse than no header — audit the forbidden set against the
  actual `fn` definitions, not against the import statements.**

## Resume 2026-10-22 — Phase 7 (Rust-idiom amendments + quality baseline) started

- **Task source of truth:** `specs/api-rewrite-rs-specs/50-tasks.md` **Phase 7** (T54–T67);
  rationale + scorecard in `54-rust-idiom-hardening.md`. Ordering is
  **dependency-first, feature-second** (the NestJS data is NOT migrated yet, so the
  toolchain / resolver / dependency set / DB-connection semantics are cheap NOW and
  expensive after the cutover — `54` Part C §C8).
- **Spec patch landed first** (`d3d0eac9`): a third verification pass **executed** the
  plan's gates instead of reading them → **6 more corrections, 2 blocking** (the new
  grep gate contradicted its own prescribed fix; T63's AC contradicted itself).
  Details in `54` §"Second verification pass (2026-10-22 (c))" + `00-overview.md`
  Amendment 13.
- **T54 DONE** (`8b0cef64`, merged ff into `dev-rs-local`): `APP_ENV` replaces
  `NODE_ENV` (boot FAILS when `NODE_ENV` is present — `ConfigError::LegacyNodeEnv`,
  checked *before* `envy` reads `APP_ENV`); cookie `connect.sid` → `kanpad.sid` /
  `__Host-kanpad.sid`. Gates: fmt 0 | clippy -D warnings 0 | lib 60/60 | **api 82/82** |
  `sqlx prepare --check` 0 | `.sqlx/` unchanged | `openapi.json` + `schema.d.ts`
  UNCHANGED. Red-proof: removing the `NODE_ENV` check makes the new test fail with
  `app_env: "development"` — the exact silent downgrade the rule prevents.
- **T54's red-proof lesson** (now in AGENTS.md): a rename must not fail open, and a
  boot-failure gate must be proved RED on a probe before it is trusted.

### Phase 7 task table

| Task | Branch | Worktree | Depends | Status |
|------|--------|----------|---------|--------|
| T54 naming (`APP_ENV` + `kanpad.sid`) | (merged, branch deleted) | — | none | **DONE `8b0cef64`** |
| T55 CI activation + lint-gate repair | (merged, branch deleted) | — | T54 | **DONE `689dc895`** |
| T57 build & supply chain | (merged, branch deleted) | — | T55 | **DONE `98fb1ce2`** |
| T63 dead deps | (merged, branch deleted) | — | T57 | **DONE `85f433f3`** |
| T65 in-tree session store | (merged, branch deleted) | — | T63 | **DONE `55071642`** |
| T67 library majors | (merged, branch deleted) | — | T65 | **DONE `a0b25543`** |
| T66 sqlx 0.9 | (merged, branch deleted) | — | T67 | **DONE `afd87c0e`** |
| T69 frontend types vs the snake_case contract | (merged, branch deleted) | — | T67 | **DONE `93ca7016`** |
| T56 CSRF + CSP + span redaction | (merged, branch deleted) | — | T67 | **DONE `ddbcf43b`** |
| T58 public-API ergonomics | (merged, branch deleted) | — | T56 | **DONE `cdc05c48`** |
| T59 shared-kernel DRY + `/ready` in OpenAPI | (merged, branch deleted) | — | T58 | **DONE `c6f11995`** |
| T60 observability (`/metrics` + JSON logs) | (merged, branch deleted) | — | T59 | **DONE `dd3ba477`** |
| T61 concurrency + pool tuning | (merged, branch deleted) | — | T59 | **DONE `aff43567`; cards_move 3/3** |
| T62 coverage gate + CI pins + Docker hardening | `rs-t62-gates` | — | T60 | pending |
| T68 CI first-run defect repair | (merged, branch deleted) | — | T63 | **DONE `0b03ea58`** |

### Environment notes (2026-10-22)

- The shared test DB container `kanpad-rs-testdb` had been **stopped**, and its data
  dir is **tmpfs (RAM)** → a restart WIPES the schema. Recreate with:
  `docker start kanpad-rs-testdb` + `SQLX_OFFLINE=true cargo run -- migrate run`
  (`migrate info` → `applied: 2 pending: 0`). The `query!` macros need the schema at
  compile time **unless** `SQLX_OFFLINE=true` — use it for the migrate step itself
  (chicken-and-egg: the lib will not compile without a schema *or* the offline cache).
- Every cargo command needs `CARGO_TARGET_DIR=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-target-kanpad`
  (+ `CARGO_HOME=…/.cargo-home`), `DATABASE_URL=mysql://root:dev_root_pw@127.0.0.1:3337/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci` (**the collation pin is REQUIRED since T66** — sqlx 0.9 lets the server pick the collation, and MySQL 8's default is `utf8mb4_0900_ai_ci`, not this schema's `utf8mb4_unicode_ci`; `Settings::validate` rejects a URL without it),
  and `SESSION_SECRET` (32+ bytes) — the worktrees have **no `.env`** (gitignored), so
  `Settings::load()` fails with *"missing value for field session_secret"* without it.
- **`gh` is NOT installed** → T55's AC *"the workflow runs green on the PR (a `gh run
  list` link)"* cannot be satisfied locally. Everything else in T55 is verified by
  replaying each gate by hand; the CI run itself needs a push to GitHub.

## Resume 2026-10-22 (T55 done — the CI can finally run)

- **T55 MERGED `689dc895`** (ff into `dev-rs-local`; worktree + branch deleted per the standing rule).
  - **B1**: `backend-rs/.github/workflows/ci.yml` → **repo root** `.github/workflows/backend-rs.yml`
    (a RENAME — the nested file is deleted; two copies would drift). The trigger path filter now
    includes the workflow file itself.
  - **B2**: the 3 sites the `unwrap`/`expect` gate reported are fixed **in the code**:
    `columns/store.rs::to_response_one` (the create/update paths no longer build a one-element
    `Vec`, pop it and `expect` it) + `state.rs` now uses the file's `unwrap_or_else(|_| panic!(…))`
    fail-fast idiom. A **second gate** was added (`panic!`/`todo!`/`unimplemented!` outside
    `main.rs`+`#[cfg(test)]`, with the boot-time `state.rs` fail-fasts allow-listed).
  - **B3**: `allow_methods` gained **`PUT`** + the new `tests/api/cors.rs` preflight guard
    (**the `PUT` fix moved here from T56** — the guard must not land red). T56 keeps the
    Origin check + CSP + span redaction.
  - **The THIRD gate was also repaired**: `no format! in store.rs` had never run and failed on
    two false positives (a doc comment mentioning `format!`; the sanctioned LIKE-escape VALUE
    builder). The GATE was fixed (skip comments; flag a `format!` only when the built text
    carries a SQL keyword), not the code.
- **Evidence (local replay, since `gh` is not installed):** all three grep gates green AND each
  proved **RED** on a probe; fmt 0 | clippy -D warnings 0 | lib 60/60 | **api 86/86** (82 + 4
  CORS) | `sqlx prepare --check` 0 | `.sqlx/` unchanged. The CORS guard is red-proofed too
  (removing `PUT` → `CORS must allow PUT, got "GET,POST,PATCH,DELETE,OPTIONS"`).
- **Open AC**: "the workflow runs green on the PR (a `gh run list` link)" — needs a push to
  GitHub + the `gh` CLI. Everything else in T55 is satisfied locally.
- **Next**: T57 (build & supply chain: `[lints]`, `[profile.release]`, `rust-toolchain.toml`
  @ 1.99.1 + the CI toolchain-override fix, `deny.toml`, edition 2024) → T63 → T65 → T67 →
  {T56, T66} → T58 → T59 → T60 → T61 → T62.

## Resume 2026-10-22 (T57 done — edition 2024 + toolchain 1.99.1 + deny.toml)

- **T57 MERGED `98fb1ce2`** (ff into `dev-rs-local`; worktree + branch deleted).
- **THE TOOLCHAIN MOVED: `rustc 1.99.1`** is now the default for every cargo command in
  `backend-rs/` (the `rust-toolchain.toml` pin). The Dockerfile base moved to
  `rust:1.99-bookworm` in the same commit. **The CI's `dtolnay/rust-toolchain@stable` step
  OVERRODE the pin** (it exports `RUSTUP_TOOLCHAIN`) — it now names the pinned channel.
- **`edition = "2024"`** landed with two REAL blockers the spec's "no blockers" claim missed:
  - `std::env::set_var`/`remove_var` became **UNSAFE** → the config tests now load from an
    explicit map (`Settings::from_pairs`) instead of mutating the process env → no `unsafe`
    (the crate forbids it) and the tests are parallel-safe (the `ENV_LOCK` mutex is gone).
  - clippy 1.99 adds 8 `collapsible_if` errors → fixed with edition-2024 let-chains.
  - rustfmt's style edition follows the crate edition → the WHOLE tree was re-ordered by
    `cargo fmt`. That is mandatory (the fmt gate runs the pinned toolchain), not drift.
- **`deny.toml` is committed and DERIVED** (the CI's `cargo deny init` fallback is gone — a
  missing policy now fails the build). The first run reported **6 real advisories**, and every
  one is removed by a LATER task in this phase, so each is a **task-bound ignore**:
  `idna 0.5` + `paste` (via the dead `axum-valid`/`utoipa-axum`) → **T63**; `zip 3.0` ×3
  (build-time, utoipa-swagger-ui 9) → **T67**; `rsa 0.9` Marvin (sqlx-mysql's
  `caching_sha2_password` auth) → **T66**. **Those tasks MUST delete their ignore entries.**
- **`Cargo.lock`**: the `resolver = "3"` bump changed NOTHING (MSRV 1.99 is newer than every
  dependency's floor → no downgrades). The only movement is the advisory fix: **25 crates, all
  UPWARD** (incl. `rustls 0.23.45 → 0.23.46`).
- **`openapi.json`**: utoipa reads `CARGO_PKG_*`, so the crate description + licence now appear
  in the `info` block (3 lines). **The typegen artifact is unchanged** (verified with the real
  `openapi-typescript`).
- **Gates**: fmt 0 | clippy -D warnings -A pedantic 0 | `cargo doc --no-deps` **0 warnings**
  (8 pre-existing broken intra-doc links fixed) | lib 60/60 | **api 86/86** | prepare --check 0 |
  `.sqlx/` unchanged | `cargo deny advisories licenses bans` **ok** | release binary **stripped**
  (`file` → stripped, `nm` → no symbols).
- **NEXT**: T63 (drop `axum-valid` / `utoipa-axum` / `zeroize` + the 2 dead tower-http features
  — **and delete the 2 advisory ignores it owns**) → T65 (in-tree session store) → T67 (library
  majors — **delete the 3 `zip` ignores**) → {T56, T66 (**delete the `rsa` ignore**)} → …

## Resume 2026-10-22 (T63 done — the dead dependency set is gone)

- **T63 MERGED `85f433f3`** (ff into `dev-rs-local`; worktree + branch deleted).
- **Removed**: `axum-valid` (0 refs — it dragged a SECOND complete axum 0.7.9 + `validator` 0.18 +
  a second `garde` 0.20), `utoipa-axum` (0 refs), `zeroize` (only `secrecy` uses it), and the
  tower-http `timeout` + `limit` features (both unused — the timeout is `tower`'s feature, the
  body cap is axum's `DefaultBodyLimit`).
- **Kept (load-bearing)**: `time` (REQUIRED by `tower_sessions::Expiry::OnInactivity`) and
  `secrecy/serde` (`SecretString: Deserialize` for `envy`).
- **The deny.toml ignore list SHRANK**: the `idna` + `paste` advisory ignores were DELETED with
  their crates, and `cargo deny check advisories` is still green without them. **The remaining
  ignores are T67's 3 × `zip` and T66's `rsa`.**
- Evidence: `axum@0.7.9` / `axum-core@0.4.5` / `validator` → not found; `garde` → no ambiguity;
  grep gate → 0 hits; duplicated crate names **33 → 27**; fmt 0 | clippy 0 | lib 60/60 |
  api 86/86 | prepare --check 0 | cargo deny x3 ok.
- **NEXT**: T65 (the hinge — the in-tree `SessionStore`; it replaces `tower-sessions-sqlx-store`
  and is what makes `tower-sessions 0.15` + `sqlx 0.9` reachable) → T67 (library majors —
  **delete the 3 `zip` advisory ignores**) → {T56, T66 (**delete the `rsa` ignore**)} → …

## Resume 2026-10-22 (T68 done — the pipeline is green-able for the first time)

- **T68 MERGED `0b03ea58`** (ff into `dev-rs-local`; worktree + branch deleted).
- **T55's "the workflow runs green on the PR" AC is RE-OPENED** — the pipeline had never run,
  and when every step was REPLAYED by hand it was **RED in two places**:
  1. **`cargo audit` contradicted the committed `deny.toml`** — two advisory gates, two
     policies (`cargo-audit` reads `.cargo/audit.toml`, absent; `cargo-deny` reads
     `deny.toml`). Red-proof: `error: 4 vulnerabilities found!` (exit 1). **The audit step is
     DELETED** — `cargo deny check advisories` reads the same RustSec DB, checks yanked
     crates, honours the task-bound ignores, and IS the committed policy.
  2. **The SHOW CREATE TABLE assert queried the pre-2026-10-19 `tower_sessions` schema**,
     which the app-database store change made non-existent → empty row → `exit 1`. The
     special case is **deleted** (the baseline creates `sessions` with the same explicit
     `ENGINE=InnoDB … COLLATE=utf8mb4_unicode_ci` as every other app table, so the general
     query covers it). The general query now also excludes `_sqlx_test_databases` (sqlx's
     `#[sqlx::test]` harness table — server-default collation), so the assert is correct
     regardless of the step order.
- **`cargo-deny` is now PINNED** (`--version 0.20.2`) — a `deny.toml` is schema-version-sensitive
  (the first run of the T57 policy failed with `unexpected-keys … deny-multiple-versions`).
- **Closed (2026-10-27):** `gh` installed (2.103.2, `instrumaniak`, scopes `repo`/`read:org`); `dev-rs` pipeline `40431691743` completed/success (T62 docker build); `40410191100` (sqlx prepare, fixed by `SQLX_OFFLINE`) and `40425144777` (llvm-cov/DB-alignment) are intermediate failures, both recovered in `efb7bf52`. T55's "green on PR" AC satisfied (`gh run list` link `40431691743`). T62's `docker-e2e` and `web` jobs still need first execution (they depend on backend passing; backend now green).
  push to GitHub.
- **NEXT:** P2 — the spec reconciliation (the `54`/`50-tasks`/`11-rewrite-deltas` drift found by
  the 2026-10-22 (e) review: the status banner, the stale `ci.yml` paths, the A1/A2 blast-radius
  file names, **T65's "five store files" → EIGHT**, the `openapi.json` claim, the matrix's clippy
  command, and "two lint-gate greps" → three) — then **T65** (the hinge).

## Resume 2026-10-22 (P2 done — the specs match the tree again)

- **P2 MERGED `65eb76b2`** (ff into `dev-rs-local`; worktree + branch deleted). Spec-only PR — **no runtime file touched**.
- **8 drift defects fixed**, in BOTH directions:
  - *the plan described a tree that no longer exists*: the `54` status banner (now a single accurate
    paragraph + an **implementation-status table**), 5 stale `backend-rs/.github/workflows/ci.yml`
    pointers, the "repo-side claims" paragraph (now labelled a SNAPSHOT + re-measured), and
    `11-rewrite-deltas.md`'s E5 banner (the build chain + gate landed; the observability half is still
    PLANNED).
  - *the plan described a tree that never existed*: **T65's "five store files" are EIGHT** (measured:
    boards, cards, checklists, columns, labels, notes, projects, tags — `labels`/`projects`/`tags` were
    missing, so **T65's own AC would have failed on them**), and "no task touches `openapi.json`" is
    false (T57 changed its `info` block — 3 lines; `schema.d.ts` byte-identical).
  - *the plan mandated a command that FAILS*: the matrix's clippy command now carries the
    `-A clippy::pedantic` advisory exemption (the un-exempted form fails with ~290 pedantic errors),
    and the T55 matrix row counts **three** grep gates, not two.
- **Verification was mechanical, not eyeballed**: every path `54` names resolves; the stale patterns
  are gone; every count `54` asserts re-measured PASS; the code gates still hold (fmt 0 | clippy 0 |
  lib 60/60 | prepare --check 0).
- **NEXT: T65** — the hinge. Corrected scope: the in-tree `tower_sessions_core::SessionStore` over the
  app pool, MessagePack payload + the `sessions` DDL byte-identical, `tower-sessions-sqlx-store` dropped,
  the `AS "…chrono::NaiveDateTime"` overrides deleted from **EIGHT** store files, `sqlx/time` off, and
  the `state.rs` grep-gate allow-list entry dropped (T55's sequencing note).

## Resume 2026-10-22 (T65 done — the hinge is through; both dependency majors are reachable)

- **T65 MERGED `55071642`** (ff into `dev-rs-local`; worktree + branch deleted).
- **The in-tree `SessionStore` lives at `backend-rs/src/common/session_store.rs`.** `tower-sessions-sqlx-store`
  is gone → **`sqlx 0.9` and `tower-sessions 0.15` are now reachable** (they were blocked by it: it required
  `sqlx ^0.8` AND `tower-sessions-core ^0.14`).
- **The on-disk format is byte-identical** — the blob is asserted byte-for-byte equal to
  `rmp_serde::to_vec(&record)` (the previous store's exact call), the `sessions` DDL is untouched, and
  `SHOW CREATE TABLE sessions` still matches the baseline (the runbook's step 5(d) drift gate stays green).
- **THREE findings the plan did not anticipate** (all resolved in the PR):
  1. **The sweeper's loop was behind a transitive FEATURE** (`tower-sessions-core/deletion-task`, enabled by
     the store) — dropping the store dropped the feature and `main.rs` stopped compiling. Fix: declare
     `tower-sessions-core` **directly** with the feature (the same version `tower-sessions 0.14` pins, so no
     second core).
  2. **The invariant became STRUCTURAL** — the old store interpolated a schema name (so it had to be
     configured + validated + it could `panic!`); the in-tree store interpolates nothing, so the
     `with_pool` panic, its `# Panics` contract and the CI grep gate's `state.rs` allow-list entry are all
     **gone**. The gate now has **no allow-list** and still passes.
  3. **`sqlx/time` was the only reason for the `AS "…chrono::NaiveDateTime"` overrides** — confirmed, and
     the overrides are gone from **all eight** feature store files.
- **The `.sqlx/` diff was reviewed**: 76 → 83 (60 unchanged, 16 re-keyed with **identical column
  signatures**, 7 new). **The spec's "the 76 entries are unchanged" was wrong** and is corrected — removing
  an override changes the SQL text, so those entries are re-keyed.
- **`Cargo.lock` SHRANK by exactly one package**; nothing else moved and nothing was downgraded.
- **Gates**: fmt 0 | clippy 0 | lib 69/69 (9 new store tests) | api 86/86 | prepare --check 0 | cargo deny ×3 ok.
- **A note on the test suite's duration**: it ranged 21s → 239s across runs during this session — the cause
  is the MACHINE (load average 5.4 from the desktop; the base commit measured 120s under the same load
  while the branch measured 27.6s right after). A/B against the base commit under identical conditions is
  the only reliable comparison; a single timing is not.
- **NEXT: T67** (library majors: tower-http 0.7, tower-sessions 0.15, utoipa 6 + swagger-ui 10, argon2 0.6,
  bcrypt 0.19, sha2 0.11, garde 0.23 — **and delete the 3 `zip` advisory ignores**) → then {T56, T66
  (**delete the `rsa` ignore**)}.

## Resume 2026-10-22 (T67 done — the library majors are taken; a third pipeline red surfaced)

- **T67 MERGED `a0b25543`** (ff into `dev-rs-local`; worktree + branch deleted).
- **tower-http 0.7.1** (both behaviour changes TEST-LOCKED: the compression **406** and the `ServeDir`
  **trailing slash** — `tests/api/tower_http_07.rs`, 4 tests) · **tower-sessions 0.15 + core 0.15** (one
  core) · **utoipa 6 + swagger-ui 10** (the OpenAPI union ordering changed → `openapi.json` **and**
  `schema.d.ts` regenerated **in this PR**; the typegen diff is 98 lines, all union re-ordering) ·
  **argon2 0.6 + bcrypt 0.19** (the PHC types moved to the `phc` crate; the salt is generated internally
  and the verifier takes the PHC string — **the PHC OUTPUT is unchanged**, the existing test still pins
  `$argon2id$v=19$m=19456,t=1,p=1$`) · **sha2 0.11, garde 0.23, dialoguer 0.12, tower_governor `0.8`**.
- **THE COMPRESSION DECISION**: `compression-gzip` + `compression-br` (no browser accepts zstd) — the lock
  loses `zstd`/`zstd-safe`/`zstd-sys` **and their C-build tails**: the C-toolchain requirement is gone.
- **THE 3 `zip` ADVISORY IGNORES ARE DELETED** (the lock moves `zip 3.0.0 → 9.0.5`). **The ignore list now
  holds ONE entry** — the `rsa` Marvin advisory, which **T66 owns**.
- **`sha2` did NOT deduplicate** — `sqlx-core` pins `sha2 ^0.10.0` in both 0.8 and 0.9. The spec's
  "the lock now carries ONE sha2 version" was impossible; corrected.
- **NEW TASK T69 — the THIRD latent pipeline red, and it is in the FRONTEND:** the frontend does not
  type-check against the generated `schema.d.ts` (**15 `tsc` errors**; `npm run build` = `tsc -b && vite
  build` **FAILS**), so the CI's `web` job and the `docker-e2e` job are red for a reason that has nothing
  to do with the backend. The errors are **pre-existing** (the committed schema.d.ts produces the same 15)
  and **not caused by utoipa 6** (identical count before/after). Root causes: Nest-era **camelCase**
  leftovers (`boardCount` against a snake_case API) and hand-written type surgery that drops a field's
  nullability. **The generated types are the contract — the frontend is the wrong side.** T69 touches
  `frontend/src/**` only.
- **Gates**: fmt 0 | clippy 0 | lib 69/69 | api 90/90 | prepare --check 0 | **.sqlx/ UNCHANGED** (no query
  changed) | cargo deny ×3 ok | the three grep gates green.
- **NEXT**: **{T56, T66}** — T66 is the driver major (sqlx 0.9 + the pinned collation + the `sqlx-cli` bump
  + **delete the `rsa` advisory ignore**) and it BLOCKS T58/T59; T56 is the security hardening (the
  `CsrfLayer` Origin check + the CSP + the span redaction) and it can run in parallel. **T69 (the frontend
  types) is needed before the pipeline can be green.** Then T58 → T59 → T60 → T61 → T62.

## Resume 2026-10-22 (T66 done — the driver major is taken; the collation is pinned)

- **T66 MERGED `afd87c0e`** (ff into `dev-rs-local`; worktree + branch deleted).
- **`sqlx 0.9`**: `runtime-tokio` + `tls-rustls-ring-native-roots`, and **`mysql-rsa` is now OPT-IN and REQUIRED** (the `caching_sha2_password` handshake needs the server's RSA key on a PLAINTEXT connection — the dev/test DB; production is TLS, so the RSA path never runs there).
- **THE COLLATION PIN** — sqlx 0.9 sends only `SET NAMES utf8mb4` and lets the SERVER pick (`utf8mb4_0900_ai_ci` on MySQL 8 — a DIFFERENT collation from this schema's `utf8mb4_unicode_ci`), so the driver version alone would have silently changed it. The pin is now **enforced at boot** (`Settings::validate` rejects a URL without it), present in the **derived** URL (so `compose.yaml`'s `DB_*`-only topology is covered), in all five URL places, and **asserted by a test** (`@@collation_connection`).
- **`backend-rs/sqlx.toml` DELETED** — the plan assumed it would be *inert* under 0.9; it is a **HARD COMPILE ERROR** without the `sqlx-toml` feature. Deleting it is safe (its only setting IS 0.9's default).
- **The `Migrate` trait is adopted** — `main.rs` uses `Migrate::list_applied_migrations` instead of a hand-rolled `SELECT version FROM _sqlx_migrations` (the table name is a sqlx implementation detail). `migrate info` → `applied: 2 pending: 0`.
- **The `.sqlx/` diff was reviewed**: 45 modified, **0 added/removed**, and **NO Rust type changed** (same SQL, same column signatures, same nullability — the diffs are sqlx 0.9 ADDING `collation`+`origin` metadata).
- **The `rsa` advisory ignore STAYS** — the plan was wrong: RUSTSEC-2023-0071 has **NO patched version** (it covers `rsa` 0.10.0-rc too). The reason was rewritten to state the real exposure.
- **Gates**: fmt 0 | clippy 0 | lib 69/69 | api 92/92 | prepare --check 0 | cargo deny ×3 ok | the three grep gates green.
- **NEXT**: **T56** (the security hardening — the `CsrfLayer` Origin check + the CSP + the span redaction) and **T58** (the public-API ergonomics — unblocked now that T66 is in) can run in parallel; **T69** (the frontend's 15 type errors) is needed before the pipeline can be green. Then T59 → T60 → T61 → T62.

## Resume 2026-10-22 (T69 done — the frontend type errors are gone; T56 in flight)

- **T69 MERGED `93ca7016`** (ff into `dev-rs-local`; branch deleted). 10 files, +69/−32,
  `frontend/src/**` only. **No `schema.d.ts` edit** — the generated type is the contract.
- **Verified by the orchestrator (command-level, no subagent):** `tsc --noEmit -p
  tsconfig.app.json` **0 errors** (was 15) | `npm run build` green (9.5 s) | `gen:api --check`
  "schema.d.ts is up to date" | vitest **723/723 (77 files)** | no `any` / `@ts-ignore` /
  `as unknown as` introduced | `frontend/dist/` not staged.
- **Flake note:** the first vitest run (concurrent with Rust compilation) failed 2/723; a clean
  rerun was 723/723. Load-induced flakes are environmental — do not "fix" a test for them.
- `dev-rs-local` has no remote branch (no push); `gh` 2.103.2 installed, `dev-rs` CI `40431691743` green (docker-e2e passed); T55 AC (`gh run list` link) verified; T62 docker-e2e and web jobs executed for first time.
- **T56 IN FLIGHT** on `opencode/deepseek-v4.1-flash-free` (the configured default
  `minimax-m3.1-flash-free` no longer exists in the provider catalogue; the PAID siblings now
  answer *"Insufficient account funds"*, so **free models only** from here on).
- **Fourth latent pipeline red (owned by T62):** `backend-rs/Dockerfile:29` still copies
  `sqlx.toml` — deleted by T66 — so `docker build` fails in the `docker-e2e` job. Docker
  builds are also disk-infeasible here (`/` is 98% full), exactly as recorded for T06.
- **NEXT**: T56 (in flight) → **T58** (config.rs is free once T56 lands; T56 also touches it) →
  T59 → {T60 ∥ T61} → T62. **One backend subagent at a time** — two parallel agents on the free
  model contend for the shared cargo target and get rate-limited.
- **Spec/status writers:** the orchestrator is the SINGLE writer for `50-tasks.md` ACs,
  `54-rust-idiom-hardening.md`'s status table and this file. Executors must not flip ACs —
  parallel agents editing `50-tasks.md` is how the AC table drifted before.

## PAUSED 2026-10-22 — T56 merged; the security hardening is in

**Resume point: `dev-rs-local` @ `ddbcf43b`, working tree clean, ZERO worktrees.**
Next task: **T58** (`rs-t58-ergonomics`, worktree `KanPad-wt/wt-t58`).

### What landed this session (T54 → T56 + T69)

| Task | Commit | One line |
|---|---|---|
| T54 | `8b0cef64` | `APP_ENV` replaces `NODE_ENV`; cookie `connect.sid` → `kanpad.sid` / `__Host-kanpad.sid` |
| T55 | `689dc895` | CI moved to the repo root (discoverable at last) + `PUT` in `allow_methods` + the preflight guard |
| T57 | `98fb1ce2` | edition 2024 + toolchain 1.99.1 + `deny.toml` + `[lints]` + release profile |
| T63 | `85f433f3` | the three dead direct dependencies (`axum-valid`, `zeroize`, …) |
| T65 | `55071642` | in-tree `SessionStore` (the hinge: `tower-sessions-sqlx-store` gone) |
| T67 | `a0b25543` | library majors (tower-http 0.7, tower-sessions 0.15, utoipa 6, argon2 0.6, sha2 0.11) |
| T66 | `afd87c0e` | sqlx 0.9 + the collation pin (boot-enforced + test-asserted) |
| T68 | `0b03ea58` | the CI's first-run defect repair (`cargo audit` step deleted, session-schema assert fixed) |
| T69 | `93ca7016` | the frontend's 15 `tsc` errors → 0 (the generated schema is the contract) |
| **T56** | **`ddbcf43b`** | **`CsrfLayer` (Go 1.25 CrossOriginProtection) + canonical `CORS_ORIGINS` boot gate + CSP with boot-computed inline-script hashes + span redaction (46 handlers)** |

### T56 verification (replayed by the orchestrator on the REBASED tree, not trusted from the report)

- `cargo fmt --check` **0** | `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic` **0**
- `cargo test -p backend-rs` → lib **77/77**, api **101/101**, doc 0/0 | `cargo sqlx prepare --check` **0**
- the **four** grep gates, extracted from the workflow and executed verbatim → all **PASS**
- `Cargo.lock` **zero diff** (`csrf` is a dependency-free feature) — the T56 contract is one feature name
- **the CSP hash is genuinely out of band:** the fixture's inline script hashes to `sha256-QMHClxD2…` under an
  independent `python3 hashlib.sha256` **and** is byte-identical to `frontend/index.html:12-21` (same length,
  same digest) — so the constant in the test is a real drift gate, not the code agreeing with itself
- red-proofs (recorded in the T56 bullet of `50-tasks.md`): re-added `instrument(skip(state))` → gate fails
  **and** the span test prints `title`, `description` and `email`; edited fixture → hash mismatch; accepting a
  non-canonical origin → boot succeeds; removing `.layer(csrf)` → 404 instead of 403

### The one thing that is still NOT verified

**The GitHub Actions pipeline has never run.** `gh` is not installed and `dev-rs-local` has no remote
branch, so T55/T68/T62's "green on the PR" ACs stay open. Everything else in them is verified by replaying
each step by hand. Before the first push, fix the **fourth latent red** (T62 owns it):

- `backend-rs/Dockerfile:29` still does `COPY Cargo.toml Cargo.lock sqlx.toml ./` — T66 **deleted**
  `sqlx.toml`, so `docker build` (and the `docker-e2e` job) fails immediately. One-line fix, but it belongs
  to T62 so the Dockerfile has a single owner.

### Next steps, in order (`50-tasks.md` Phase 7)

1. **T58** public-API ergonomics — `Role`/`AppEnv` enums, `AppResult`, `#[non_exhaustive]`,
   `AppError::Validation(Vec<FieldError>)`. Touch: `config.rs`, `error.rs`, `extract.rs`,
   `features/auth/{routes,store,types}.rs`, `features/users/store.rs`, `common/ids.rs`.
2. **T59** shared-kernel DRY — `common::time::format_timestamp` (9 copies → 1), `common::ownership`
   (the empty stub becomes the real SQL owner), drop `db::with_executor`, `/ready` into `ApiDoc`
   (+ the `schema.d.ts` regeneration in the same PR).
3. **{T60 ∥ T61}** observability (`/metrics` on its own `METRICS_PORT`, JSON logs in prod, bounded
   shutdown) and concurrency/pool tuning. They touch `config.rs` in different regions — a real
   conflict is unlikely but rebase one at a time.
4. **T62** coverage gate + CI pins + Docker hardening (**including the `sqlx.toml` Dockerfile red**).

### Working rules that this session proved the hard way

- **ONE backend subagent at a time.** Two in parallel contend for the shared cargo target and hit the free
  model's rate limits; both died mid-task twice (first "model unavailable", then "insufficient funds" on the
  paid sibling). Always **WIP-commit before re-dispatch** — the take-over pattern recovered ~15 minutes of
  work twice, with zero rework.
- **Free models only:** `opencode/deepseek-v4.1-flash-free`. The configured default
  (`minimax-m3.1-flash-free`) is **gone from the provider catalogue**, so every dispatch MUST pass
  `model:` explicitly or it fails instantly.
- **The orchestrator is the single writer** for `50-tasks.md` ACs, `54`'s status table and this file.
  Executors are told not to flip ACs — parallel agents editing the AC table is how it drifted before.
- **Rebase before merge, then re-verify on the rebased tree.** T56 was based on `50a59ef8`; T69 landed in
  between. The rebase was clean (disjoint halves) and the full gate set was re-run afterwards — the merge
  commit is the tree that was actually tested, not the branch that was reported.

## Resume 2026-10-23 — `gh` installed; the pipeline HAS run (T70 fixes its red)

- **`gh` is installed and authenticated** (2.103.0, account `instrumaniak`, keyring, ssh protocol,
  scopes `repo`/`read:org`). That immediately falsified a claim in this file: **the pipeline is not
  "never run" — it has run twice on `dev-rs`, both `failure`**:
  `39795598592` (2026-10-22T14:25Z) and `39817195098` (16:49Z). `dev-rs` == `origin/dev-rs` ==
  `dev-rs-local` == `396b2d8b`, and the workflow is registered (`backend-rs CI`, id `392991536`).
- **T70 DONE `7800985c`** (NOT pushed — the user asked for a local commit only): the clippy step now
  carries `SQLX_OFFLINE: "true"`. Both runs died at step 2 with 90 × `1146 Table 'kanpad_ci.X' doesn't
  exist` because `clippy` expands the 83 `query!` macros **before** `migrate run` (step 5) and the job
  had no `SQLX_OFFLINE`. Red/green reproduced locally on an empty DB (90 × 1146, exit 101 → exit 0).
  Because `web: needs: [backend]` and `docker-e2e: needs: [backend, web]`, **2 of 3 jobs have still
  never executed** — the frontend gates and the Docker build remain unproven.
- **NEXT: T62 IS DONE (`efb7bf52`, pushed `dev-rs` → `origin/dev-rs`; `gh run`
  `40410191100` in progress).** The `sqlx prepare --check` red (first CI
  execution of this gate ever — recovered by T62's `SQLX_OFFLINE: "true"`) was
  the **DB-binding trap**: sqlx 0.9 binds `.sqlx/` query files to DB-qualified
  table names (`kanpad.` dev / `kanpad_ci.` CI); without `SQLX_OFFLINE`, the
  gate regenerates against `kanpad_ci.` then fails against the committed
  `kanpad.` files. Fix: `env: SQLX_OFFLINE: "true"` on the step (restores the
  gate's original intent). Evidence listed in `50-tasks.md`. `docker-e2e` and
  `web` still need their first execution (they were always skipped because
  `backend` failed first).

- **PREDICTED NEXT RED (recovered by T62):** `docker-e2e` — once backend is
  green — the `Dockerfile:29` `COPY ... sqlx.toml` (file deleted by T66) is
  fixed by T62 (commit `e2ec5773` removes the line). `HEALTHCHECK /health`
  (DB-free; uses `$PORT`) and digest-pinned bases (`rust` + `debian-slim`)
  also land with T62 (`e2ec5773`). `.dockerignore` excludes `target/`
  (1.2 GB context reduction) and `.env`, keeps `.sqlx/` + `tests/` + `public/`.

- **Gates re-verified today on `396b2d8b` + T70 + T62** (DB `kanpad-rs-testdb`
  restarted, tmpfs → `applied: 2 pending: 0`): `fmt` 0 | `clippy -D warnings -A
  clippy::pedantic` 0 | lib **93/93** (post-dep-bump, `crypto-bigint` 0.7.10 +
  `ctutils` 0.4.6) | api **108/108** | 4 grep gates OK | `sqlx prepare --check`
  0 (`SQLX_OFFLINE` set) | `migrate info` applied: 2 pending: 0 | openapi snapshot 0
  diffs; vitest **723/723** (77 test files, 70.97s); `dockerignore` verified
  (context shrinks from 1.2 GB → 1.6 MB, excludes `target/`, `.env*`, keeps
  build inputs); `dependabot.yml` covers cargo/npm/github-actions.

- **NEXT TASK: T58** (`public-API ergonomics`: `AppEnv`/`Role` enums, `AppResult`,
  `#[non_exhaustive]`, `AppError::Validation(Vec<FieldError>)`). Worktree `KanPad-wt/wt-t58`,
  branch `rs-t58-ergonomics`. Then T59 → {T60 ∥ T61, one at a time} → T62.
  **`src/common/time.rs` does not exist yet, `ownership.rs` is still a 1-line stub, `format_timestamp`
  is still duplicated 9× — that is T59's whole point, do not "tidy" it during T58.**

## T58 merged 2026-10-23 — the public error API stops leaking `garde` and stringly values

- **Three slices, one branch (`rs-t58-ergonomics`, worktree `KanPad-wt/wt-t58`), FF-merged:**
  `02bbffcf` `AppEnv` enum (manual `Deserialize` keeps the `APP_ENV must be one of …` boot message;
  the `validate()` whitelist is gone because the type cannot hold a bad value) →
  `5d4b4221` `Role { User, Admin }` in `common::ids` (`AuthSession`/`StoredUser`/`UserData`;
  `UserData.role` keeps `#[schema(value_type = String)]`, so **`openapi.json` is byte-identical** and
  no `Role` component appears; `role_from_db` turns an unknown VARCHAR into 500) →
  `cdc05c48` `AppError::Validation(Vec<FieldError>)` + `impl From<garde::Report>` +
  `AppResult<T>` + `#[non_exhaustive]` (the rendered `detail` is rebuilt exactly as garde's
  `Report` Display did, so the problem body is unchanged).
- **Gates:** `fmt` 0 | `clippy … -D warnings -A clippy::pedantic` 0 | lib **77/77** | api **101/101** |
  `sqlx prepare --check` 0 (**no `.sqlx/` change** — no SQL text moved) | `openapi_snapshot` 3/3
  (`openapi.json` untouched). `53f477a1` records the four ACs in `50-tasks.md`.
- **Touch-list deviations (mechanical only):** `tests/api/{helpers,auth_session,users}.rs` fixtures,
  one `#[cfg(test)]` `AuthSession` literal in `cards/routes.rs`, and the `AppError::Validation(report)`
  call sites in `extract.rs`, `columns/routes.rs`, `notes/store.rs`, `cards/routes.rs` → `AppError::from`.
  A public enum variant's type change cannot be contained to the files that name the variant.
- **Progress:** 53/58 deliverables ≈ **91.4%**; Phase 7 **12/16** (T54–T70, no T64).
- **NEXT: T59** — shared-kernel DRY (`common/time.rs` owns `format_timestamp`; delete the 8 copies),
  `common/ownership.rs` owns the 4 `check_*_owned` helpers, delete `db::with_executor`, collapse the
  duplicate `AppError::Json` arm, `problem_response(impl Into<String>)`, **`/ready` into `ApiDoc`**
  (the phase's only `openapi.json` + `schema.d.ts` path change), and replace the hand-rolled
  `_sqlx_migrations` journal in `main.rs` with `sqlx::Migrator`. Slice it: time → ownership → error/db/main
  → `/ready` + regen.

## T59 merged 2026-10-23 — one owner for time + ownership, and `/ready` documented

- **Five commits, one branch (`rs-t59-shared-kernel`, worktree `KanPad-wt/wt-t59`), FF-merged:**
  `a5fe7634` `common::time::format_timestamp` is the single definition (the other 9 copies became
  `pub use` re-exports, so every `use super::types::{…}` site compiled untouched) →
  `2464fb92` + `ce5fe500` `common::ownership` now owns **six** `check_*_owned` helpers
  (board/card/column/label/project/tags): the AC is "none in `src/features/`", so the duplicates in
  `columns`, `notes` and `checklists` moved as well, not just cards/boards. SQL text identical, so
  **`.sqlx/` stayed byte-identical** (sqlx normalizes whitespace in the query hash) →
  `f7e9cde3` deleted `db::with_executor` (zero call sites), collapsed the two identical `Json` arms,
  widened `problem_response` to `impl Into<String>` → `c6f11995` documented `GET /ready`
  (`ReadyResponse` 200 / `ProblemDetails` 503) and regenerated `openapi.json` **and**
  `schema.d.ts` in the same commit.
- **(g) was already done:** `main.rs` has used `sqlx::migrate!` + `Migrate::list_applied_migrations`
  since T66 (`afd87c0e`) — the hand-rolled `_sqlx_migrations` journal is gone. Do not re-open it.
- **Gates:** `fmt` 0 | `clippy … -D warnings -A clippy::pedantic` 0 | lib **78/78** | api **101/101**
  (snapshot 3/3) | `sqlx prepare --check` 0 | `gen:api --check` "up to date" | `tsc --noEmit` 0.
  AC greps: `format_timestamp` definitions **1**, `check_*_owned` in `src/features/` **0**.
- **Progress:** 54/59 deliverables ≈ **91.5%**; Phase 7 **13/16**.
- **NEXT: T60** — observability. `tracing-subscriber` `json` feature selected by `AppEnv`;
  `/metrics` (Prometheus text) on a **separate `METRICS_PORT` listener** (unset = no endpoint,
  fail-closed), route label from `MatchedPath` (never the raw URI; 404/405 → `"unknown"`),
  no scrape feedback loop, `kanpad_db_pool_*` from `Pool::size()/num_idle()` + our own
  `acquire_timed()` histogram (sqlx has no pool metrics), throttle-rejection counter, process
  metrics, and a bounded (10 s default) graceful pool close. Touch: `Cargo.toml`, `Cargo.lock`,
  `lib.rs`, `main.rs`, `state.rs`, `config.rs`. Then T61 (deadlock retry 1213/1205 + READ COMMITTED
  before `BEGIN` + `DB_MAX_CONNECTIONS`) and T62 (coverage floor, SHA pins, `.dockerignore`,
  `HEALTHCHECK`, and the stale `Dockerfile:29` `sqlx.toml` COPY that will be the next CI red).

## T60 merged 2026-10-23 — `/metrics` on its own listener, pattern labels, JSON-in-prod

- **Four commits, one branch (`rs-t60-observability`, worktree `KanPad-wt/wt-t60`), FF-merged:**
  `4d9be5c9` the registry (`src/metrics.rs`, ~490 lines with 10 unit tests) + `METRICS_PORT`
  (`Option<u16>`, defaults `None`, `0` rejected) → `4bc888cc` wiring (`AppState.metrics`,
  `acquire_timed()`, `track_http_metrics`, `metrics_router`, `tests/api/metrics.rs` 5 tests) →
  `b564d381` the separate listener, the 10 s bounded `pool.close()`, and JSON logs →
  `dd3ba477` the six ACs recorded in `50-tasks.md`.
- **No new crate, deliberately.** A process-global recorder (the `metrics` crate's model) cannot be
  isolated per test — the integration suites build dozens of apps in one process — and it would make
  the "a scrape does not record itself" AC unprovable. `metrics::Metrics` is a per-`AppState`
  `BTreeMap` + atomics: request counter keyed by (method, matched pattern, status), two fixed-bucket
  histograms, an in-flight gauge, a throttle counter, and best-effort `/proc` process series.
- **The route label is `MatchedPath`, never `Uri::path()`.** `999` and `888` fold into ONE
  `route="/api/v1/cards/{id}"` series (asserted); an unmatched request is `"unknown"` (a `/*`
  fallback pattern is filtered too). A raw path would let any client mint unbounded series.
- **Layer placement is load-bearing.** `track_http_metrics` is the OUTERMOST layer of the shared
  stack but still applied via `Router::layer` (so routing has already set `MatchedPath`) and it sits
  OUTSIDE the timeout layer: a timed-out handler future is dropped, so an in-flight increment inside
  the timeout would never be paired with its decrement.
- **Fail closed:** `metrics_addr()` is `None` unless `METRICS_PORT` is set; `main.rs` binds a second
  socket only then. The main app 404s `/metrics` (asserted), and the metrics router has no tracking
  middleware and is never merged into `create_app`.
- **Logs:** `tracing-subscriber` gained `json`; `log_json`/`init_tracing`/`init_tracing_from_env`
  live in the library. `main` picks the format from a LENIENT `APP_ENV` parse BEFORE
  `Settings::load()` — a bad `DATABASE_URL` must still be logged in the right format — while
  `Settings::load()` keeps the strict rejection.
- **Live probes (not just tests):** test env `PORT=13001 METRICS_PORT=19090` → `/health` 200, main
  `/metrics` 404, `/api/v1/cards/999` 401 recorded as the pattern, `unknown` 404 series, pool gauges,
  acquire histogram, human logs, `database pool closed`. Production `13002/19091` → **6/6** log lines
  parse as one JSON object each.
- **Gates:** `fmt` 0 | `clippy … -D warnings -A clippy::pedantic` 0 | lib **90/90** | api **106/106** |
  `sqlx prepare --check` 0 (`.sqlx/` unchanged — no SQL text moved).
- **Touch-list deviations (justified in the ledger):** new `src/metrics.rs`; `src/db.rs` for the
  `acquire_time_level(Info)` line the spec itself requires; `tests/api/{metrics.rs,api.rs}`.
  (`log::LevelFilter` has `Info`, not `INFO`; `WithGracefulShutdown` is spawned directly.)
- **Progress:** 55/60 deliverables ≈ **91.7%**; Phase 7 **14/16**.
- **NEXT: T61** — `move_card` retry must match MySQL error **NUMBERS** (`1213` AND `1205`; `1205` is
  SQLSTATE `HY000`, so a `40001`-only match silently misses lock-wait timeouts), explicitly
  `ROLLBACK` the WHOLE transaction (a `1205` rolls back only the statement), run the move tx at
  `READ COMMITTED` issued **before** `BEGIN` (`SET TRANSACTION …` inside a tx is error **1568**;
  `SET SESSION …` leaks onto the pooled connection), env-driven `DB_MAX_CONNECTIONS`/
  `DB_MIN_CONNECTIONS` (10/1), and a test that proves the retry path is TAKEN. Then T62 (the stale
  `Dockerfile:29` `sqlx.toml` COPY is the next guaranteed CI red).

## T61 verified 2026-10-23 — transient lock retry + per-transaction isolation

- T61 was already in the current branch (`81f57106`, `0cb7366f`, `24c5f167`, reconciled by `aff43567`): pool sizing uses `DB_MAX_CONNECTIONS` / `DB_MIN_CONNECTIONS` (defaults 10/1); card moves set one-shot `READ COMMITTED` before `BEGIN`; MySQL error numbers 1213 and 1205 trigger bounded retries after whole-transaction rollback and short backoff.
- Verification: `cargo test --test api cards_move -- --nocapture` → **3 passed, 0 failed**. The full coverage run also passed `pool_sizing_defaults_and_env_keys` and `pool_options_honour_settings` before it was stopped.
- T61 acceptance criteria are checked in `50-tasks.md`. The remaining planned task is T62; work stopped here as requested.

## 2026-10-27 — docker build fixed (`cargo --offline` was never possible)

- **CI run `40425148777`** (`dev-rs`, commit `237caa24`): `backend` ✓ (8m4s), `web` ✓ (49s), `docker-e2e` ✗ at `docker build backend-rs` — the FIRST time that build had ever run (T06 had substituted it with a code review because `docker build` is disk-infeasible locally: `/` is 98% full, 2.4 G free).
- **Root cause:** `Dockerfile:46` was `RUN cargo build --locked --offline --release`. `rust:1.99-bookworm@sha256:48b7f435…` ships `CARGO_HOME=/usr/local/cargo` with an EMPTY `registry/`, and the build context ships no vendored crates, so offline cargo fails at dependency RESOLUTION: `error: no matching package named 'anyhow' found` / `location searched: crates.io index` / exit 101, 0.09 s in. Reproduced on the host with `CARGO_HOME=<empty> cargo metadata --offline --locked` → same message, same 101.
- **Fix (this commit):** `RUN cargo build --locked --release` (network enabled, keep `--locked`). Reproducibility = committed `Cargo.lock` (360 packages / 359 checksums) + digest-pinned bases; `--offline` was an unverified aspiration, never the integrity mechanism. `ENV SQLX_OFFLINE=true` STAYS — different axis: `query!` compiles from the committed `.sqlx/` with no DB. `--offline` on the `openapi snapshot` step STAYS (that job restores `~/.cargo/registry` via `Swatinem/rust-cache`).
- **Gate re-aimed, and proven red:** the `docker-e2e` contract gate used to `grep -q -- '--locked --offline'` (i.e. it ASSERTED the broken command). It now requires `cargo build … --locked` AND fails if `--offline` reappears; verified by injecting `--offline` into a Dockerfile copy (gate exits 1) and against the real file (exits 0).
- **No cache mounts.** Dockerfile reference: cache mounts are "not included in exported build cache (`--cache-to`), and are not restored by `--cache-from` … on CI runners that provision a fresh machine for each job, cache mounts start empty". With `setup-buildx-action` provisioning a fresh builder per job they would be a no-op here (the workaround is the extra `reproducible-containers/buildkit-cache-dance` action). Build speed, if it ever matters, comes from the layer cache the job already exports (`cache-to: type=gha,mode=max`).
- **Docs corrected in the same patch** (P2 rule): `50-tasks.md:64` (T06 AC), `50-tasks.md:363` (T62 record), `10-architecture.md:100` (CI chain) all recorded `--locked --offline` as a PASSING gate. The `Dockerfile:29` line pin in the T62 record was also stale.
- **Resume/first-run risk:** everything after the build had never executed either — `Image must not bake DATABASE_URL`, `Serve :3001 + smoke`, `npm ci`, `vitest`, `playwright pinned`. The smoke is designed to pass unmigrated (pool is `connect_lazy`, `/health` is DB-free, `/api-docs/openapi.json` is `include_str!`, `public/` was populated by the `web` job ✓). Watch the first release build of 360 crates (`lto = "thin"`, `codegen-units = 1`) — it is the slowest step in the pipeline.

## 2026-10-27 — the whole pipeline is GREEN (run `40431691743`, `0ab55b05`)

- **All three jobs passed:** `backend` 5m39s, `web` 44s, `docker-e2e` **6m37s** — the FIRST fully green `backend-rs CI` run. Every previously-unrun step executed for the first time and passed: `Dockerfile contract gates` (the re-aimed one), `docker build backend-rs`, `Image must not bake DATABASE_URL`, `Serve :3001 + smoke test`, `npm ci`, `vitest`, `playwright pinned`.
- **The release build is not a problem:** `#20 [builder 8/8] RUN cargo build --locked --release` compiled **293 crates** and finished in **2m54s** (`lto = "thin"` + `codegen-units = 1` included). So the layer-cache/cargo-chef optimization discussed during the fix is UNNECESSARY — the job already exports `cache-to: type=gha,mode=max`, and a cold build fits comfortably in the budget.
- **The unmigrated-DB prediction held:** the container booted against `kanpad_ci` with no migrations and the smoke passed (`/health` DB-free, `openapi.json` embedded, `/app/public/index.html` present). Still true: any future smoke that touches `/api/v1/*` needs `migrate run` in that job first.
- **Remaining CI noise (non-blocking):** GitHub annotations only — Node 20 deprecation for the SHA-pinned actions, and the `ubuntu-latest` → Ubuntu 26 migration notice (2026-10-19). Neither fails a job.
