# T024: Metrics fixes — acquire_timed + db probe + route labels

> **STATUS: pending.** Wave 1. Closes metrics half of **P7** (LOW).

## Goal

Make `/metrics` actually tell the truth:

1. **Acquire-duration histogram is always empty in production** —
   `AppState::acquire_timed` (`state.rs:177-184`) records into
   `kanpad_db_pool_acquire_duration_seconds` but its only caller is a test
   (`tests/api/metrics.rs:157`). Route real pool acquires through it (single
   chokepoint — prefer wrapping at the state/pool level so call sites don't change).
2. **`kanpad_db_up` goes stale** — refreshed only by `/ready` hits + boot
   (`metrics.rs:104-161`). Add a background probe task (30–60 s, budget-bounded
   `wait_until_ready`, feeds the existing CAS transition logger) that aborts on
   shutdown (mirror the sweeper's abort pattern).
3. **Per-route latency** — add a `route` label to the duration histogram using the
   same `MatchedPath` cardinality guard as `requests_total` (never raw URI; `/*` →
   `"unknown"`).

Fail-closed `METRICS_PORT` model, no-self-scrape rule and cardinality discipline stay
untouched.

## Depends On

(None — independent backend task.)

## Phase

1

## Critical

No — observability completeness; P7 LOW but cheap and it's what T025's alerting and
T026's diagnosis rest on.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Metrics fixes (design + edge cases).
- `specs/arch-hardening-specs/00-overview.md` — finding P7; global DoD.
- `backend-rs/src/state.rs:177-184` (dead `acquire_timed`), `backend-rs/src/metrics.rs:104-161`
  (db_up staleness + series list), `backend-rs/src/lib.rs:462-481` (MatchedPath guard),
  `backend-rs/src/main.rs:145-181` (sweeper/metrics task lifecycle to mirror),
  `backend-rs/tests/api/metrics.rs` (existing scrape tests).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/state.rs` (or pool construction site) | Acquires recorded via `acquire_timed` (chokepoint — call sites unchanged if possible) |
| `backend-rs/src/metrics.rs` (+ `lib.rs`/`main.rs`) | Background DB probe task feeding `record_db_probe`; abort on shutdown; duration histogram gains guarded `route` label |
| `backend-rs/tests/api/metrics.rs` | Red-first: non-empty acquire histogram after N requests; `db_up` freshness without `/ready`; cardinality guard for duration series; shutdown aborts probe |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** unauthenticated-metrics changes (port restriction is
the documented control), new metric names beyond the label addition (keep series set
stable for any dashboards), `/ready`'s own probe semantics.

## Implementation Steps

TDD (red-first against the current scrape tests).

1. **Red:** scrape after N authenticated requests → `kanpad_db_pool_acquire_duration_…`
   count = 0 today → must be > 0.
2. **Red:** `db_up` freshness — stop hitting `/ready`, run background probe interval,
   scrape shows updated value (today: stale).
3. Implement acquire chokepoint (verify no double-counting; keep `test_before_acquire`
   ping semantics — the ping is part of acquire cost, note in doc).
4. Background probe: spawn in `create_app`? **No** — house rule: task spawns in
   `main.rs` beside the sweeper (guard comment at `lib.rs:140-145` exists for exactly
   this); abort on shutdown alongside sweeper/metrics.
5. Duration histogram `route` label: reuse the `MatchedPath` filter logic (extract to
   shared fn if duplicated).
6. Cardinality test: `/api/v1/cards/{id}` variants → no per-id series (mirror the
   existing requests_total guard).
7. Gates: fmt/clippy/full suite both engines; `sqlx prepare --check` (if any query
   touched — likely none); lesson.

## Constraints

- Probe must reuse `wait_until_ready` + `record_db_probe` (single transition logger —
  no second writer with different semantics; verify gauge writes are atomic under
  concurrent `/ready` + probe).
- Background probe must not force connections during an outage (budget-bounded, same
  as `/ready`).
- Task lifecycle mirrors sweeper (spawn in `main.rs`, abort on shutdown — no leaked
  tasks in test output).
- Metric series names unchanged (only the label set grows on the duration histogram).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Acquire histogram non-empty after real requests (red-proof pasted).
- [ ] `db_up` fresh via background probe with no `/ready` traffic (red-proof).
- [ ] Duration histogram carries the guarded `route` label; cardinality test green
      (no per-id series).
- [ ] Probe aborts on shutdown (no leaked-task warnings; test green).
- [ ] `/metrics` fail-closed + no-self-scrape behavior unchanged; full suite both
      engines; `sqlx prepare --check` 0; lesson appended (closes metrics half of
      **P7**; pool/timeout interplay documented → input for T026).

## Notes

(filled in during/after implementation)

## Lessons

(pending)
