# Backend-rs Orchestration — pause/resume state

> Manager: orchestrator. Executors: subagents in git worktrees.
> Source of truth for behavior: `specs/api-rewrite-rs-specs/50-tasks.md` (one task = one PR).
> This file is the pause/resume pointer. Update it on every task complete/merge.

## Base
- Base branch: `dev-rs-local` (commit `f2a0cc0` specs)
- Worktree root: `/tmp/opencode/kanpad/` (preferred over /tmp)
- Merge target: `dev-rs-local` (squash or merge --no-ff per task, then push only on demand)
- Resume: `git worktree list` + this table + `git log --oneline -20` on `dev-rs-local`

## Wave 1 (in-progress)
| Task | Branch | Worktree | Depends | Status | Subagent session |
|------|--------|----------|---------|--------|------------------|
| T01 contract+compose | `rs-t01-compose` | `/tmp/opencode/kanpad/wt-t01` | none | MERGED be37720 | ses_ed6b464b8ffe0k6NoBgUExmLOh |
| T03 crate skeleton | `rs-t03-skeleton` | `/tmp/opencode/kanpad/wt-t03` | none (parallel w/ T02) | running (ses_ed6b4648cffeTTMKAFtdD0GrNB) — on pre-T01 base, merge later, no file conflict expected | ses_ed6b4648cffeTTMKAFtdD0GrNB |
| T02 baseline DB | `rs-t02-baseline` | `/tmp/opencode/kanpad/wt-t02` | T01 DONE (rebased onto ae7f787) | dispatched | ses_ed6b1914fffe25Qs2hjPhFWyhq |

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
