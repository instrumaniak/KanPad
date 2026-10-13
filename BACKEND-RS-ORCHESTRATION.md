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
- `clear_sessions()` DELETED (compile error on reintroduction). No global DELETE/UPDATE-all/DROP on `tower_sessions`, ever. Store assertions via set difference only. Stale "coordinated wipe" comments remain in 6 suite files (comments only, no calls — verified by grep).

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
