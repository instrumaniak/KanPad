# Backend-rs Orchestration — pause/resume state

> Manager: orchestrator. Executors: subagents in git worktrees.
> Source of truth for behavior: `specs/api-rewrite-rs-specs/50-tasks.md` (one task = one PR).
> This file is the pause/resume pointer. Update it on every task complete/merge.

## Base
- Base branch: `dev-rs-local` (T01 merged `be37720`)
- Worktree root: `/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/KanPad-wt/` (PERSISTENT volume — /tmp/opencode was wiped by 2026-10-11 server restart, losing uncommitted T03 skeleton + T02 progress; never use /tmp for worktrees)
- Shared cargo target cache (deps prebuilt): `/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-target-kanpad` — every Rust command MUST export `CARGO_TARGET_DIR=<that>` (+ `CARGO_HOME=/mnt/mint/home/raziur/CodeLab/Personal/AITestLab/.cargo-home` for installs; root disk / is 98% full)
- Merge target: `dev-rs-local`. Resume: `git worktree list` + this table + `git log --oneline -20`.

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
- Scratch containers stopped (`kanpad-t02-scratch`, `kanpad-t02-mysql`); T02
  datadir bind-mount was at `/mnt/mint/.../.mysql-t02` (outside repo, verify
  removal on next disk cleanup).

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
| T23b columns sort | `rs-t23b-sort` | `KanPad-wt/wt-t23b` | T23a | dispatched | ses_ed49da16dffeOB97WpPRCPOnle |
| T23c columns move-cards | `rs-t23c-move` | `KanPad-wt/wt-t23c` | T23a | dispatched | ses_ed49da169ffewmXhihpb6RGj93 |
| T24a cards CRUD+list | `rs-t24a-cards` | `KanPad-wt/wt-t24a` | T02+T04+T07+T08+T11+T23a | dispatched | ses_ed49da151ffePjTVdFQJjRcDsj |

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
