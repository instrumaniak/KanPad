# T020: Tx-retry helper for position-shifting transactions

> **STATUS: pending.** Wave 1. Closes **R4** (MEDIUM). **Depends on T010** (locked
> decision D-5: T010's fractional sort keys rewrite these write paths first and extend
> the matcher to `1062`; this task then extracts the helper and covers what remains).

## Goal

Deadlock/lock-wait (`1213`/`1205`, scoped `1062`) retry exists inline in **six**
write sites post-T010: `create_card` and `update_card`'s move worker
(`cards/routes.rs:112,325`), checklist-item create/update (`checklists/routes.rs:260,359`),
column update (`columns/routes.rs:192`), and bulk `move_cards` (`columns/routes.rs:348`).
Transactions that still shift positions or take `FOR UPDATE` single-shot → fall into the
generic-500 arm (`error.rs`) → client-visible 500 under concurrent use: `delete_card`,
`delete_checklist`/`delete_item`, `create_column`/`delete_column`, notes/boards/labels/tags,
`common/rebalance.rs` (32 `FOR UPDATE` sites across 7 files — re-enumerate, don't assume).
*(Amendment 2026-11-16: the pre-T010 scope note listing `create_card`/`move_cards` et al. as
single-shot was stale — corrected against the tree.)*

1. Extract a **shared kernel helper** (`src/common/tx_retry.rs`): bounded attempts
   (5), linear backoff, explicit rollback between attempts, optional
   `SET TRANSACTION ISOLATION LEVEL READ COMMITTED` before `BEGIN`, retrying on
   `is_transient_lock_error` (**1213 | 1205 | 1062** — post-T010 matcher).
2. Apply it to the remaining lock-taking transactions: `delete_card` and whichever
   single-shot sites a fresh `FOR UPDATE` enumeration finds (notes/boards/labels/tags,
   checklist/column deletes, `common/rebalance.rs`) — the six inline sites switch to
   the helper behavior-preserving; bulk `move_cards` is **not** single-shot (already
   retried). Audit of every remaining `tx.begin()` site — each gets the helper or a
   comment why not.
3. **Exhausted retries → 503 + `Retry-After`** (busy), not generic 500.

## Depends On

**T010** (fractional sort keys — do not start before it merges; both touch
cards/columns write paths).

## Phase

1

## Critical

Yes — highest-probability source of user-visible 500s under concurrent use at 100+
users (D-2).

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Retry helper (post-T010 scope,
  design, edge cases, test plan — amended 2026-11-16, pass 2: six inline sites).
- `specs/arch-hardening-specs/00-overview.md` — finding R4 (re-derived 2026-11-16); **D-5 (T010 → T020)**;
  global DoD.
- `backend-rs/src/features/cards/routes.rs:325` (`update_card`'s move worker — the
  reference implementation to generalize) and `:112` (`create_card`),
  `backend-rs/src/error.rs` (generic-500 arm), `backend-rs/src/features/columns/routes.rs:348`
  (bulk move, already retried), T010's task file (`specs/tasks/T010-*.md` — matcher extension to 1062,
  lock semantics it removed).
- `backend-rs/tests/README.md` (harness rules), `backend-rs/tests/api/cards_move.rs`
  (existing race/fuzz suite).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/common/tx_retry.rs` | **NEW** — `with_tx_retry` helper + unit tests |
| `backend-rs/src/common/mod.rs` | Register module |
| `backend-rs/src/features/cards/routes.rs` | Replace inline retry in `move_card` and `create_card` with the helper (behavior-preserving); apply to remaining single-shot sites |
| `backend-rs/src/features/columns/routes.rs`, `.../checklists/routes.rs` | Switch the four existing inline retry loops (`move_cards`, column update, checklist-item create/update) to the helper — no behavior change |
| Audit: boards/columns/notes/checklists/projects/auth `tx.begin()` sites | Helper or explanatory comment (one-line each) |
| `backend-rs/src/error.rs` | Exhausted-retry → 503 + `Retry-After` mapping |
| `backend-rs/.sqlx/` | Regen if queries change (pinned CLI, MySQL, db `kanpad` — T009) |
| `backend-rs/tests/api/` | Injected-lock-error recovery test; exhausted → 503; existing suites stay green |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** gap-lock logic T010 deleted (no regressions to
fractional keys), `_sqlx_migrations`, Argon2/tx ordering changes (hashing stays
outside txs).

## Implementation Steps

TDD throughout.

1. **Pre-flight:** confirm T010 merged; note its inline retry + matcher (`1213|1205|1062`).
2. **Red test:** hold `FOR UPDATE` on a target row from a second connection → attempt
   `create_card` (or chosen endpoint) → today: 500; helper must retry and succeed after
   the lock releases (failpoint or timed lock-release harness).
3. **Red test:** never-releasing lock → exhausted attempts → **503 + Retry-After**
   (today: 500).
4. Implement `tx_retry.rs` (bounded 5, linear backoff ≪ 30 s request timeout — compute
   and document worst case, explicit rollback, optional `READ COMMITTED`).
5. Swap `move_card`'s inline loop to the helper (existing `cards_move.rs` race/fuzz
   suites must stay green — behavior-preserving refactor).
6. Six existing inline sites switch to the helper (behavior-preserving refactor);
   apply to `delete_card` and remaining single-shot sites found by enumeration; audit all
   `tx.begin()` sites (comment each pass/skip decision).
7. Gates: fmt, clippy `-D warnings -A pedantic`, full suite MySQL + MariaDB,
   `sqlx prepare --check`; persistence assertions per touched endpoint (AGENTS.md).
8. Lesson.

## Constraints

- **T010 first (D-5)** — no parallel work with any cards/columns write-path task.
- Retries wrap **only in-tx effects** (assert by review — no non-idempotent side
  effects outside the tx).
- Same-card race keeps T010's `Restart` short-circuit (logical conflict ≠ lock
  conflict — don't retry it).
- Worst-case retry time must fit the 30 s request timeout.
- Both MySQL 8.4 + MariaDB 11.4 green (the CI matrix legs; 10.6 only if T011 confirms
  prod runs it — see `specs/arch-hardening-specs/40-operations.md` §6); harness rules
  (explicit imports, `assert_problem` labels); no commit without approval.

## Acceptance Criteria

- [ ] `with_tx_retry` exists with unit tests (bounded attempts, backoff, rollback
      called between attempts, matcher includes `1213|1205|1062`).
- [ ] Injected-lock test: operation recovers via retry (red today — pasted).
- [ ] Exhausted retries → 503 + `Retry-After`, not 500 (red-proof pasted).
- [ ] `create_card`/`delete_card`/`update_card`/bulk `move_cards` use the helper;
      every other `tx.begin()` site audited with a comment (closes **R4**).
- [ ] `cards_move.rs` race/fuzz suites green unchanged; full suite green on MySQL +
      MariaDB; `sqlx prepare --check` 0; fmt/clippy 0.
- [ ] Persistence assertion per touched endpoint; lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
