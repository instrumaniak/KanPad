# T020: Tx-retry helper for position-shifting transactions

> **STATUS: pending — amended 2026-11-18 (review pass).** The `position-shifting`
> name predates T010 and is stale: fractional `sort_key`s (T010, merged
> `c62de576` 2026-11-15) eliminated position shifting. The filename and
> `status.yaml` slug are retained for traceability; the behavior target is the
> remaining single-shot lock-taking transactions. Wave 1. Closes **R4** (MEDIUM).
> **Depends on T010** (locked decision D-5) — **satisfied**: T010 is merged and
> `status.yaml` records it `done`; `specs/tasks/status.yaml` still records this
> task `in_progress`, so reconcile the taskpad when work actually starts.

## Goal

Deadlock (`1213`) / lock-wait (`1205`) retry with constraint-`1062` scoping
exists inline in **six** write sites post-T010 (all verified at the stated
lines 2026-11-18): `create_card` and `update_card`'s move worker
(`cards/routes.rs:112,325` — there is no `move_card` function; the move path is
a worker inside `update_card`), checklist-item create/update
(`checklists/routes.rs:260,359`), column update (`rename_column_handler`,
`columns/routes.rs:192`), and bulk `move_cards` (`columns/routes.rs:348`).

Single-shot transactions fall into the generic-500 arm (`error.rs:386-393`,
plus `:411-418`) → client-visible 500. The **only un-retried transaction that
re-keys rows** is `create_column` (`columns/routes.rs:134`): serialized
`LIMIT 1 FOR UPDATE` append + full-board `rebalance_sort_keys` (phase-0
`FOR UPDATE` of every column row) — the highest-value target of this task.
The remaining single-shot txs take no `FOR UPDATE` and shift no positions, but
are deadlockable via row/FK-cascade lock acquisition: `delete_card`,
`delete_checklist`/`delete_item`, `delete_column`, `delete_note`,
`delete_board`, `delete_project`, scalars-only `update_card`, notes patches,
`register` / session-store insert. Labels/tags have **no** transaction sites;
`boards::create_board` and auth writes are in-tx but lock-free — audit and
comment, don't force-fit.

Raw `FOR UPDATE` grep = 32 literals across 7 files, but that collapses to
**11 real lock sites in 4 files** (5 literals are doc comments) — most already
retried. Re-enumerate at implementation time; don't trust the "32" figure.

Amendments vs the pre-review text (2026-11-16): dead-code `move_card` naming,
a non-red `create_card` red test, the impossible never-releasing-lock exhaustion
test (deadlock-wait default `innodb_lock_wait_timeout` = 50 s ≈ 250 s for five
attempts — it can never finish inside CI; 1205 effectively rides the 30 s
`TimeoutLayer`, see Constraints), the missing `create_column` focus, and the
undocumented exhausted-retry → 503 mechanics are all corrected below.

1. Extract a **shared kernel helper** (`src/common/tx_retry.rs`), built on the
   existing `src/common/retry.rs` (`MAX_ATTEMPTS = 5`, `is_transient_lock_error`
   — never a duplicated matcher): bounded attempts, linear base backoff
   (jitter optional — see Design decision 6), explicit rollback between
   attempts, optional `SET TRANSACTION ISOLATION LEVEL READ COMMITTED`
   before `BEGIN`, retrying on `is_transient_lock_error`
   (**1213 | 1205 | constraint-scoped 1062 on `UQ_*_sort_key`** — never
   workspace-wide 1062).
2. Switch the six inline sites to the helper (behavior-preserving). Apply the
   helper to the single-shot lock-taking txs: **`create_column` (required)**
   plus the delete/scalars transactions found by audit:
   `delete_card`, `create_column`/`delete_column`, checklist deletes, notes,
   boards, projects, `register`. Audit of **every remaining `tx.begin()`
   site** — each gets the helper or a one-line comment why not
   (`labels`/`tags`: no tx sites at all; `common/session_store.rs`: has its
   own id-collision loop; `admin.rs`: CLI, 1062→exit 1 deliberately;
   `common/rebalance.rs`: library called inside the retried closure — helper
   belongs to the enclosing tx, not here). Bulk `move_cards` migrates onto the
   helper behavior-preserving; it is not a new retry target.
3. **Exhausted retries → 503 + `Retry-After: 2`** (busy), not generic 500 —
   boundary classification in `error.rs` (see Design decision 1). Scoped-1062
   exhaustion stays **409** (see Design decision 2).

## Depends On

**T010** — **satisfied**: merged `c62de576` (2026-11-15); fractional
`sort_key`s, gap-lock logic removed (application level), scoped-1062 matcher
in `common/retry.rs`. Step-1 pre-flight in this task is a re-verification, not
a gate.

## Phase

1

## Critical

Yes — highest-probability source of user-visible 500s under concurrent use at
100+ users (D-2).

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Retry helper (post-T010 scope,
  design, edge cases, test plan) — scope contradictions there
  (`:131` "not `move_cards`", `:146` "do not re-touch the six sites", reference
  naming at `:116`) are fixed by this task (same PR).
- `specs/arch-hardening-specs/00-overview.md` — finding R4 (re-derived
  2026-11-16); D-5; global DoD (frontend gates + coverage floor apply).
- `specs/arch-hardening-specs/40-operations.md` §6 — CI legs test MySQL 8.4 +
  MariaDB 11.4; a 10.6 leg only if the T011 drill records prod as 10.6 (it may
  instead record a written accepted delta, or confirm prod already matches CI).
- `CONTRACT-CHANGES.md` §Concurrency (D3) — retry semantics; this task appends
  the exhaustion-behavior paragraph (same PR).
- `backend-rs/src/common/retry.rs` (matcher + `MAX_ATTEMPTS`),
  `backend-rs/src/features/cards/routes.rs` (`:112` `create_card`, `:325`
  move worker — the reference implementation to generalize),
  `backend-rs/src/error.rs` (generic-500 arm `Db`, 503 precedents:
  `Timeout` → 503 + `Retry-After: 30`, pool errors → 503, 429 + jitter pattern),
  `backend-rs/src/common/rebalance.rs` (`phase_read` 3 query sites).
- `specs/api-rewrite-rs-specs/54-rust-idiom-hardening.md` D6 — "1205 is rarely
  reached end-to-end (50 s lock wait vs 30 s request timeout)".
- `backend-rs/tests/README.md`; `backend-rs/tests/api/cards_move.rs`
  (T61 fault injectors at `:845`,`:888`; race/fuzz §13/§14),
  `backend-rs/tests/api/sort_key_ordering.rs` (`install_fault_injector` at
  `:1041`).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/common/tx_retry.rs` | **NEW** — `with_tx_retry` helper (on `retry.rs`) + unit tests |
| `backend-rs/src/common/mod.rs` | Register module |
| `backend-rs/src/common/retry.rs` | Reuse only (no changes expected; matcher/constant stay here) |
| `backend-rs/src/error.rs` | Db arm: `1213`/`1205` → 503 `service-unavailable` + `Retry-After: 2` (`BUSY_RETRY_AFTER`); unit tests (status, header, no driver text leak in body); scoped-1062 stays 409 |
| `backend-rs/src/features/cards/routes.rs` | Six sites → helper (`create_card`, update_card's two paths incl. scalars-only patch `:285`); **delete_card** gets helper |
| `backend-rs/src/features/columns/routes.rs`, `.../checklists/routes.rs` | Inline loops (`rename_column_handler`, `move_cards`, checklist-item create/update) → helper; **create_column**, `delete_column`, `delete_checklist`, `delete_item` get helper |
| Audit: boards/notes/projects/auth/session_store/admin/labels/tags | Helper or one-line comment; labels/tags: comment "no tx sites" |
| `backend-rs/tests/api/` (cards_move.rs / sort_key_ordering.rs / new) | Always-fire injector mode; exhausted → 503 + `Retry-After: 2`; recovery test; persistence assertions |
| `backend-rs/.sqlx/` | Regen only if queries change (pinned CLI, MySQL, db `kanpad` — T009) |
| `specs/arch-hardening-specs/20-performance.md` | Same PR: fix scope contradictions (`:131`, `:146`), reference naming (`:116`), edge-case wording (`:151-152`) |
| `CONTRACT-CHANGES.md` | Same PR: exhaustion-behavior paragraph after the Concurrency (D3) block |
| `specs/tasks/status.yaml` | Same PR: stale `name:` annotated (keep `status: in_progress` untouched) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** gap-lock logic T010 deleted (no regressions to
fractional keys), `_sqlx_migrations`, Argon2/tx ordering changes (hashing stays
outside txs), blanket 1062 retry, re-running the closure after a commit error
of unknown outcome.

## Implementation Steps

TDD throughout. Paste red proofs into the PR.

1. **Pre-flight (done 2026-11-18):** T010 merged; `retry.rs` matcher scoped
   (`1213|1205|UQ*_sort_key 1062`); six inline sites verified at stated lines.
2. **Red test A (recovery):** target a single-shot site — `delete_card` or
   `create_column` (**not** `create_card`; it already retries at `:112`).
   Use the T61-style SIGNAL-trigger injector in *always-fire* mode (extend
   `install_fault_injector` with an `always` mode; one-shot mode unchanged)
   to raise `1213` N times then clear; today the endpoint 500s, after the work
   it succeeds. Paste red proof.
3. **Red test B (exhaustion):** always-fire `1213` (and separately `1205`) →
   bounded attempts → **503 + `Retry-After: 2`** (today: 500). Do **not** use a
   real never-releasing `FOR UPDATE` lock — `innodb_lock_wait_timeout` defaults
   to 50 s, so exhaustion via a real lock would take ~250 s and never reach
   the 30 s `TimeoutLayer` deadline anyway; see Constraints. Paste red proof.
4. Implement `tx_retry.rs` on top of `retry.rs` (Design decisions):
   acquire per attempt; optional `SET TRANSACTION ... READ COMMITTED` before
   `BEGIN` (default: on, matching post-T010 paths; per-call opt-out);
   explicit `ROLLBACK` before the retry decision (1213 unwinds the whole tx;
   1205 rolls back only the current statement — always roll back explicitly);
   linear base backoff `10·(attempt+1)` ms with jitter preferred (or fixed
   linear + recorded justification); backoff sleeps **without** holding a
   pool connection; `warn!` per attempt, `error!` on exhaustion.
5. Swap the six inline loops to the helper (behavior-preserving). Existing
   `cards_move.rs` (incl. T61 503/409 injection tests) and `sort_key_ordering.rs`
   suites stay green unchanged.
6. Apply the helper to `create_column` first, then the delete/scalars txs
   (list above); audit every remaining `tx.begin()` site (one-line comment
   each); `rebalance.rs` gets none — it runs inside the enclosing retried tx.
7. Gates: `cargo fmt --check` 0; `cargo clippy --workspace --all-targets -- -D warnings -A clippy::pedantic` 0; `cargo sqlx prepare --check` 0; full suite green **MySQL 8.4 + MariaDB 11.4**; frontend `npm run gen:api -- --check`, `tsc --noEmit`, `vitest`; coverage floor green; persistence assertion per touched endpoint (different API call or sqlx `SELECT`).
8. Lesson appended to `docs/lessons/rust-backend.md` (dated, tagged).

## Design decisions (locked 2026-11-18 review)

1. **Exhaustion → 503 via boundary classification (Option A).** `error.rs`
   `Db` arm maps inner codes `1213`/`1205` → 503 `service-unavailable` +
   `Retry-After: 2` (new `BUSY_RETRY_AFTER = "2"` constant; 1–5 s range —
   DB-busy contention clears sub-second; `TIMEOUT_RETRY_AFTER = "30"` stays
   for the timeout layer only). Rationale (industry standard): sqlx does no
   retries; ecosystem wrappers (SQLAlchemy, psycopg, Rails) propagate the
   driver error to the HTTP boundary, and this codebase's established pattern
   is exactly that — `Db` arm classifies driver codes (RowNotFound→404,
   1062/23000→409, pool/`Io`→503). No new `AppError` variant; reuses the
   existing `service-unavailable` problem slug; uniform for helper-exhausted
   and any non-retried straggler. (Option B — `AppError::Busy` — was
   considered and rejected: redundant taxonomy for the identical wire shape.)
   The `warn!`→`error!` log flip on exhaustion supplies the saturation signal
   for T024/T026 capacity work, without a new variant.
2. **Scoped-1062 exhaustion stays 409 Conflict.** 1062 is SQLSTATE `23000`
   (class 23, integrity violation), not a serialization failure
   (`40001`/`HY000`); the unique index is doing its job and the collision is
   genuine. Keeping 409 is both the current mapping and the standards-aligned
   reading (reform = restructure find-or-create, not a different status).
   Decided 2026-11-18; documented in `CONTRACT-CHANGES.md`.
3. **Never re-run the closure after a commit error.** A failed `commit()` has
   an unknown outcome (it may have applied) — replaying the transaction can
   double-apply effects; the helper therefore performs commit **inside** the
   wrapper but **never retries it**; commit failures of unknown outcome map
   to the Db arm (503 if lock-class, else the existing generic internal 500).
4. **`Retry-After` value = 2 s** (floor for clients). Same problem+json shape,
   `service-unavailable` type, headers `CONTENT_TYPE` + `RETRY_AFTER`; the
   body carries no driver/SQL text (extend the no-leak unit test).
5. **Exhaustion tests use an always-fire SIGNAL injector**, not a real
   never-releasing lock: default `innodb_lock_wait_timeout` = 50 s exceeds
   the 30 s `TimeoutLayer`; exhaustion through a real lock would be
   ~5 × 50 s and can never complete. Fast-path worst case (1213/scoped-1062):
   ~5 × (tx work + Σ backoff ≤ 100 ms) ≪ 30 s. 1205-via-lock is *documented*
   as riding the TimeoutLayer (→ 503 + `Retry-After: 30`), consistent with D6.
   If per-attempt bounding is wanted later, a save/restore
   `SET SESSION innodb_lock_wait_timeout = <1–5>` around retried txs is a
   follow-up decision — not silently adopted here (pooled connections leak
   session state; save/restore required).
6. **Backoff jitter:** preferred — `sleep ∈ rand(5, 20·attempt) ms` (full-ish
   jitter; AWS/SRE guidance) — or keep the current fixed linear 10·n with a
   written justification (behavior-preserving). Record which was chosen in
   Notes.
7. **`create_column` is in-scope by requirement**, not by enumeration luck:
   it is the only un-retried position-rewriting tx in the tree (FOR UPDATE
   append + full-board rebalance) — `needs_rebalance` rewrites every column
   row there just as it does in retried append paths.
8. **Connection/backoff pooling invariants:** acquire the connection per
   attempt; sleep between attempts while holding no connection; re-derive
   everything from fresh reads inside the closure (sort_key recompute);
   `Restart` short-circuit from the same-card race passes through unretried;
   Argon2 stays outside the retried closure.

## Constraints

- **T010 (D-5) — satisfied** (merged `c62de576`); still no parallel
  cards/columns write-path task.
- Retries wrap **only in-tx effects** (assert by review; Argon2/hashing
  outside the closure — already true, keep it).
- Same-card race keeps T010's `Restart` short-circuit (logical conflict ≠
  lock conflict — don't retry it).
- **Worst-case honesty:** fast transient paths ≪ 30 s; a real 1205 rides the
  30 s `TimeoutLayer` (503 + `Retry-After: 30`) — do not claim exhaustion is
  reachable for real lock waits without capping `innodb_lock_wait_timeout`.
- **Scoped 1062 only** — never blanket-`1062`; unrelated unique violations
  remain 409 and never burn the retry budget.
- Both MySQL 8.4 + MariaDB 11.4 green (CI legs). 10.6 only if T011 records
  prod as 10.6 — per `40-operations.md` §6 this either adds/restores a 10.6
  CI leg **or** records a written accepted delta in §6 (10.6 is EOL
  2026-07-06); note `status.yaml` doesn't track T011 at all — flag to the
  taskpad owner.
- Harness rules (explicit imports, `assert_problem` labels); frontend DoD
  gates; coverage floor; no commit without explicit approval; Conventional
  Commit (20–50-char summary).

## Acceptance Criteria

- [ ] `with_tx_retry` exists in `common/tx_retry.rs` on top of `retry.rs`
      with unit tests (bounded attempts = 5, backoff bounds/jitter, matcher
      reachable for 1213|1205|scoped-1062, rollback called between attempts
      — assertable via injector invocation count, commit-failure not
      re-executed, backoff sleeps hold no pool connection).
- [ ] Injected-`1213` recovery test against a single-shot site
      (`delete_card` or `create_column`) — red today, proof pasted.
- [ ] Always-inject `1213` and `1205` → exhausted → **503 + `Retry-After: 2`
      (`service-unavailable`)**; red today, proof pasted; no driver/SQL text
      in the body.
- [ ] `create_column`, `delete_card`, and the audited delete/scalars txs use
      the helper; the six inline sites migrated behavior-preserving
      (`cards_move.rs` + `sort_key_ordering.rs` suites green unchanged);
      **every** remaining `tx.begin()` site audited with a one-line comment;
      bulk `move_cards` on the helper.
- [ ] Scoped-1062 exhaustion renders 409 (unchanged, tested); Db 1213/1205
      renders 503 + `Retry-After: 2` (new tests).
- [ ] Full suite green on MySQL 8.4 + MariaDB 11.4; `sqlx prepare --check` 0;
      fmt/clippy 0; frontend `gen:api --check`, `tsc --noEmit`, `vitest`
      green; coverage floor green.
- [ ] Persistence assertion per touched endpoint (re-read via a different API
      call or sqlx `SELECT`); lesson appended to `docs/lessons/rust-backend.md`.
- [ ] `20-performance.md`, `CONTRACT-CHANGES.md`, and `status.yaml` name
      reconciled in the same PR (scope notes corrected; exhaustion behavior
      documented).

## Notes

(2026-11-18 — amendment pass applying the review; see review disposition in the
thread. Decisions 1–8 above are locked or recorded for follow-up.)

- Open items for the implementation: 1205 `innodb_lock_wait_timeout` option
  (save/restore `SET SESSION`, or stay documented-on-TimeoutLayer); jitter
  choice (decision 6); `status.yaml` `status:` field says `in_progress` while
  no code exists — flip at implementation start; T011 (the drill recording
  `SELECT VERSION()`) remains untracked in `status.yaml`.
- Pre-flight evidence (2026-11-18): T010 `c62de576` on this branch;
  `common/retry.rs:24,29` (`MAX_ATTEMPTS = 5`, scoped matcher); six inline
  sites at the cited lines (verified by grep); `git grep "for attempt"`
  shows no other retry loops; 24 total `tx.begin()` sites enumerated
  (6 retried / 18 single-shot); raw `FOR UPDATE` grep = 32 literals,
  11 real lock sites in 4 files.

(Filled in further during/after implementation.)

## Lessons

(pending)
