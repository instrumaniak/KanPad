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
- Scratch containers stopped (`kanpad-t02-scratch`, `kanpad-t02-mysql`); T02
  datadir bind-mount was at `/mnt/mint/.../.mysql-t02` (outside repo, verify
  removal on next disk cleanup).

## Wave 2 (T04+T06+T07 parallel; T05+T08 after T04 merges)
| Task | Branch | Worktree | Depends | Status | Subagent session |
|------|--------|----------|---------|--------|------------------|
| T04 shared kernel | `rs-t04-kernel` | `KanPad-wt/wt-t04` | T03 DONE | dispatched | ses_ed649d952ffecL08IPMMU1UcCL |
| T06 CI + .sqlx + docker | `rs-t06-ci` | `KanPad-wt/wt-t06` | T03 DONE | dispatched | ses_ed649d77bffedDan6nSONLwTKq |
| T07 fixtures + seed | `rs-t07-fixtures` | `KanPad-wt/wt-t07` | T02+T03 DONE | dispatched | ses_ed649d777ffeSB3nv6SKKWepOj |
| T05 config/obs/versioning | — | — | T03 (+T04 if lib.rs collides) | HOLD until T04 merges | — |
| T08 validation core | — | — | T04 | HOLD until T04 merges | — |

## Stale sessions (closed, no action)
- `ses_ed6b1914...` (pre-restart T02) reported post-merge: verified merge, changed
  nothing. Cross-checks from its report: store version in T03 `Cargo.lock` is
  exactly `0.15.0` (matches T02's DDL reference — no checksum risk); Nest-tip
  dump confirms `utf8mb4_unicode_ci` + all §30 index names present.
- Dispatch hygiene (from its duplicate-dispatch note): one live executor per
  task; re-dispatch only after prior session is confirmed dead + worktree state
  inspected. Applied to wave 2.

## Lessons (orchestrator fixes applied during T03 finish)
- Reused target dir moved across paths breaks utoipa-swagger-ui build (stale
  OUT_DIR in embed.rs `#[folder]`): fix with `cargo clean -p utoipa-swagger-ui`.
- `tests/api.rs` suite mods need `#[path = "api/<name>.rs"]` (bare `mod x;`
  resolves to `tests/x.rs`). Noted in-file; future suite tasks must follow it.

## Queue (do not start until gate green)
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
